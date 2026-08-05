use crate::archive::extract_verified_archive;
pub use crate::archive::ExtractionLimits;
use crate::manifest::{compare_version, safe_relative_path, validate_component};
use crate::{
    verify_file, verify_signed_catalog, verify_signed_manifest, CancellationToken, DownloadPlan,
    ModelManagerError, SignedModelCatalog, SignedModelManifest, SourceDownloader,
};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const INSTALLED_MANIFEST: &str = ".scoreleap-manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelVersionRef {
    pub model_id: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    Installed,
    ValidCacheHit,
}

#[derive(Debug, Clone)]
pub struct InstallReport {
    pub outcome: InstallOutcome,
    pub install_path: PathBuf,
    pub download_plan: Option<DownloadPlan>,
}

pub struct ModelManager {
    root: PathBuf,
    verifying_key: VerifyingKey,
    engine_id: String,
    engine_version: String,
    limits: ExtractionLimits,
}

impl ModelManager {
    pub fn new(
        root: impl Into<PathBuf>,
        verifying_key: VerifyingKey,
        engine_id: impl Into<String>,
        engine_version: impl Into<String>,
    ) -> Self {
        Self {
            root: root.into(),
            verifying_key,
            engine_id: engine_id.into(),
            engine_version: engine_version.into(),
            limits: ExtractionLimits::default(),
        }
    }

    pub fn with_limits(mut self, limits: ExtractionLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn version_path(&self, model_id: &str, version: &str) -> PathBuf {
        self.root.join(model_id).join(version)
    }

    pub fn install(
        &self,
        signed: &SignedModelManifest,
        downloader: &dyn SourceDownloader,
        cancellation: &CancellationToken,
    ) -> Result<InstallReport, ModelManagerError> {
        self.verify_for_engine(signed)?;
        let final_path = self.version_path(&signed.manifest.model_id, &signed.manifest.version);
        if self.validate_expected_cache(signed).is_ok() {
            return Ok(InstallReport {
                outcome: InstallOutcome::ValidCacheHit,
                install_path: final_path,
                download_plan: None,
            });
        }

        fs::create_dir_all(&self.root)?;
        let _lock = InstallLock::acquire(&self.root, &signed.manifest.model_id)?;
        if self.validate_expected_cache(signed).is_ok() {
            return Ok(InstallReport {
                outcome: InstallOutcome::ValidCacheHit,
                install_path: final_path,
                download_plan: None,
            });
        }

        let downloads = self.root.join(".downloads");
        fs::create_dir_all(&downloads)?;
        let part_path = downloads.join(format!(
            "{}-{}.zip.part",
            signed.manifest.model_id, signed.manifest.version
        ));
        let mut plan = DownloadPlan::new(&signed.manifest.package.sources);
        if let Err(error) = plan.execute(
            downloader,
            &part_path,
            &signed.manifest.package,
            cancellation,
        ) {
            let _ = fs::remove_file(&part_path);
            return Err(error);
        }

        // 同盘临时目录中完成解包、逐文件摘要校验和清单落盘，最后 rename 发布。
        let model_parent = self.root.join(&signed.manifest.model_id);
        fs::create_dir_all(&model_parent)?;
        let stage = model_parent.join(format!(
            ".{}.installing-{}",
            signed.manifest.version,
            unique_suffix()
        ));
        fs::create_dir(&stage)?;
        if let Err(error) = extract_verified_archive(
            &part_path,
            &stage,
            &signed.manifest.artifacts,
            self.limits,
            cancellation,
        ) {
            cleanup_failed_install(&stage, &part_path);
            return Err(error);
        }
        if let Err(error) = write_json_synced(&stage.join(INSTALLED_MANIFEST), signed) {
            cleanup_failed_install(&stage, &part_path);
            return Err(error);
        }

        if final_path.exists() {
            // 损坏缓存不直接删除，隔离保留后再发布新版本目录。
            let quarantine = model_parent.join(format!(
                ".{}.invalid-{}",
                signed.manifest.version,
                unique_suffix()
            ));
            fs::rename(&final_path, quarantine)?;
        }
        if let Err(error) = fs::rename(&stage, &final_path) {
            cleanup_failed_install(&stage, &part_path);
            return Err(error.into());
        }
        let _ = fs::remove_file(&part_path);
        Ok(InstallReport {
            outcome: InstallOutcome::Installed,
            install_path: final_path,
            download_plan: Some(plan),
        })
    }

    pub fn validate_cached(
        &self,
        model_id: &str,
        version: &str,
    ) -> Result<SignedModelManifest, ModelManagerError> {
        validate_component(model_id, "model_id")?;
        validate_component(version, "version")?;
        let version_path = self.version_path(model_id, version);
        let manifest_path = version_path.join(INSTALLED_MANIFEST);
        if !manifest_path.is_file() {
            return Err(ModelManagerError::CacheMissing(
                model_id.into(),
                version.into(),
            ));
        }
        let signed: SignedModelManifest = serde_json::from_reader(File::open(manifest_path)?)?;
        if signed.manifest.model_id != model_id || signed.manifest.version != version {
            return Err(ModelManagerError::CacheInvalid(
                "缓存路径与清单身份不一致".into(),
            ));
        }
        self.verify_for_engine(&signed)?;
        for artifact in &signed.manifest.artifacts {
            let relative = safe_relative_path(&artifact.path)?;
            let path = version_path.join(relative);
            if !path.is_file() {
                return Err(ModelManagerError::CacheInvalid(format!(
                    "缺少 {}",
                    artifact.path
                )));
            }
            verify_file(&path, artifact.size_bytes, &artifact.sha256)
                .map_err(|error| ModelManagerError::CacheInvalid(error.to_string()))?;
        }
        Ok(signed)
    }

    pub fn active_version(
        &self,
        model_id: &str,
    ) -> Result<Option<ModelVersionRef>, ModelManagerError> {
        validate_component(model_id, "model_id")?;
        read_optional_json(&self.state_path(model_id, "active.json"))
    }

    pub fn last_good_version(
        &self,
        model_id: &str,
    ) -> Result<Option<ModelVersionRef>, ModelManagerError> {
        validate_component(model_id, "model_id")?;
        read_optional_json(&self.state_path(model_id, "last-good.json"))
    }

    /// 验证整个目录签名后，返回与当前引擎兼容的最高版本。
    pub fn latest_from_catalog(
        &self,
        catalog: &SignedModelCatalog,
        model_id: &str,
    ) -> Result<Option<SignedModelManifest>, ModelManagerError> {
        validate_component(model_id, "model_id")?;
        verify_signed_catalog(catalog, &self.verifying_key)?;
        let mut candidates = Vec::new();
        for signed in &catalog.catalog.models {
            if signed.manifest.model_id == model_id && self.verify_for_engine(signed).is_ok() {
                candidates.push(signed.clone());
            }
        }
        candidates.sort_by(|left, right| {
            compare_version(&left.manifest.version, &right.manifest.version)
                .expect("已验证目录中的模型版本必须可比较")
        });
        Ok(candidates.pop())
    }

    pub fn activate(&self, model_id: &str, version: &str) -> Result<(), ModelManagerError> {
        self.validate_cached(model_id, version)?;
        fs::create_dir_all(&self.root)?;
        let _lock = InstallLock::acquire(&self.root, model_id)?;
        let next = ModelVersionRef {
            model_id: model_id.into(),
            version: version.into(),
        };
        let active_path = self.state_path(model_id, "active.json");
        let previous: Option<ModelVersionRef> = read_optional_json(&active_path)?;
        if let Some(previous) = previous.filter(|value| value != &next) {
            if self
                .validate_cached(&previous.model_id, &previous.version)
                .is_ok()
            {
                atomic_write_json(&self.state_path(model_id, "last-good.json"), &previous)?;
            }
        }
        atomic_write_json(&active_path, &next)
    }

    pub fn rollback(&self, model_id: &str) -> Result<ModelVersionRef, ModelManagerError> {
        validate_component(model_id, "model_id")?;
        let target: ModelVersionRef =
            read_optional_json(&self.state_path(model_id, "last-good.json"))?
                .ok_or_else(|| ModelManagerError::NoRollbackVersion(model_id.into()))?;
        self.validate_cached(&target.model_id, &target.version)?;
        fs::create_dir_all(&self.root)?;
        let _lock = InstallLock::acquire(&self.root, model_id)?;
        let active_path = self.state_path(model_id, "active.json");
        let current: Option<ModelVersionRef> = read_optional_json(&active_path)?;
        atomic_write_json(&active_path, &target)?;
        if let Some(current) = current.filter(|value| value != &target) {
            atomic_write_json(&self.state_path(model_id, "last-good.json"), &current)?;
        }
        Ok(target)
    }

    fn verify_for_engine(&self, signed: &SignedModelManifest) -> Result<(), ModelManagerError> {
        verify_signed_manifest(signed, &self.verifying_key)?;
        let compat = &signed.manifest.engine_compat;
        let below_minimum = compare_version(&self.engine_version, &compat.min_version)?.is_lt();
        let above_maximum = match &compat.max_version {
            Some(maximum) => compare_version(&self.engine_version, maximum)?.is_gt(),
            None => false,
        };
        if compat.engine_id != self.engine_id || below_minimum || above_maximum {
            return Err(ModelManagerError::IncompatibleEngine(format!(
                "当前 {} {}，要求 {} {}..{:?}",
                self.engine_id,
                self.engine_version,
                compat.engine_id,
                compat.min_version,
                compat.max_version
            )));
        }
        Ok(())
    }

    fn validate_expected_cache(
        &self,
        expected: &SignedModelManifest,
    ) -> Result<(), ModelManagerError> {
        let cached =
            self.validate_cached(&expected.manifest.model_id, &expected.manifest.version)?;
        if &cached != expected {
            return Err(ModelManagerError::CacheInvalid(
                "缓存清单不是请求的已签名清单".into(),
            ));
        }
        Ok(())
    }

    fn state_path(&self, model_id: &str, file: &str) -> PathBuf {
        self.root.join(".state").join(model_id).join(file)
    }
}

fn cleanup_failed_install(stage: &Path, part_path: &Path) {
    let _ = fs::remove_dir_all(stage);
    let _ = fs::remove_file(part_path);
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}

fn write_json_synced(path: &Path, value: &impl Serialize) -> Result<(), ModelManagerError> {
    let mut file = File::create(path)?;
    serde_json::to_writer(&mut file, value)?;
    file.flush()?;
    file.sync_all()?;
    Ok(())
}

fn atomic_write_json(path: &Path, value: &impl Serialize) -> Result<(), ModelManagerError> {
    let parent = path
        .parent()
        .ok_or_else(|| ModelManagerError::CacheInvalid("状态路径没有父目录".into()))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".state-{}.tmp", unique_suffix()));
    write_json_synced(&temporary, value)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temporary, path)?;
    Ok(())
}

fn read_optional_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<Option<T>, ModelManagerError> {
    match File::open(path) {
        Ok(file) => Ok(Some(serde_json::from_reader(file)?)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

struct InstallLock {
    path: PathBuf,
}

impl InstallLock {
    fn acquire(root: &Path, model_id: &str) -> Result<Self, ModelManagerError> {
        validate_component(model_id, "model_id")?;
        let directory = root.join(".locks");
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!("{model_id}.lock"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                writeln!(file, "{}", std::process::id())?;
                file.sync_all()?;
                Ok(Self { path })
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                Err(ModelManagerError::InstallLocked(model_id.into()))
            }
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

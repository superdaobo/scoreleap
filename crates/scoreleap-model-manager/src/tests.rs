use super::*;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};
use std::sync::Mutex;
use tempfile::TempDir;
use zip::write::SimpleFileOptions;

struct FakeDownloader {
    bytes: Vec<u8>,
    failing_url: Option<String>,
    calls: Mutex<Vec<String>>,
}

impl SourceDownloader for FakeDownloader {
    fn download(
        &self,
        source: &ModelSource,
        destination: &mut dyn Write,
        cancellation: &CancellationToken,
    ) -> Result<(), String> {
        self.calls.lock().unwrap().push(source.url.clone());
        if cancellation.is_cancelled() {
            return Err("cancelled".into());
        }
        if self.failing_url.as_deref() == Some(&source.url) {
            return Err("primary unavailable".into());
        }
        destination
            .write_all(&self.bytes)
            .map_err(|error| error.to_string())
    }
}

fn key() -> SigningKey {
    SigningKey::from_bytes(&[7_u8; 32])
}

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    for &(name, bytes) in entries {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn signed_manifest(version: &str, archive: &[u8], artifact: &[u8]) -> SignedModelManifest {
    let manifest = ModelManifest {
        schema_version: 1,
        model_id: "basic-pitch".into(),
        version: version.into(),
        engine_compat: EngineCompatibility {
            engine_id: "scoreleap-onnx".into(),
            min_version: "1.0.0".into(),
            max_version: Some("1.9.0".into()),
        },
        artifacts: vec![ArtifactDescriptor {
            path: "model/model.onnx".into(),
            size_bytes: artifact.len() as u64,
            sha256: hash(artifact),
        }],
        package: PackageDescriptor {
            size_bytes: archive.len() as u64,
            sha256: hash(archive),
            sources: vec![
                ModelSource {
                    kind: SourceKind::Cdn,
                    url: "https://cdn.example/model.zip".into(),
                },
                ModelSource {
                    kind: SourceKind::GithubRelease,
                    url: "https://github.com/example/releases/model.zip".into(),
                },
            ],
        },
        license: LicenseDescriptor {
            spdx_id: "Apache-2.0".into(),
            name: "Apache License 2.0".into(),
            url: "https://www.apache.org/licenses/LICENSE-2.0".into(),
            redistribution_allowed: true,
        },
    };
    let signature = key().sign(&manifest_signing_bytes(&manifest).unwrap());
    SignedModelManifest {
        manifest,
        signature: SignatureEnvelope {
            algorithm: "ed25519".into(),
            key_id: "test-key".into(),
            signature_hex: hex::encode(signature.to_bytes()),
        },
    }
}

fn resign(signed: &mut SignedModelManifest) {
    let signature = key().sign(&manifest_signing_bytes(&signed.manifest).unwrap());
    signed.signature.signature_hex = hex::encode(signature.to_bytes());
}

fn manager(temp: &TempDir) -> ModelManager {
    ModelManager::new(
        temp.path(),
        key().verifying_key(),
        "scoreleap-onnx",
        "1.2.0",
    )
}

fn downloader(bytes: &[u8]) -> FakeDownloader {
    FakeDownloader {
        bytes: bytes.to_vec(),
        failing_url: None,
        calls: Mutex::new(Vec::new()),
    }
}

#[test]
fn signature_success_and_failure() {
    let artifact = b"onnx-model";
    let archive = make_zip(&[("model/model.onnx", artifact)]);
    let mut signed = signed_manifest("1.0.0", &archive, artifact);
    verify_signed_manifest(&signed, &key().verifying_key()).unwrap();
    signed.manifest.version = "1.0.1".into();
    assert!(matches!(
        verify_signed_manifest(&signed, &key().verifying_key()),
        Err(ModelManagerError::InvalidSignature(_))
    ));
}

#[test]
fn hash_mismatch_rejects_all_sources() {
    let temp = TempDir::new().unwrap();
    let artifact = b"onnx-model";
    let archive = make_zip(&[("model/model.onnx", artifact)]);
    let mut signed = signed_manifest("1.0.0", &archive, artifact);
    signed.manifest.package.sha256 = "0".repeat(64);
    resign(&mut signed);
    let error = manager(&temp)
        .install(
            &signed,
            &downloader(&archive),
            &CancellationToken::default(),
        )
        .unwrap_err();
    assert!(matches!(error, ModelManagerError::AllSourcesFailed(_)));
}

#[test]
fn primary_source_falls_back_in_declared_order() {
    let temp = TempDir::new().unwrap();
    let artifact = b"onnx-model";
    let archive = make_zip(&[("model/model.onnx", artifact)]);
    let signed = signed_manifest("1.0.0", &archive, artifact);
    let client = FakeDownloader {
        bytes: archive,
        failing_url: Some("https://cdn.example/model.zip".into()),
        calls: Mutex::new(Vec::new()),
    };
    let report = manager(&temp)
        .install(&signed, &client, &CancellationToken::default())
        .unwrap();
    assert_eq!(report.outcome, InstallOutcome::Installed);
    assert_eq!(client.calls.lock().unwrap().len(), 2);
    let plan = report.download_plan.unwrap();
    assert_eq!(plan.attempts[0].state, AttemptState::Failed);
    assert_eq!(plan.attempts[1].state, AttemptState::Succeeded);
}

#[test]
fn zip_slip_is_rejected_without_publishing_version() {
    let temp = TempDir::new().unwrap();
    let artifact = b"onnx-model";
    let archive = make_zip(&[("model/model.onnx", artifact), ("../evil", b"evil")]);
    let signed = signed_manifest("1.0.0", &archive, artifact);
    let error = manager(&temp)
        .install(
            &signed,
            &downloader(&archive),
            &CancellationToken::default(),
        )
        .unwrap_err();
    assert!(matches!(error, ModelManagerError::UnsafeArchivePath(_)));
    assert!(!manager(&temp).version_path("basic-pitch", "1.0.0").exists());
}

#[test]
fn unexpected_file_is_rejected() {
    let temp = TempDir::new().unwrap();
    let artifact = b"onnx-model";
    let archive = make_zip(&[("model/model.onnx", artifact), ("surprise.txt", b"x")]);
    let signed = signed_manifest("1.0.0", &archive, artifact);
    let error = manager(&temp)
        .install(
            &signed,
            &downloader(&archive),
            &CancellationToken::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        ModelManagerError::UnexpectedArchiveEntry(_)
    ));
}

#[test]
fn cancellation_removes_partial_download() {
    let temp = TempDir::new().unwrap();
    let artifact = b"onnx-model";
    let archive = make_zip(&[("model/model.onnx", artifact)]);
    let signed = signed_manifest("1.0.0", &archive, artifact);
    let token = CancellationToken::default();
    token.cancel();
    let error = manager(&temp)
        .install(&signed, &downloader(&archive), &token)
        .unwrap_err();
    assert!(matches!(error, ModelManagerError::Cancelled));
    assert!(!manager(&temp).version_path("basic-pitch", "1.0.0").exists());
}

#[test]
fn valid_offline_cache_skips_downloader() {
    let temp = TempDir::new().unwrap();
    let artifact = b"onnx-model";
    let archive = make_zip(&[("model/model.onnx", artifact)]);
    let signed = signed_manifest("1.0.0", &archive, artifact);
    let manager = manager(&temp);
    manager
        .install(
            &signed,
            &downloader(&archive),
            &CancellationToken::default(),
        )
        .unwrap();
    let offline = FakeDownloader {
        bytes: Vec::new(),
        failing_url: Some("https://cdn.example/model.zip".into()),
        calls: Mutex::new(Vec::new()),
    };
    let report = manager
        .install(&signed, &offline, &CancellationToken::default())
        .unwrap();
    assert_eq!(report.outcome, InstallOutcome::ValidCacheHit);
    assert!(offline.calls.lock().unwrap().is_empty());
}

#[test]
fn install_is_published_only_after_full_validation() {
    let temp = TempDir::new().unwrap();
    let artifact = b"onnx-model";
    let archive = make_zip(&[("model/model.onnx", artifact)]);
    let signed = signed_manifest("1.0.0", &archive, artifact);
    let manager = manager(&temp);
    let report = manager
        .install(
            &signed,
            &downloader(&archive),
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(report.outcome, InstallOutcome::Installed);
    assert!(report.install_path.join("model/model.onnx").is_file());
    manager.validate_cached("basic-pitch", "1.0.0").unwrap();
}

#[test]
fn activation_and_rollback_preserve_last_good() {
    let temp = TempDir::new().unwrap();
    let artifact_v1 = b"onnx-v1";
    let archive_v1 = make_zip(&[("model/model.onnx", artifact_v1)]);
    let artifact_v2 = b"onnx-v2";
    let archive_v2 = make_zip(&[("model/model.onnx", artifact_v2)]);
    let signed_v1 = signed_manifest("1.0.0", &archive_v1, artifact_v1);
    let signed_v2 = signed_manifest("1.1.0", &archive_v2, artifact_v2);
    let manager = manager(&temp);
    manager
        .install(
            &signed_v1,
            &downloader(&archive_v1),
            &CancellationToken::default(),
        )
        .unwrap();
    manager
        .install(
            &signed_v2,
            &downloader(&archive_v2),
            &CancellationToken::default(),
        )
        .unwrap();
    manager.activate("basic-pitch", "1.0.0").unwrap();
    manager.activate("basic-pitch", "1.1.0").unwrap();
    let rolled_back = manager.rollback("basic-pitch").unwrap();
    assert_eq!(rolled_back.version, "1.0.0");
    assert_eq!(
        manager
            .active_version("basic-pitch")
            .unwrap()
            .unwrap()
            .version,
        "1.0.0"
    );
}

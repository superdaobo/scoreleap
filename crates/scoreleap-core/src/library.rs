//! 曲谱库：清单持久化、分组、排序、删除/重命名、自动去重与曲目信息。

use crate::{AppState, CoreError};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::path::Path;

/// 转录元数据（随曲谱持久化；来自 worker 的 metadata.json，仅取展示所需字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionMeta {
    /// 转录引擎标识（如 scoreleap-native-basic-pitch / scoreleap-transkun-v2）。
    pub engine: String,
    pub engine_version: String,
    /// 模型文件名（如 basic-pitch.onnx / 2.0.pt）。
    pub model_file: String,
    /// 转录耗时（毫秒）。
    pub elapsed_ms: u64,
    /// 转录完成时间（毫秒时间戳；导入曲谱库时填充）。
    #[serde(default)]
    pub completed_at_ms: u64,
}

/// 曲谱库分组定义（library/groups.json 持久化）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    pub group_id: String,
    pub name: String,
    pub created_at_ms: u64,
}

/// 曲谱库条目摘要。
#[derive(Debug, Clone, Serialize)]
pub struct DocumentSummary {
    pub doc_id: String,
    pub name: String,
    pub format: String,
    pub track_count: usize,
    pub note_count: usize,
    pub duration_ms: i64,
    pub bpm_range: (f64, f64),
    /// 来源类型：midi / audio_transcription（serde default 向后兼容）。
    #[serde(default = "default_source_type")]
    pub source_type: String,
    /// 导入时间（毫秒时间戳）。
    pub imported_at: u64,
    /// 所属分组；None = 未分类。
    #[serde(default)]
    pub group_id: Option<String>,
    /// MIDI 内容指纹（SHA-256，自动去重用）。
    #[serde(default)]
    pub content_hash: Option<String>,
    /// 转录元数据（仅 audio_transcription 来源可能有；旧曲谱为 None）。
    #[serde(default)]
    pub transcription: Option<TranscriptionMeta>,
    /// 曲目标题（音频标签自动读取或手动编辑）。
    #[serde(default)]
    pub title: Option<String>,
    /// 曲目艺术家。
    #[serde(default)]
    pub artist: Option<String>,
}

/// manifest 持久化条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub doc_id: String,
    pub name: String,
    pub format: String,
    pub track_count: usize,
    pub note_count: usize,
    pub duration_ms: i64,
    pub bpm_range: (f64, f64),
    pub imported_at: u64,
    /// 来源类型：midi / audio_transcription（旧 manifest 缺省 = midi）。
    #[serde(default = "default_source_type")]
    pub source_type: String,
    /// 所属分组；None = 未分类（旧 manifest 缺省 = None）。
    #[serde(default)]
    pub group_id: Option<String>,
    /// MIDI 内容指纹（SHA-256，自动去重用；旧 manifest 缺省 = None）。
    #[serde(default)]
    pub content_hash: Option<String>,
    /// 转录元数据（旧 manifest 缺省 = None）。
    #[serde(default)]
    pub transcription: Option<TranscriptionMeta>,
    /// 曲目标题。
    #[serde(default)]
    pub title: Option<String>,
    /// 曲目艺术家。
    #[serde(default)]
    pub artist: Option<String>,
}

impl From<&ManifestEntry> for DocumentSummary {
    fn from(e: &ManifestEntry) -> Self {
        DocumentSummary {
            doc_id: e.doc_id.clone(),
            name: e.name.clone(),
            format: e.format.clone(),
            track_count: e.track_count,
            note_count: e.note_count,
            duration_ms: e.duration_ms,
            bpm_range: e.bpm_range,
            source_type: e.source_type.clone(),
            imported_at: e.imported_at,
            group_id: e.group_id.clone(),
            content_hash: e.content_hash.clone(),
            transcription: e.transcription.clone(),
            title: e.title.clone(),
            artist: e.artist.clone(),
        }
    }
}

/// 导入结果摘要。
#[derive(Debug, Clone, Serialize)]
pub struct ImportSummary {
    pub doc_id: String,
    pub name: String,
    pub format: String,
    pub track_count: usize,
    pub note_count: usize,
    pub duration_ms: i64,
    pub bpm_range: (f64, f64),
    /// 来源类型：midi / audio_transcription。
    #[serde(default = "default_source_type")]
    pub source_type: String,
    /// 是否检测为重复曲谱并跳过导入（此时 doc_id 为空串）。
    #[serde(default)]
    pub duplicated: bool,
    /// 被判定重复的已有曲谱 doc_id（duplicated 时有效）。
    #[serde(default)]
    pub duplicate_of: Option<String>,
}

fn default_source_type() -> String {
    "midi".into()
}

/// 导入附加信息（转录元数据 + 曲目信息；直接导入 MIDI 用默认值）。
#[derive(Debug, Clone, Default)]
pub struct ImportMeta {
    /// 转录元数据（仅转录来源传入）。
    pub transcription: Option<TranscriptionMeta>,
    /// 曲目标题（音频标签自动读取或手动编辑）。
    pub title: Option<String>,
    /// 曲目艺术家。
    pub artist: Option<String>,
}

/// 毫秒时间戳。
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 导入 MIDI 文件（解析在后台线程执行）。来源类型为 midi。
pub fn import_midi(state: &AppState, path: String) -> Result<ImportSummary, CoreError> {
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".into());
    import_midi_from_path(state, &path, &name, "midi", ImportMeta::default())
}

/// 共享 MIDI 导入入口：直接导入的 MIDI 与转录生成的 MIDI 均经此进入曲谱库。
/// `display_name` 用于曲谱库显示名；`source_type` 为 midi 或 audio_transcription。
/// `transcription` 为转录元数据（仅转录来源传入）；`title`/`artist` 为曲目信息
/// （音频标签自动读取或手动编辑；直接导入为 None）。
///
/// 自动去重：内容指纹（SHA-256）与已有曲谱一致，或曲名 + 来源类型均相同
/// （如同一首歌转录两次）时，跳过导入并返回 `duplicated = true`（doc_id 为空串）。
pub fn import_midi_from_path(
    state: &AppState,
    path: &str,
    display_name: &str,
    source_type: &str,
    meta: ImportMeta,
) -> Result<ImportSummary, CoreError> {
    let bytes =
        std::fs::read(path).map_err(|e| CoreError::Invalid(format!("读取文件失败: {e}")))?;
    let content_hash = hex::encode(sha2::Sha256::digest(&bytes));
    if let Some(dup_doc_id) = find_duplicate(state, display_name, source_type, &content_hash) {
        tracing::info!(duplicate_of = %dup_doc_id, name = %display_name, "检测到重复曲谱，跳过导入");
        return Ok(ImportSummary {
            doc_id: String::new(),
            note_count: 0,
            duration_ms: 0,
            format: String::new(),
            track_count: 0,
            bpm_range: (0.0, 0.0),
            name: display_name.to_string(),
            source_type: source_type.to_string(),
            duplicated: true,
            duplicate_of: Some(dup_doc_id),
        });
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(scoreleap_midi::parse_midi(&bytes));
    });
    let doc = rx
        .recv()
        .map_err(|_| CoreError::Invalid("解析线程失败".into()))?
        .map_err(CoreError::from)?;
    let doc_id = format!("doc-{}", uuid::Uuid::new_v4());
    let summary = ImportSummary {
        doc_id: doc_id.clone(),
        note_count: doc.note_count(),
        duration_ms: doc.duration_us / 1000,
        format: format!("{:?}", doc.format),
        track_count: doc.tracks.len(),
        bpm_range: doc.bpm_range(),
        name: display_name.to_string(),
        source_type: source_type.to_string(),
        duplicated: false,
        duplicate_of: None,
    };
    state.documents.lock().unwrap().insert(doc_id.clone(), doc);

    // 持久化：复制源文件到曲谱库并更新 manifest（失败不阻断导入，仅记录日志）
    let mut meta = meta;
    if let Some(t) = meta.transcription.as_mut() {
        if t.completed_at_ms == 0 {
            t.completed_at_ms = now_ms();
        }
    }
    if let Err(e) = persist_import(state, &doc_id, path, &summary, Some(content_hash), meta) {
        tracing::warn!("曲谱库持久化失败（本次会话仍可用）: {e}");
    }

    Ok(summary)
}

/// 在曲谱库中查找与本次导入重复的条目 doc_id（仅比较文件仍存在的条目）。
/// 判定：内容指纹一致，或曲名 + 来源类型均一致。
fn find_duplicate(
    state: &AppState,
    display_name: &str,
    source_type: &str,
    content_hash: &str,
) -> Option<String> {
    let dir = state.library_dir.lock().unwrap().clone();
    read_manifest(&dir)
        .into_iter()
        .find(|e| {
            dir.join(format!("{}.mid", e.doc_id)).exists()
                && (e.content_hash.as_deref() == Some(content_hash)
                    || (e.source_type == source_type && e.name == display_name))
        })
        .map(|e| e.doc_id)
}

/// 将导入的 MIDI 复制到曲谱库并更新 manifest（原子写）。
fn persist_import(
    state: &AppState,
    doc_id: &str,
    src_path: &str,
    summary: &ImportSummary,
    content_hash: Option<String>,
    meta: ImportMeta,
) -> Result<(), CoreError> {
    let dir = state.library_dir.lock().unwrap().clone();
    std::fs::create_dir_all(&dir)
        .map_err(|e| CoreError::Invalid(format!("创建曲谱库目录失败: {e}")))?;
    let dest = dir.join(format!("{doc_id}.mid"));
    std::fs::copy(src_path, &dest)
        .map_err(|e| CoreError::Invalid(format!("复制 MIDI 失败: {e}")))?;

    let mut entries = read_manifest(&dir);
    // 新导入追加到末尾（顺序 = 分组内展示顺序，由用户手动排序控制）
    entries.push(ManifestEntry {
        doc_id: doc_id.to_string(),
        name: summary.name.clone(),
        format: summary.format.clone(),
        track_count: summary.track_count,
        note_count: summary.note_count,
        source_type: summary.source_type.clone(),
        duration_ms: summary.duration_ms,
        bpm_range: summary.bpm_range,
        imported_at: now_ms(),
        group_id: None,
        content_hash,
        transcription: meta.transcription,
        title: meta.title,
        artist: meta.artist,
    });
    write_manifest(&dir, &entries)
}

/// 读取 manifest；不存在返回空；损坏时重置为空数组并记录日志。
fn read_manifest(dir: &Path) -> Vec<ManifestEntry> {
    let path = dir.join("manifest.json");
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            tracing::warn!("manifest 损坏，重置为空: {e}");
            let _ = std::fs::write(&path, "[]");
            Vec::new()
        }),
        Err(_) => Vec::new(),
    }
}

/// 原子写 manifest。
fn write_manifest(dir: &Path, entries: &[ManifestEntry]) -> Result<(), CoreError> {
    let path = dir.join("manifest.json");
    let tmp = dir.join("manifest.json.tmp");
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| CoreError::Invalid(format!("manifest 序列化失败: {e}")))?;
    std::fs::write(&tmp, json)
        .map_err(|e| CoreError::Invalid(format!("manifest 写入失败: {e}")))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| CoreError::Invalid(format!("manifest 替换失败: {e}")))?;
    Ok(())
}

/// 曲谱库列表（过滤文件缺失条目；顺手清理 manifest 中已缺失条目）。
pub fn list_documents(state: &AppState) -> Result<Vec<DocumentSummary>, CoreError> {
    let dir = state.library_dir.lock().unwrap().clone();
    let entries = read_manifest(&dir);
    let kept: Vec<ManifestEntry> = entries
        .iter()
        .filter(|e| dir.join(format!("{}.mid", e.doc_id)).exists())
        .cloned()
        .collect();
    if kept.len() != entries.len() {
        let _ = write_manifest(&dir, &kept);
    }
    Ok(kept.iter().map(DocumentSummary::from).collect())
}

/// 读取分组列表；不存在返回空；损坏时重置为空并记录日志。
fn read_groups(dir: &Path) -> Vec<GroupInfo> {
    let path = dir.join("groups.json");
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            tracing::warn!("groups.json 损坏，重置为空: {e}");
            let _ = std::fs::write(&path, "[]");
            Vec::new()
        }),
        Err(_) => Vec::new(),
    }
}

/// 原子写分组列表。
fn write_groups(dir: &Path, groups: &[GroupInfo]) -> Result<(), CoreError> {
    std::fs::create_dir_all(dir)
        .map_err(|e| CoreError::Invalid(format!("创建曲谱库目录失败: {e}")))?;
    let path = dir.join("groups.json");
    let tmp = dir.join("groups.json.tmp");
    let json = serde_json::to_string_pretty(groups)
        .map_err(|e| CoreError::Invalid(format!("groups.json 序列化失败: {e}")))?;
    std::fs::write(&tmp, json)
        .map_err(|e| CoreError::Invalid(format!("groups.json 写入失败: {e}")))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| CoreError::Invalid(format!("groups.json 替换失败: {e}")))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 分组管理
// ---------------------------------------------------------------------------

/// 分组列表。
pub fn list_groups(state: &AppState) -> Result<Vec<GroupInfo>, CoreError> {
    let dir = state.library_dir.lock().unwrap().clone();
    Ok(read_groups(&dir))
}

/// 创建分组（名称去空白；同名拒绝）。
pub fn create_group(state: &AppState, name: &str) -> Result<GroupInfo, CoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Invalid("分组名不能为空".into()));
    }
    let dir = state.library_dir.lock().unwrap().clone();
    let mut groups = read_groups(&dir);
    if groups.iter().any(|g| g.name == name) {
        return Err(CoreError::Invalid(format!("分组「{name}」已存在")));
    }
    let group = GroupInfo {
        group_id: format!("group-{}", uuid::Uuid::new_v4()),
        name: name.to_string(),
        created_at_ms: now_ms(),
    };
    groups.push(group.clone());
    write_groups(&dir, &groups)?;
    Ok(group)
}

/// 重命名分组。
pub fn rename_group(state: &AppState, group_id: &str, name: &str) -> Result<GroupInfo, CoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Invalid("分组名不能为空".into()));
    }
    let dir = state.library_dir.lock().unwrap().clone();
    let mut groups = read_groups(&dir);
    if groups
        .iter()
        .any(|g| g.group_id != group_id && g.name == name)
    {
        return Err(CoreError::Invalid(format!("分组「{name}」已存在")));
    }
    let group = groups
        .iter_mut()
        .find(|g| g.group_id == group_id)
        .ok_or_else(|| CoreError::Invalid(format!("分组不存在: {group_id}")))?;
    group.name = name.to_string();
    let result = group.clone();
    write_groups(&dir, &groups)?;
    Ok(result)
}

/// 删除分组；组内曲谱回到未分类。
pub fn delete_group(state: &AppState, group_id: &str) -> Result<(), CoreError> {
    let dir = state.library_dir.lock().unwrap().clone();
    let mut groups = read_groups(&dir);
    groups.retain(|g| g.group_id != group_id);
    write_groups(&dir, &groups)?;
    let mut entries = read_manifest(&dir);
    let mut changed = false;
    for e in entries.iter_mut() {
        if e.group_id.as_deref() == Some(group_id) {
            e.group_id = None;
            changed = true;
        }
    }
    if changed {
        write_manifest(&dir, &entries)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 曲谱管理：移动分组 / 排序 / 删除 / 重命名 / 曲目信息
// ---------------------------------------------------------------------------

/// 将曲谱移入分组；group_id 为 None 时移回未分类。
pub fn move_document_to_group(
    state: &AppState,
    doc_id: &str,
    group_id: Option<String>,
) -> Result<(), CoreError> {
    let dir = state.library_dir.lock().unwrap().clone();
    if let Some(gid) = &group_id {
        if !read_groups(&dir).iter().any(|g| &g.group_id == gid) {
            return Err(CoreError::Invalid(format!("分组不存在: {gid}")));
        }
    }
    let mut entries = read_manifest(&dir);
    let entry = entries
        .iter_mut()
        .find(|e| e.doc_id == doc_id)
        .ok_or_else(|| CoreError::DocumentNotFound(doc_id.to_string()))?;
    entry.group_id = group_id;
    write_manifest(&dir, &entries)
}

/// 按给定 doc_id 顺序重排曲谱库（未提及的条目保持在尾部；忽略不存在的 id）。
pub fn reorder_documents(state: &AppState, doc_ids: &[String]) -> Result<(), CoreError> {
    let dir = state.library_dir.lock().unwrap().clone();
    let mut entries = read_manifest(&dir);
    let mut ordered: Vec<ManifestEntry> = Vec::with_capacity(entries.len());
    for id in doc_ids {
        if let Some(pos) = entries.iter().position(|e| &e.doc_id == id) {
            ordered.push(entries.remove(pos));
        }
    }
    ordered.extend(entries);
    write_manifest(&dir, &ordered)
}

/// 删除曲谱：manifest 移除 + 磁盘文件删除 + 内存缓存移除。
pub fn delete_document(state: &AppState, doc_id: &str) -> Result<(), CoreError> {
    let dir = state.library_dir.lock().unwrap().clone();
    let mut entries = read_manifest(&dir);
    if !entries.iter().any(|e| e.doc_id == doc_id) {
        return Err(CoreError::DocumentNotFound(doc_id.to_string()));
    }
    entries.retain(|e| e.doc_id != doc_id);
    write_manifest(&dir, &entries)?;
    let _ = std::fs::remove_file(dir.join(format!("{doc_id}.mid")));
    state.documents.lock().unwrap().remove(doc_id);
    Ok(())
}

/// 重命名曲谱（库列表与详情页同步生效）。
pub fn rename_document(state: &AppState, doc_id: &str, name: &str) -> Result<(), CoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Invalid("曲名不能为空".into()));
    }
    let dir = state.library_dir.lock().unwrap().clone();
    let mut entries = read_manifest(&dir);
    let entry = entries
        .iter_mut()
        .find(|e| e.doc_id == doc_id)
        .ok_or_else(|| CoreError::DocumentNotFound(doc_id.to_string()))?;
    entry.name = name.to_string();
    write_manifest(&dir, &entries)
}

/// 更新曲目信息（标题/艺术家；空串按 None 处理）。
pub fn update_piece_info(
    state: &AppState,
    doc_id: &str,
    title: Option<String>,
    artist: Option<String>,
) -> Result<(), CoreError> {
    let norm = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let title = norm(title);
    let artist = norm(artist);
    let dir = state.library_dir.lock().unwrap().clone();
    let mut entries = read_manifest(&dir);
    let entry = entries
        .iter_mut()
        .find(|e| e.doc_id == doc_id)
        .ok_or_else(|| CoreError::DocumentNotFound(doc_id.to_string()))?;
    entry.title = title;
    entry.artist = artist;
    write_manifest(&dir, &entries)
}

//! Game Profile 加载与校验（ADR-0004）。
//!
//! Profile 目录约定：`game-profiles/<id>/` 下含 `profile.json`、
//! `windows-keymap.json`、`android-layout.json`（后两者可缺省）。

use scoreleap_music_ir::{GameProfile, InstrumentLayout, KeyCode, KeySlot};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// 解析错误。
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("profile 目录不存在: {0}")]
    NotFound(PathBuf),
    #[error("profile.json 解析失败: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("profile 校验失败: {0}")]
    Invalid(String),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}

/// profile.json 的磁盘结构（含文件引用）。
#[derive(Debug, Deserialize)]
struct ProfileFile {
    id: String,
    #[serde(default)]
    display_name: String,
    #[serde(default = "default_version")]
    version: u32,
    instrument: InstrumentFile,
    #[serde(default)]
    keymap_windows: Option<String>,
    #[serde(default)]
    layout_android: Option<String>,
    #[serde(default)]
    warning: String,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
struct InstrumentFile {
    keys: u8,
    midi_low: u8,
    midi_high: u8,
    #[serde(default = "default_polyphony")]
    max_polyphony: u8,
}

fn default_polyphony() -> u8 {
    4
}

/// Windows 键位映射文件：`{ "note": "scan" | "ext:scan" }`。
#[derive(Debug, Deserialize)]
struct KeymapFile {
    #[serde(flatten)]
    entries: HashMap<String, String>,
}

/// Android 布局文件。
#[derive(Debug, Deserialize)]
struct LayoutFile {
    keys: Vec<LayoutKeyFile>,
}

#[derive(Debug, Deserialize)]
struct LayoutKeyFile {
    note: u8,
    x: f32,
    y: f32,
}

/// Profile 存储：从目录加载并缓存。
#[derive(Debug, Default)]
pub struct ProfileStore {
    root: Option<PathBuf>,
    cache: HashMap<String, GameProfile>,
}

impl ProfileStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        ProfileStore {
            root: Some(root.into()),
            cache: HashMap::new(),
        }
    }

    /// 列出可用 profile id（目录名）。
    pub fn list_ids(&self) -> Result<Vec<String>, ProfileError> {
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| ProfileError::Invalid("ProfileStore 未配置根目录".to_string()))?;
        if !root.exists() {
            return Ok(vec![]);
        }
        let mut ids = vec![];
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if entry.path().is_dir() && entry.path().join("profile.json").exists() {
                ids.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// 加载并校验 profile（带缓存）。
    pub fn load(&mut self, id: &str) -> Result<GameProfile, ProfileError> {
        if let Some(p) = self.cache.get(id) {
            return Ok(p.clone());
        }
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| ProfileError::Invalid("ProfileStore 未配置根目录".to_string()))?;
        let dir = root.join(id);
        if !dir.exists() {
            return Err(ProfileError::NotFound(dir));
        }
        let profile = load_profile(&dir)?;
        self.cache.insert(id.to_string(), profile.clone());
        Ok(profile)
    }
}

/// 从目录加载并校验一个 profile。
pub fn load_profile(dir: &Path) -> Result<GameProfile, ProfileError> {
    let file_path = dir.join("profile.json");
    let raw = std::fs::read_to_string(&file_path)?;
    let pf: ProfileFile = serde_json::from_str(&raw)?;

    if pf.instrument.keys == 0 {
        return Err(ProfileError::Invalid("instrument.keys 必须 > 0".into()));
    }
    if pf.instrument.midi_low > pf.instrument.midi_high {
        return Err(ProfileError::Invalid(format!(
            "音域非法: midi_low {} > midi_high {}",
            pf.instrument.midi_low, pf.instrument.midi_high
        )));
    }
    let key_count = (pf.instrument.midi_high as i32 - pf.instrument.midi_low as i32 + 1) as u32;
    if key_count != pf.instrument.keys as u32 {
        return Err(ProfileError::Invalid(format!(
            "键数不一致: keys={} 但音域跨度={}",
            pf.instrument.keys, key_count
        )));
    }

    // Windows 键位映射
    let mut keymap = HashMap::new();
    let mut seen_codes = HashSet::new();
    if let Some(km) = &pf.keymap_windows {
        let km_path = dir.join(km);
        let km_raw = std::fs::read_to_string(km_path)?;
        let kmf: KeymapFile = serde_json::from_str(&km_raw)?;
        for (note_str, code_str) in &kmf.entries {
            let note: u8 = note_str
                .parse()
                .map_err(|_| ProfileError::Invalid(format!("非法音名键: {note_str}")))?;
            if note < pf.instrument.midi_low || note > pf.instrument.midi_high {
                return Err(ProfileError::Invalid(format!(
                    "键位映射包含音域外音符: {note}"
                )));
            }
            let code = parse_keycode(code_str)?;
            if keymap.insert(note, code).is_some() {
                return Err(ProfileError::Invalid(format!("重复映射: {note}")));
            }
            if !seen_codes.insert(code) {
                return Err(ProfileError::Invalid(format!(
                    "同一扫描码映射到多个音符: {code_str}"
                )));
            }
        }
        // 音域内每个半音都必须有映射
        for n in pf.instrument.midi_low..=pf.instrument.midi_high {
            if !keymap.contains_key(&n) {
                return Err(ProfileError::Invalid(format!("键位映射缺少音符 {n}")));
            }
        }
    }

    // Android 布局
    let layout = if let Some(lf) = &pf.layout_android {
        let l_path = dir.join(lf);
        let l_raw = std::fs::read_to_string(l_path)?;
        let lf2: LayoutFile = serde_json::from_str(&l_raw)?;
        let mut slots = Vec::with_capacity(lf2.keys.len());
        let mut seen = HashSet::new();
        for k in lf2.keys {
            if k.note < pf.instrument.midi_low || k.note > pf.instrument.midi_high {
                return Err(ProfileError::Invalid(format!(
                    "布局包含音域外音符: {}",
                    k.note
                )));
            }
            if !(0.0..=1.0).contains(&k.x) || !(0.0..=1.0).contains(&k.y) {
                return Err(ProfileError::Invalid(format!(
                    "坐标越界: note={} ({}, {})",
                    k.note, k.x, k.y
                )));
            }
            if !seen.insert(k.note) {
                return Err(ProfileError::Invalid(format!("布局重复音符: {}", k.note)));
            }
            slots.push(KeySlot {
                note: k.note,
                x: k.x,
                y: k.y,
            });
        }
        InstrumentLayout { keys: slots }
    } else {
        InstrumentLayout { keys: vec![] }
    };

    Ok(GameProfile {
        id: pf.id.clone(),
        display_name: if pf.display_name.is_empty() {
            pf.id
        } else {
            pf.display_name
        },
        version: pf.version,
        keys: pf.instrument.keys,
        midi_low: pf.instrument.midi_low,
        midi_high: pf.instrument.midi_high,
        max_polyphony: pf.instrument.max_polyphony,
        keymap,
        layout,
        warning: pf.warning,
    })
}

/// 解析键位值："1E" → Scan(0x1E)；"E0:1D" → ExtendedScan(0x1D)。
fn parse_keycode(s: &str) -> Result<KeyCode, ProfileError> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("E0:") {
        let v = u16::from_str_radix(rest, 16)
            .map_err(|_| ProfileError::Invalid(format!("非法扩展扫描码: {s}")))?;
        Ok(KeyCode::ExtendedScan(v))
    } else {
        let v = u16::from_str_radix(s, 16)
            .map_err(|_| ProfileError::Invalid(format!("非法扫描码: {s}")))?;
        Ok(KeyCode::Scan(v))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::all)] // 测试代码风格类警告不阻塞
    use super::*;
    use std::fs;

    fn write(dir: &Path, name: &str, content: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn valid_profile_loads() {
        let dir = std::env::temp_dir().join("slp-test-valid");
        let _ = fs::remove_dir_all(&dir);
        write(
            &dir,
            "profile.json",
            r#"{
                "id": "test",
                "display_name": "Test Instrument",
                "version": 1,
                "instrument": { "keys": 3, "midi_low": 60, "midi_high": 62, "max_polyphony": 2 },
                "keymap_windows": "windows-keymap.json",
                "layout_android": "android-layout.json",
                "warning": "test only"
            }"#,
        );
        write(
            &dir,
            "windows-keymap.json",
            r#"{ "60": "1E", "61": "11", "62": "E0:1D" }"#,
        );
        write(
            &dir,
            "android-layout.json",
            r#"{ "keys": [ { "note": 60, "x": 0.1, "y": 0.5 }, { "note": 61, "x": 0.2, "y": 0.5 }, { "note": 62, "x": 0.3, "y": 0.5 } ] }"#,
        );
        let p = load_profile(&dir).unwrap();
        assert_eq!(p.keys, 3);
        assert_eq!(p.keymap.len(), 3);
        assert_eq!(p.keymap[&62], KeyCode::ExtendedScan(0x1D));
        assert_eq!(p.layout.keys.len(), 3);
    }

    #[test]
    fn invalid_key_count_rejected() {
        let dir = std::env::temp_dir().join("slp-test-badcount");
        let _ = fs::remove_dir_all(&dir);
        write(
            &dir,
            "profile.json",
            r#"{
                "id": "test",
                "instrument": { "keys": 5, "midi_low": 60, "midi_high": 62 }
            }"#,
        );
        let err = load_profile(&dir).unwrap_err();
        assert!(err.to_string().contains("键数不一致"));
    }

    #[test]
    fn missing_keymap_entry_rejected() {
        let dir = std::env::temp_dir().join("slp-test-missingkey");
        let _ = fs::remove_dir_all(&dir);
        write(
            &dir,
            "profile.json",
            r#"{
                "id": "test",
                "instrument": { "keys": 3, "midi_low": 60, "midi_high": 62 },
                "keymap_windows": "windows-keymap.json"
            }"#,
        );
        write(&dir, "windows-keymap.json", r#"{ "60": "1E" }"#);
        let err = load_profile(&dir).unwrap_err();
        assert!(err.to_string().contains("缺少音符"));
    }

    #[test]
    fn duplicate_mapping_rejected() {
        let dir = std::env::temp_dir().join("slp-test-dup");
        let _ = fs::remove_dir_all(&dir);
        write(
            &dir,
            "profile.json",
            r#"{
                "id": "test",
                "instrument": { "keys": 3, "midi_low": 60, "midi_high": 62 },
                "keymap_windows": "windows-keymap.json"
            }"#,
        );
        write(
            &dir,
            "windows-keymap.json",
            r#"{ "60": "1E", "61": "1E", "62": "11" }"#,
        );
        let err = load_profile(&dir).unwrap_err();
        assert!(err.to_string().contains("扫描码映射到多个音符"));
    }

    #[test]
    fn missing_file_errors() {
        let dir = std::env::temp_dir().join("slp-test-nofile");
        let _ = fs::remove_dir_all(&dir);
        let err = load_profile(&dir).unwrap_err();
        assert!(matches!(err, ProfileError::Io(_)));
    }
}

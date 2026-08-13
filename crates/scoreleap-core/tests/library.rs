//! scoreleap-core 曲谱库持久化集成测试（不依赖 Tauri runtime）。

use scoreleap_core::{
    compile, get_sequence_notes, get_tracks, import_midi, list_documents, AppState,
};
use scoreleap_midi::parse_midi;
use std::path::PathBuf;
use std::sync::Mutex;

/// 构造一个 C 大调音阶 SMF 并写入给定路径。
fn write_smf(path: &PathBuf) {
    use midly::num::u28;
    use midly::{Format, Header, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
    let mut track: Vec<TrackEvent> = Vec::new();
    for note in [60u8, 62, 64, 65, 67, 69, 71, 72] {
        // 每个音符：NoteOn 紧跟前一 NoteOff（delta=0），持续 480 ticks 后 NoteOff
        track.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Midi {
                channel: 0.into(),
                message: MidiMessage::NoteOn {
                    key: note.into(),
                    vel: 100.into(),
                },
            },
        });
        track.push(TrackEvent {
            delta: u28::new(480),
            kind: TrackEventKind::Midi {
                channel: 0.into(),
                message: MidiMessage::NoteOff {
                    key: note.into(),
                    vel: 0.into(),
                },
            },
        });
    }
    let mut smf = Smf::new(Header::new(
        Format::SingleTrack,
        Timing::Metrical(480.into()),
    ));
    smf.tracks.push(track);
    let mut bytes = Vec::new();
    smf.write(&mut bytes).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// 构造带 keymap 的测试环境：临时目录 + 初始状态。
fn setup() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState {
        library_dir: Mutex::new(dir.path().join("library")),
        profile_dir: Mutex::new(dir.path().join("profiles")),
        ..Default::default()
    };
    // 复制真实 identity-v Profile（与打包/开发目录一致：src-tauri/resources/game-profiles）
    let src_profile = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/scoreleap/src-tauri/resources/game-profiles/identity-v");
    assert!(src_profile.exists(), "缺少 identity-v Profile 资源");
    let dst = dir.path().join("profiles/identity-v");
    std::fs::create_dir_all(&dst).unwrap();
    for entry in std::fs::read_dir(&src_profile).unwrap() {
        let e = entry.unwrap();
        std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
    }
    (dir, state)
}

/// 最小有效 SMF format 0（division 96，1 个 note_on/off）。
const VALID_MIDI: &[u8] = &[
    0x4D, 0x54, 0x68, 0x64, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x00, 0x60, //
    0x4D, 0x54, 0x72, 0x6B, 0x00, 0x00, 0x00, 0x0B, //
    0x00, 0x90, 0x3C, 0x64, 0x60, 0x80, 0x3C, 0x00, 0x00, 0xFF, 0x2F, 0x00,
];

#[test]
fn import_then_list_roundtrip() {
    let (dir, state) = setup();
    let src = dir.path().join("song.mid");
    write_smf(&src);

    let summary = import_midi(&state, src.to_string_lossy().to_string()).unwrap();
    assert_eq!(summary.note_count, 8);

    // 文件已复制到曲谱库
    assert!(dir
        .path()
        .join("library")
        .join(format!("{}.mid", summary.doc_id))
        .exists());

    // list_documents 与摘要一致
    let docs = list_documents(&state).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].doc_id, summary.doc_id);
    assert_eq!(docs[0].note_count, 8);
}

#[test]
fn restart_reloads_document() {
    let (dir, state1) = setup();
    let src = dir.path().join("song.mid");
    write_smf(&src);
    let summary = import_midi(&state1, src.to_string_lossy().to_string()).unwrap();

    // 模拟重启：新 AppState（同一 library_dir），内存为空
    let state2 = AppState {
        library_dir: Mutex::new(dir.path().join("library")),
        profile_dir: Mutex::new(dir.path().join("profiles")),
        ..Default::default()
    };
    assert!(state2.documents.lock().unwrap().is_empty());

    let tracks = get_tracks(&state2, summary.doc_id.clone()).unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].note_count, 8);
}

#[test]
fn compile_caches_sequence_notes() {
    let (dir, state) = setup();
    let src = dir.path().join("song.mid");
    write_smf(&src);
    let summary = import_midi(&state, src.to_string_lossy().to_string()).unwrap();

    // 加载 Profile
    let profile = scoreleap_core::load_profile(&state, "identity-v".into()).unwrap();
    assert_eq!(profile.id, "identity-v");

    let opts = scoreleap_arranger::ArrangementOptions {
        auto_fit_range: false,
        transpose_semitones: 0,
        range_strategy: scoreleap_arranger::RangeStrategy::OctaveDown,
        max_polyphony: 4,
        quantize_grid: None,
        simplify_chords: false,
        melody_protection: false,
    };
    let compiled = compile(&state, summary.doc_id, vec![0], opts).unwrap();
    assert_eq!(compiled.stats.output_notes, 8);

    // 音符缓存：数量一致、时间有序非负
    let notes = get_sequence_notes(&state, compiled.seq_id).unwrap();
    assert_eq!(notes.len(), 8);
    let mut prev_end = 0i64;
    for n in &notes {
        assert!(n.start_us >= prev_end);
        assert!(n.duration_us > 0);
        prev_end = n.start_us + n.duration_us;
    }
    // 单轨八分音符 500ms（120BPM）——C 大调音阶
    assert_eq!(notes[0].start_us, 0);
    assert_eq!(notes[0].duration_us, 500_000);
    assert_eq!(notes[7].start_us, 3_500_000);

    // 未知 seq_id 报错
    assert!(get_sequence_notes(&state, "seq-nope".into()).is_err());
}

#[test]
fn transcription_import_sets_source_type() {
    let (dir, state) = setup();
    // 构造一个最小有效 MIDI 并写入临时路径（模拟转录产物）
    let lib = dir.path().join("library");
    std::fs::create_dir_all(&lib).unwrap();
    let generated = dir.path().join("generated.mid");
    std::fs::write(&generated, VALID_MIDI).unwrap();

    let summary = scoreleap_core::import_midi_from_path(
        &state,
        generated.to_str().unwrap(),
        "测试（音频转录）",
        "audio_transcription",
        scoreleap_core::ImportMeta::default(),
    )
    .unwrap();
    assert_eq!(summary.source_type, "audio_transcription");
    assert_eq!(summary.name, "测试（音频转录）");

    // 重启后（新 AppState 从 manifest 读取）source_type 保留
    let state2 = scoreleap_core::AppState::default();
    *state2.library_dir.lock().unwrap() = lib;
    let docs = scoreleap_core::list_documents(&state2).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].source_type, "audio_transcription");
    assert_eq!(docs[0].name, "测试（音频转录）");
}

#[test]
fn corrupt_manifest_resets_empty() {
    let (dir, state) = setup();
    let lib = dir.path().join("library");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("manifest.json"), "{ not valid json !!").unwrap();

    let docs = list_documents(&state).unwrap();
    assert!(docs.is_empty());
    // 损坏 manifest 被重置为合法空数组
    let after = std::fs::read_to_string(lib.join("manifest.json")).unwrap();
    let _: serde_json::Value = serde_json::from_str(&after).unwrap();
}

#[test]
fn missing_source_file_filtered() {
    let (dir, state) = setup();
    let lib = dir.path().join("library");
    std::fs::create_dir_all(&lib).unwrap();
    // 手工构造 manifest：一条有文件、一条无文件
    let json = serde_json::json!([
        {
            "doc_id": "doc-a", "name": "a.mid", "format": "SingleTrack",
            "track_count": 1, "note_count": 1, "duration_ms": 1000,
            "bpm_range": [120.0, 120.0], "imported_at": 1
        },
        {
            "doc_id": "doc-b", "name": "b.mid", "format": "SingleTrack",
            "track_count": 1, "note_count": 1, "duration_ms": 1000,
            "bpm_range": [120.0, 120.0], "imported_at": 2
        }
    ]);
    std::fs::write(lib.join("manifest.json"), json.to_string()).unwrap();
    std::fs::write(lib.join("doc-a.mid"), b"MThd").unwrap();

    let docs = list_documents(&state).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].doc_id, "doc-a");
}

#[test]
fn parse_known_smf_bytes() {
    // 冒烟：parse_midi 对测试生成文件可解析（依赖正确性由 midi crate 覆盖）
    let (dir, _) = setup();
    let src = dir.path().join("song.mid");
    write_smf(&src);
    let bytes = std::fs::read(&src).unwrap();
    let doc = parse_midi(&bytes).unwrap();
    assert_eq!(doc.note_count(), 8);
}

// ---------------------------------------------------------------------------
// Issue #59：自动去重
// ---------------------------------------------------------------------------

#[test]
fn duplicate_import_skipped_by_hash() {
    let (dir, state) = setup();
    let src = dir.path().join("song.mid");
    write_smf(&src);

    let first = import_midi(&state, src.to_string_lossy().to_string()).unwrap();
    assert!(!first.duplicated);
    assert_eq!(list_documents(&state).unwrap().len(), 1);

    // 同一文件再次导入：内容指纹一致 → 自动跳过
    let second = import_midi(&state, src.to_string_lossy().to_string()).unwrap();
    assert!(second.duplicated);
    assert!(second.doc_id.is_empty());
    assert_eq!(second.duplicate_of.as_deref(), Some(first.doc_id.as_str()));
    assert_eq!(list_documents(&state).unwrap().len(), 1);
}

#[test]
fn duplicate_import_skipped_by_name_and_source() {
    let (dir, state) = setup();
    // 内容不同但曲名 + 来源类型相同的两条（模拟同一首歌转录两次）
    let src1 = dir.path().join("take1.mid");
    let src2 = dir.path().join("take2.mid");
    write_smf(&src1);
    std::fs::write(&src2, VALID_MIDI).unwrap();

    let first = scoreleap_core::import_midi_from_path(
        &state,
        src1.to_str().unwrap(),
        "同一首歌（音频转录）",
        "audio_transcription",
        scoreleap_core::ImportMeta::default(),
    )
    .unwrap();
    assert!(!first.duplicated);

    let second = scoreleap_core::import_midi_from_path(
        &state,
        src2.to_str().unwrap(),
        "同一首歌（音频转录）",
        "audio_transcription",
        scoreleap_core::ImportMeta::default(),
    )
    .unwrap();
    assert!(second.duplicated);
    assert_eq!(list_documents(&state).unwrap().len(), 1);
}

// ---------------------------------------------------------------------------
// Issue #59：分组管理
// ---------------------------------------------------------------------------

#[test]
fn group_crud_and_delete_moves_docs_to_ungrouped() {
    let (dir, state) = setup();
    let src = dir.path().join("song.mid");
    write_smf(&src);
    let summary = import_midi(&state, src.to_string_lossy().to_string()).unwrap();

    // 创建 + 移动曲谱进组
    let group = scoreleap_core::create_group(&state, "练习曲").unwrap();
    scoreleap_core::move_document_to_group(&state, &summary.doc_id, Some(group.group_id.clone()))
        .unwrap();
    let docs = list_documents(&state).unwrap();
    assert_eq!(docs[0].group_id.as_deref(), Some(group.group_id.as_str()));

    // 重命名
    let renamed = scoreleap_core::rename_group(&state, &group.group_id, "考级曲").unwrap();
    assert_eq!(renamed.name, "考级曲");
    assert_eq!(
        scoreleap_core::list_groups(&state).unwrap()[0].name,
        "考级曲"
    );

    // 移回未分类
    scoreleap_core::move_document_to_group(&state, &summary.doc_id, None).unwrap();
    assert_eq!(list_documents(&state).unwrap()[0].group_id, None);

    // 再移入后删除分组 → 曲谱回未分类
    scoreleap_core::move_document_to_group(&state, &summary.doc_id, Some(group.group_id.clone()))
        .unwrap();
    scoreleap_core::delete_group(&state, &group.group_id).unwrap();
    assert!(scoreleap_core::list_groups(&state).unwrap().is_empty());
    assert_eq!(list_documents(&state).unwrap()[0].group_id, None);

    // 同名分组拒绝、空名拒绝、移动到不存在分组报错
    assert!(scoreleap_core::create_group(&state, "练习曲").is_ok());
    assert!(scoreleap_core::create_group(&state, "练习曲").is_err());
    assert!(scoreleap_core::create_group(&state, "  ").is_err());
    assert!(scoreleap_core::move_document_to_group(
        &state,
        &summary.doc_id,
        Some("group-nope".into())
    )
    .is_err());
}

#[test]
fn groups_persist_across_restart() {
    let (dir, state) = setup();
    let group = scoreleap_core::create_group(&state, "专辑 A").unwrap();

    // 模拟重启：新 AppState（同一 library_dir）
    let state2 = AppState {
        library_dir: Mutex::new(dir.path().join("library")),
        profile_dir: Mutex::new(dir.path().join("profiles")),
        ..Default::default()
    };
    let groups = scoreleap_core::list_groups(&state2).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].group_id, group.group_id);
    assert_eq!(groups[0].name, "专辑 A");
}

// ---------------------------------------------------------------------------
// Issue #59：排序 / 删除 / 重命名 / 曲目信息 / 转录元数据
// ---------------------------------------------------------------------------

#[test]
fn reorder_documents_persists() {
    let (dir, state) = setup();
    let src1 = dir.path().join("a.mid");
    let src2 = dir.path().join("b.mid");
    write_smf(&src1);
    std::fs::write(&src2, VALID_MIDI).unwrap();
    let s1 = import_midi(&state, src1.to_string_lossy().to_string()).unwrap();
    let s2 = import_midi(&state, src2.to_string_lossy().to_string()).unwrap();

    // 新导入追加到末尾：顺序 [s1, s2]
    let docs = list_documents(&state).unwrap();
    assert_eq!(docs[0].doc_id, s1.doc_id);
    assert_eq!(docs[1].doc_id, s2.doc_id);

    // 反转顺序并验证重启后保留
    scoreleap_core::reorder_documents(&state, &[s2.doc_id.clone(), s1.doc_id.clone()]).unwrap();
    let state2 = AppState {
        library_dir: Mutex::new(dir.path().join("library")),
        profile_dir: Mutex::new(dir.path().join("profiles")),
        ..Default::default()
    };
    let docs = list_documents(&state2).unwrap();
    assert_eq!(docs[0].doc_id, s2.doc_id);
    assert_eq!(docs[1].doc_id, s1.doc_id);
}

#[test]
fn delete_document_removes_file_and_entry() {
    let (dir, state) = setup();
    let src = dir.path().join("song.mid");
    write_smf(&src);
    let summary = import_midi(&state, src.to_string_lossy().to_string()).unwrap();

    scoreleap_core::delete_document(&state, &summary.doc_id).unwrap();
    assert!(list_documents(&state).unwrap().is_empty());
    assert!(!dir
        .path()
        .join("library")
        .join(format!("{}.mid", summary.doc_id))
        .exists());

    // 未知 doc_id 报错；删除后同一文件可重新导入
    assert!(scoreleap_core::delete_document(&state, "doc-nope").is_err());
    let again = import_midi(&state, src.to_string_lossy().to_string()).unwrap();
    assert!(!again.duplicated);
    assert_eq!(list_documents(&state).unwrap().len(), 1);
}

#[test]
fn rename_document_updates_name() {
    let (dir, state) = setup();
    let src = dir.path().join("song.mid");
    write_smf(&src);
    let summary = import_midi(&state, src.to_string_lossy().to_string()).unwrap();

    scoreleap_core::rename_document(&state, &summary.doc_id, "新名字").unwrap();
    assert_eq!(list_documents(&state).unwrap()[0].name, "新名字");
    assert!(scoreleap_core::rename_document(&state, &summary.doc_id, "  ").is_err());
}

#[test]
fn piece_info_update_persists() {
    let (dir, state) = setup();
    let src = dir.path().join("song.mid");
    write_smf(&src);
    let summary = import_midi(&state, src.to_string_lossy().to_string()).unwrap();

    scoreleap_core::update_piece_info(
        &state,
        &summary.doc_id,
        Some("致爱丽丝".into()),
        Some("贝多芬".into()),
    )
    .unwrap();
    let docs = list_documents(&state).unwrap();
    assert_eq!(docs[0].title.as_deref(), Some("致爱丽丝"));
    assert_eq!(docs[0].artist.as_deref(), Some("贝多芬"));

    // 空白串按 None 处理
    scoreleap_core::update_piece_info(&state, &summary.doc_id, Some("   ".into()), None).unwrap();
    let docs = list_documents(&state).unwrap();
    assert_eq!(docs[0].title, None);
    assert_eq!(docs[0].artist, None);
}

#[test]
fn transcription_meta_persists_with_import() {
    let (dir, state) = setup();
    let lib = dir.path().join("library");
    std::fs::create_dir_all(&lib).unwrap();
    let generated = dir.path().join("generated.mid");
    std::fs::write(&generated, VALID_MIDI).unwrap();

    let meta = scoreleap_core::TranscriptionMeta {
        engine: "scoreleap-transkun-v2".into(),
        engine_version: "1.0.0".into(),
        model_file: "2.0.pt".into(),
        elapsed_ms: 12345,
        // completed_at_ms 为 0 时导入自动填充当前时间
        completed_at_ms: 0,
    };
    let summary = scoreleap_core::import_midi_from_path(
        &state,
        generated.to_str().unwrap(),
        "测试（音频转录）",
        "audio_transcription",
        scoreleap_core::ImportMeta {
            transcription: Some(meta),
            title: Some("致爱丽丝".into()),
            artist: Some("贝多芬".into()),
        },
    )
    .unwrap();
    assert!(!summary.duplicated);

    // 重启后转录元数据与曲目信息保留
    let state2 = scoreleap_core::AppState::default();
    *state2.library_dir.lock().unwrap() = lib;
    let docs = scoreleap_core::list_documents(&state2).unwrap();
    assert_eq!(docs.len(), 1);
    let tx = docs[0].transcription.as_ref().unwrap();
    assert_eq!(tx.engine, "scoreleap-transkun-v2");
    assert_eq!(tx.engine_version, "1.0.0");
    assert_eq!(tx.model_file, "2.0.pt");
    assert_eq!(tx.elapsed_ms, 12345);
    assert!(tx.completed_at_ms > 0);
    assert_eq!(docs[0].title.as_deref(), Some("致爱丽丝"));
    assert_eq!(docs[0].artist.as_deref(), Some("贝多芬"));
    assert!(docs[0].content_hash.is_some());
    assert!(docs[0].imported_at > 0);
}

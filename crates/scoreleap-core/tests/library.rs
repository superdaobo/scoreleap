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
    // 复制真实 identity-v Profile（与打包/开发目录一致）
    let src_profile =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../game-profiles/identity-v");
    assert!(src_profile.exists(), "缺少 game-profiles/identity-v");
    let dst = dir.path().join("profiles/identity-v");
    std::fs::create_dir_all(&dst).unwrap();
    for entry in std::fs::read_dir(&src_profile).unwrap() {
        let e = entry.unwrap();
        std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
    }
    (dir, state)
}

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

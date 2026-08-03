//! 黄金样例集成测试：解析 fixtures/midi/*.mid 并断言关键属性。
//! fixture 文件由 scripts/gen-fixtures.mjs 生成（构造说明见 fixtures/README.md）。

use scoreleap_midi::parse_midi;

fn load(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../../fixtures/midi/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("读取 fixture {name} 失败: {e}"))
}

#[test]
fn single_track_scale() {
    let doc = parse_midi(&load("single-track.mid")).unwrap();
    assert_eq!(doc.tracks.len(), 1);
    // C 大调音阶 8 个四分音符（MIDI 60-67），120BPM，division 480 → 每音 500ms
    let notes = &doc.tracks[0].notes;
    assert_eq!(notes.len(), 8);
    for (i, n) in notes.iter().enumerate() {
        assert_eq!(n.note, 60 + i as u8);
        assert_eq!(n.start_us, i as i64 * 500_000);
        assert_eq!(n.duration_us, 500_000);
    }
    assert_eq!(doc.duration_us, 4_000_000);
    // tempo 120BPM
    assert_eq!(doc.tempo_events[0].tempo_us_per_quarter, 500_000);
}

#[test]
fn multi_track_with_tempo_change() {
    let doc = parse_midi(&load("multi-track.mid")).unwrap();
    assert_eq!(doc.tracks.len(), 3);
    // tempo：120BPM @0 → 240BPM @1440 tick（3 拍 @120BPM = 1.5s）
    assert_eq!(doc.tempo_events.len(), 2);
    assert_eq!(doc.tempo_events[0].tempo_us_per_quarter, 500_000);
    assert_eq!(doc.tempo_events[1].time_us, 1_500_000);
    assert_eq!(doc.tempo_events[1].tempo_us_per_quarter, 250_000);
    // 拍号 4/4
    assert_eq!(doc.time_signature_events[0].numerator, 4);
    assert_eq!(doc.time_signature_events[0].denominator, 4);
    // 轨道 1 旋律 4 音；轨道 2 和弦 3 音
    assert_eq!(doc.tracks[1].notes.len(), 4);
    assert_eq!(doc.tracks[2].notes.len(), 3);
}

#[test]
fn running_status_file_parses() {
    let doc = parse_midi(&load("running-status.mid")).unwrap();
    assert!(!doc.tracks.is_empty());
    // 5 个连续 NoteOn（running status 压缩）
    assert_eq!(doc.tracks[0].notes.len(), 5);
}

#[test]
fn velocity_zero_as_note_off() {
    let doc = parse_midi(&load("velocity-zero.mid")).unwrap();
    let notes = &doc.tracks[0].notes;
    assert!(!notes.is_empty());
    // 全部音符有正时值（vel=0 正确关闭）
    for n in notes {
        assert!(n.duration_us > 0);
        assert!(n.velocity > 0);
    }
}

#[test]
fn repeated_notes_preserved() {
    let doc = parse_midi(&load("repeated-notes.mid")).unwrap();
    // C5 连续两次 + C4 对照 = 3 个音符
    assert_eq!(doc.tracks[0].notes.len(), 3);
}

#[test]
fn out_of_range_notes_parsed() {
    let doc = parse_midi(&load("out-of-range.mid")).unwrap();
    let notes = &doc.tracks[0].notes;
    assert_eq!(notes.len(), 3);
    assert!(notes.iter().any(|n| n.note == 20));
    assert!(notes.iter().any(|n| n.note == 100));
}

#[test]
fn invalid_file_rejected() {
    assert!(parse_midi(b"garbage").is_err());
}

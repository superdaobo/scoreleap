//! 黄金样例集成测试：fixtures/midi → 编排 → 断言 CompiledSequence。
//! 固定使用 ArrangementOptions::default() + game-profiles/identity-v。

use scoreleap_arranger::{arrange, ArrangementOptions};
use scoreleap_game_profile::load_profile;
use scoreleap_midi::parse_midi;
use scoreleap_sequence::PlatformAction;

fn profile() -> scoreleap_music_ir::GameProfile {
    let path = format!(
        "{}/../../game-profiles/identity-v",
        env!("CARGO_MANIFEST_DIR")
    );
    load_profile(std::path::Path::new(&path)).expect("identity-v profile 应可加载")
}

fn doc(name: &str) -> scoreleap_music_ir::MusicDocument {
    let path = format!(
        "{}/../../fixtures/midi/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let bytes = std::fs::read(&path).unwrap();
    parse_midi(&bytes).unwrap()
}

#[test]
fn identity_v_profile_valid() {
    let p = profile();
    assert_eq!(p.id, "identity-v");
    assert_eq!(p.keys, 36);
    assert_eq!(p.keymap.len(), 36);
    assert_eq!(p.midi_low, 48);
    assert_eq!(p.midi_high, 83);
}

#[test]
fn single_track_compiles_to_16_actions() {
    let d = doc("single-track.mid");
    let p = profile();
    let (seq, stats) = arrange(&d, &ArrangementOptions::default(), &p, &[0]).unwrap();
    assert_eq!(stats.output_notes, 8);
    // 8 音符 × (Down + Up) = 16
    assert_eq!(seq.actions.len(), 16);
    assert_eq!(seq.duration_us, 4_000_000);
    // 每个 KeyDown 有对应 KeyUp，时值 500ms；Down 不晚于同刻 Up
    let mut downs: std::collections::HashMap<scoreleap_music_ir::KeyCode, i64> = Default::default();
    let mut last_at = -1i64;
    for a in &seq.actions {
        assert!(a.at_us() >= last_at, "动作时间应单调不减");
        last_at = a.at_us();
        match a {
            PlatformAction::KeyDown { at_us, key } => {
                assert!(downs.insert(*key, *at_us).is_none(), "重复 Down: {key:?}");
            }
            PlatformAction::KeyUp { at_us, key } => {
                let d = downs.remove(key).expect("KeyUp 前应有 KeyDown");
                assert_eq!(at_us - d, 500_000);
            }
            _ => panic!("意外动作"),
        }
    }
    assert!(downs.is_empty(), "所有按键都应抬起");
}

#[test]
fn multi_track_melody_only() {
    let d = doc("multi-track.mid");
    let p = profile();
    // 只启用旋律轨（轨道 1）
    let (seq, stats) = arrange(&d, &ArrangementOptions::default(), &p, &[1]).unwrap();
    assert_eq!(stats.output_notes, 4);
    assert_eq!(seq.actions.len(), 8);
    // 音符 60, 64, 67, 72 在音域内（48-83），无需移调/折叠
    assert_eq!(stats.applied_transpose, 0);
    assert_eq!(stats.folded, 0);
}

#[test]
fn out_of_range_folded_or_dropped() {
    let d = doc("out-of-range.mid");
    let p = profile();
    let (seq, stats) = arrange(&d, &ArrangementOptions::default(), &p, &[0]).unwrap();
    // 100 → 折叠（降八度 88 → 76 落入）；20 → 升八度 32 仍低于 48 → 再升 44 → 仍低于 → 丢弃
    // 60 保留。总计：3 输入，60/100 输出或按策略统计
    assert_eq!(stats.input_notes, 3);
    assert!(stats.output_notes >= 2);
    assert_eq!(seq.actions.len(), stats.output_notes * 2);
    // 所有输出音符都映射到音域内按键
    for a in &seq.actions {
        match a {
            PlatformAction::KeyDown { key, .. } | PlatformAction::KeyUp { key, .. } => {
                assert!(p.keymap.values().any(|k| k == key));
            }
            _ => {}
        }
    }
}

#[test]
fn polyphony_limited_on_chord_track() {
    let d = doc("multi-track.mid");
    let p = profile();
    // 和弦轨（轨道 2）C4+E4+G4 同时发声，max 默认 4 > 3 → 不裁剪
    let (seq, stats) = arrange(&d, &ArrangementOptions::default(), &p, &[2]).unwrap();
    assert_eq!(stats.output_notes, 3);
    assert_eq!(seq.actions.len(), 6);
    // 同刻触发（和弦同时）
    let downs: Vec<i64> = seq
        .actions
        .iter()
        .filter_map(|a| match a {
            PlatformAction::KeyDown { at_us, .. } => Some(*at_us),
            _ => None,
        })
        .collect();
    assert_eq!(downs.len(), 3);
    assert_eq!(downs[0], downs[1]);
    assert_eq!(downs[1], downs[2]);
}

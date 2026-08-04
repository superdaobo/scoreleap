//! 端到端验证工具：解析 MIDI 文件 → 加载默认 Profile → 编排 → 打印统计。
//!
//! 用法：cargo run -p scoreleap-arranger --example compile_file -- <midi 文件路径>

use scoreleap_arranger::{arrange, ArrangementOptions};
use scoreleap_game_profile::load_profile;
use scoreleap_midi::parse_midi;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "fixtures/midi/single-track.mid".to_string());
    println!("解析: {path}");

    let bytes = std::fs::read(&path).expect("读取 MIDI 文件失败");
    let doc = parse_midi(&bytes).expect("MIDI 解析失败");

    println!(
        "格式: {:?} | 轨道数: {} | 音符数: {} | 时长: {} ms",
        doc.format,
        doc.tracks.len(),
        doc.note_count(),
        doc.duration_us / 1000
    );
    let (lo, hi) = doc.bpm_range();
    println!("BPM 范围: {lo:.1} - {hi:.1}");
    for t in &doc.tracks {
        println!("  轨道 {}: {} ({} 音符)", t.id, t.name, t.notes.len());
    }

    let profile_path = format!(
        "{}/../../game-profiles/identity-v",
        env!("CARGO_MANIFEST_DIR")
    );
    let profile =
        load_profile(std::path::Path::new(&profile_path)).expect("加载 identity-v Profile 失败");
    println!(
        "Profile: {}（{} 键，音域 {} - {}，复音上限 {}）",
        profile.display_name,
        profile.keys,
        profile.midi_low,
        profile.midi_high,
        profile.max_polyphony
    );

    // 全轨道启用
    let enabled: Vec<u16> = doc.tracks.iter().map(|t| t.id).collect();
    let options = ArrangementOptions::default();
    let (seq, stats) = arrange(&doc, &options, &profile, &enabled).expect("编排失败");

    println!("\n编排结果:");
    println!("  移调量: {} 半音", stats.applied_transpose);
    println!(
        "  输出音符: {}（输入 {}，折叠 {}，丢弃 {}，裁剪 {}，静音 {}）",
        stats.output_notes,
        stats.input_notes,
        stats.folded,
        stats.dropped_out_of_range,
        stats.dropped_polyphony,
        stats.muted
    );
    println!(
        "  动作数: {}（按下/抬起）| 时长: {} ms",
        seq.actions.len(),
        seq.duration_us / 1000
    );
    let first = seq.actions.first();
    let last = seq.actions.last();
    println!("  首动作: {first:?}");
    println!("  末动作: {last:?}");

    // 校验：Down/Up 配对完整
    let mut downs = std::collections::HashMap::new();
    for a in &seq.actions {
        match a {
            scoreleap_sequence::PlatformAction::KeyDown { at_us, key } => {
                if downs.insert(*key, *at_us).is_some() {
                    panic!("重复 Down: key={key:?} at={at_us}us（同键重叠修正失效）");
                }
            }
            scoreleap_sequence::PlatformAction::KeyUp { at_us, key } => {
                let d = downs.remove(key).expect("KeyUp 无对应 KeyDown");
                assert!(*at_us >= d, "KeyUp 早于 KeyDown");
            }
            _ => {}
        }
    }
    assert!(downs.is_empty(), "存在未抬起按键");
    println!("\n✅ 校验通过：所有按键 Down/Up 配对完整，无残留。");
}

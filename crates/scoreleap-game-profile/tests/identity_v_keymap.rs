//! identity-v Profile 完整键位回归测试。
//!
//! 防止「36 个映射全部错误」类问题：逐项断言 MIDI 48..=83 与
//! 《第五人格》黑键钢琴实际键位的扫描码映射（用户实测）。

use scoreleap_game_profile::load_profile;
use scoreleap_music_ir::KeyCode;

/// 真实 identity-v Profile 目录（相对本 crate）。
fn profile_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../game-profiles/identity-v")
}

/// 期望映射表：(MIDI note, Scan Code Set 1 十六进制扫描码)。
const EXPECTED: &[(u8, u16)] = &[
    // 低音区 48-59：`, L . ; / I 9 O 0 P - [`
    (48, 0x33),
    (49, 0x26),
    (50, 0x34),
    (51, 0x27),
    (52, 0x35),
    (53, 0x17),
    (54, 0x0A),
    (55, 0x18),
    (56, 0x0B),
    (57, 0x19),
    (58, 0x0C),
    (59, 0x1A),
    // 中音区 60-71：`Z S X D C V G B H N J M`
    (60, 0x2C),
    (61, 0x1F),
    (62, 0x2D),
    (63, 0x20),
    (64, 0x2E),
    (65, 0x2F),
    (66, 0x22),
    (67, 0x30),
    (68, 0x23),
    (69, 0x31),
    (70, 0x24),
    (71, 0x32),
    // 高音区 72-83：`Q 2 W 3 E R 5 T 6 Y 7 U`
    (72, 0x10),
    (73, 0x03),
    (74, 0x11),
    (75, 0x04),
    (76, 0x12),
    (77, 0x13),
    (78, 0x06),
    (79, 0x14),
    (80, 0x07),
    (81, 0x15),
    (82, 0x08),
    (83, 0x16),
];

#[test]
fn identity_v_keymap_matches_game_layout() {
    let p = load_profile(&profile_dir()).expect("identity-v Profile 应能加载");
    // 逐项断言（任一映射错误立即失败并指出具体音符）
    for (note, scan) in EXPECTED {
        assert_eq!(
            p.keymap.get(note),
            Some(&KeyCode::Scan(*scan)),
            "MIDI {note} 映射错误：期望扫描码 0x{scan:02X}"
        );
    }
}

#[test]
fn identity_v_keymap_has_exactly_36_unique_scans() {
    let p = load_profile(&profile_dir()).unwrap();
    assert_eq!(p.keymap.len(), 36, "必须恰好 36 个映射");
    // MIDI 48..=83 全部存在
    for note in 48u8..=83 {
        assert!(p.keymap.contains_key(&note), "缺少 MIDI {note} 的映射");
    }
    // 无越界条目
    assert!(
        p.keymap.keys().all(|n| (48u8..=83).contains(n)),
        "存在超出 48–83 的条目"
    );
    // 扫描码唯一且无 ExtendedScan
    let mut seen = std::collections::HashSet::new();
    for code in p.keymap.values() {
        match code {
            KeyCode::Scan(s) => {
                assert!(seen.insert(*s), "扫描码 0x{s:02X} 重复");
            }
            KeyCode::ExtendedScan(s) => {
                panic!("identity-v 不应使用扩展扫描码，发现 E0:{s:02X}");
            }
        }
    }
}

#[test]
fn identity_v_keymap_spots_missing_entry() {
    // 防御：若未来有人删掉某个条目，下面应失败（提示测试本身仍覆盖全表）
    let p = load_profile(&profile_dir()).unwrap();
    let count = EXPECTED.len();
    assert_eq!(p.keymap.len(), count, "映射数量与期望表不一致");
}

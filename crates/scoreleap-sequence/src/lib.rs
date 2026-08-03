//! 平台无关演奏时间轴（CompiledSequence）与播放状态类型。
//!
//! 时间单位：整数微秒（`at_us`）。

use scoreleap_music_ir::{KeyCode, NoteEvent};
use serde::{Deserialize, Serialize};

/// 手势种类（Android 后端）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)] // v0.2 Android 手势使用
pub enum GestureKind {
    Tap,
    LongPress,
    Chord,
}

/// 归一化坐标点（0-1）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)] // v0.2 Android 手势使用
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// 平台动作：按键或手势。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PlatformAction {
    KeyDown {
        at_us: i64,
        key: KeyCode,
    },
    KeyUp {
        at_us: i64,
        key: KeyCode,
    },
    Gesture {
        at_us: i64,
        points: (Point, Point),
        kind: GestureKind,
    },
}

impl PlatformAction {
    pub fn at_us(&self) -> i64 {
        match self {
            PlatformAction::KeyDown { at_us, .. }
            | PlatformAction::KeyUp { at_us, .. }
            | PlatformAction::Gesture { at_us, .. } => *at_us,
        }
    }
}

/// 序列元数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceMeta {
    pub source_name: String,
    pub track_ids: Vec<u16>,
    pub note_count: usize,
    pub transpose_semitones: i8,
}

/// 编译后的平台无关时间轴（按 at_us 升序，同刻顺序稳定）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledSequence {
    pub actions: Vec<PlatformAction>,
    pub duration_us: i64,
    pub meta: SequenceMeta,
}

impl CompiledSequence {
    /// 空序列。
    pub fn empty(meta: SequenceMeta) -> Self {
        CompiledSequence {
            actions: vec![],
            duration_us: 0,
            meta,
        }
    }

    /// 序列是否为空。
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

/// 播放状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Idle,
    Countdown,
    Playing,
    Paused,
    /// 停止/紧急停止后的终态；重新发起会话回到 Idle。
    Stopped,
    Finished,
}

/// 播放命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackCommand {
    Start,
    Pause,
    Resume,
    Stop,
    EmergencyStop,
}

/// 播放进度。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackProgress {
    pub position_us: i64,
    pub current_note: Option<NoteEvent>,
    pub pressed_keys: u32,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::all)] // 测试代码风格类警告不阻塞
    use super::*;
    use scoreleap_music_ir::KeyCode;

    #[test]
    fn action_ordering_and_serde() {
        let a = PlatformAction::KeyDown {
            at_us: 1_000,
            key: KeyCode::scan(0x1E),
        };
        let b = PlatformAction::KeyUp {
            at_us: 2_000,
            key: KeyCode::scan(0x1E),
        };
        assert!(a.at_us() < b.at_us());
        let json = serde_json::to_string(&vec![a, b]).unwrap();
        let back: Vec<PlatformAction> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
    }

    #[test]
    fn empty_sequence() {
        let seq = CompiledSequence::empty(SequenceMeta {
            source_name: "x".into(),
            track_ids: vec![],
            note_count: 0,
            transpose_semitones: 0,
        });
        assert!(seq.is_empty());
        assert_eq!(seq.duration_us, 0);
    }

    #[test]
    fn gesture_serde() {
        let g = PlatformAction::Gesture {
            at_us: 500,
            points: (Point { x: 0.1, y: 0.2 }, Point { x: 0.3, y: 0.4 }),
            kind: GestureKind::Chord,
        };
        let json = serde_json::to_string(&g).unwrap();
        let back: PlatformAction = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
    }
}

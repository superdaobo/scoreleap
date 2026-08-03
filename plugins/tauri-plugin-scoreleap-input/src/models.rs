//! 平台动作模型（与 scoreleap-sequence::PlatformAction 对齐）。

use serde::{Deserialize, Serialize};

/// 归一化坐标点（0-1）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)] // v0.2 Android 手势使用
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// 手势动作（Android 后端；v0.2）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)] // v0.2 使用
pub struct GestureAction {
    /// 相对播放起点的时间偏移（微秒）。
    pub at_us: i64,
    /// 触点（单点 = 相同点）。
    pub points: (Point, Point),
    /// 手势种类。
    pub kind: GestureKind,
    /// 持续时长（微秒）。
    pub duration_us: i64,
}

/// 手势种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GestureKind {
    Tap,
    LongPress,
    Chord,
}

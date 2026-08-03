//! tauri-plugin-scoreleap-input
//!
//! 输入后端插件：Windows SendInput（desktop.rs）、Android 手势桥接（mobile.rs，v0.2）。

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

#[cfg(windows)]
pub use desktop::SendInputBackend;

#[cfg(windows)]
mod desktop;

mod mobile;
mod models;

/// 初始化插件（当前仅注册；命令在 v0.2 Android 版本开放）。
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("scoreleap-input").build()
}

//! Windows 输入后端：SendInput + 扫描码。
//!
//! 用法：KEYEVENTF_SCANCODE + KEYEVENTF_KEYUP；扩展扫描码用 KEYEVENTF_EXTENDEDKEY。
//! 维护按下集合，release_all 幂等释放。
//! UIPI 注意：只能注入同/低完整性级别进程（管理员游戏需同权限运行本程序）。

use scoreleap_music_ir::KeyCode;
use scoreleap_scheduler::{BackendError, InputBackend};
use std::collections::HashSet;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

/// Windows SendInput 后端。
pub struct SendInputBackend {
    pressed: HashSet<KeyCode>,
}

impl SendInputBackend {
    pub fn new() -> Self {
        SendInputBackend {
            pressed: HashSet::new(),
        }
    }

    /// 当前按下集合大小（诊断/测试页）。
    pub fn pressed_count(&self) -> usize {
        self.pressed.len()
    }

    /// 前台窗口句柄（测试页显示用）。
    pub fn foreground_window(&self) -> isize {
        unsafe { GetForegroundWindow().0 as isize }
    }
}

impl Default for SendInputBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn inject(key: KeyCode, key_up: bool) -> Result<(), BackendError> {
    let (scan, extended) = match key {
        KeyCode::Scan(s) => (s, false),
        KeyCode::ExtendedScan(s) => (s, true),
    };
    let mut flags = KEYEVENTF_SCANCODE;
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    let ki = KEYBDINPUT {
        wVk: VIRTUAL_KEY(0),
        wScan: scan,
        dwFlags: flags,
        time: 0,
        dwExtraInfo: 0,
    };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki },
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        return Err(BackendError::Inject(format!(
            "SendInput 返回 {sent}（可能被 UIPI 阻止：游戏是否以管理员运行？）"
        )));
    }
    Ok(())
}

impl InputBackend for SendInputBackend {
    fn key_down(&mut self, key: KeyCode) -> Result<(), BackendError> {
        inject(key, false)?;
        self.pressed.insert(key);
        Ok(())
    }
    fn key_up(&mut self, key: KeyCode) -> Result<(), BackendError> {
        inject(key, true)?;
        self.pressed.remove(&key);
        Ok(())
    }
    fn release_all(&mut self) -> Result<(), BackendError> {
        let keys: Vec<KeyCode> = self.pressed.iter().copied().collect();
        for k in keys {
            let _ = inject(k, true);
            self.pressed.remove(&k);
        }
        Ok(())
    }
}

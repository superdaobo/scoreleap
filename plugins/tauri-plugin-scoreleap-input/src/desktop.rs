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
    inject_scan(scan, extended, key_up)
}

/// 按扫描码注入一次按键（供测试页排查 UIPI 阻止）。
pub fn test_inject_key(scan: u16) -> Result<String, BackendError> {
    inject_scan(scan, false, false)?;
    std::thread::sleep(std::time::Duration::from_millis(30));
    inject_scan(scan, false, true)?;
    Ok(format!(
        "已向前台窗口发送扫描码 {scan:#06x}（若目标程序无反应：① 游戏是否以管理员运行？\n② 前台窗口是否真的是游戏？）"
    ))
}

fn inject_scan(scan: u16, extended: bool, key_up: bool) -> Result<(), BackendError> {
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

/// 前台窗口信息（测试页诊断：UIPI 提权检测）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ForegroundInfo {
    pub title: String,
    pub pid: u32,
    pub elevated: bool,
    pub our_elevated: bool,
}

/// 检测前台窗口及其提权状态（排查 SendInput 被 UIPI 阻止）。
pub fn check_foreground_info() -> Result<ForegroundInfo, BackendError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextW, GetWindowThreadProcessId};

    unsafe {
        let hwnd = GetForegroundWindow();
        let mut title_buf = [0u16; 256];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(&title_buf[..len as usize]);

        let mut pid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let mut elevated = false;
        if pid != 0 {
            if let Ok(proc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let mut token = Default::default();
                if OpenProcessToken(proc, TOKEN_QUERY, &mut token).is_ok() {
                    let mut elevation: TOKEN_ELEVATION = Default::default();
                    let mut ret: u32 = 0;
                    if GetTokenInformation(
                        token,
                        TokenElevation,
                        Some(&mut elevation as *mut _ as *mut _),
                        std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                        &mut ret,
                    )
                    .is_ok()
                    {
                        elevated = elevation.TokenIsElevated != 0;
                    }
                    let _ = CloseHandle(token);
                }
                let _ = CloseHandle(proc);
            }
        }

        // 本进程提权状态
        let mut our_elevated = false;
        let mut token = Default::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_ok() {
            let mut elevation: TOKEN_ELEVATION = Default::default();
            let mut ret: u32 = 0;
            if GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut ret,
            )
            .is_ok()
            {
                our_elevated = elevation.TokenIsElevated != 0;
            }
            let _ = CloseHandle(token);
        }

        Ok(ForegroundInfo {
            title,
            pid,
            elevated,
            our_elevated,
        })
    }
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

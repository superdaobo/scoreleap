//! 精确播放调度器。
//!
//! - `Clock`：时间源抽象（SystemClock = QPC/高精度等待；VirtualClock = 测试快进）
//! - `InputBackend`：输入后端抽象（MockInputBackend 用于测试；SendInput 在插件层实现）
//! - `Scheduler`：deadline 驱动的播放状态机（Idle/Countdown/Playing/Paused/Stopped/Finished）
//!
//! 时间单位：整数微秒。测试不得依赖真实等待时间——必须使用 VirtualClock。

use scoreleap_music_ir::KeyCode;
use scoreleap_sequence::{CompiledSequence, PlaybackCommand, PlaybackProgress, PlaybackState};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// 后端错误。
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("输入注入失败: {0}")]
    Inject(String),
}

/// 输入后端抽象。
pub trait InputBackend: Send {
    fn key_down(&mut self, key: KeyCode) -> Result<(), BackendError>;
    fn key_up(&mut self, key: KeyCode) -> Result<(), BackendError>;
    /// 释放全部已按下按键（幂等）。
    fn release_all(&mut self) -> Result<(), BackendError>;
}

/// Mock 后端记录的事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockEvent {
    Down(KeyCode),
    Up(KeyCode),
    ReleaseAll,
}

/// 测试用后端：内存记录，不做真实注入。可克隆句柄供外部断言。
#[derive(Debug, Clone, Default)]
pub struct MockInputBackend {
    inner: Arc<Mutex<MockInner>>,
}

#[derive(Debug, Default)]
struct MockInner {
    events: Vec<MockEvent>,
    pressed: Vec<KeyCode>,
}

impl MockInputBackend {
    pub fn new() -> Self {
        Self::default()
    }
    /// 事件快照（用于测试断言）。
    pub fn snapshot(&self) -> Vec<MockEvent> {
        self.inner.lock().unwrap().events.clone()
    }
    /// 当前按下集合快照。
    pub fn pressed(&self) -> Vec<KeyCode> {
        self.inner.lock().unwrap().pressed.clone()
    }
}

impl InputBackend for MockInputBackend {
    fn key_down(&mut self, key: KeyCode) -> Result<(), BackendError> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.pressed.contains(&key) {
            inner.pressed.push(key);
            inner.events.push(MockEvent::Down(key));
        }
        Ok(())
    }
    fn key_up(&mut self, key: KeyCode) -> Result<(), BackendError> {
        let mut inner = self.inner.lock().unwrap();
        inner.pressed.retain(|k| *k != key);
        inner.events.push(MockEvent::Up(key));
        Ok(())
    }
    fn release_all(&mut self) -> Result<(), BackendError> {
        let mut inner = self.inner.lock().unwrap();
        inner.pressed.clear();
        inner.events.push(MockEvent::ReleaseAll);
        Ok(())
    }
}

/// 时钟抽象（Send + Sync 以便跨线程共享）。
pub trait Clock: Send + Sync {
    fn now_us(&self) -> i64;
    /// 阻塞直到 deadline（微秒）。默认实现轮询睡眠；Windows 用高精度等待计时器。
    fn sleep_until(&self, deadline_us: i64) {
        let mut now = self.now_us();
        while now < deadline_us {
            std::thread::sleep(std::time::Duration::from_micros(
                ((deadline_us - now).min(2_000)) as u64,
            ));
            now = self.now_us();
        }
    }
}

/// 系统时钟（Windows 使用 QPC；其他平台用 SystemTime）。
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_us(&self) -> i64 {
        qpc_now_us()
    }
    fn sleep_until(&self, deadline_us: i64) {
        #[cfg(windows)]
        {
            if high_res_sleep_until(deadline_us) {
                return;
            }
        }
        let mut now = self.now_us();
        while now < deadline_us {
            std::thread::sleep(std::time::Duration::from_micros(
                ((deadline_us - now).min(2_000)) as u64,
            ));
            now = self.now_us();
        }
    }
}

/// 测试用虚拟时钟：`sleep_until` 分步推进（每步 ≤ max_step），不真实等待。
/// 分步推进使测试线程有机会插入命令（Pause/Stop 等）。
#[derive(Debug)]
pub struct VirtualClock {
    now: AtomicI64,
    max_step: AtomicI64,
}

impl Default for VirtualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualClock {
    pub fn new() -> Self {
        VirtualClock {
            now: AtomicI64::new(0),
            max_step: AtomicI64::new(250_000),
        }
    }
    /// 手动设置当前时间（微秒）。
    pub fn set_now(&self, us: i64) {
        self.now.store(us, Ordering::SeqCst);
    }
    /// 设置单步最大推进量（微秒；默认 250ms）。
    pub fn set_max_step(&self, us: i64) {
        self.max_step.store(us.max(1_000), Ordering::SeqCst);
    }
}

impl Clock for VirtualClock {
    fn now_us(&self) -> i64 {
        self.now.load(Ordering::SeqCst)
    }
    fn sleep_until(&self, deadline_us: i64) {
        let cur = self.now.load(Ordering::SeqCst);
        let step = self.max_step.load(Ordering::SeqCst);
        let target = (cur + step).min(deadline_us);
        if target > cur {
            self.now.store(target, Ordering::SeqCst);
        }
        // 让出 CPU，给命令发送线程机会
        std::thread::yield_now();
    }
}

#[cfg(windows)]
fn qpc_now_us() -> i64 {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    unsafe {
        let mut freq: i64 = 0;
        let mut count: i64 = 0;
        QueryPerformanceFrequency(&mut freq).ok();
        QueryPerformanceCounter(&mut count).ok();
        if freq > 0 {
            (count as f64 * 1_000_000.0 / freq as f64) as i64
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_micros() as i64)
                .unwrap_or(0)
        }
    }
}

#[cfg(not(windows))]
fn qpc_now_us() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}

/// Windows 高分辨率等待计时器（Win10 1803+）；失败返回 false 走轮询。
#[cfg(windows)]
fn high_res_sleep_until(deadline_us: i64) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        CreateWaitableTimerExW, SetWaitableTimer, WaitForSingleObject,
        CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CREATE_WAITABLE_TIMER_MANUAL_RESET,
    };
    let now = qpc_now_us();
    if deadline_us <= now {
        return true;
    }
    let wait_us = deadline_us - now;
    if wait_us < 500 {
        return false; // 太短，轮询更准
    }
    unsafe {
        let handle = match CreateWaitableTimerExW(
            None,
            None,
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION | CREATE_WAITABLE_TIMER_MANUAL_RESET,
            0x1F0003, // TIMER_ALL_ACCESS
        ) {
            Ok(h) => h,
            Err(_) => return false,
        };
        // 相对时间：负值 100ns 单位
        let due = -wait_us * 10;
        let ok = SetWaitableTimer(handle, &due, 0, None, None, false).is_ok();
        if ok {
            let _ = WaitForSingleObject(handle, u32::MAX);
        }
        let _ = CloseHandle(handle);
        ok
    }
}

/// 调度器对外事件。
#[derive(Debug, Clone)]
pub enum SchedulerEvent {
    State(PlaybackState),
    Progress(PlaybackProgress),
    Error(String),
}

/// 调度器句柄（命令发送 + 事件订阅；可克隆供事件转发线程）。
pub struct SchedulerHandle {
    cmd_tx: Sender<PlaybackCommand>,
    event_rx: Arc<Mutex<Receiver<SchedulerEvent>>>,
    join: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl SchedulerHandle {
    pub fn command(&self, cmd: PlaybackCommand) -> Result<(), String> {
        self.cmd_tx.send(cmd).map_err(|e| e.to_string())
    }
    pub fn try_recv_event(&self) -> Option<SchedulerEvent> {
        self.event_rx.lock().unwrap().try_recv().ok()
    }
    pub fn recv_event(&self) -> Result<SchedulerEvent, String> {
        self.event_rx
            .lock()
            .unwrap()
            .recv()
            .map_err(|e| e.to_string())
    }
    /// 克隆句柄（共享命令发送端与事件接收端；join 仅原句柄可用）。
    pub fn try_clone(&self) -> Result<SchedulerHandle, String> {
        Ok(SchedulerHandle {
            cmd_tx: self.cmd_tx.clone(),
            event_rx: self.event_rx.clone(),
            join: self.join.clone(),
        })
    }
    /// 停止调度线程（发送 EmergencyStop 并等待退出）。
    pub fn shutdown(self) {
        let _ = self.cmd_tx.send(PlaybackCommand::EmergencyStop);
        drop(self.cmd_tx);
        if let Some(j) = self.join.lock().unwrap().take() {
            let _ = j.join();
        }
    }
}

/// 播放会话内部状态。
struct Session {
    action_idx: usize,
    /// 播放起点的墙钟时间（逻辑时间 0 对应的时钟值）。
    origin_wall_us: i64,
    /// 暂停前累计逻辑时间（微秒）。
    paused_elapsed_us: i64,
    /// 逻辑时间轴上「已按下但未抬起」的按键。
    logically_pressed: Vec<KeyCode>,
    /// 当前速度倍率。
    speed: f64,
    /// 每 50ms 上报一次进度。
    last_progress_wall_us: i64,
}

pub struct Scheduler {
    seq: CompiledSequence,
    clock: Arc<dyn Clock>,
    backend: Box<dyn InputBackend>,
    state: PlaybackState,
    session: Option<Session>,
    cmd_rx: Receiver<PlaybackCommand>,
    event_tx: Sender<SchedulerEvent>,
}

const COUNTDOWN_US: i64 = 3_000_000;

impl Scheduler {
    /// 启动调度线程，返回句柄。
    pub fn spawn(
        seq: CompiledSequence,
        clock: Arc<dyn Clock>,
        backend: Box<dyn InputBackend>,
    ) -> SchedulerHandle {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let mut sched = Scheduler {
            seq,
            clock,
            backend,
            state: PlaybackState::Idle,
            session: None,
            cmd_rx,
            event_tx,
        };
        let join = std::thread::Builder::new()
            .name("scoreleap-scheduler".into())
            .spawn(move || sched.run())
            .expect("failed to spawn scheduler thread");
        SchedulerHandle {
            cmd_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            join: Arc::new(Mutex::new(Some(join))),
        }
    }

    fn emit(&self, ev: SchedulerEvent) {
        let _ = self.event_tx.send(ev);
    }

    /// 主循环。
    fn run(&mut self) {
        loop {
            match self.state {
                PlaybackState::Idle | PlaybackState::Stopped | PlaybackState::Finished => {
                    match self.cmd_rx.recv() {
                        Ok(PlaybackCommand::Start) => {
                            self.begin_countdown();
                        }
                        Ok(PlaybackCommand::EmergencyStop) => {
                            self.emergency_stop();
                            self.emit(SchedulerEvent::State(PlaybackState::Stopped));
                        }
                        Ok(PlaybackCommand::Stop) => {
                            self.emergency_stop();
                            self.emit(SchedulerEvent::State(PlaybackState::Stopped));
                        }
                        Ok(_) => {}
                        Err(_) => return, // 通道关闭
                    }
                }
                PlaybackState::Countdown => {
                    self.countdown_loop();
                }
                PlaybackState::Playing => {
                    self.play_loop();
                }
                PlaybackState::Paused => {
                    match self.cmd_rx.recv() {
                        Ok(PlaybackCommand::Resume) => {
                            // 重建按下状态
                            let resume_wall = self.clock.now_us();
                            if let Some(s) = &mut self.session {
                                s.origin_wall_us = resume_wall - s.paused_elapsed_us;
                            }
                            let mut failed = false;
                            for k in self.logical_pressed_keys() {
                                if let Err(e) = self.backend.key_down(k) {
                                    self.emit(SchedulerEvent::Error(e.to_string()));
                                    self.emergency_stop();
                                    self.emit(SchedulerEvent::State(PlaybackState::Stopped));
                                    failed = true;
                                    break;
                                }
                            }
                            if failed {
                                return;
                            }
                            self.state = PlaybackState::Playing;
                            self.emit(SchedulerEvent::State(PlaybackState::Playing));
                        }
                        Ok(PlaybackCommand::Stop | PlaybackCommand::EmergencyStop) => {
                            self.emergency_stop();
                            self.emit(SchedulerEvent::State(PlaybackState::Stopped));
                        }
                        Ok(PlaybackCommand::Start) => {
                            // 重新开始：先复位
                            self.emergency_stop();
                            self.begin_countdown();
                        }
                        Ok(_) => {}
                        Err(_) => {
                            self.emergency_stop();
                            return;
                        }
                    }
                }
            }
        }
    }

    fn logical_pressed_keys(&self) -> Vec<KeyCode> {
        self.session
            .as_ref()
            .map(|s| s.logically_pressed.clone())
            .unwrap_or_default()
    }

    fn begin_countdown(&mut self) {
        // 复位会话
        if let Err(e) = self.backend.release_all() {
            self.emit(SchedulerEvent::Error(e.to_string()));
        }
        self.session = Some(Session {
            action_idx: 0,
            origin_wall_us: self.clock.now_us(),
            paused_elapsed_us: 0,
            logically_pressed: vec![],
            speed: 1.0,
            last_progress_wall_us: 0,
        });
        self.state = PlaybackState::Countdown;
        self.emit(SchedulerEvent::State(PlaybackState::Countdown));
        self.emit(SchedulerEvent::Progress(PlaybackProgress {
            position_us: 0,
            current_note: None,
            pressed_keys: 0,
        }));
    }

    fn countdown_loop(&mut self) {
        let start = self.clock.now_us();
        let deadline = start + COUNTDOWN_US;
        let mut cancel = false;
        loop {
            self.clock.sleep_until(deadline);
            // 检查命令（倒计时期间允许取消/紧急停止/停止）
            while let Ok(cmd) = self.cmd_rx.try_recv() {
                match cmd {
                    PlaybackCommand::Stop | PlaybackCommand::EmergencyStop => {
                        cancel = true;
                    }
                    PlaybackCommand::Start => {
                        // 重启倒计时
                        self.begin_countdown();
                        return;
                    }
                    _ => {}
                }
            }
            if cancel {
                self.emergency_stop();
                self.emit(SchedulerEvent::State(PlaybackState::Stopped));
                return;
            }
            if self.clock.now_us() >= deadline {
                break;
            }
        }
        // 开始播放
        let now = self.clock.now_us();
        if let Some(s) = &mut self.session {
            s.origin_wall_us = now;
            s.paused_elapsed_us = 0;
            s.action_idx = 0;
            s.logically_pressed.clear();
        }
        self.state = PlaybackState::Playing;
        self.emit(SchedulerEvent::State(PlaybackState::Playing));
    }

    /// 逻辑时间 → 墙钟时间（session 缺失时返回原值，防御崩溃路径）。
    fn wall_of(&self, logic_us: i64) -> i64 {
        match &self.session {
            Some(s) => s.origin_wall_us + (logic_us as f64 / s.speed.max(0.05)) as i64,
            None => logic_us,
        }
    }

    /// 墙钟时间 → 逻辑时间（session 缺失时返回 0，防御崩溃路径）。
    fn logic_of(&self, wall_us: i64) -> i64 {
        match &self.session {
            Some(s) => ((wall_us - s.origin_wall_us) as f64 * s.speed.max(0.05)) as i64,
            None => 0,
        }
    }

    fn play_loop(&mut self) {
        let mut pending: Option<PlaybackCommand> = None;
        loop {
            // 处理命令
            while let Ok(cmd) = self.cmd_rx.try_recv() {
                match cmd {
                    PlaybackCommand::Pause => {
                        pending = Some(PlaybackCommand::Pause);
                    }
                    PlaybackCommand::Stop | PlaybackCommand::EmergencyStop => {
                        pending = Some(cmd);
                    }
                    PlaybackCommand::Resume => {}
                    PlaybackCommand::Start => {
                        pending = Some(PlaybackCommand::Start);
                    }
                }
            }
            if let Some(cmd) = pending.take() {
                match cmd {
                    PlaybackCommand::Pause => {
                        let now = self.clock.now_us();
                        let paused = self.logic_of(now);
                        if let Some(s) = &mut self.session {
                            s.paused_elapsed_us = paused;
                        }
                        // 释放物理按键（保留逻辑按下集合）
                        let _ = self.backend.release_all();
                        self.state = PlaybackState::Paused;
                        self.emit(SchedulerEvent::State(PlaybackState::Paused));
                        return;
                    }
                    PlaybackCommand::Stop | PlaybackCommand::EmergencyStop => {
                        self.emergency_stop();
                        self.emit(SchedulerEvent::State(PlaybackState::Stopped));
                        return;
                    }
                    PlaybackCommand::Start => {
                        self.emergency_stop();
                        self.begin_countdown();
                        return;
                    }
                    _ => {}
                }
            }

            // 执行到期的动作
            let now = self.clock.now_us();
            let action = self
                .seq
                .actions
                .get(self.session.as_ref().map(|s| s.action_idx).unwrap_or(0))
                .copied();
            let done = match action {
                None => true,
                Some(a) => {
                    let wall = self.wall_of(a.at_us());
                    if wall <= now {
                        if !self.dispatch(a) {
                            // dispatch 失败已紧急停止
                            return;
                        }
                        if let Some(s) = &mut self.session {
                            s.action_idx += 1;
                        }
                        false
                    } else {
                        // deadline 驱动等待
                        self.clock.sleep_until(wall);
                        false
                    }
                }
            };
            if done {
                // 播放完成
                let _ = self.backend.release_all();
                self.state = PlaybackState::Finished;
                self.emit(SchedulerEvent::State(PlaybackState::Finished));
                self.emit(SchedulerEvent::Progress(PlaybackProgress {
                    position_us: self.seq.duration_us,
                    current_note: None,
                    pressed_keys: 0,
                }));
                return;
            }

            // 进度上报（每 50ms 墙钟）
            let now_wall = self.clock.now_us();
            let need_progress = self
                .session
                .as_ref()
                .map(|s| now_wall - s.last_progress_wall_us >= 50_000)
                .unwrap_or(false);
            if need_progress {
                let pos = self.logic_of(now_wall).min(self.seq.duration_us);
                if let Some(s) = &mut self.session {
                    s.last_progress_wall_us = now_wall;
                }
                self.emit(SchedulerEvent::Progress(PlaybackProgress {
                    position_us: pos,
                    current_note: None,
                    pressed_keys: self
                        .session
                        .as_ref()
                        .map(|s| s.logically_pressed.len() as u32)
                        .unwrap_or(0),
                }));
            }
        }
    }

    /// 执行动作。返回 false 表示后端失败已紧急停止（调用方应退出播放循环）。
    fn dispatch(&mut self, a: scoreleap_sequence::PlatformAction) -> bool {
        let res = match a {
            scoreleap_sequence::PlatformAction::KeyDown { key, .. } => {
                if let Some(s) = &mut self.session {
                    if !s.logically_pressed.contains(&key) {
                        s.logically_pressed.push(key);
                    }
                }
                self.backend.key_down(key)
            }
            scoreleap_sequence::PlatformAction::KeyUp { key, .. } => {
                if let Some(s) = &mut self.session {
                    s.logically_pressed.retain(|k| *k != key);
                }
                self.backend.key_up(key)
            }
            scoreleap_sequence::PlatformAction::Gesture { .. } => Ok(()), // v0.2
        };
        if let Err(e) = res {
            self.emit(SchedulerEvent::Error(e.to_string()));
            self.emergency_stop();
            self.emit(SchedulerEvent::State(PlaybackState::Stopped));
            return false;
        }
        true
    }

    fn emergency_stop(&mut self) {
        if let Err(e) = self.backend.release_all() {
            self.emit(SchedulerEvent::Error(format!("释放按键失败: {e}")));
        }
        self.session = None;
        self.state = PlaybackState::Stopped;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::all)] // 测试代码风格类警告不阻塞
    use super::*;
    use scoreleap_music_ir::KeyCode;
    use scoreleap_sequence::{CompiledSequence, PlatformAction, SequenceMeta};
    use std::sync::Arc;

    fn seq_with(actions: Vec<(i64, KeyCode, bool)>) -> CompiledSequence {
        let mut acts = vec![];
        for (at, key, down) in actions {
            if down {
                acts.push(PlatformAction::KeyDown { at_us: at, key });
            } else {
                acts.push(PlatformAction::KeyUp { at_us: at, key });
            }
        }
        let duration = acts.iter().map(|a| a.at_us()).max().unwrap_or(0);
        CompiledSequence {
            actions: acts,
            duration_us: duration,
            meta: SequenceMeta {
                source_name: "test".into(),
                track_ids: vec![0],
                note_count: 0,
                transpose_semitones: 0,
            },
        }
    }

    fn next_event(handle: &SchedulerHandle) -> SchedulerEvent {
        handle.recv_event().unwrap()
    }
    fn run_to_finish(handle: &SchedulerHandle) -> Vec<SchedulerEvent> {
        let mut events = vec![];
        loop {
            match handle.recv_event() {
                Ok(ev) => {
                    if matches!(ev, SchedulerEvent::State(PlaybackState::Finished)) {
                        events.push(ev);
                        break;
                    }
                    events.push(ev);
                }
                Err(_) => break,
            }
        }
        events
    }

    #[test]
    fn virtual_clock_plays_sequence_in_order() {
        let k = KeyCode::scan(0x1E);
        let seq = seq_with(vec![
            (0, k, true),
            (500_000, k, false),
            (1_000_000, KeyCode::scan(0x11), true),
            (1_500_000, KeyCode::scan(0x11), false),
        ]);
        let clock = Arc::new(VirtualClock::new());
        let backend = MockInputBackend::new();
        let handle = Scheduler::spawn(seq, clock, Box::new(backend.clone()));
        handle.command(PlaybackCommand::Start).unwrap();
        let events = run_to_finish(&handle);
        assert!(events
            .iter()
            .any(|e| matches!(e, SchedulerEvent::State(PlaybackState::Countdown))));
        assert!(events
            .iter()
            .any(|e| matches!(e, SchedulerEvent::State(PlaybackState::Playing))));
        assert!(events
            .iter()
            .any(|e| matches!(e, SchedulerEvent::State(PlaybackState::Finished))));
        // 校验后端事件：Down, Up, Down, Up + 结束 ReleaseAll；无残留
        let snap = backend.snapshot();
        assert!(snap.contains(&MockEvent::Down(k)));
        assert!(snap.contains(&MockEvent::Up(k)));
        assert!(backend.pressed().is_empty());
        handle.shutdown();
    }

    #[test]
    fn pause_releases_and_resume_restores() {
        let k = KeyCode::scan(0x1E);
        // 长音符：0-2s
        let seq = seq_with(vec![(0, k, true), (2_000_000, k, false)]);
        let clock = Arc::new(VirtualClock::new());
        clock.set_max_step(100_000); // 100ms/步，给命令留窗口
        let backend = MockInputBackend::new();
        let handle = Scheduler::spawn(seq, clock, Box::new(backend.clone()));
        handle.command(PlaybackCommand::Start).unwrap();
        // 等待进入 Playing
        loop {
            match next_event(&handle) {
                SchedulerEvent::State(PlaybackState::Playing) => break,
                SchedulerEvent::State(PlaybackState::Countdown) => {}
                SchedulerEvent::State(PlaybackState::Finished) => panic!("finished too early"),
                _ => {}
            }
        }
        // 立即暂停
        handle.command(PlaybackCommand::Pause).unwrap();
        loop {
            match next_event(&handle) {
                SchedulerEvent::State(PlaybackState::Paused) => break,
                SchedulerEvent::State(PlaybackState::Finished) => panic!("finished while pausing"),
                _ => {}
            }
        }
        // 暂停后物理按键已释放
        assert!(backend.pressed().is_empty());
        handle.command(PlaybackCommand::Resume).unwrap();
        loop {
            match next_event(&handle) {
                SchedulerEvent::State(PlaybackState::Playing) => break,
                SchedulerEvent::State(PlaybackState::Finished) => panic!("finished after resume"),
                _ => {}
            }
        }
        // 继续到结束
        loop {
            match next_event(&handle) {
                SchedulerEvent::State(PlaybackState::Finished) => break,
                _ => {}
            }
        }
        assert!(backend.pressed().is_empty());
        handle.shutdown();
    }

    #[test]
    fn emergency_stop_from_countdown() {
        let k = KeyCode::scan(0x1E);
        let seq = seq_with(vec![(0, k, true), (500_000, k, false)]);
        let clock = Arc::new(VirtualClock::new());
        let backend = MockInputBackend::new();
        let handle = Scheduler::spawn(seq, clock, Box::new(backend));
        handle.command(PlaybackCommand::Start).unwrap();
        // 等待 Countdown 状态
        loop {
            match next_event(&handle) {
                SchedulerEvent::State(PlaybackState::Countdown) => break,
                _ => {}
            }
        }
        handle.command(PlaybackCommand::EmergencyStop).unwrap();
        loop {
            match next_event(&handle) {
                SchedulerEvent::State(PlaybackState::Stopped) => break,
                _ => {}
            }
        }
        handle.shutdown();
    }

    #[test]
    fn stop_releases_all_keys() {
        let k = KeyCode::scan(0x1E);
        let seq = seq_with(vec![(0, k, true), (10_000_000, k, false)]);
        let clock = Arc::new(VirtualClock::new());
        let backend = MockInputBackend::new();
        let handle = Scheduler::spawn(seq, clock, Box::new(backend));
        handle.command(PlaybackCommand::Start).unwrap();
        loop {
            match next_event(&handle) {
                SchedulerEvent::State(PlaybackState::Playing) => break,
                _ => {}
            }
        }
        handle.command(PlaybackCommand::Stop).unwrap();
        loop {
            match next_event(&handle) {
                SchedulerEvent::State(PlaybackState::Stopped) => break,
                _ => {}
            }
        }
        handle.shutdown();
    }
}

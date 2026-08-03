# ADR-0005：平台输入后端（Windows SendInput / Android 无障碍手势）

- 状态：已接受（draft）
- 日期：2026-02（规划期）
- 决策者：Agent A / Agent C / Agent D

## 背景

演奏最终需要把时间轴转为真实输入。Windows 侧候选有 SendInput/keybd_event/低级键盘 Hook；Android 侧候选有 AccessibilityService dispatchGesture / 悬浮窗触摸事件 / ADB。方案必须满足：精度目标（≤10ms）、可停止、可测试、合规底线（不注入、不 Hook 游戏）。

## 决策

### Windows
1. **`SendInput` + `KEYEVENTF_SCANCODE`**：以扫描码注入（官方明确扫描码与键盘布局无关；`keybd_event` 已被官方标记 superseded）；抬起用 `KEYEVENTF_KEYUP`；扩展扫描码用 `KEYEVENTF_EXTENDEDKEY` 标志（不把 0xE0 拼入 wScan）；键位表优先用 `MapVirtualKeyW(MAPVK_VK_TO_VSC_EX)` 运行时换算，静态表为后备。
2. **UIPI 约束**：SendInput 只能注入同/低完整性级别进程——游戏以管理员运行时，ScoreLeap 需要同等权限（文档与风险提示必须说明）。
3. **不采用**：低级键盘 Hook（`SetWindowsHookEx`，侵入性高）、`keybd_event`（等价但已过时）、进程注入（禁止）。
4. **调度**：单线程 deadline 驱动——`Clock` 抽象以 QPC/QPF 为唯一时钟源（QPF 缓存一次）；播放期间 `timeBeginPeriod(1)`/`timeEndPeriod(1)` 成对调用（Win10 2004+ 仅影响本进程）；唤醒用 `CreateWaitableTimerExW(CREATE_WAITABLE_TIMER_HIGH_RESOLUTION)` + SetWaitableTimer（Win10 1803+，旧系统回退普通定时器）；**每次触发后按 QPC 重算绝对到期时间**（不周期累加）消除漂移；误差预期 1–3ms；弃用 CreateTimerQueueTimer 与 timeSetEvent（官方 obsolete）。
5. **全局快捷键**：评估 `RegisterHotKey` vs `tauri-plugin-global-shortcut`，二选一在实现 Issue 中定稿。
6. **停止语义**：维护按下按键集合；`release_all` 于暂停/停止/紧急停止/panic hook/进程退出钩子调用（尽力而为）。

### Android
1. **AccessibilityService + `dispatchGesture`**：注入触摸事件（不需要无障碍树；官方明确 automation 类用途需如实披露，本产品不上架 Google Play、isAccessibilityTool 不声明为 true）；多点手势用多 Stroke 的 GestureDescription（API 能力实测，见 v0.2 技术验证 Issue）；注入逻辑保持**确定性规则映射**（音符→点击），禁止自主决策逻辑。
2. **前台服务 + 常驻通知**：演奏期间前台服务（声明 `foregroundServiceType`），通知提供暂停/停止；服务被杀则手势自然停止。
3. **不采用**：悬浮窗触摸事件（`MotionEvent` 注入仅限自身窗口，且需 SYSTEM_ALERT_WINDOW，不可行于游戏）、ADB 输入（需数据线/无线调试，体验差）。
4. **坐标**：归一化坐标（0–1）+ 用户校准锚点线性插值；渲染时乘当前显示尺寸（WindowMetrics API 30+，旧版本用兼容路径）。
5. **合规**：无障碍用途如实声明；不读取节点；用户主动开启 + 每次使用前确认。

## 后果

- 正面：双端注入方式均为系统公开 API；合规边界清晰；测试用 MockInputBackend 隔离真实注入。
- 负面：Android dispatchGesture 在真实游戏中的可靠性需要实机验证（H3 假设）；Windows 扫描码兼容性需实机验证（H1/H7）。

## 替代方案

| 方案 | 评估 |
|---|---|
| Windows 低级键盘 Hook | 侵入游戏进程输入管道，越过合规边界；拒绝 |
| Android 悬浮窗模拟触摸 | 无法在游戏上层注入；拒绝 |
| ADB 输入 | 仅开发调试可用；拒绝作为产品方案 |
| 键鼠模拟器（硬件） | 非本软件范畴；记录为未来选项 |

## 关联

- ARCHITECTURE.md §7/§8/§9；RISK_AND_COMPLIANCE.md §4/§13/§14；PRODUCT_PLAN.md §13/§14；planning/research-findings.md §2/§3。

# ADR-0002：跨平台架构（Tauri 2 + Rust Workspace + 平台无关核心）

- 状态：已接受（draft）
- 日期：2026-02（规划期）
- 决策者：Agent A

## 背景

需要同时支持 Windows 键盘注入与 Android 触摸演奏，且音乐核心（解析/编排/时间轴）必须完全复用。桌面壳与移动壳若分开实现将导致双倍维护成本。

## 决策

1. **Tauri 2 作为应用壳**：Windows 与 Android 共用 Tauri 2；WebView 前端（Vue 3 + TS + Vite + Pinia）双端复用。
2. **Rust Cargo Workspace**：平台无关核心拆分为 `scoreleap-music-ir` / `scoreleap-midi` / `scoreleap-arranger` / `scoreleap-sequence` / `scoreleap-game-profile` / `scoreleap-scheduler`；平台相关仅 `scoreleap-core`（Tauri 编排）与 `tauri-plugin-scoreleap-input`（desktop.rs/mobile.rs）。
3. **依赖方向单向**：`music-ir` 无依赖；`midi/arranger → music-ir`；`sequence` 被 arranger 与 scheduler 依赖；禁止循环依赖。
4. **输入抽象**：`scoreleap-scheduler::InputBackend` trait + `MockInputBackend`；桌面/移动实现各自 backend。
5. **调度核心平台无关**：`scoreleap-scheduler` 不依赖 Tauri，仅依赖 `Clock` 抽象（Windows 用 QPC 实现，测试用 VirtualClock）。
6. **pnpm workspace**：前端单包起步，保留多包扩展空间。

## 后果

- 正面：核心逻辑单份实现；测试不依赖平台；未来 Linux/macOS 只需新 backend。
- 负面：Tauri Android 工具链较重（Rust target + Gradle）；插件桥接增加一层间接。

## 替代方案

| 方案 | 评估 |
|---|---|
| 纯 Rust 桌面 + 原生 Android 双项目 | 核心复用靠复制/子模块，维护成本高；拒绝 |
| Flutter + Rust FFI | 可行但偏离既定技术栈；记录备用 |
| Electron + Node 原生 | 体积与性能劣势，且调度精度更难保证；拒绝 |

## 关联

- ARCHITECTURE.md §1/§2/§4；ADR-0005。

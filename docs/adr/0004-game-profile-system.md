# ADR-0004：Game Profile 系统

- 状态：已接受（draft）
- 日期：2026-02（规划期）
- 决策者：Agent A / Agent B

## 背景

不同游戏乐器有不同的键数、音域、复音上限、键位布局。架构必须允许「新增一个游戏/乐器 = 新增一个 JSON Profile」，且公共接口不得绑定单一游戏。

## 决策

1. **Profile = 目录 + JSON 文件**：`game-profiles/<game-id>/` 下包含：
   - `profile.json`：`id`、`display_name`、`version`、`instrument { keys, midi_low, midi_high, max_polyphony }`、`keymap_windows`、`layout_android` 引用、`warning`；
   - `windows-keymap.json`：`midi_note → scancode`（含扩展扫描码标记）；
   - `android-layout.json`：键位归一化坐标（`x ∈ [0,1], y ∈ [0,1]`）+ 音名；
2. **运行时校验**：serde 反序列化 + 结构化校验（键数一致、音域合法、映射唯一、坐标范围）；失败返回可读错误并禁用演奏。
3. **公共接口仅暴露 `GameProfile` 类型**；游戏名只出现在 profile 元数据与 UI 文案，不进入 crate 名/公共 API。
4. **首个 Profile**：`identity-v`（36 键；音域与键位以公开游戏内布局为准，不读取游戏数据）。
5. **CalibrationProfile 独立于 GameProfile**：Android 校准产物（设备/分辨率/锚点）存用户数据目录，可重新校准。

## 后果

- 正面：新增游戏成本 = 新增 profile 目录 + 测试；社区可贡献 profile；游戏名不出现在核心代码。
- 负面：profile 文件需 schema 版本管理（`version` 字段 + 迁移策略 v0.3 后评估）。

## 替代方案

| 方案 | 评估 |
|---|---|
| 游戏适配逻辑写死在代码 | 违反不绑定原则；拒绝 |
| 数据库存储 profile | 过度设计；JSON 文件 + Git 管理更利于贡献；接受 |
| 运行时远程拉取 profile | 违反本地优先；不做（v0.5 再评估可选在线服务） |

## 关联

- ARCHITECTURE.md §10；PRODUCT_PLAN.md §12。

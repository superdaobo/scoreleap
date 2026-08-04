# 前后端契约 — v0.1.2 曲谱库持久化与卷帘音符

> 本契约由 Agent A（协调者）先行定义，后端 Agent 与前端 Agent 并行实现时必须严格遵守。
> 字段命名：Rust 端 snake_case 自动转 camelCase 到 JS（Tauri 2 默认）；事件监听不变。

## 1. 新命令

### list_documents → DocumentSummary[]

从持久化曲谱库（`<app_data>/library/manifest.json`）读取全部曲谱摘要。

```ts
interface DocumentSummary {
  doc_id: string
  name: string
  format: 'SingleTrack' | 'Parallel' | 'Sequential'
  track_count: number
  note_count: number
  duration_ms: number
  bpm_range: [number, number]
}
```

### import_midi(path) → ImportSummary（行为增强，返回结构不变）

- 解析成功后，将源文件**复制**到 `<app_data>/library/<doc_id>.mid`（doc_id 仍是 `doc-<uuid>`）；
- 摘要写入 `manifest.json`（数组，按导入时间倒序，最多保留 50 条；文件缺失的条目在启动与读取时清理）；
- 返回结构保持现状（与 DocumentSummary 字段一致，前端无需改动调用点）。

### get_sequence_notes(seq_id) → NoteView[]

从 compile 时缓存的音符数据返回卷帘绘制数据（非 PlatformAction 还原）。

```ts
interface NoteView {
  note: number      // MIDI note number（已应用移调与折叠）
  start_us: number  // 绝对微秒
  duration_us: number
}
```

### get_tracks / compile（行为增强）

文档不在内存（`AppState.documents`）时，按 doc_id 从 library 自动加载：
`<app_data>/library/<doc_id>.mid` 存在 → 解析并放入内存后继续；不存在 → 返回「文档不存在」错误（现有错误不变）。

## 2. 目录与文件

- 曲谱库目录：`<app_data>/library/`
- manifest：`<app_data>/library/manifest.json`
  ```json
  [{ "doc_id": "doc-xxx", "name": "晴天.mid", "format": "Parallel",
     "track_count": 3, "note_count": 1109, "duration_ms": 268235,
     "bpm_range": [68.0, 120.0], "imported_at": 1754000000000 }]
  ```
- 曲谱文件：`<app_data>/library/<doc_id>.mid`
- manifest 读写须原子（写临时文件后 rename）；损坏时重置为空数组并记录日志。

## 3. 命令注册（src-tauri）

新增 `list_documents`、`get_sequence_notes`；命令函数保持非 pub（tauri-macros E0255 约束）。

## 4. 前端要求

- `libraryStore.documents` 数据源改为 `list_documents()`（onMounted 拉取；导入成功后刷新列表而非本地 unshift）；
- 移除 localStorage 作为曲谱列表主来源（可保留旧 key 读取兼容，但不再写入）；
- ArrangePage：编译成功后调用 `get_sequence_notes(seqId)`，恢复 Canvas 音符矩形绘制（现有注释代码），音高轴 PITCH_LOW=48 / PITCH_HIGH=83 不变；
- `types/index.ts` 增加 `DocumentSummary`、`NoteView`；`services/api.ts` 增加 `listDocuments()`、`getSequenceNotes(seqId)`。

## 5. 测试要求

- Rust：manifest 读写往返、导入→列出→模拟重启（新 AppState 重新 load）→get_tracks 自动加载、get_sequence_notes 数据正确（音符数/时间与编排统计一致）；
- 前端：libraryStore 拉取/刷新逻辑（mock api）、ArrangePage 音符数据接入（组件测试或 store 测试）。

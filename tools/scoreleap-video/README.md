# ScoreLeap Promo Video

这是 ScoreLeap 的独立 Remotion 宣传视频工程，用于生成 B 站主宣传片及算法演示片段。

它是 **开发/宣传工具**，不会进入 `apps/scoreleap` 的 Tauri 运行时，也不会增加 ScoreLeap 安装包体积。

## 核心叙事

主片按真实产品使用流程演示功能，不做原理课：

> **把 MP3，变成第五人格能弹的曲谱：本地转录 → 36 键编排 → 一键演奏。**

默认主片为 1920×1080、60 FPS，时间线（含 24s 实机插槽时约 76.5s）：

- 0–3s：片头一句卖点；
- 3–43s：**同一个 AppWindow 内的产品流程**（曲谱库 → 从音频转录 → 转录确认/引擎选择 → 波形+真实 Note Events 进度 → 曲谱库新曲目 → 编排工作台：Identity V Profile / SmartFold / 旋律保护 / 最大复音 4 → 播放预览：时间线移动 + 36 键触发 → 设置页引擎切换 + 本地隐私角标），全程光标点击、按钮按压、面板滑入、卡片生成等微交互；
- 43–49s：Identity V 段（程序化哥特/庄园风格，36 键琴盘向镜头推进）；
- 49s 起：旧纸门帘打开进入 gameplay 插槽（可替换实机录像，否则为明确标注的 PLACEHOLDER）；
- 末尾 5s：品牌与 GitHub。

场景切换统一为有限几种转场语言（crossfade / push / zoom-in / rise），每个场景入场/退场约 18–30 帧 overlap，CG→Gameplay 用「36 键琴盘推进 + 旧纸门帘打开」。

实机片段默认 24 秒，可根据真实录像自动调整到 12–45 秒范围内，因此替换录像后不需要手动重写后续 Sequence。

## 环境

仓库根目录：

```powershell
pnpm install
```

视频工程使用：

- Remotion 4；
- React 19；
- TypeScript；
- Lucide 图标；
- ScoreLeap 自有 Logo / 背景资产；
- Fontsource 分发的 Geist / JetBrains Mono / Playfair Display。

Remotion 首次渲染时会按其官方流程准备 Headless Chrome。

## 不带私有素材直接预览

无需任何本地音频即可启动 fallback 版本：

```powershell
pnpm video:studio
```

fallback 使用确定性的 synthetic note data，因此贡献者 clone 仓库后不会因为缺少用户私有媒体而打不开 Studio。

## 使用真实本地钢琴音频

不要把音频复制到 tracked 目录。使用准备脚本：

```powershell
pnpm --filter scoreleap-video prepare:local -- `
  -AudioPath "D:\path\to\piano.mp3" `
  -WorkerPath "D:\path\to\scoreleap-transkun-worker.exe"
```

脚本会在本地完成：

1. 将音频复制到 `public/local/`；
2. 使用 ffmpeg 生成 waveform PNG；
3. 调用 ScoreLeap 自包含 Transkun Worker；
4. 从真实 metadata 提取 `start/end/pitch/velocity`；
5. 生成 `local-props.json`；
6. 输出音符数、音域、高音区数量、48–83 直接可弹数量。

以下内容均已 `.gitignore`：

- `public/local/`；
- `.video-local/`；
- `local-props.json`；
- `out/`。

真实数据模式启动：

```powershell
pnpm video:studio:local
```

真实数据渲染：

```powershell
pnpm video:render:local
```

### 版权提醒

“本机存在一个音频文件”不等于拥有公开传播权。本地第三方歌曲可用于开发、调动画和测试，但正式上传 B 站前应确认音频授权，或替换成自己录制/明确授权的纯钢琴素材。

仓库不得提交第三方 MP3、完整转录 MIDI、长 metadata 或其他可还原第三方歌曲的派生资产。

## 替换真实游戏录像

推荐只通过同一个准备入口替换：

```powershell
pnpm --filter scoreleap-video prepare:local -- `
  -AudioPath "D:\path\to\authorized-piano.mp3" `
  -WorkerPath "D:\path\to\scoreleap-transkun-worker.exe" `
  -GameplayPath "D:\path\to\gameplay.mp4"
```

脚本会：

- 将实机录像复制到 ignored 的 `public/local/gameplay.mp4`；
- 用 ffprobe 读取真实时长；
- 把 `gameplaySrc` 与 `gameplayDurationSeconds` 写入 `local-props.json`；
- 主 Composition 自动移动 Ending。

如果没有提供 `GameplayPath`，视频会显示一个明确的 `REPLACE WITH REAL GAMEPLAY` 占位段，不会伪造游戏实机。

## 常用命令

```powershell
# 视频 TypeScript
pnpm --filter scoreleap-video typecheck

# Remotion Studio（fallback）
pnpm video:studio

# Remotion Studio（本地真实数据）
pnpm video:studio:local

# 15 秒快速预览
pnpm --filter scoreleap-video render:preview

# 完整 fallback 主片
pnpm video:render

# 完整真实数据主片
pnpm video:render:local

# 封面/关键静帧
pnpm video:still
```

## Composition

- `ScoreLeapTrailer90`：完整主宣传片；
- `ScoreLeapAppFlowDemo`：40s 应用流程 QA 段；
- `ScoreLeapHookDemo` / `ScoreLeapIdentityVDemo` / `ScoreLeapGameplaySlot` / `ScoreLeapEndingDemo`：各段独立 QA；
- `ScoreLeapRangeDemo`：独立 SmartFold / 88→36 QA；
- `ScoreLeapMelodyDemo`：独立 Melody First QA。

## 视觉规则

宣传片延续 ScoreLeap 现有 High-Tech Minimalist 视觉：

- 大面积深黑；
- 金色只用于关键价值、选择结果与主旋律；
- 深绿用于状态、数据与次级结构；
- 技术网格/HUD 作为背景，不堆叠素材包；
- 中文主叙事 + 极短英文技术标签；
- 高音始终位于 Piano Roll 上方；
- 算法可视化与产品真实行为保持一致。
- Identity V 段使用程序化哥特第二色板（旧纸张/暗红褐/旧金属/烛光）与真实 36 键位映射，不引入任何未经授权的官方角色图、Logo 或截图。

不要为了“炫”加入与产品无关的云端、自动编曲、游戏搜索下载等尚未实现能力。

## 发布前 QA

正式出片至少检查：

- 1080p / 60 FPS；
- 首帧无白屏；
- MP3 Hook 文字和文件卡无裁切；
- waveform 与 Note Events 正常；
- SmartFold 最终落在游戏音域；
- Melody First before/after 可一眼看懂；
- 产品 UI 中文字体无 fallback 错位；
- CG → Gameplay 无黑帧；
- Ending 信息停留足够长；
- ffprobe 的 codec/fps/duration 正确；
- 最终 `git status` 不包含 MP3、gameplay、metadata、MIDI 和 render 输出。

素材来源和许可证见 `ASSET_LICENSES.md`。

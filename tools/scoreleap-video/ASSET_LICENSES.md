# ScoreLeap Promo Video — Asset & License Inventory

本文件只记录宣传视频工程使用的外部依赖/视觉资产来源。最终发布前如新增图片、音效、视频或字体，必须同步更新。

## Project-owned assets

以下文件复制自 ScoreLeap 仓库自身，仅用于 ScoreLeap 自有宣传：

- `public/brand/scoreleap-icon.png`
  - source: `apps/scoreleap/src-tauri/icons/icon.png`
- `public/brand/bg-library.webp`
  - source: `apps/scoreleap/public/bg-library3.webp`
- `public/brand/bg-arrange.webp`
  - source: `apps/scoreleap/public/bg-arrange.webp`
- `public/brand/bg-settings.webp`
  - source: `apps/scoreleap/public/bg-settings.webp`

这些不是第三方素材包。

## Remotion

- package: `remotion`
- pinned version for this tool: `4.0.512`
- package license field: `SEE LICENSE IN LICENSE.md`
- official project: `https://www.remotion.dev/`
- usage: React-based programmatic video composition and rendering

Remotion 使用自有许可证条款，不应简单标记为 MIT。开发者/团队在发布和商业使用前应按官方当前许可证页面确认适用条件。

## Lucide

- package: `lucide-react`
- pinned version: `1.31.0`
- license: `ISC`
- official project: `https://lucide.dev/`
- usage: generic line icons such as audio, CPU, cloud-off, account-off and keyboard symbols

未复制搜索引擎图片或来源不明 SVG。

## Fonts via Fontsource

- `@fontsource/geist` — `OFL-1.1`
- `@fontsource/jetbrains-mono` — `OFL-1.1`
- `@fontsource/playfair-display` — `OFL-1.1`

字体由 npm 依赖提供，不在仓库中单独复制/分享字体文件。

## Local-only media (not distributable by this repository)

以下路径被 `.gitignore`，不属于仓库公开资产：

- `public/local/*`
- `.video-local/*`
- `local-props.json`
- `out/*`

它们可能包含：

- 用户本地 MP3/WAV/FLAC；
- Transkun 完整 metadata/MIDI；
- 用户游戏实机录像；
- 最终/测试渲染输出。

第三方歌曲即使可以在开发机读取，也不能因此推定拥有公开传播权。正式发布视频前，应由发布者确认音乐和游戏录像的使用授权/平台规则。

## Prohibited asset sourcing

除非后续明确记录来源和许可，不得加入：

- 搜索引擎直接下载的图片；
- 来源不明 HUD / 粒子 / 转场素材包；
- 未授权音乐或音效包；
- 游戏官方宣传图、Logo 或角色图并声称为 ScoreLeap 自有资产；
- 许可证无法追溯的字体文件。

当前视频的核心 HUD、网格、音符、连线、卡片和动效均使用 React/CSS 程序化绘制。

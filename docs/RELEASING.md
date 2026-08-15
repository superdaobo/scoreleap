# 发版流程 — ScoreLeap（谱跃）

本文档说明版本号如何管理、如何打 tag 触发 GitHub Actions 自动构建发布。

## 命名与版本规范

- 软件名固定为 **ScoreLeap**（驼峰），不随版本变化、不加后缀：
  - 安装包：`ScoreLeap-setup.exe`（由 `scripts/Rename-NsisInstaller.ps1` 从 Tauri 默认的 `ScoreLeap_<版本>_<架构>-setup.exe` 重命名而来）
  - 主程序：安装后为 `ScoreLeap.exe`（`tauri.conf.json` 的 `mainBinaryName`）
  - 内部 sidecar（`scoreleap-transcriber-native.exe`、`scoreleap-transkun-worker.exe`）属技术组件，按代码常量名加载，不参与统一命名
- **版本号不进入任何产物文件名**，只显示在：
  - 应用内：顶栏 `v0.3.1`、设置页「关于」、风险确认页页脚 `SCORELEAP_SYS_v0.3.1`（均为 `getVersion()` 动态获取）
  - 系统文件属性（Windows 文件版本信息）
- 版本策略：**固定当前版本，正式发版时手动递增**（patch/minor），不做 CI 自动递增。

## 版本号存放位置（4 处 + 前端兜底）

发版递增版本时，必须同步修改以下全部位置：

| 位置 | 文件 |
|---|---|
| 工作区版本 | `Cargo.toml`（`[workspace.package] version`） |
| Tauri 应用版本 | `apps/scoreleap/src-tauri/tauri.conf.json`（`version`） |
| 根前端包 | `package.json`（`version`） |
| 应用前端包 | `apps/scoreleap/package.json`（`version`） |
| 前端兜底字符串 | `apps/scoreleap/src/App.vue`、`apps/scoreleap/src/views/SettingsPage.vue`、`apps/scoreleap/src/views/RiskPage.vue` 中的 `'0.3.1'` fallback（`getVersion()` 失败时显示） |

## 发布步骤

以下以 `0.3.1 → 0.3.2` 为例：

1. 新建分支（如 `release/0.3.2`），同步修改上表 4 处版本号 + 前端兜底字符串。
2. 本地验证全绿：

   ```bash
   pnpm typecheck && pnpm test
   cargo fmt --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
   ```

3. （可选）本地打包验证：`powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1`，产物输出到 `dist/release/ScoreLeap-setup.exe`。
   - 注意：打包前需先退出正在运行的 ScoreLeap（否则 `target/release/ScoreLeap.exe` 被锁定，tauri build 报 os error 5）。
4. （建议）安装包验收（Issue #61 起必做，静默命令不受 MSYS 路径转换干扰，须用 PowerShell 启动）：
   - 中文向导：双击运行 `ScoreLeap-setup.exe`，确认安装向导界面为简体中文（标题「ScoreLeap 安装」、按钮「下一步(N)」「取消(C)」），确认后取消；
   - 静默安装：`Start-Process ... -ArgumentList '/S','/D=<测试目录>' -Wait`，检查安装目录含 `ScoreLeap.exe` / `resources` / `uninstall.exe`；
   - 卸载入口：检查「程序和功能」注册表项（`HKCU\...\Uninstall\ScoreLeap`）与开始菜单「卸载 ScoreLeap」快捷方式；
   - 覆盖升级：再次运行安装包，确认退出码 0 且 `%APPDATA%\com.superdaobo.scoreleap` 用户数据保留；
   - 静默卸载：运行 `<安装目录>\uninstall.exe /S`，确认安装目录/快捷方式/注册表项全部清理。
5. 提交（Conventional Commits，如 `chore: bump version to 0.3.2`），合并到 `main`。
6. 打 tag 并推送：

   ```bash
   git tag v0.3.2
   git push origin v0.3.2
   ```

7. GitHub Actions（`.github/workflows/windows-build.yml`）自动触发：
   - 构建 NSIS 安装包 → 重命名为 `ScoreLeap-setup.exe` → 计算 SHA256；
   - 推 `v*` tag 时自动创建 GitHub **Release**，上传 `ScoreLeap-setup.exe` 与 `scoreleap-windows-installer.sha256`。

8. 在 Release 页确认安装包与 SHA256 后，向用户公布。

## 相关脚本

| 脚本 | 用途 |
|---|---|
| `scripts/package.ps1` | 一键打包：NSIS 构建（含前端构建）→ 重命名 → 输出 `dist/release/ScoreLeap-setup.exe`（`-SkipBuild` 复用现有产物） |
| `scripts/Rename-NsisInstaller.ps1` | 把 Tauri 默认命名的安装包重命名为 `ScoreLeap-setup.exe`（幂等） |
| `tools/native-transcriber-packaging/Publish-WindowsArtifactMetadata.ps1` | CI 定位安装包并计算 SHA256 |

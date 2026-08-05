[CmdletBinding()]
param(
    [string]$RepositoryRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Join-Path $PSScriptRoot "../.."
}
$RepositoryRoot = [IO.Path]::GetFullPath($RepositoryRoot)

$tauriConfigPath = Join-Path $RepositoryRoot "apps/scoreleap/src-tauri/tauri.conf.json"
$tauriConfig = [IO.File]::ReadAllText($tauriConfigPath, [Text.Encoding]::UTF8) | ConvertFrom-Json
$resources = @($tauriConfig.bundle.resources)
if ("resources/scoreleap-transcriber/" -notin $resources) {
    throw "tauri.conf.json 未声明原生转录资源目录"
}
if (@($resources | Where-Object { $_ -match '(?i)(transcription-worker|python|venv)' }).Count -gt 0) {
    throw "tauri.conf.json 仍引用 Python worker 或虚拟环境"
}

$workflowPath = Join-Path $RepositoryRoot ".github/workflows/windows-build.yml"
$workflow = [IO.File]::ReadAllText($workflowPath, [Text.Encoding]::UTF8)
$requiredWorkflowFragments = @(
    "cargo.exe test --locked -p scoreleap-transcribe -p scoreleap-transcriber-native",
    "pnpm.cmd test",
    "pnpm.cmd build",
    "Prepare-NativeTranscriber.ps1",
    "Test-NativeTranscriberBundle.ps1",
    "Invoke-NativeTranscriberE2E.ps1",
    "tauri build --bundles nsis",
    "Test-CleanInstall.ps1"
)
foreach ($fragment in $requiredWorkflowFragments) {
    if (-not $workflow.Contains($fragment)) {
        throw "Windows workflow 缺少必要步骤: $fragment"
    }
}
if ($workflow -match '(?i)(pyinstaller|pip install|transcription-worker/.venv)') {
    throw "Windows workflow 不得依赖 Python worker"
}

$prepareScript = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "Prepare-NativeTranscriber.ps1"), [Text.Encoding]::UTF8)
$readme = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "README.md"), [Text.Encoding]::UTF8)
$fixedDigest = "d2319fddfb6ea4db99ccc4b60c85c517bcd855721f5daa6a06d40d7cb2ee2357"
if (-not $prepareScript.Contains($fixedDigest) -or -not $readme.Contains($fixedDigest) -or -not $workflow.Contains($fixedDigest)) {
    throw "准备脚本、README 与 workflow 的 ONNX Runtime 固定摘要不一致"
}

Write-Host "打包源配置通过：Tauri JSON 可解析，Windows workflow 契约完整，未引用 Python worker。"

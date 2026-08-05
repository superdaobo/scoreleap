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
    "Test-PackagingSecurity.ps1",
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
$commonScript = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "Packaging.Common.ps1"), [Text.Encoding]::UTF8)
$e2eScript = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "Invoke-NativeTranscriberE2E.ps1"), [Text.Encoding]::UTF8)
$readme = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "README.md"), [Text.Encoding]::UTF8)
$fixedDigest = "d2319fddfb6ea4db99ccc4b60c85c517bcd855721f5daa6a06d40d7cb2ee2357"
if (-not $prepareScript.Contains($fixedDigest) -or -not $readme.Contains($fixedDigest) -or -not $workflow.Contains($fixedDigest)) {
    throw "准备脚本、README 与 workflow 的 ONNX Runtime 固定摘要不一致"
}
$assetUrl = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.4/onnxruntime-win-x64-1.24.4.zip"
if (-not $readme.Contains($assetUrl) -or -not $prepareScript.Contains('https://github.com/microsoft/onnxruntime/releases/download/v$runtimeVersion/$runtimeArchiveName')) {
    throw "准备脚本与 README 的官方资产 URL 构造不一致"
}
foreach ($fragment in @("74442783", "376015528")) {
    if (-not $prepareScript.Contains($fragment) -or -not $readme.Contains($fragment)) {
        throw "准备脚本与 README 的官方资产身份不一致: $fragment"
    }
}
if (-not $prepareScript.Contains("Assert-SafeZipArchive") -or -not $commonScript.Contains("ZIP 条目将逃逸解压目录")) {
    throw "准备脚本缺少 ZIP 路径穿越防护"
}
if (-not $e2eScript.Contains("metadata.duration_seconds") -or $e2eScript.Contains("metadata.source.duration_ms")) {
    throw "真实 E2E 与原生 runtime metadata 契约不一致"
}

Write-Host "打包源配置通过：Tauri JSON 可解析，Windows workflow 契约完整，未引用 Python worker。"

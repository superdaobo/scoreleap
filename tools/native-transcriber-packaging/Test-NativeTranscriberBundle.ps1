[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ResourceDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ResourceDirectory = [IO.Path]::GetFullPath($ResourceDirectory)
if (-not (Test-Path -LiteralPath $ResourceDirectory -PathType Container)) {
    throw "原生资源目录不存在: $ResourceDirectory"
}

$requiredNames = @(
    "scoreleap-transcriber-native.exe",
    "onnxruntime.dll",
    "onnxruntime_providers_shared.dll",
    "LICENSE.onnxruntime.txt",
    "ThirdPartyNotices.onnxruntime.txt",
    "runtime-manifest.json"
)
$files = @(Get-ChildItem -LiteralPath $ResourceDirectory -Recurse -File)
$relativeNames = @($files | ForEach-Object {
    $_.FullName.Substring($ResourceDirectory.Length).TrimStart('\', '/').Replace('\', '/')
})

$missing = @($requiredNames | Where-Object { $_ -notin $relativeNames })
$unexpected = @($relativeNames | Where-Object { $_ -notin $requiredNames })
if ($missing.Count -gt 0) {
    throw "资源缺少必要文件: $($missing -join ', ')"
}
if ($unexpected.Count -gt 0) {
    throw "资源包含未批准文件: $($unexpected -join ', ')"
}

$forbiddenPattern = '(?i)(^|/)(python[^/]*\.dll|librosa|numba|tensorflow|site-packages|\.venv)(/|$)|(?i)\.(onnx|tflite|pb)$'
$forbidden = @($relativeNames | Where-Object { $_ -match $forbiddenPattern })
if ($forbidden.Count -gt 0) {
    throw "资源包含 Python 运行时、禁用依赖或内置模型: $($forbidden -join ', ')"
}

$manifestPath = Join-Path $ResourceDirectory "runtime-manifest.json"
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.schema_version -ne 1) {
    throw "不支持的 runtime-manifest schema_version: $($manifest.schema_version)"
}
if ($manifest.onnx_runtime.version -ne "1.24.4" -or $manifest.onnx_runtime.api_version -ne 24) {
    throw "ONNX Runtime 版本或 API 版本不符合发布契约"
}
if (
    $manifest.onnx_runtime.architecture -ne "x64" -or
    $manifest.onnx_runtime.execution_provider -ne "cpu" -or
    $manifest.onnx_runtime.asset_id -ne 376015528 -or
    $manifest.onnx_runtime.archive_name -ne "onnxruntime-win-x64-1.24.4.zip" -or
    $manifest.onnx_runtime.archive_size_bytes -ne 74442783 -or
    $manifest.onnx_runtime.asset_url -ne "https://github.com/microsoft/onnxruntime/releases/download/v1.24.4/onnxruntime-win-x64-1.24.4.zip" -or
    $manifest.onnx_runtime.asset_api_url -ne "https://api.github.com/repos/microsoft/onnxruntime/releases/assets/376015528"
) {
    throw "runtime-manifest 中的官方资产身份信息不符合固定发布契约"
}
if ($manifest.onnx_runtime.archive_sha256 -ne "d2319fddfb6ea4db99ccc4b60c85c517bcd855721f5daa6a06d40d7cb2ee2357") {
    throw "runtime-manifest 中的官方资产 SHA-256 不符合固定值"
}
if ($manifest.bundled_model -ne $false -or $manifest.python_required -ne $false) {
    throw "runtime-manifest 错误声明了内置模型或 Python 依赖"
}

$expectedManifestNames = @($requiredNames | Where-Object { $_ -ne "runtime-manifest.json" })
$manifestEntries = @($manifest.files)
$manifestNames = @($manifestEntries | ForEach-Object { [string]$_.name })
if ($manifestEntries.Count -ne $expectedManifestNames.Count) {
    throw "runtime-manifest 必须且只能记录五个运行文件/许可证，实际 $($manifestEntries.Count) 个"
}
$duplicateManifestNames = @(
    $manifestNames |
        Group-Object |
        Where-Object { $_.Count -gt 1 } |
        ForEach-Object { $_.Name }
)
if ($duplicateManifestNames.Count -gt 0) {
    throw "runtime-manifest 包含重复文件条目: $($duplicateManifestNames -join ', ')"
}
$missingManifestNames = @($expectedManifestNames | Where-Object { $_ -notin $manifestNames })
$unexpectedManifestNames = @($manifestNames | Where-Object { $_ -notin $expectedManifestNames })
if ($missingManifestNames.Count -gt 0 -or $unexpectedManifestNames.Count -gt 0) {
    throw "runtime-manifest 文件集合不完整：缺少 $($missingManifestNames -join ', ')；未知 $($unexpectedManifestNames -join ', ')"
}

foreach ($entry in $manifestEntries) {
    if ($entry.name -notin $requiredNames -or $entry.name -eq "runtime-manifest.json") {
        throw "runtime-manifest 包含未知文件: $($entry.name)"
    }
    $filePath = Join-Path $ResourceDirectory $entry.name
    $file = Get-Item -LiteralPath $filePath
    if ($file.Length -ne [long]$entry.size_bytes) {
        throw "资源大小与 manifest 不一致: $($entry.name)"
    }
    $actualHash = (Get-FileHash -LiteralPath $filePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $entry.sha256.ToLowerInvariant()) {
        throw "资源 SHA-256 与 manifest 不一致: $($entry.name)"
    }
}

$sidecarPath = Join-Path $ResourceDirectory "scoreleap-transcriber-native.exe"
$sidecarBytes = [IO.File]::ReadAllBytes($sidecarPath)
if ($sidecarBytes.Length -lt 2 -or $sidecarBytes[0] -ne 0x4d -or $sidecarBytes[1] -ne 0x5a) {
    throw "原生 sidecar 不是 Windows PE 文件"
}

Write-Host "原生资源审计通过：$($relativeNames.Count) 个文件，无 Python、虚拟环境或内置模型。"

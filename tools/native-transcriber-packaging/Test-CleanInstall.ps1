[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$InstallerPath,

    [string]$InstallDirectory,
    [string]$ModelPath = $env:SCORELEAP_E2E_MODEL,
    [string]$AudioPath = $env:SCORELEAP_E2E_AUDIO,
    [ValidateRange(1, 60)]
    [int]$InstallTimeoutSeconds = 60,
    [ValidateRange(1, 60)]
    [int]$TranscriptionTimeoutSeconds = 60,
    [switch]$RequireRealAssets
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$InstallerPath = [IO.Path]::GetFullPath($InstallerPath)
if (-not (Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
    throw "NSIS 安装包不存在: $InstallerPath"
}

$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
if ([string]::IsNullOrWhiteSpace($InstallDirectory)) {
    $InstallDirectory = Join-Path $repositoryRoot ".build-tmp/clean-install-$([Guid]::NewGuid().ToString('N'))"
}
$InstallDirectory = [IO.Path]::GetFullPath($InstallDirectory)
if (Test-Path -LiteralPath $InstallDirectory) {
    throw "clean-install 目标必须不存在，拒绝覆盖: $InstallDirectory"
}
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $InstallDirectory) | Out-Null

$installProcess = Start-Process `
    -FilePath $InstallerPath `
    -ArgumentList @("/S", "/D=$InstallDirectory") `
    -PassThru
if (-not $installProcess.WaitForExit($InstallTimeoutSeconds * 1000)) {
    try { $installProcess.Kill() } catch { Write-Warning $_.Exception.Message }
    throw "NSIS 静默安装超过 ${InstallTimeoutSeconds}s"
}
$installProcess.WaitForExit()
if ($installProcess.ExitCode -ne 0) {
    throw "NSIS 静默安装失败，退出码 $($installProcess.ExitCode)"
}

$mainExecutables = @(Get-ChildItem -LiteralPath $InstallDirectory -Recurse -File -Filter "ScoreLeap.exe")
if ($mainExecutables.Count -ne 1) {
    throw "安装目录中 ScoreLeap.exe 数量应为 1，实际 $($mainExecutables.Count)"
}
$sidecars = @(Get-ChildItem -LiteralPath $InstallDirectory -Recurse -File -Filter "scoreleap-transcriber-native.exe")
if ($sidecars.Count -ne 1) {
    throw "安装目录中原生 sidecar 数量应为 1，实际 $($sidecars.Count)"
}

$runtimeDirectory = $sidecars[0].Directory.FullName
& (Join-Path $PSScriptRoot "Test-NativeTranscriberBundle.ps1") -ResourceDirectory $runtimeDirectory

$e2eArguments = @{
    SidecarPath = $sidecars[0].FullName
    RuntimeDirectory = $runtimeDirectory
    ModelPath = $ModelPath
    AudioPath = $AudioPath
    OutputDirectory = (Join-Path $repositoryRoot ".build-tmp/native-e2e-installed")
    TimeoutSeconds = $TranscriptionTimeoutSeconds
}
if ($RequireRealAssets) {
    $e2eArguments.RequireRealAssets = $true
}
& (Join-Path $PSScriptRoot "Invoke-NativeTranscriberE2E.ps1") @e2eArguments

$forbidden = @(Get-ChildItem -LiteralPath $InstallDirectory -Recurse -File | Where-Object {
    $_.FullName -match '(?i)(python[^\\/]*\.dll|librosa|numba|tensorflow|site-packages|\.venv|\.(onnx|tflite|pb)$)'
})
if ($forbidden.Count -gt 0) {
    throw "安装包包含禁止内容: $($forbidden.FullName -join ', ')"
}

Write-Host "Windows clean-install 通过: $InstallDirectory"
Write-Host "安装包 SHA256: $((Get-FileHash -LiteralPath $InstallerPath -Algorithm SHA256).Hash.ToLowerInvariant())"

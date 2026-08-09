[CmdletBinding()]
param(
    [string]$RepositoryRoot,
    [switch]$SkipBuild
)

<#
    一键打包：pnpm tauri build --bundles nsis（前端由 beforeBuildCommand 构建）→
    重命名安装包 → 输出到固定目录 dist/release/ScoreLeap-setup.exe（版本号不进入文件名）。
    -SkipBuild：跳过构建，仅对现有产物重命名并复制到 dist/release。
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Join-Path $PSScriptRoot ".."
}
$RepositoryRoot = [IO.Path]::GetFullPath($RepositoryRoot)
if (-not (Test-Path -LiteralPath (Join-Path $RepositoryRoot "pnpm-workspace.yaml") -PathType Leaf)) {
    throw "仓库根目录不正确：$RepositoryRoot（未找到 pnpm-workspace.yaml）"
}

$installerName = "ScoreLeap-setup.exe"

Push-Location $RepositoryRoot
try {
    if (-not $SkipBuild) {
        # 前端构建由 tauri.conf.json 的 beforeBuildCommand（pnpm build）在 tauri build 时执行
        Write-Host "==> 构建 NSIS 安装包 (pnpm tauri build --bundles nsis)"
        & pnpm.cmd tauri build --bundles nsis
        if ($LASTEXITCODE -ne 0) { throw "tauri build 失败 (exit $LASTEXITCODE)" }
    }
    else {
        Write-Host "==> 跳过构建（-SkipBuild），复用现有产物"
    }
}
finally {
    Pop-Location
}

Write-Host "==> 重命名安装包为 $installerName"
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "Rename-NsisInstaller.ps1") -RepositoryRoot $RepositoryRoot
if ($LASTEXITCODE -ne 0) { throw "安装包重命名失败 (exit $LASTEXITCODE)" }

$searchDirectories = @(
    (Join-Path $RepositoryRoot "target/release/bundle/nsis"),
    (Join-Path $RepositoryRoot "apps/scoreleap/src-tauri/target/release/bundle/nsis")
)
$installer = $null
foreach ($directory in $searchDirectories) {
    $candidate = Join-Path $directory $installerName
    if (Test-Path -LiteralPath $candidate -PathType Leaf) {
        if ($null -ne $installer) {
            throw "多个目录同时存在 $installerName：$installer 与 $candidate"
        }
        $installer = $candidate
    }
}
if ($null -eq $installer) {
    throw "未找到安装包 $installerName（请先运行 tauri build）"
}

$releaseDir = Join-Path $RepositoryRoot "dist/release"
New-Item -ItemType Directory -Path $releaseDir -Force | Out-Null
$target = Join-Path $releaseDir $installerName
Copy-Item -LiteralPath $installer -Destination $target -Force

$hash = (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host ""
Write-Host "打包完成：$target"
Write-Host "SHA256: $hash"

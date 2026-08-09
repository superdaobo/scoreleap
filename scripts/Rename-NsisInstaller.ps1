[CmdletBinding()]
param(
    [string]$RepositoryRoot,
    [string]$ProductName = "ScoreLeap",
    [string]$DesiredName = "$ProductName-setup.exe"
)

<#
    构建后把 Tauri 默认命名的 NSIS 安装包
    ScoreLeap_<version>_<arch>-setup.exe
    重命名为固定文件名 ScoreLeap-setup.exe（版本号不进入文件名）。
    幂等：如果安装包已命名为目标名则直接返回。
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Join-Path $PSScriptRoot ".."
}
$RepositoryRoot = [IO.Path]::GetFullPath($RepositoryRoot)

$searchDirectories = @(
    (Join-Path $RepositoryRoot "target/release/bundle/nsis"),
    (Join-Path $RepositoryRoot "apps/scoreleap/src-tauri/target/release/bundle/nsis")
)

# 模板名：<ProductName>_<version>_<arch>-setup.exe，例如 ScoreLeap_0.3.1_x64-setup.exe
$templatePattern = "$ProductName`_*-setup.exe"

$templateInstallers = @()
$alreadyNamed = @()
foreach ($directory in $searchDirectories) {
    if (Test-Path -LiteralPath $directory -PathType Container) {
        $templateInstallers += Get-ChildItem -LiteralPath $directory -File -Filter $templatePattern
        $alreadyNamed += Get-ChildItem -LiteralPath $directory -File -Filter $DesiredName
    }
}
$templateInstallers = @($templateInstallers | Sort-Object FullName -Unique)
$alreadyNamed = @($alreadyNamed | Sort-Object FullName -Unique)

if ($templateInstallers.Count -eq 0 -and $alreadyNamed.Count -eq 1) {
    Write-Host "安装包已是目标名 $DesiredName，无需重命名：$($alreadyNamed[0].FullName)"
    return
}

if ($templateInstallers.Count -ne 1) {
    throw "NSIS 安装包（模板 $templatePattern）数量应为 1，实际 $($templateInstallers.Count)：$($templateInstallers.FullName -join ', ')"
}
if ($alreadyNamed.Count -gt 0) {
    # 上次构建的旧目标文件（本次构建产物以模板名重新生成），先移除旧目标再重命名
    foreach ($f in $alreadyNamed) {
        Remove-Item -LiteralPath $f.FullName -Force
        Write-Host "移除旧安装包: $($f.FullName)"
    }
}

$source = $templateInstallers[0]
$target = Join-Path $source.DirectoryName $DesiredName
Rename-Item -LiteralPath $source.FullName -NewName $DesiredName
Write-Host "安装包重命名完成: $($source.FullName) -> $target"

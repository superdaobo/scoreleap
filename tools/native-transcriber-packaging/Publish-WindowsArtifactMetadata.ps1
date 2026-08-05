[CmdletBinding()]
param(
    [string]$RepositoryRoot,
    [string]$GithubOutput = $env:GITHUB_OUTPUT
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Join-Path $PSScriptRoot "../.."
}
$RepositoryRoot = [IO.Path]::GetFullPath($RepositoryRoot)

$searchDirectories = @(
    (Join-Path $RepositoryRoot "target/release/bundle/nsis"),
    (Join-Path $RepositoryRoot "apps/scoreleap/src-tauri/target/release/bundle/nsis")
)
$installers = @()
foreach ($directory in $searchDirectories) {
    if (Test-Path -LiteralPath $directory -PathType Container) {
        $installers += Get-ChildItem -LiteralPath $directory -File -Filter "*.exe"
    }
}
$installers = @($installers | Sort-Object FullName -Unique)
if ($installers.Count -ne 1) {
    throw "NSIS 安装包数量应为 1，实际 $($installers.Count)：$($installers.FullName -join ', ')"
}

$installer = $installers[0]
$hash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
$hashFile = Join-Path $RepositoryRoot "scoreleap-windows-installer.sha256"
"$hash  $($installer.Name)" | Set-Content -LiteralPath $hashFile -Encoding ASCII

if (-not [string]::IsNullOrWhiteSpace($GithubOutput)) {
    "installer_path=$($installer.FullName)" | Add-Content -LiteralPath $GithubOutput -Encoding UTF8
    "installer_sha256=$hash" | Add-Content -LiteralPath $GithubOutput -Encoding UTF8
}

Write-Host "NSIS: $($installer.FullName)"
Write-Host "SHA256: $hash"

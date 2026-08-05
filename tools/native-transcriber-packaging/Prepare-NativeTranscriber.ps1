[CmdletBinding()]
param(
    [string]$RepositoryRoot,
    [string]$CacheDirectory,
    [string]$ResourceDirectory,
    [string]$SidecarPath,
    [switch]$SkipNativeBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$runtimeVersion = "1.24.4"
$runtimeArchiveName = "onnxruntime-win-x64-$runtimeVersion.zip"
$runtimeArchiveSize = 74442783L
$runtimeArchiveSha256 = "d2319fddfb6ea4db99ccc4b60c85c517bcd855721f5daa6a06d40d7cb2ee2357"
$runtimeAssetId = 376015528
$runtimeUrl = "https://github.com/microsoft/onnxruntime/releases/download/v$runtimeVersion/$runtimeArchiveName"
$runtimeApiUrl = "https://api.github.com/repos/microsoft/onnxruntime/releases/assets/$runtimeAssetId"

function Get-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return [IO.Path]::GetFullPath($Path)
}

function Assert-FileHash {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedSha256
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "文件不存在: $Path"
    }
    $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $ExpectedSha256.ToLowerInvariant()) {
        throw "SHA-256 校验失败: $Path，期望 $ExpectedSha256，实际 $actual"
    }
}

function Copy-RequiredFile {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        throw "官方 ONNX Runtime 资产缺少必要文件: $Source"
    }
    Copy-Item -LiteralPath $Source -Destination $Destination
}

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Join-Path $PSScriptRoot "../.."
}
$RepositoryRoot = Get-FullPath $RepositoryRoot

if ([string]::IsNullOrWhiteSpace($CacheDirectory)) {
    $CacheDirectory = Join-Path $RepositoryRoot ".build-tmp/packaging-cache"
}
$CacheDirectory = Get-FullPath $CacheDirectory

if ([string]::IsNullOrWhiteSpace($ResourceDirectory)) {
    $ResourceDirectory = Join-Path $RepositoryRoot "apps/scoreleap/src-tauri/resources/scoreleap-transcriber"
}
$ResourceDirectory = Get-FullPath $ResourceDirectory

if ([string]::IsNullOrWhiteSpace($SidecarPath)) {
    $SidecarPath = Join-Path $RepositoryRoot "target/release/scoreleap-transcriber-native.exe"
}
$SidecarPath = Get-FullPath $SidecarPath

$taskTemp = Join-Path $RepositoryRoot ".build-tmp"
New-Item -ItemType Directory -Force -Path $taskTemp | Out-Null
New-Item -ItemType Directory -Force -Path $CacheDirectory | Out-Null

if (-not $SkipNativeBuild) {
    $env:TEMP = $taskTemp
    $env:TMP = $taskTemp
    & cargo.exe build --locked --release -p scoreleap-transcriber-native
    if ($LASTEXITCODE -ne 0) {
        throw "scoreleap-transcriber-native release 构建失败，退出码 $LASTEXITCODE"
    }
}

if (-not (Test-Path -LiteralPath $SidecarPath -PathType Leaf)) {
    throw "原生 sidecar 不存在: $SidecarPath"
}
if ((Get-Item -LiteralPath $SidecarPath).Length -eq 0) {
    throw "原生 sidecar 为空: $SidecarPath"
}
$sidecarHeader = [IO.File]::ReadAllBytes($SidecarPath)
if ($sidecarHeader.Length -lt 2 -or $sidecarHeader[0] -ne 0x4d -or $sidecarHeader[1] -ne 0x5a) {
    throw "原生 sidecar 不是有效的 Windows PE 文件: $SidecarPath"
}

$cachedArchive = Join-Path $CacheDirectory $runtimeArchiveName
$cacheIsValid = $false
if (Test-Path -LiteralPath $cachedArchive -PathType Leaf) {
    try {
        $cachedSize = (Get-Item -LiteralPath $cachedArchive).Length
        if ($cachedSize -ne $runtimeArchiveSize) {
            throw "缓存大小不符，期望 $runtimeArchiveSize，实际 $cachedSize"
        }
        Assert-FileHash -Path $cachedArchive -ExpectedSha256 $runtimeArchiveSha256
        $cacheIsValid = $true
        Write-Host "复用已校验的 ONNX Runtime 缓存: $cachedArchive"
    }
    catch {
        Write-Warning "忽略无效缓存: $($_.Exception.Message)"
    }
}

if (-not $cacheIsValid) {
    $downloadPath = "$cachedArchive.download-$([Guid]::NewGuid().ToString('N'))"
    try {
        Write-Host "下载微软官方 ONNX Runtime $runtimeVersion x64 CPU 资产"
        for ($attempt = 1; $attempt -le 3; $attempt++) {
            try {
                Invoke-WebRequest -Uri $runtimeUrl -OutFile $downloadPath -UseBasicParsing
                break
            }
            catch {
                if ($attempt -eq 3) {
                    throw
                }
                Write-Warning "第 $attempt 次下载失败，2 秒后重试: $($_.Exception.Message)"
                Start-Sleep -Seconds 2
            }
        }
        $downloadSize = (Get-Item -LiteralPath $downloadPath).Length
        if ($downloadSize -ne $runtimeArchiveSize) {
            throw "下载大小不符，期望 $runtimeArchiveSize，实际 $downloadSize"
        }
        Assert-FileHash -Path $downloadPath -ExpectedSha256 $runtimeArchiveSha256
        Move-Item -LiteralPath $downloadPath -Destination $cachedArchive -Force
    }
    finally {
        if (Test-Path -LiteralPath $downloadPath) {
            Remove-Item -LiteralPath $downloadPath -Force
        }
    }
}

# 即使来自缓存也再次校验，缓存命中绝不绕过供应链校验。
if ((Get-Item -LiteralPath $cachedArchive).Length -ne $runtimeArchiveSize) {
    throw "ONNX Runtime 缓存大小校验失败: $cachedArchive"
}
Assert-FileHash -Path $cachedArchive -ExpectedSha256 $runtimeArchiveSha256

$resourceParent = Split-Path -Parent $ResourceDirectory
$resourceLeaf = Split-Path -Leaf $ResourceDirectory
New-Item -ItemType Directory -Force -Path $resourceParent | Out-Null
$extractDirectory = Join-Path $taskTemp "onnxruntime-extract-$([Guid]::NewGuid().ToString('N'))"
$stagingDirectory = Join-Path $resourceParent "$resourceLeaf.staging-$([Guid]::NewGuid().ToString('N'))"
$backupDirectory = Join-Path $resourceParent "$resourceLeaf.backup-$([Guid]::NewGuid().ToString('N'))"
$destinationMoved = $false
$stagingInstalled = $false

try {
    New-Item -ItemType Directory -Path $extractDirectory | Out-Null
    New-Item -ItemType Directory -Path $stagingDirectory | Out-Null
    Expand-Archive -LiteralPath $cachedArchive -DestinationPath $extractDirectory

    $archiveRoot = Join-Path $extractDirectory "onnxruntime-win-x64-$runtimeVersion"
    $runtimeLibDirectory = Join-Path $archiveRoot "lib"
    Copy-RequiredFile -Source (Join-Path $runtimeLibDirectory "onnxruntime.dll") -Destination (Join-Path $stagingDirectory "onnxruntime.dll")
    Copy-RequiredFile -Source (Join-Path $runtimeLibDirectory "onnxruntime_providers_shared.dll") -Destination (Join-Path $stagingDirectory "onnxruntime_providers_shared.dll")
    Copy-RequiredFile -Source (Join-Path $archiveRoot "LICENSE") -Destination (Join-Path $stagingDirectory "LICENSE.onnxruntime.txt")
    Copy-RequiredFile -Source (Join-Path $archiveRoot "ThirdPartyNotices.txt") -Destination (Join-Path $stagingDirectory "ThirdPartyNotices.onnxruntime.txt")
    Copy-Item -LiteralPath $SidecarPath -Destination (Join-Path $stagingDirectory "scoreleap-transcriber-native.exe")

    $files = @()
    foreach ($file in Get-ChildItem -LiteralPath $stagingDirectory -File | Sort-Object Name) {
        $files += [ordered]@{
            name = $file.Name
            size_bytes = $file.Length
            sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }
    $manifest = [ordered]@{
        schema_version = 1
        generated_at_utc = [DateTime]::UtcNow.ToString("o")
        sidecar = [ordered]@{
            name = "scoreleap-transcriber-native.exe"
            cargo_package = "scoreleap-transcriber-native"
        }
        onnx_runtime = [ordered]@{
            version = $runtimeVersion
            api_version = 24
            architecture = "x64"
            execution_provider = "cpu"
            asset_url = $runtimeUrl
            asset_api_url = $runtimeApiUrl
            asset_id = $runtimeAssetId
            archive_name = $runtimeArchiveName
            archive_size_bytes = $runtimeArchiveSize
            archive_sha256 = $runtimeArchiveSha256
            license = "MIT"
        }
        files = $files
        bundled_model = $false
        python_required = $false
    }
    $manifestPath = Join-Path $stagingDirectory "runtime-manifest.json"
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $manifestPath -Encoding UTF8

    if (Test-Path -LiteralPath $ResourceDirectory) {
        Move-Item -LiteralPath $ResourceDirectory -Destination $backupDirectory
        $destinationMoved = $true
    }
    Move-Item -LiteralPath $stagingDirectory -Destination $ResourceDirectory
    $stagingInstalled = $true

    if ($destinationMoved -and (Test-Path -LiteralPath $backupDirectory)) {
        Remove-Item -LiteralPath $backupDirectory -Recurse -Force
        $destinationMoved = $false
    }
    Write-Host "原生转录资源已准备: $ResourceDirectory"
}
catch {
    if ($stagingInstalled -and (Test-Path -LiteralPath $ResourceDirectory)) {
        Remove-Item -LiteralPath $ResourceDirectory -Recurse -Force
        $stagingInstalled = $false
    }
    if ($destinationMoved -and (Test-Path -LiteralPath $backupDirectory)) {
        Move-Item -LiteralPath $backupDirectory -Destination $ResourceDirectory
        $destinationMoved = $false
    }
    throw
}
finally {
    foreach ($temporaryDirectory in @($extractDirectory, $stagingDirectory, $backupDirectory)) {
        if (Test-Path -LiteralPath $temporaryDirectory) {
            Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force
        }
    }
}

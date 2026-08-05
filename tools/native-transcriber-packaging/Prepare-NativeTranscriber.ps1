[CmdletBinding()]
param(
    [string]$RepositoryRoot,
    [string]$CacheDirectory,
    [string]$ResourceDirectory,
    [string]$SidecarPath,
    [string]$LocalRuntimeDirectory,
    [switch]$SkipNativeBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "Packaging.Common.ps1")

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
Assert-SafeDirectoryTarget -Path $ResourceDirectory -Label "原生资源目录"

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

$useLocalRuntime = -not [string]::IsNullOrWhiteSpace($LocalRuntimeDirectory)
if ($useLocalRuntime) {
    $LocalRuntimeDirectory = Get-FullPath $LocalRuntimeDirectory
    if (-not (Test-Path -LiteralPath (Join-Path $LocalRuntimeDirectory "onnxruntime.dll") -PathType Leaf)) {
        throw "本地 ONNX Runtime 目录缺少 onnxruntime.dll: $LocalRuntimeDirectory"
    }
    Write-Host "使用本地 ONNX Runtime 资产（跳过 GitHub 下载）: $LocalRuntimeDirectory"
}

if (-not $cacheIsValid -and -not $useLocalRuntime) {
    $downloadPath = "$cachedArchive.download-$([Guid]::NewGuid().ToString('N'))"
    try {
        Write-Host "下载微软官方 ONNX Runtime $runtimeVersion x64 CPU 资产"
        for ($attempt = 1; $attempt -le 3; $attempt++) {
            try {
                # Windows PowerShell 5.1 在旧系统上可能仍默认 TLS 1.0；GitHub 仅接受现代 TLS。
                [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
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
        if (Test-Path -LiteralPath $cachedArchive -PathType Leaf) {
            # 下载文件与缓存位于同一目录；Replace 在 NTFS 上原子替换且失败时保留旧缓存。
            [IO.File]::Replace($downloadPath, $cachedArchive, $null)
        }
        else {
            [IO.File]::Move($downloadPath, $cachedArchive)
        }
    }
    finally {
        if (Test-Path -LiteralPath $downloadPath) {
            Remove-Item -LiteralPath $downloadPath -Force
        }
    }
}

# 即使来自缓存也再次校验，缓存命中绝不绕过供应链校验；本地资产模式跳过（资产来自已验证的本机安装）。
if (-not $useLocalRuntime) {
    if ((Get-Item -LiteralPath $cachedArchive).Length -ne $runtimeArchiveSize) {
        throw "ONNX Runtime 缓存大小校验失败: $cachedArchive"
    }
    Assert-FileHash -Path $cachedArchive -ExpectedSha256 $runtimeArchiveSha256
}

$resourceParent = Split-Path -Parent $ResourceDirectory
$resourceLeaf = Split-Path -Leaf $ResourceDirectory
New-Item -ItemType Directory -Force -Path $resourceParent | Out-Null
$extractDirectory = Join-Path $taskTemp "onnxruntime-extract-$([Guid]::NewGuid().ToString('N'))"
$stagingDirectory = Join-Path $resourceParent "$resourceLeaf.staging-$([Guid]::NewGuid().ToString('N'))"
$backupDirectory = Join-Path $resourceParent "$resourceLeaf.backup-$([Guid]::NewGuid().ToString('N'))"
$destinationMoved = $false
$stagingInstalled = $false

try {
    New-Item -ItemType Directory -Path $stagingDirectory | Out-Null
    if ($useLocalRuntime) {
        $runtimeLibDirectory = $LocalRuntimeDirectory
        $licenseSource = Join-Path $LocalRuntimeDirectory "LICENSE.onnxruntime.txt"
        $noticesSource = Join-Path $LocalRuntimeDirectory "ThirdPartyNotices.onnxruntime.txt"
        if (-not (Test-Path -LiteralPath $licenseSource -PathType Leaf)) {
            $licenseSource = Join-Path $LocalRuntimeDirectory "LICENSE"
        }
        if (-not (Test-Path -LiteralPath $noticesSource -PathType Leaf)) {
            $noticesSource = Join-Path $LocalRuntimeDirectory "ThirdPartyNotices.txt"
        }
        $runtimeVersion = (Get-Item -LiteralPath (Join-Path $LocalRuntimeDirectory "onnxruntime.dll")).VersionInfo.FileVersion
        $runtimeArchiveName = "local"
        $runtimeArchiveSize = 0L
        $runtimeArchiveSha256 = ""
        $runtimeUrl = "local:$LocalRuntimeDirectory"
        $runtimeApiUrl = ""
        $runtimeAssetId = 0
    }
    else {
        New-Item -ItemType Directory -Path $extractDirectory | Out-Null
        Assert-SafeZipArchive `
            -ArchivePath $cachedArchive `
            -ExtractionRoot $extractDirectory `
            -ExpectedRoot "onnxruntime-win-x64-$runtimeVersion"
        Expand-Archive -LiteralPath $cachedArchive -DestinationPath $extractDirectory

        $archiveRoot = Join-Path $extractDirectory "onnxruntime-win-x64-$runtimeVersion"
        $runtimeLibDirectory = Join-Path $archiveRoot "lib"
        $licenseSource = Join-Path $archiveRoot "LICENSE"
        $noticesSource = Join-Path $archiveRoot "ThirdPartyNotices.txt"
    }
    Copy-RequiredFile -Source (Join-Path $runtimeLibDirectory "onnxruntime.dll") -Destination (Join-Path $stagingDirectory "onnxruntime.dll")
    Copy-RequiredFile -Source (Join-Path $runtimeLibDirectory "onnxruntime_providers_shared.dll") -Destination (Join-Path $stagingDirectory "onnxruntime_providers_shared.dll")
    Copy-RequiredFile -Source $licenseSource -Destination (Join-Path $stagingDirectory "LICENSE.onnxruntime.txt")
    Copy-RequiredFile -Source $noticesSource -Destination (Join-Path $stagingDirectory "ThirdPartyNotices.onnxruntime.txt")
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

    # 先完整审计 staging，再触碰现有资源目录，确保部署失败闭合。
    if ($useLocalRuntime) {
        & (Join-Path $PSScriptRoot "Test-NativeTranscriberBundle.ps1") -ResourceDirectory $stagingDirectory -AllowLocalRuntime
    }
    else {
        & (Join-Path $PSScriptRoot "Test-NativeTranscriberBundle.ps1") -ResourceDirectory $stagingDirectory
    }

    if (Test-Path -LiteralPath $ResourceDirectory) {
        Move-Item -LiteralPath $ResourceDirectory -Destination $backupDirectory
        $destinationMoved = $true
    }
    Move-Item -LiteralPath $stagingDirectory -Destination $ResourceDirectory
    $stagingInstalled = $true

    if ($destinationMoved -and (Test-Path -LiteralPath $backupDirectory)) {
        try {
            Remove-Item -LiteralPath $backupDirectory -Recurse -Force
        }
        catch {
            # 新资源已经完整安装；旧备份清理失败不应触发对部分删除备份的回滚。
            Write-Warning "新资源已安装，但旧备份未能完全清理: $backupDirectory；$($_.Exception.Message)"
        }
        finally {
            $destinationMoved = $false
        }
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
    # 若原目录恢复失败，backup 是唯一可恢复副本，必须保留并让构建失败闭合。
    foreach ($temporaryDirectory in @($extractDirectory, $stagingDirectory)) {
        if (Test-Path -LiteralPath $temporaryDirectory) {
            Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force
        }
    }
    if ($destinationMoved -and (Test-Path -LiteralPath $backupDirectory)) {
        Write-Warning "原资源目录恢复失败，已保留备份以便人工恢复: $backupDirectory"
    }
}

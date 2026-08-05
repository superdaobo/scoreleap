[CmdletBinding()]
param()

Set-StrictMode -Version Latest

function Assert-SafeDirectoryTarget {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $fullPath = [IO.Path]::GetFullPath($Path)
    $rootPath = [IO.Path]::GetPathRoot($fullPath)
    $normalizedPath = $fullPath.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $normalizedRoot = $rootPath.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    if ([string]::IsNullOrWhiteSpace($normalizedPath) -or $normalizedPath -eq $normalizedRoot) {
        throw "$Label 不能是文件系统根目录: $fullPath"
    }
    if ([string]::IsNullOrWhiteSpace((Split-Path -Leaf $normalizedPath))) {
        throw "$Label 缺少安全的末级目录名: $fullPath"
    }
}

function Assert-SafeZipEntries {
    param(
        [Parameter(Mandatory = $true)][object[]]$Entries,
        [Parameter(Mandatory = $true)][string]$ExtractionRoot,
        [Parameter(Mandatory = $true)][string]$ExpectedRoot
    )

    $extractionRootFull = [IO.Path]::GetFullPath($ExtractionRoot)
    $destinationPrefix = $extractionRootFull.TrimEnd(
        [IO.Path]::DirectorySeparatorChar,
        [IO.Path]::AltDirectorySeparatorChar
    ) + [IO.Path]::DirectorySeparatorChar
    $expectedPrefix = "$ExpectedRoot/"
    $seen = @{}
    $required = @(
        "$ExpectedRoot/lib/onnxruntime.dll",
        "$ExpectedRoot/lib/onnxruntime_providers_shared.dll",
        "$ExpectedRoot/LICENSE",
        "$ExpectedRoot/ThirdPartyNotices.txt"
    )
    $totalExpandedBytes = 0L
    foreach ($entry in $Entries) {
        $name = $entry.FullName.Replace('\', '/')
        if ([string]::IsNullOrWhiteSpace($name)) {
            throw "ZIP 包含空路径条目"
        }
        if ($name.StartsWith('/') -or $name.Contains(':') -or $name -match '(^|/)\.\.(/|$)') {
            throw "ZIP 包含不安全路径: $name"
        }
        $canonicalName = $name.TrimEnd('/')
        $segments = @($canonicalName -split '/')
        foreach ($segment in $segments) {
            if (
                [string]::IsNullOrWhiteSpace($segment) -or
                $segment -eq '.' -or
                $segment.EndsWith('.') -or
                $segment.EndsWith(' ') -or
                $segment -match '^(?i:con|prn|aux|nul|com[1-9]|lpt[1-9])(\..*)?$'
            ) {
                throw "ZIP 包含 Windows 不安全路径段: $name"
            }
        }
        if ($canonicalName -ne $ExpectedRoot -and -not $canonicalName.StartsWith($expectedPrefix, [StringComparison]::Ordinal)) {
            throw "ZIP 条目不在固定根目录 $ExpectedRoot 内: $name"
        }
        if ($seen.ContainsKey($canonicalName)) {
            throw "ZIP 包含重复路径（忽略大小写）: $name"
        }
        $seen[$canonicalName] = $true

        $relativePath = $name.Replace('/', [IO.Path]::DirectorySeparatorChar)
        $destination = [IO.Path]::GetFullPath((Join-Path $extractionRootFull $relativePath))
        if (-not $destination.StartsWith($destinationPrefix, [StringComparison]::OrdinalIgnoreCase)) {
            throw "ZIP 条目将逃逸解压目录: $name"
        }

        # 拒绝 Unix 符号链接；官方 Windows ZIP 只应包含普通文件和目录。
        $unixFileType = (($entry.ExternalAttributes -shr 16) -band 0xF000)
        if ($unixFileType -eq 0xA000) {
            throw "ZIP 包含符号链接: $name"
        }
        if ($entry.Length -gt 512MB) {
            throw "ZIP 单个条目解压后过大: $name"
        }
        $totalExpandedBytes += $entry.Length
        if ($totalExpandedBytes -gt 1GB) {
            throw "ZIP 解压后总体积超过 1GB 安全上限"
        }
    }

    foreach ($requiredName in $required) {
        if (-not $seen.ContainsKey($requiredName)) {
            throw "官方 ONNX Runtime ZIP 缺少必要条目: $requiredName"
        }
    }
}

function Assert-SafeZipArchive {
    param(
        [Parameter(Mandatory = $true)][string]$ArchivePath,
        [Parameter(Mandatory = $true)][string]$ExtractionRoot,
        [Parameter(Mandatory = $true)][string]$ExpectedRoot
    )

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archivePathFull = [IO.Path]::GetFullPath($ArchivePath)
    $archive = [IO.Compression.ZipFile]::OpenRead($archivePathFull)
    try {
        Assert-SafeZipEntries `
            -Entries @($archive.Entries) `
            -ExtractionRoot $ExtractionRoot `
            -ExpectedRoot $ExpectedRoot
    }
    finally {
        $archive.Dispose()
    }
}

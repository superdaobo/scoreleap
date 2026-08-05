[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "Packaging.Common.ps1")

function New-ZipEntryStub {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [long]$Length = 1,
        [int]$ExternalAttributes = 0
    )

    return [pscustomobject]@{
        FullName = $Name
        Length = $Length
        ExternalAttributes = $ExternalAttributes
    }
}

function Assert-Rejected {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][object[]]$Entries
    )

    try {
        Assert-SafeZipEntries `
            -Entries $Entries `
            -ExtractionRoot (Join-Path $PSScriptRoot "synthetic-extract") `
            -ExpectedRoot "onnxruntime-win-x64-1.24.4"
    }
    catch {
        Write-Host "安全负例通过: $Label -> $($_.Exception.Message)"
        return
    }
    throw "安全负例未被拒绝: $Label"
}

$root = "onnxruntime-win-x64-1.24.4"
$validEntries = @(
    (New-ZipEntryStub -Name "$root/" -Length 0),
    (New-ZipEntryStub -Name "$root/lib/onnxruntime.dll"),
    (New-ZipEntryStub -Name "$root/lib/onnxruntime_providers_shared.dll"),
    (New-ZipEntryStub -Name "$root/LICENSE"),
    (New-ZipEntryStub -Name "$root/ThirdPartyNotices.txt")
)
Assert-SafeZipEntries `
    -Entries $validEntries `
    -ExtractionRoot (Join-Path $PSScriptRoot "synthetic-extract") `
    -ExpectedRoot $root

Assert-Rejected -Label "父目录穿越" -Entries @(
    $validEntries + (New-ZipEntryStub -Name "$root/../escape.dll")
)
Assert-Rejected -Label "绝对盘符路径" -Entries @(
    $validEntries + (New-ZipEntryStub -Name "C:/escape.dll")
)
Assert-Rejected -Label "NTFS 备用数据流" -Entries @(
    $validEntries + (New-ZipEntryStub -Name "$root/lib/onnxruntime.dll:evil")
)
Assert-Rejected -Label "Windows 设备名" -Entries @(
    $validEntries + (New-ZipEntryStub -Name "$root/NUL.txt")
)
Assert-Rejected -Label "固定根目录外条目" -Entries @(
    $validEntries + (New-ZipEntryStub -Name "sibling/escape.dll")
)
Assert-Rejected -Label "忽略大小写的重复条目" -Entries @(
    $validEntries + (New-ZipEntryStub -Name "$root/LIB/ONNXRUNTIME.DLL")
)
Assert-Rejected -Label "Unix 符号链接" -Entries @(
    $validEntries + (New-ZipEntryStub -Name "$root/link" -ExternalAttributes -1610612736)
)
Assert-Rejected -Label "缺少必要 DLL" -Entries @(
    $validEntries | Where-Object { $_.FullName -ne "$root/lib/onnxruntime.dll" }
)

Assert-SafeDirectoryTarget -Path (Join-Path $PSScriptRoot "safe-target") -Label "测试目录"
Write-Host "打包安全负例全部通过。"

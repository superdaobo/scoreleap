param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

$taskTemp = Join-Path $PSScriptRoot "..\.build-tmp"
New-Item -ItemType Directory -Force -Path $taskTemp | Out-Null

# C 盘空间紧张时，确保 rustc 临时文件落在项目所在的 D 盘。
$env:TEMP = $taskTemp
$env:TMP = $taskTemp

& cargo.exe @CargoArgs
exit $LASTEXITCODE

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, ValueFromRemainingArguments = $true)]
    [string[]]$Paths
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$hasErrors = $false
foreach ($path in $Paths) {
    $fullPath = [IO.Path]::GetFullPath($path)
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        Write-Host "PowerShell 文件不存在: $fullPath" -ForegroundColor Red
        $hasErrors = $true
        continue
    }
    $tokens = $null
    $errors = $null
    [void][Management.Automation.Language.Parser]::ParseFile($fullPath, [ref]$tokens, [ref]$errors)
    if ($errors.Count -gt 0) {
        $hasErrors = $true
        foreach ($parseError in $errors) {
            Write-Host "${fullPath}:$($parseError.Extent.StartLineNumber):$($parseError.Extent.StartColumnNumber): $($parseError.Message)" -ForegroundColor Red
        }
    }
    else {
        Write-Host "PowerShell 语法通过: $fullPath"
    }
}

if ($hasErrors) {
    exit 1
}

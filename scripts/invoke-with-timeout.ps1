[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateRange(1, 3600)]
    [int]$TimeoutSeconds,

    [Parameter(Mandatory = $true)]
    [string]$Executable,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CommandArguments
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function ConvertTo-NativeArgument {
    param([AllowEmptyString()][string]$Value)

    if ($Value.Length -gt 0 -and $Value -notmatch '[\s"]') {
        return $Value
    }
    $builder = New-Object Text.StringBuilder
    [void]$builder.Append('"')
    $backslashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq '\') {
            $backslashes++
            continue
        }
        if ($character -eq '"') {
            [void]$builder.Append(('\' * (($backslashes * 2) + 1)))
            [void]$builder.Append('"')
            $backslashes = 0
            continue
        }
        if ($backslashes -gt 0) {
            [void]$builder.Append(('\' * $backslashes))
            $backslashes = 0
        }
        [void]$builder.Append($character)
    }
    if ($backslashes -gt 0) {
        [void]$builder.Append(('\' * ($backslashes * 2)))
    }
    [void]$builder.Append('"')
    return $builder.ToString()
}

$startInfo = New-Object Diagnostics.ProcessStartInfo
$startInfo.FileName = $Executable
$startInfo.Arguments = (($CommandArguments | ForEach-Object { ConvertTo-NativeArgument $_ }) -join ' ')
$startInfo.UseShellExecute = $false
$startInfo.CreateNoWindow = $false
$process = New-Object Diagnostics.Process
$process.StartInfo = $startInfo
if (-not $process.Start()) {
    throw "无法启动命令: $Executable"
}

if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
    try {
        $process.Kill()
    }
    catch {
        Write-Warning "终止超时进程失败: $($_.Exception.Message)"
    }
    throw "命令超过 ${TimeoutSeconds}s：$Executable $($CommandArguments -join ' ')"
}

$process.WaitForExit()
$exitCode = $process.ExitCode
if ($exitCode -ne 0) {
    throw "命令失败（退出码 $exitCode）：$Executable $($CommandArguments -join ' ')"
}

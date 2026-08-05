[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$SidecarPath,

    [Parameter(Mandatory = $true)]
    [string]$RuntimeDirectory,

    [string]$ModelPath = $env:SCORELEAP_E2E_MODEL,
    [string]$AudioPath = $env:SCORELEAP_E2E_AUDIO,
    [string]$OutputDirectory,
    [ValidateRange(1, 60)]
    [int]$TimeoutSeconds = 60,
    [switch]$RequireRealAssets
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

function Invoke-Worker {
    param(
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$RuntimeDll
    )

    $startInfo = New-Object Diagnostics.ProcessStartInfo
    $startInfo.FileName = $script:SidecarPath
    $startInfo.Arguments = (($Arguments | ForEach-Object { ConvertTo-NativeArgument $_ }) -join ' ')
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.StandardOutputEncoding = New-Object Text.UTF8Encoding($false)
    $startInfo.StandardErrorEncoding = New-Object Text.UTF8Encoding($false)
    $startInfo.EnvironmentVariables["SCORELEAP_ONNX_RUNTIME_PATH"] = $RuntimeDll
    $startInfo.EnvironmentVariables["ORT_DYLIB_PATH"] = $RuntimeDll

    $process = New-Object Diagnostics.Process
    $process.StartInfo = $startInfo
    $stopwatch = [Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) {
        throw "无法启动原生 sidecar: $script:SidecarPath"
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        try { $process.Kill() } catch { Write-Warning $_.Exception.Message }
        throw "原生 sidecar 超过 ${TimeoutSeconds}s 未结束"
    }
    $process.WaitForExit()
    $stopwatch.Stop()
    $stdout = $stdoutTask.Result
    $stderr = $stderrTask.Result

    $messages = @()
    foreach ($line in @($stdout -split "`r?`n" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })) {
        try {
            $messages += $line | ConvertFrom-Json
        }
        catch {
            throw "stdout 出现非 JSONL 内容: $line"
        }
    }
    return [pscustomobject]@{
        ExitCode = $process.ExitCode
        ElapsedMs = $stopwatch.ElapsedMilliseconds
        Messages = @($messages)
        StdErr = $stderr
    }
}

function New-TestToneWav {
    param([Parameter(Mandatory = $true)][string]$Path)

    $sampleRate = 22050
    $sampleCount = [int]($sampleRate / 4)
    $dataSize = $sampleCount * 2
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try {
        $writer = New-Object IO.BinaryWriter($stream)
        $writer.Write([Text.Encoding]::ASCII.GetBytes("RIFF"))
        $writer.Write([int](36 + $dataSize))
        $writer.Write([Text.Encoding]::ASCII.GetBytes("WAVEfmt "))
        $writer.Write([int]16)
        $writer.Write([int16]1)
        $writer.Write([int16]1)
        $writer.Write([int]$sampleRate)
        $writer.Write([int]($sampleRate * 2))
        $writer.Write([int16]2)
        $writer.Write([int16]16)
        $writer.Write([Text.Encoding]::ASCII.GetBytes("data"))
        $writer.Write([int]$dataSize)
        for ($index = 0; $index -lt $sampleCount; $index++) {
            $sample = [int16](8000 * [Math]::Sin(2 * [Math]::PI * 440 * $index / $sampleRate))
            $writer.Write($sample)
        }
        $writer.Flush()
    }
    finally {
        $stream.Dispose()
    }
}

$script:SidecarPath = [IO.Path]::GetFullPath($SidecarPath)
$RuntimeDirectory = [IO.Path]::GetFullPath($RuntimeDirectory)
if (-not (Test-Path -LiteralPath $script:SidecarPath -PathType Leaf)) {
    throw "原生 sidecar 不存在: $script:SidecarPath"
}
$runtimeDll = Join-Path $RuntimeDirectory "onnxruntime.dll"
if (-not (Test-Path -LiteralPath $runtimeDll -PathType Leaf)) {
    throw "onnxruntime.dll 不存在: $runtimeDll"
}
if (-not (Test-Path -LiteralPath (Join-Path $RuntimeDirectory "onnxruntime_providers_shared.dll") -PathType Leaf)) {
    throw "onnxruntime_providers_shared.dll 不存在: $RuntimeDirectory"
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
    $OutputDirectory = Join-Path $repositoryRoot ".build-tmp/native-e2e"
}
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$runDirectory = Join-Path $OutputDirectory ([Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $runDirectory | Out-Null

# 协议冒烟同时验证缺模型必须失败，且 stdout 只能出现 schema v1 JSONL。
$tonePath = Join-Path $runDirectory "protocol-tone.wav"
New-TestToneWav -Path $tonePath
$missingModelPath = Join-Path $runDirectory "missing-model.onnx"
$missingMidiPath = Join-Path $runDirectory "missing-model.mid"
$missingMetadataPath = Join-Path $runDirectory "missing-model.json"
$missingResult = Invoke-Worker -RuntimeDll $runtimeDll -Arguments @(
    "transcribe",
    "--request-id", "packaging-missing-model",
    "--input", $tonePath,
    "--model", $missingModelPath,
    "--onnx-runtime", $runtimeDll,
    "--output-midi", $missingMidiPath,
    "--output-metadata", $missingMetadataPath,
    "--preset", "piano_noise_reduced"
)
if ($missingResult.ExitCode -eq 0) {
    throw "缺模型场景错误地返回成功"
}
$readyMessages = @($missingResult.Messages | Where-Object { $_.type -eq "ready" -and $_.schema_version -eq 1 })
$errorMessages = @($missingResult.Messages | Where-Object { $_.type -eq "error" -and $_.schema_version -eq 1 })
if ($readyMessages.Count -ne 1) {
    throw "JSONL 协议冒烟失败：ready 消息数量应为 1，实际 $($readyMessages.Count)"
}
if ($errorMessages.Count -ne 1 -or $errorMessages[0].code -notin @("MODEL_LOAD_FAILED", "MODEL_NOT_FOUND")) {
    $codes = @($errorMessages | ForEach-Object { $_.code }) -join ', '
    throw "缺模型场景未返回模型加载错误，实际错误码: $codes"
}
if (
    (Test-Path -LiteralPath $missingMidiPath -PathType Leaf) -or
    (Test-Path -LiteralPath $missingMetadataPath -PathType Leaf)
) {
    throw "缺模型失败后不应留下 MIDI 或 metadata 成品"
}
Write-Host "JSONL/缺模型冒烟通过，耗时 $($missingResult.ElapsedMs)ms"

$realAssetsAvailable = -not [string]::IsNullOrWhiteSpace($ModelPath) -and -not [string]::IsNullOrWhiteSpace($AudioPath)
if (-not $realAssetsAvailable) {
    if ($RequireRealAssets) {
        throw "要求真实 E2E，但 SCORELEAP_E2E_MODEL/SCORELEAP_E2E_AUDIO 未同时提供"
    }
    if (-not [string]::IsNullOrWhiteSpace($ModelPath) -or -not [string]::IsNullOrWhiteSpace($AudioPath)) {
        throw "真实 E2E 的模型和音频必须同时提供"
    }
    Write-Host "未注入真实模型/音频，已跳过真实转录；协议与缺模型检查仍已通过。"
    return
}

$ModelPath = [IO.Path]::GetFullPath($ModelPath)
$AudioPath = [IO.Path]::GetFullPath($AudioPath)
if (-not (Test-Path -LiteralPath $ModelPath -PathType Leaf)) {
    throw "真实 E2E 模型不存在: $ModelPath"
}
if (-not (Test-Path -LiteralPath $AudioPath -PathType Leaf)) {
    throw "真实 E2E 音频不存在: $AudioPath"
}

$midiPath = Join-Path $runDirectory "transcribed.mid"
$metadataPath = Join-Path $runDirectory "transcribed.metadata.json"
$realResult = Invoke-Worker -RuntimeDll $runtimeDll -Arguments @(
    "transcribe",
    "--request-id", "packaging-real-audio",
    "--input", $AudioPath,
    "--model", $ModelPath,
    "--onnx-runtime", $runtimeDll,
    "--output-midi", $midiPath,
    "--output-metadata", $metadataPath,
    "--preset", "piano_noise_reduced"
)
if ($realResult.ExitCode -ne 0) {
    $errors = @($realResult.Messages | Where-Object { $_.type -eq "error" } | ConvertTo-Json -Compress)
    throw "真实转录失败（退出码 $($realResult.ExitCode)）：$($errors -join '; ')；stderr=$($realResult.StdErr)"
}
$resultMessages = @($realResult.Messages | Where-Object { $_.type -eq "result" -and $_.schema_version -eq 1 })
if ($resultMessages.Count -ne 1) {
    throw "真实转录未返回唯一 result 消息"
}
if (-not (Test-Path -LiteralPath $midiPath -PathType Leaf) -or -not (Test-Path -LiteralPath $metadataPath -PathType Leaf)) {
    throw "真实转录未生成 MIDI 或 metadata"
}
$midiHeader = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($midiPath), 0, 4)
if ($midiHeader -ne "MThd") {
    throw "生成文件不是标准 MIDI: $midiPath"
}
$metadata = Get-Content -LiteralPath $metadataPath -Raw | ConvertFrom-Json
$durationSeconds = [double]$metadata.duration_seconds
if ([double]::IsNaN($durationSeconds) -or [double]::IsInfinity($durationSeconds) -or $durationSeconds -le 0) {
    throw "metadata 缺少有效 duration_seconds，无法计算 RTF"
}
$durationMs = $durationSeconds * 1000.0
if ([int]$resultMessages[0].note_count -le 0) {
    throw "真实钢琴音频没有识别出任何音符"
}
$rtf = [Math]::Round($realResult.ElapsedMs / $durationMs, 4)
$report = [ordered]@{
    schema_version = 1
    sidecar = $script:SidecarPath
    audio_file = [IO.Path]::GetFileName($AudioPath)
    audio_sha256 = (Get-FileHash -LiteralPath $AudioPath -Algorithm SHA256).Hash.ToLowerInvariant()
    audio_duration_ms = $durationMs
    wall_elapsed_ms = $realResult.ElapsedMs
    rtf = $rtf
    note_count = [int]$resultMessages[0].note_count
    midi_path = $midiPath
    metadata_path = $metadataPath
}
$reportPath = Join-Path $runDirectory "e2e-report.json"
$report | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $reportPath -Encoding UTF8
Write-Host "真实原生转录通过：notes=$($report.note_count)，elapsed=$($report.wall_elapsed_ms)ms，RTF=$rtf"
Write-Host "E2E 报告: $reportPath"

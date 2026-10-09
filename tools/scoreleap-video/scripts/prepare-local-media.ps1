param(
    [string]$AudioPath = $env:SCORELEAP_VIDEO_AUDIO,
    [string]$WorkerPath = $env:SCORELEAP_TRANSKUN_WORKER,
    [string]$GameplayPath = $env:SCORELEAP_VIDEO_GAMEPLAY,
    [int]$GameplayDurationSeconds = 24
)

$ErrorActionPreference = 'Stop'
$env:PYTHONUTF8 = '1'
$env:PYTHONIOENCODING = 'utf-8'

$toolRoot = Split-Path -Parent $PSScriptRoot
$publicLocal = Join-Path $toolRoot 'public\local'
$workLocal = Join-Path $toolRoot '.video-local'
$propsPath = Join-Path $toolRoot 'local-props.json'

if ([string]::IsNullOrWhiteSpace($AudioPath)) {
    throw 'AudioPath is required. Use -AudioPath or SCORELEAP_VIDEO_AUDIO.'
}

$audioItem = Get-Item -LiteralPath $AudioPath -ErrorAction Stop
if ($audioItem.PSIsContainer) {
    throw "AudioPath is not a file: $AudioPath"
}

$extension = $audioItem.Extension.ToLowerInvariant()
if ($extension -notin @('.mp3', '.wav', '.flac')) {
    throw "Unsupported local demo extension: $extension"
}

New-Item -ItemType Directory -Force -Path $publicLocal | Out-Null
New-Item -ItemType Directory -Force -Path $workLocal | Out-Null

$audioPublicName = "demo$extension"
$audioPublicPath = Join-Path $publicLocal $audioPublicName
$waveformPath = Join-Path $publicLocal 'waveform.png'
$metadataPath = Join-Path $workLocal 'transcription-metadata.json'
$midiPath = Join-Path $workLocal 'transcription.mid'
$workerLogPath = Join-Path $workLocal 'transkun-stderr.log'
$summaryPath = Join-Path $workLocal 'summary.json'

Copy-Item -LiteralPath $audioItem.FullName -Destination $audioPublicPath -Force

$ffmpeg = (Get-Command ffmpeg.exe -ErrorAction Stop).Source
$waveformFilter = 'aformat=channel_layouts=mono,showwavespic=s=1600x220:colors=#d0c6ab'
$ffmpegArgs = @(
    '-hide_banner', '-loglevel', 'error', '-y',
    '-i', $audioItem.FullName,
    '-filter_complex', $waveformFilter,
    '-frames:v', '1',
    $waveformPath
)
& $ffmpeg @ffmpegArgs
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $waveformPath -PathType Leaf)) {
    throw 'ffmpeg waveform generation failed.'
}

if ([string]::IsNullOrWhiteSpace($WorkerPath)) {
    throw 'WorkerPath is required. Use -WorkerPath or SCORELEAP_TRANSKUN_WORKER.'
}
$workerItem = Get-Item -LiteralPath $WorkerPath -ErrorAction Stop
if ($workerItem.PSIsContainer -or $workerItem.Extension.ToLowerInvariant() -ne '.exe') {
    throw "WorkerPath is not a valid .exe: $WorkerPath"
}

$workerArgs = @(
    'transcribe',
    '--request-id', 'scoreleap-video-demo',
    '--input', $audioItem.FullName,
    '--output-midi', $midiPath,
    '--output-metadata', $metadataPath,
    '--preset', 'piano_balanced'
)
$previousErrorPreference = $ErrorActionPreference
$ErrorActionPreference = 'Continue'
$protocolLines = & $workerItem.FullName @workerArgs 2> $workerLogPath
$workerExitCode = $LASTEXITCODE
$ErrorActionPreference = $previousErrorPreference
if ($workerExitCode -ne 0) {
    $detail = if (Test-Path -LiteralPath $workerLogPath) { Get-Content -LiteralPath $workerLogPath -Raw -Encoding UTF8 } else { '' }
    throw "Transkun worker failed with exit code $workerExitCode. $detail"
}
if (-not (Test-Path -LiteralPath $metadataPath -PathType Leaf)) {
    throw 'Transkun worker did not produce metadata.'
}

$metadata = Get-Content -LiteralPath $metadataPath -Raw -Encoding UTF8 | ConvertFrom-Json
$notes = @(
    foreach ($note in $metadata.notes) {
        $start = [double]$note.start_seconds
        $end = [double]$note.end_seconds
        $pitch = [int]$note.pitch
        $velocity = [int]$note.velocity
        if ($end -gt $start -and $pitch -ge 21 -and $pitch -le 108) {
            [ordered]@{
                start = [Math]::Round($start, 6)
                end = [Math]::Round($end, 6)
                pitch = $pitch
                velocity = $velocity
            }
        }
    }
)

$pitches = @($notes | ForEach-Object { [int]$_.pitch })
$minPitch = if ($pitches.Count -gt 0) { ($pitches | Measure-Object -Minimum).Minimum } else { $null }
$maxPitch = if ($pitches.Count -gt 0) { ($pitches | Measure-Object -Maximum).Maximum } else { $null }
$highCount = @($notes | Where-Object { $_.pitch -ge 84 -and $_.pitch -le 108 }).Count
$playableCount = @($notes | Where-Object { $_.pitch -ge 48 -and $_.pitch -le 83 }).Count

$cleanLabel = $audioItem.BaseName -replace '\s+-\s+MIDI.*$', ''
if ([string]::IsNullOrWhiteSpace($cleanLabel)) { $cleanLabel = 'PIANO DEMO' }
$sampleLabel = "$cleanLabel - $($extension.TrimStart('.').ToUpperInvariant())"

$gameplayPublicPath = Join-Path $publicLocal 'gameplay.mp4'
if (-not [string]::IsNullOrWhiteSpace($GameplayPath)) {
    $gameplayItem = Get-Item -LiteralPath $GameplayPath -ErrorAction Stop
    if ($gameplayItem.PSIsContainer -or $gameplayItem.Extension.ToLowerInvariant() -ne '.mp4') {
        throw "GameplayPath must point to an MP4 file: $GameplayPath"
    }
    Copy-Item -LiteralPath $gameplayItem.FullName -Destination $gameplayPublicPath -Force
}

$gameplaySrc = $null
$resolvedGameplayDuration = $GameplayDurationSeconds
if (Test-Path -LiteralPath $gameplayPublicPath -PathType Leaf) {
    $gameplaySrc = 'local/gameplay.mp4'
    $ffprobe = (Get-Command ffprobe.exe -ErrorAction Stop).Source
    $probeArgs = @('-v', 'error', '-show_entries', 'format=duration', '-of', 'default=noprint_wrappers=1:nokey=1', $gameplayPublicPath)
    $durationText = (& $ffprobe @probeArgs | Select-Object -First 1).Trim()
    if (-not [string]::IsNullOrWhiteSpace($durationText)) {
        $resolvedGameplayDuration = [double]::Parse($durationText, [System.Globalization.CultureInfo]::InvariantCulture)
    }
}
$resolvedGameplayDuration = [Math]::Max(12, [Math]::Min(45, [int][Math]::Round($resolvedGameplayDuration)))

$props = [ordered]@{
    audioSrc = "local/$audioPublicName"
    waveformSrc = 'local/waveform.png'
    gameplaySrc = $gameplaySrc
    gameplayDurationSeconds = $resolvedGameplayDuration
    notes = $notes
    sampleLabel = $sampleLabel
    dataMode = 'local-real'
}
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$propsJson = $props | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($propsPath, $propsJson, $utf8NoBom)

$summary = [ordered]@{
    file_name = $audioItem.Name
    duration_seconds = [double]$metadata.duration_seconds
    note_count = $notes.Count
    min_pitch = $minPitch
    max_pitch = $maxPitch
    high_register_84_108 = $highCount
    directly_playable_48_83 = $playableCount
    generated_at = (Get-Date).ToString('o')
    worker_protocol_messages = @($protocolLines).Count
}
$summaryJson = $summary | ConvertTo-Json -Depth 5
[System.IO.File]::WriteAllText($summaryPath, $summaryJson, $utf8NoBom)

Write-Host "Local video media prepared: $propsPath"
Write-Host "Notes: $($notes.Count) | Pitch: $minPitch..$maxPitch | High(84-108): $highCount | Playable(48-83): $playableCount"
Write-Host 'public/local, .video-local and local-props.json are gitignored.'

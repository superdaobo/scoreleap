# Basic Pitch Windows Spike 转录脚本（项目 venv，不修改全局环境）
# 用法: .\run.ps1 [-Input <mp3>] [-OutDir <dir>]
param(
    [string]$Input = "out/sample-25s.mp3",
    [string]$OutDir = "out"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

$venvPy = Join-Path $root "..\..\tools\transcription-worker\.venv\Scripts\python.exe"
if (-not (Test-Path $venvPy)) { throw "venv 不存在: $venvPy（先创建 venv 并 pip install basic-pitch）" }

$inputPath = Join-Path $root $Input
if (-not (Test-Path $inputPath)) { throw "输入不存在: $inputPath" }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$sw = [System.Diagnostics.Stopwatch]::StartNew()
& $venvPy -m basic_pitch -i $inputPath -o $OutDir 2>&1
if ($LASTEXITCODE -ne 0) { throw "basic_pitch 转录失败，退出码 $LASTEXITCODE" }
$sw.Stop()

Write-Host ""
Write-Host "== Spike 结果 =="
Write-Host "输入: $inputPath"
Write-Host "转录耗时: $($sw.Elapsed.TotalSeconds) 秒"
Get-ChildItem -Recurse $OutDir -Filter *.mid | ForEach-Object {
    Write-Host "MIDI: $($_.FullName) ($([math]::Round($_.Length/1KB,1)) KB)"
}
Write-Host "输出目录: $OutDir"

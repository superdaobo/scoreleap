[CmdletBinding()]
param(
    [string]$ResourceDirectory,
    [switch]$AllowPlaceholder
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-CompatibleRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$TargetPath
    )
    $base = [IO.Path]::GetFullPath($BasePath).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    $target = [IO.Path]::GetFullPath($TargetPath)
    $baseUri = [Uri]::new($base)
    $targetUri = [Uri]::new($target)
    return [Uri]::UnescapeDataString($baseUri.MakeRelativeUri($targetUri).ToString()).Replace('/', [IO.Path]::DirectorySeparatorChar)
}

if ([string]::IsNullOrWhiteSpace($ResourceDirectory)) {
    $ResourceDirectory = Join-Path $PSScriptRoot "../../apps/scoreleap/src-tauri/resources/scoreleap-transkun"
}
$ResourceDirectory = [IO.Path]::GetFullPath($ResourceDirectory)
if (-not (Test-Path -LiteralPath $ResourceDirectory -PathType Container)) {
    throw "Transkun 资源目录不存在: $ResourceDirectory"
}

$exe = Join-Path $ResourceDirectory "scoreleap-transkun-worker.exe"
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    if ($AllowPlaceholder -and (Test-Path -LiteralPath (Join-Path $ResourceDirectory "README.md") -PathType Leaf)) {
        Write-Host "Transkun 资源尚未构建：仅检测到占位 README（允许跳过二进制审计）"
        exit 0
    }
    throw "Transkun Worker EXE 缺失: $exe"
}

$manifestPath = Join-Path $ResourceDirectory "runtime-manifest.json"
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Transkun runtime-manifest.json 缺失"
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ($manifest.schema_version -ne 1 -or $manifest.engine -ne "transkun-v2") {
    throw "Transkun manifest schema/engine 无效"
}
if (
    $manifest.device -ne "cpu" -or
    $manifest.external_python_required -or
    $manifest.external_pytorch_required -or
    $manifest.external_cuda_required -or
    $manifest.external_ffmpeg_required -or
    $manifest.external_vc_runtime_required
) {
    throw "Transkun manifest 未声明完全自包含 CPU 运行时"
}

$files = @(Get-ChildItem -LiteralPath $ResourceDirectory -File -Recurse)
foreach ($required in @("2.0.pt", "2.0.conf", "torch_cpu.dll", "vcruntime140.dll", "vcruntime140_1.dll")) {
    if (-not ($files | Where-Object Name -eq $required)) {
        throw "Transkun 资源缺少必要文件: $required"
    }
}
foreach ($pattern in @("cudart*.dll", "cudnn*.dll", "cublas*.dll", "nvrtc*.dll", "ffmpeg*.exe", "python.exe", "pip.exe")) {
    $found = $files | Where-Object Name -Like $pattern | Select-Object -First 1
    if ($null -ne $found) {
        throw "Transkun 资源包含禁止文件: $($found.FullName)"
    }
}

$manifestEntries = @{}
foreach ($entry in $manifest.files) {
    $manifestEntries[$entry.path.ToLowerInvariant()] = $entry
}
foreach ($file in $files) {
    $relative = (Get-CompatibleRelativePath -BasePath $ResourceDirectory -TargetPath $file.FullName).Replace('\', '/')
    if ($relative -eq "runtime-manifest.json") {
        continue
    }
    $key = $relative.ToLowerInvariant()
    if (-not $manifestEntries.ContainsKey($key)) {
        throw "Transkun manifest 未记录文件: $relative"
    }
    $entry = $manifestEntries[$key]
    if ([int64]$entry.size_bytes -ne $file.Length) {
        throw "Transkun 文件大小与 manifest 不一致: $relative"
    }
    $actualHash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne ([string]$entry.sha256).ToLowerInvariant()) {
        throw "Transkun 文件 SHA-256 与 manifest 不一致: $relative"
    }
}

$selfTestOutput = & $exe self-test
if ($LASTEXITCODE -ne 0) {
    throw "Transkun Worker self-test 失败，退出码 $LASTEXITCODE；输出: $selfTestOutput"
}
$selfTest = $selfTestOutput | Select-Object -Last 1 | ConvertFrom-Json
if (
    $selfTest.type -ne "result" -or
    -not $selfTest.cpu_only -or
    $selfTest.ffmpeg_required -or
    $selfTest.python_install_required
) {
    throw "Transkun Worker self-test 返回了无效自包含声明: $selfTestOutput"
}

$size = ($files | Measure-Object Length -Sum).Sum
Write-Host "Transkun 资源审计通过：$($files.Count) 个文件，$size bytes，CPU-only，无外部 Python/CUDA/ffmpeg/VC++ 安装依赖。"

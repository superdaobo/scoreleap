[CmdletBinding()]
param(
    [string]$RepositoryRoot,
    [string]$ResourceDirectory,
    [string]$PythonPath,
    [string]$VcRedistDirectory,
    [string]$WheelhouseDirectory,
    [switch]$SkipDependencyInstall,
    [switch]$SkipBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "../native-transcriber-packaging/Packaging.Common.ps1")

$torchVersion = "2.6.0"
$transkunVersion = "2.0.1"
$pythonMinor = "3.11"
$workerName = "scoreleap-transkun-worker"

function Get-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return [IO.Path]::GetFullPath($Path)
}

function Get-CompatibleRelativePath {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$TargetPath
    )
    $base = (Get-FullPath $BasePath).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    $target = Get-FullPath $TargetPath
    $baseUri = [Uri]::new($base)
    $targetUri = [Uri]::new($target)
    return [Uri]::UnescapeDataString($baseUri.MakeRelativeUri($targetUri).ToString()).Replace('/', [IO.Path]::DirectorySeparatorChar)
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Program,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Description
    )
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Description 失败，退出码 $LASTEXITCODE"
    }
}

function Resolve-Python311 {
    param([string]$ExplicitPath)
    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        $candidate = Get-FullPath $ExplicitPath
        if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            throw "指定的 Python 不存在: $candidate"
        }
        return $candidate
    }

    $uv = Get-Command uv.exe -ErrorAction SilentlyContinue
    if ($null -ne $uv) {
        $candidate = (& $uv.Source python find $pythonMinor).Trim()
        if ($LASTEXITCODE -eq 0 -and (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            return (Get-FullPath $candidate)
        }
    }

    throw "未找到 Python $pythonMinor x64。构建机请安装 uv Python $pythonMinor，或传入 -PythonPath；最终用户不需要 Python。"
}

function Copy-AppLocalCrt {
    param(
        [Parameter(Mandatory = $true)][string]$SourcePython,
        [Parameter(Mandatory = $true)][string]$Destination,
        [string]$ExplicitRedistDirectory
    )

    $pythonRoot = Split-Path -Parent $SourcePython
    foreach ($name in @("vcruntime140.dll", "vcruntime140_1.dll")) {
        $source = Join-Path $pythonRoot $name
        if (Test-Path -LiteralPath $source -PathType Leaf) {
            Copy-Item -LiteralPath $source -Destination (Join-Path $Destination $name) -Force
        }
    }

    $redist = $ExplicitRedistDirectory
    if ([string]::IsNullOrWhiteSpace($redist)) {
        $programFilesX86 = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFilesX86)
        if ([string]::IsNullOrWhiteSpace($programFilesX86)) {
            $programFilesX86 = "C:\Program Files (x86)"
        }
        $programFiles = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)
        if ([string]::IsNullOrWhiteSpace($programFiles)) {
            $programFiles = "C:\Program Files"
        }
        $installCandidates = @()
        $vswhere = Join-Path $programFilesX86 "Microsoft Visual Studio/Installer/vswhere.exe"
        if (Test-Path -LiteralPath $vswhere -PathType Leaf) {
            $installCandidates += @(& $vswhere -products * -property installationPath)
        }
        $installCandidates += @(
            (Join-Path $programFiles "Microsoft Visual Studio/2022/Community"),
            (Join-Path $programFilesX86 "Microsoft Visual Studio/2022/BuildTools")
        )
        foreach ($install in $installCandidates | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }) {
            $redistRoot = Join-Path $install "VC/Redist/MSVC"
            $latest = Get-ChildItem -LiteralPath $redistRoot -Directory -ErrorAction SilentlyContinue |
                Sort-Object Name -Descending |
                Select-Object -First 1
            if ($null -ne $latest) {
                $candidate = Join-Path $latest.FullName "x64/Microsoft.VC143.CRT"
                if (Test-Path -LiteralPath $candidate -PathType Container) {
                    $redist = $candidate
                    break
                }
            }
        }
    }

    if (-not [string]::IsNullOrWhiteSpace($redist) -and (Test-Path -LiteralPath $redist -PathType Container)) {
        foreach ($name in @("msvcp140.dll", "msvcp140_1.dll", "msvcp140_2.dll", "concrt140.dll", "vccorlib140.dll")) {
            $source = Join-Path $redist $name
            if (Test-Path -LiteralPath $source -PathType Leaf) {
                Copy-Item -LiteralPath $source -Destination (Join-Path $Destination $name) -Force
            }
        }
        $license = Get-ChildItem -LiteralPath (Split-Path -Parent (Split-Path -Parent $redist)) -Filter "license*.rtf" -File -Recurse -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($null -ne $license) {
            $licenseDir = Join-Path $Destination "licenses"
            New-Item -ItemType Directory -Force -Path $licenseDir | Out-Null
            Copy-Item -LiteralPath $license.FullName -Destination (Join-Path $licenseDir "Microsoft-Visual-Cpp-Runtime.rtf") -Force
        }
    }

    # 若依赖库目录已含 CRT，把它们复制到 EXE 同目录，避免依赖系统级 VC++ Redistributable。
    foreach ($name in @("msvcp140.dll", "msvcp140_1.dll", "msvcp140_2.dll", "vcruntime140.dll", "vcruntime140_1.dll", "concrt140.dll")) {
        $existing = Get-ChildItem -LiteralPath $Destination -Filter $name -File -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.DirectoryName -ne $Destination } |
            Select-Object -First 1
        if ($null -ne $existing -and -not (Test-Path -LiteralPath (Join-Path $Destination $name))) {
            Copy-Item -LiteralPath $existing.FullName -Destination (Join-Path $Destination $name) -Force
        }
    }
}

function Write-LicenseInventory {
    param(
        [Parameter(Mandatory = $true)][string]$VenvPython,
        [Parameter(Mandatory = $true)][string]$Destination
    )
    $licenseDirectory = Join-Path $Destination "licenses"
    New-Item -ItemType Directory -Force -Path $licenseDirectory | Out-Null
    $inventoryScript = @'
import importlib.metadata as metadata
import json
from pathlib import Path
import shutil
import sys

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
packages = []
for dist in sorted(metadata.distributions(), key=lambda item: (item.metadata.get("Name") or "").lower()):
    name = dist.metadata.get("Name") or "unknown"
    version = dist.version
    license_value = dist.metadata.get("License") or ""
    classifiers = [value for value in dist.metadata.get_all("Classifier", []) if value.startswith("License ::")]
    packages.append({"name": name, "version": version, "license": license_value, "classifiers": classifiers})
    root = Path(dist.locate_file(""))
    copied = 0
    for candidate in root.glob(f"{name.replace('-', '_')}*.dist-info/LICENSE*"):
        if candidate.is_file() and copied < 4:
            safe = f"{name}-{version}-{candidate.name}".replace("/", "_").replace("\\", "_")
            shutil.copy2(candidate, out / safe)
            copied += 1
(out / "THIRD-PARTY-PACKAGES.json").write_text(json.dumps(packages, ensure_ascii=False, indent=2), encoding="utf-8")
'@
    $inventoryScript | & $VenvPython - $licenseDirectory
    if ($LASTEXITCODE -ne 0) {
        throw "第三方许可证清单生成失败，退出码 $LASTEXITCODE"
    }

    $basePrefix = (& $VenvPython -c "import sys; print(sys.base_prefix)").Trim()
    foreach ($candidate in @(
        (Join-Path $basePrefix "LICENSE.txt"),
        (Join-Path $basePrefix "LICENSE")
    )) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            Copy-Item -LiteralPath $candidate -Destination (Join-Path $licenseDirectory "Python-License.txt") -Force
            break
        }
    }
}

function Assert-BundleContents {
    param([Parameter(Mandatory = $true)][string]$Directory)
    $exe = Join-Path $Directory "$workerName.exe"
    if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
        throw "Transkun Worker EXE 缺失: $exe"
    }
    if ((Get-Item -LiteralPath $exe).Length -eq 0) {
        throw "Transkun Worker EXE 为空"
    }
    $header = [IO.File]::ReadAllBytes($exe)
    if ($header.Length -lt 2 -or $header[0] -ne 0x4d -or $header[1] -ne 0x5a) {
        throw "Transkun Worker 不是有效 Windows PE 文件"
    }

    $requiredPatterns = @(
        "2.0.pt",
        "2.0.conf",
        "torch_cpu.dll",
        "vcruntime140.dll",
        "vcruntime140_1.dll"
    )
    $allFiles = @(Get-ChildItem -LiteralPath $Directory -File -Recurse)
    foreach ($required in $requiredPatterns) {
        if (-not ($allFiles | Where-Object Name -eq $required)) {
            throw "高质量运行时缺少必要文件: $required"
        }
    }

    $banned = @(
        "cudart*.dll",
        "cudnn*.dll",
        "cublas*.dll",
        "nvrtc*.dll",
        "ffmpeg*.exe",
        "python.exe",
        "pip.exe"
    )
    foreach ($pattern in $banned) {
        $found = $allFiles | Where-Object Name -Like $pattern | Select-Object -First 1
        if ($null -ne $found) {
            throw "高质量运行时包含禁止文件: $($found.FullName)"
        }
    }

    $totalBytes = ($allFiles | Measure-Object Length -Sum).Sum
    if ($totalBytes -gt 3GB) {
        throw "高质量运行时超过 3GB 安全上限: $totalBytes bytes"
    }
}

if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $RepositoryRoot = Join-Path $PSScriptRoot "../.."
}
$RepositoryRoot = Get-FullPath $RepositoryRoot

if ([string]::IsNullOrWhiteSpace($ResourceDirectory)) {
    $ResourceDirectory = Join-Path $RepositoryRoot "apps/scoreleap/src-tauri/resources/scoreleap-transkun"
}
$ResourceDirectory = Get-FullPath $ResourceDirectory
Assert-SafeDirectoryTarget -Path $ResourceDirectory -Label "Transkun 资源目录"

if ([string]::IsNullOrWhiteSpace($WheelhouseDirectory)) {
    $WheelhouseDirectory = Join-Path $RepositoryRoot ".build-tmp/wheelhouse"
}
$WheelhouseDirectory = Get-FullPath $WheelhouseDirectory
New-Item -ItemType Directory -Force -Path $WheelhouseDirectory | Out-Null

$sourcePython = Resolve-Python311 -ExplicitPath $PythonPath
$buildRoot = Join-Path $RepositoryRoot ".build-tmp/transkun-worker"
$venvDirectory = Join-Path $buildRoot "venv"
$venvPython = Join-Path $venvDirectory "Scripts/python.exe"
$distRoot = Join-Path $buildRoot "dist"
$workRoot = Join-Path $buildRoot "pyinstaller-work"
$pipCache = Join-Path $RepositoryRoot ".build-tmp/pip-cache"
$specPath = Join-Path $PSScriptRoot "scoreleap-transkun-worker.spec"
$requirementsPath = Join-Path $PSScriptRoot "requirements-runtime.txt"

New-Item -ItemType Directory -Force -Path $buildRoot, $pipCache | Out-Null
if (-not (Test-Path -LiteralPath $venvPython -PathType Leaf)) {
    Invoke-Checked -Program $sourcePython -Arguments @("-m", "venv", $venvDirectory) -Description "创建 Python $pythonMinor 隔离环境"
}

$env:PIP_CACHE_DIR = $pipCache
$env:PIP_DISABLE_PIP_VERSION_CHECK = "1"
$env:PIP_NO_INPUT = "1"
$env:PYTHONIOENCODING = "utf-8"
$env:PYTHONUTF8 = "1"

if (-not $SkipDependencyInstall) {
    Invoke-Checked -Program $venvPython -Arguments @("-m", "pip", "install", "--upgrade", "pip==25.2") -Description "固定 pip"

    $torchWheel = Get-ChildItem -LiteralPath $WheelhouseDirectory -Filter "torch-$torchVersion+cpu-cp311-cp311-win_amd64.whl" -File -ErrorAction SilentlyContinue | Select-Object -First 1
    $torchaudioWheel = Get-ChildItem -LiteralPath $WheelhouseDirectory -Filter "torchaudio-$torchVersion+cpu-cp311-cp311-win_amd64.whl" -File -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $torchWheel -and $null -ne $torchaudioWheel) {
        Invoke-Checked -Program $venvPython -Arguments @(
            "-m", "pip", "install", "--only-binary=:all:",
            $torchWheel.FullName, $torchaudioWheel.FullName
        ) -Description "从本地 wheelhouse 安装 CPU-only PyTorch"
    }
    else {
        Invoke-Checked -Program $venvPython -Arguments @(
            "-m", "pip", "install", "--only-binary=:all:",
            "torch==$torchVersion+cpu", "torchaudio==$torchVersion+cpu",
            "--index-url", "https://download.pytorch.org/whl/cpu"
        ) -Description "安装 CPU-only PyTorch"
    }

    Invoke-Checked -Program $venvPython -Arguments @(
        "-m", "pip", "install", "--prefer-binary", "-r", $requirementsPath
    ) -Description "安装 Transkun Worker 运行依赖"

    $transkunWheel = Get-ChildItem -LiteralPath $WheelhouseDirectory -Filter "transkun-$transkunVersion-py3-none-any.whl" -File -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $transkunWheel) {
        Invoke-Checked -Program $venvPython -Arguments @(
            "-m", "pip", "install", "--only-binary=:all:", "--no-deps", $transkunWheel.FullName
        ) -Description "从本地 wheelhouse 安装固定 Transkun 模型包"
    }
    else {
        Invoke-Checked -Program $venvPython -Arguments @(
            "-m", "pip", "install", "--only-binary=:all:", "--no-deps", "transkun==$transkunVersion"
        ) -Description "安装固定 Transkun 模型包"
    }
}

$verification = @'
import sys
import torch
import torchaudio
import transkun
assert sys.version_info[:2] == (3, 11), sys.version
assert torch.__version__.startswith("2.6.0"), torch.__version__
assert torchaudio.__version__.startswith("2.6.0"), torchaudio.__version__
assert torch.version.cuda is None, torch.version.cuda
assert not torch.cuda.is_available()
print(f"python={sys.version.split()[0]} torch={torch.__version__} torchaudio={torchaudio.__version__} cpu_only=true")
'@
$verification | & $venvPython -
if ($LASTEXITCODE -ne 0) {
    throw "CPU-only Transkun 依赖验证失败，退出码 $LASTEXITCODE"
}

if (-not $SkipBuild) {
    if (Test-Path -LiteralPath $distRoot) {
        Remove-Item -LiteralPath $distRoot -Recurse -Force
    }
    if (Test-Path -LiteralPath $workRoot) {
        Remove-Item -LiteralPath $workRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $distRoot, $workRoot | Out-Null
    Invoke-Checked -Program $venvPython -Arguments @(
        "-m", "PyInstaller", "--noconfirm", "--clean",
        "--distpath", $distRoot,
        "--workpath", $workRoot,
        $specPath
    ) -Description "构建自包含 Transkun Worker"
}

$builtDirectory = Join-Path $distRoot $workerName
if (-not (Test-Path -LiteralPath $builtDirectory -PathType Container)) {
    throw "PyInstaller onedir 输出不存在: $builtDirectory"
}

$resourceParent = Split-Path -Parent $ResourceDirectory
$resourceLeaf = Split-Path -Leaf $ResourceDirectory
$stagingDirectory = Join-Path $resourceParent "$resourceLeaf.staging-$([Guid]::NewGuid().ToString('N'))"
$backupDirectory = Join-Path $resourceParent "$resourceLeaf.backup-$([Guid]::NewGuid().ToString('N'))"
$destinationMoved = $false
$stagingInstalled = $false

try {
    New-Item -ItemType Directory -Force -Path $resourceParent | Out-Null
    New-Item -ItemType Directory -Path $stagingDirectory | Out-Null
    Copy-Item -Path (Join-Path $builtDirectory "*") -Destination $stagingDirectory -Recurse -Force

    $placeholderReadme = Join-Path $ResourceDirectory "README.md"
    if (Test-Path -LiteralPath $placeholderReadme -PathType Leaf) {
        Copy-Item -LiteralPath $placeholderReadme -Destination (Join-Path $stagingDirectory "README.md") -Force
    }

    Copy-AppLocalCrt -SourcePython $sourcePython -Destination $stagingDirectory -ExplicitRedistDirectory $VcRedistDirectory
    Write-LicenseInventory -VenvPython $venvPython -Destination $stagingDirectory
    Assert-BundleContents -Directory $stagingDirectory

    $selfTestExe = Join-Path $stagingDirectory "$workerName.exe"
    $selfTestOutput = & $selfTestExe self-test
    if ($LASTEXITCODE -ne 0) {
        throw "Transkun Worker self-test 失败，退出码 $LASTEXITCODE；输出: $selfTestOutput"
    }
    $selfTest = $selfTestOutput | Select-Object -Last 1 | ConvertFrom-Json
    if ($selfTest.type -ne "result" -or -not $selfTest.cpu_only -or $selfTest.ffmpeg_required -or $selfTest.python_install_required) {
        throw "Transkun Worker self-test 契约无效: $selfTestOutput"
    }

    $files = @()
    foreach ($file in Get-ChildItem -LiteralPath $stagingDirectory -File -Recurse | Sort-Object FullName) {
        $relative = (Get-CompatibleRelativePath -BasePath $stagingDirectory -TargetPath $file.FullName).Replace('\', '/')
        $files += [ordered]@{
            path = $relative
            size_bytes = $file.Length
            sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }
    $manifest = [ordered]@{
        schema_version = 1
        generated_at_utc = [DateTime]::UtcNow.ToString("o")
        engine = "transkun-v2"
        worker_version = "1.0.0"
        transkun_version = $transkunVersion
        torch_version = "$torchVersion+cpu"
        python_version = $pythonMinor
        architecture = "x64"
        device = "cpu"
        pyinstaller_layout = "onedir"
        external_python_required = $false
        external_pytorch_required = $false
        external_cuda_required = $false
        external_ffmpeg_required = $false
        external_vc_runtime_required = $false
        files = $files
    }
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $stagingDirectory "runtime-manifest.json") -Encoding UTF8

    & (Join-Path $PSScriptRoot "Test-TranskunBundle.ps1") -ResourceDirectory $stagingDirectory
    if ($LASTEXITCODE -ne 0) {
        throw "Transkun 资源审计失败，退出码 $LASTEXITCODE"
    }

    if (Test-Path -LiteralPath $ResourceDirectory) {
        Move-Item -LiteralPath $ResourceDirectory -Destination $backupDirectory
        $destinationMoved = $true
    }
    Move-Item -LiteralPath $stagingDirectory -Destination $ResourceDirectory
    $stagingInstalled = $true

    if ($destinationMoved -and (Test-Path -LiteralPath $backupDirectory)) {
        Remove-Item -LiteralPath $backupDirectory -Recurse -Force
        $destinationMoved = $false
    }
    Write-Host "Transkun 高质量转录资源已准备: $ResourceDirectory"
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
    if (Test-Path -LiteralPath $stagingDirectory) {
        Remove-Item -LiteralPath $stagingDirectory -Recurse -Force
    }
    if ($destinationMoved -and (Test-Path -LiteralPath $backupDirectory)) {
        Write-Warning "Transkun 原资源目录恢复失败，已保留备份: $backupDirectory"
    }
}

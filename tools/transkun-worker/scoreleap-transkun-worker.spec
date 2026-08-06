# -*- mode: python ; coding: utf-8 -*-
import sys
from pathlib import Path

from PyInstaller.utils.hooks import collect_data_files

# PyInstaller 6.21 执行 spec 时不注入 __file__，用 sys.argv[0]（spec 路径）
root = Path(sys.argv[0]).resolve().parent
worker = root / "scoreleap_transkun_worker.py"
transkun_data = collect_data_files("transkun", includes=["pretrained/*"])

# Transkun 用 TorchScript（torch.jit.script）编译模型，运行期必须能读取原始 .py 源码。
# PyInstaller 会把模块编译进 PYZ，inspect.getsource 失效；这里把整个 transkun 包
# 以源码形式复制进 sidecar，worker 启动时优先 sys.path 加载。
import transkun  # noqa: E402
_transkun_src = Path(transkun.__file__).resolve().parent
transkun_data += [(str(_transkun_src), "transkun_src")]

analysis = Analysis(
    [str(worker)],
    pathex=[str(root)],
    binaries=[],
    datas=transkun_data,
    hiddenimports=[
        "moduleconf",
        "transkun.ModelTransformer",
        "transkun.Data",
        "transkun.Util",
        "transkun.Evaluation",
        "transkun.LayersTransformer",
        "transkun.CRF",
        "torchaudio.functional",
        "pretty_midi",
        "miniaudio",
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        "audiomentations",
        "matplotlib",
        "ncls",
        "pandas",
        "pydub",
        "sklearn",
        "sox",
        "soxr",
        "tensorboard",
        "torchvision",
    ],
    noarchive=False,
    optimize=1,
)
pyz = PYZ(analysis.pure)

exe = EXE(
    pyz,
    analysis.scripts,
    [],
    exclude_binaries=True,
    name="scoreleap-transkun-worker",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)

collection = COLLECT(
    exe,
    analysis.binaries,
    analysis.datas,
    strip=False,
    upx=False,
    upx_exclude=[],
    name="scoreleap-transkun-worker",
)

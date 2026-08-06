# -*- mode: python ; coding: utf-8 -*-
from pathlib import Path

from PyInstaller.utils.hooks import collect_data_files

root = Path(__file__).resolve().parent
worker = root / "scoreleap_transkun_worker.py"
transkun_data = collect_data_files("transkun", includes=["pretrained/*"])

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

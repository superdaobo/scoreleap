# -*- mode: python ; coding: utf-8 -*-
# scoreleap-transcriber PyInstaller spec（onedir，依据 Spike 决策 ADR-0007）
# 数据文件（saved_models 等）必须显式收集：collect_data_files
import os
from PyInstaller.utils.hooks import collect_data_files, collect_submodules

block_cipher = None

datas = collect_data_files("basic_pitch")
hiddenimports = collect_submodules("basic_pitch")

a = Analysis(
    [os.path.abspath("packaging/entry.py")],
    pathex=[os.path.abspath(".")],
    binaries=[],
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=["tkinter", "matplotlib.tests", "PIL"],
    noarchive=False,
)
pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="scoreleap-transcriber",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=True,
    disable_windowed_traceback=False,
)
coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name="scoreleap-transcriber",
)

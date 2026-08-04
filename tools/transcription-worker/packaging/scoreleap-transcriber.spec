# -*- mode: python ; coding: utf-8 -*-
# scoreleap-transcriber PyInstaller spec（onedir，基于 Spike 决策 ADR-0007）
import os

block_cipher = None

a = Analysis(
    ["..\\..\\..\\tools\\transcription-worker\\scoreleap_transcriber\\__main__.py"],
    pathex=[os.path.dirname(os.path.abspath("__file__"))],
    binaries=[],
    datas=[],
    hiddenimports=[
        "basic_pitch",
        "basic_pitch.inference",
        "basic_pitch.predict",
        "basic_pitch.icassp2022",
        "mido",
        "pretty_midi",
        "librosa",
        "soundfile",
        "audioread",
    ],
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

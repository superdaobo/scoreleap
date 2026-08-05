from __future__ import annotations

import hashlib
import json
import math
import re
from pathlib import Path

SHA256_RE = re.compile(r"^[0-9a-fA-F]{64}$")
MAX_ASSET_BYTES = 200 * 1024 * 1024
HASH_CHUNK_BYTES = 1024 * 1024


def _require_mapping(value: object, location: str) -> dict:
    if not isinstance(value, dict):
        raise ValueError(f"{location} 必须是对象")
    return value


def _validate_asset(value: object, location: str) -> None:
    asset = _require_mapping(value, location)
    if not isinstance(asset.get("path"), str) or not asset["path"].strip():
        raise ValueError(f"{location}.path 必须是非空字符串")
    if not isinstance(asset.get("sha256"), str) or not SHA256_RE.fullmatch(asset["sha256"]):
        raise ValueError(f"{location}.sha256 必须是 64 位十六进制摘要")


def resolve_asset_path(manifest_path: str | Path, asset_path: str) -> Path:
    """将资产限制在清单目录内，解析路径时同时跟随已有符号链接。"""
    root = Path(manifest_path).resolve(strict=True).parent
    relative = Path(asset_path)
    if relative.is_absolute():
        raise ValueError(f"资产路径必须相对于数据集根目录: {asset_path}")
    resolved = (root / relative).resolve(strict=False)
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise ValueError(f"资产路径越过数据集根目录: {asset_path}") from error
    return resolved


def _stream_sha256(path: Path) -> str:
    size = path.stat().st_size
    if size > MAX_ASSET_BYTES:
        raise ValueError(f"资产超过 {MAX_ASSET_BYTES} 字节限制: {path}")
    digest = hashlib.sha256()
    total = 0
    with path.open("rb") as stream:
        while chunk := stream.read(HASH_CHUNK_BYTES):
            total += len(chunk)
            if total > MAX_ASSET_BYTES:
                raise ValueError(f"资产超过 {MAX_ASSET_BYTES} 字节限制: {path}")
            digest.update(chunk)
    return digest.hexdigest()


def load_and_validate_manifest(path: str | Path, verify_files: bool = False) -> dict:
    manifest_path = Path(path).resolve(strict=True)
    data = _require_mapping(json.loads(manifest_path.read_text(encoding="utf-8")), "manifest")
    if data.get("schema_version") != 1:
        raise ValueError("schema_version 必须为 1")
    samples = data.get("samples")
    if not isinstance(samples, list) or not samples:
        raise ValueError("samples 必须是非空数组")

    identifiers: set[str] = set()
    for index, raw_sample in enumerate(samples):
        location = f"samples[{index}]"
        sample = _require_mapping(raw_sample, location)
        for field in ("id", "source", "split"):
            if not isinstance(sample.get(field), str) or not sample[field].strip():
                raise ValueError(f"{location}.{field} 必须是非空字符串")
        if sample["id"] in identifiers:
            raise ValueError(f"样本 id 重复: {sample['id']}")
        identifiers.add(sample["id"])
        _validate_asset(sample.get("audio"), f"{location}.audio")
        _validate_asset(sample.get("reference_midi"), f"{location}.reference_midi")
        resolved_assets = {
            asset_name: resolve_asset_path(manifest_path, sample[asset_name]["path"])
            for asset_name in ("audio", "reference_midi")
        }

        segment = _require_mapping(sample.get("segment"), f"{location}.segment")
        start, end = segment.get("start_seconds"), segment.get("end_seconds")
        if not isinstance(start, (int, float)) or isinstance(start, bool) or not math.isfinite(start) or start < 0:
            raise ValueError(f"{location}.segment.start_seconds 必须是有限非负数")
        if not isinstance(end, (int, float)) or isinstance(end, bool) or not math.isfinite(end) or end <= start:
            raise ValueError(f"{location}.segment.end_seconds 必须大于 start_seconds")

        if "noise" not in sample:
            raise ValueError(f"{location}.noise 必须显式为 null 或包含 seed/snr_db")
        noise = sample["noise"]
        if noise is not None:
            noise = _require_mapping(noise, f"{location}.noise")
            seed, snr = noise.get("seed"), noise.get("snr_db")
            if not isinstance(seed, int) or isinstance(seed, bool) or seed < 0:
                raise ValueError(f"{location}.noise.seed 必须是非负整数")
            if not isinstance(snr, (int, float)) or isinstance(snr, bool) or not math.isfinite(snr):
                raise ValueError(f"{location}.noise.snr_db 必须是有限数值")

        if verify_files:
            for asset_name in ("audio", "reference_midi"):
                asset = sample[asset_name]
                asset_path = resolved_assets[asset_name]
                if not asset_path.is_file():
                    raise ValueError(f"文件不存在: {asset_path}")
                digest = _stream_sha256(asset_path)
                if digest.lower() != asset["sha256"].lower():
                    raise ValueError(f"SHA256 不匹配: {asset_path}")
    return data

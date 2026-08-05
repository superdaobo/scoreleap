from __future__ import annotations

import hashlib
import io
import json
import os
import re
import urllib.request
import zipfile
from collections import OrderedDict
from pathlib import Path
from typing import BinaryIO, Callable, Final


MAESTRO_VERSION: Final = "3.0.0"
MAESTRO_ARCHIVE_URL: Final = (
    "https://storage.googleapis.com/magentadata/datasets/maestro/"
    "v3.0.0/maestro-v3.0.0.zip"
)
MAESTRO_METADATA_URL: Final = (
    "https://storage.googleapis.com/magentadata/datasets/maestro/"
    "v3.0.0/maestro-v3.0.0.json"
)
MAESTRO_ARCHIVE_SHA256: Final = (
    "6680fea5be2339ea15091a249fbd70e49551246ddbd5ca50f1b2352c08c95291"
)
MAX_ASSET_BYTES: Final = 200 * 1024 * 1024
DEFAULT_RANGE_BLOCK_BYTES: Final = 4 * 1024 * 1024
USER_AGENT: Final = "ScoreLeap-transcription-eval/0.1"

OpenUrl = Callable[..., BinaryIO]


class HttpRangeReader(io.RawIOBase):
    """为远程 ZIP 提供有界、可缓存的只读 seek 接口。"""

    def __init__(
        self,
        url: str,
        *,
        opener: OpenUrl = urllib.request.urlopen,
        block_bytes: int = DEFAULT_RANGE_BLOCK_BYTES,
        cache_blocks: int = 4,
        timeout_seconds: float = 30.0,
    ) -> None:
        if block_bytes <= 0 or cache_blocks <= 0:
            raise ValueError("range block 与缓存块数必须为正数")
        self._url = url
        self._opener = opener
        self._block_bytes = block_bytes
        self._cache_blocks = cache_blocks
        self._timeout_seconds = timeout_seconds
        self._position = 0
        self._cache: OrderedDict[int, bytes] = OrderedDict()

        request = urllib.request.Request(url, method="HEAD", headers={"User-Agent": USER_AGENT})
        with opener(request, timeout=timeout_seconds) as response:
            length = response.headers.get("Content-Length")
            accept_ranges = response.headers.get("Accept-Ranges", "")
        if length is None or not length.isdigit() or int(length) <= 0:
            raise ValueError("远程 MAESTRO ZIP 缺少有效 Content-Length")
        if accept_ranges.lower() != "bytes":
            raise ValueError("远程 MAESTRO ZIP 不支持字节范围请求")
        self._size = int(length)

    def readable(self) -> bool:
        return True

    def seekable(self) -> bool:
        return True

    def tell(self) -> int:
        return self._position

    def seek(self, offset: int, whence: int = os.SEEK_SET) -> int:
        if whence == os.SEEK_SET:
            position = offset
        elif whence == os.SEEK_CUR:
            position = self._position + offset
        elif whence == os.SEEK_END:
            position = self._size + offset
        else:
            raise ValueError(f"不支持的 seek whence: {whence}")
        if position < 0:
            raise ValueError("不能 seek 到负偏移")
        self._position = min(position, self._size)
        return self._position

    def read(self, size: int = -1) -> bytes:
        if self._position >= self._size:
            return b""
        remaining = self._size - self._position
        requested = remaining if size is None or size < 0 else min(size, remaining)
        chunks: list[bytes] = []
        while requested > 0:
            block_index = self._position // self._block_bytes
            block = self._read_block(block_index)
            block_offset = self._position % self._block_bytes
            take = min(requested, len(block) - block_offset)
            if take <= 0:
                raise OSError("远程 ZIP 范围响应提前结束")
            chunks.append(block[block_offset : block_offset + take])
            self._position += take
            requested -= take
        return b"".join(chunks)

    def _read_block(self, block_index: int) -> bytes:
        cached = self._cache.pop(block_index, None)
        if cached is not None:
            self._cache[block_index] = cached
            return cached
        start = block_index * self._block_bytes
        end = min(self._size, start + self._block_bytes) - 1
        request = urllib.request.Request(
            self._url,
            headers={"Range": f"bytes={start}-{end}", "User-Agent": USER_AGENT},
        )
        with self._opener(request, timeout=self._timeout_seconds) as response:
            status = getattr(response, "status", response.getcode())
            content_range = response.headers.get("Content-Range", "")
            payload = response.read()
        expected_range = f"bytes {start}-{end}/{self._size}"
        if status != 206 or content_range != expected_range:
            raise OSError(
                f"远程 ZIP 范围响应无效: status={status}, Content-Range={content_range!r}"
            )
        expected_size = end - start + 1
        if len(payload) != expected_size:
            raise OSError(f"远程 ZIP 范围长度错误: {len(payload)} != {expected_size}")
        self._cache[block_index] = payload
        while len(self._cache) > self._cache_blocks:
            self._cache.popitem(last=False)
        return payload


def fetch_maestro_sample(
    sample_index: int,
    output_root: str | Path,
    *,
    archive_url: str = MAESTRO_ARCHIVE_URL,
    metadata_url: str = MAESTRO_METADATA_URL,
    opener: OpenUrl = urllib.request.urlopen,
    range_block_bytes: int = DEFAULT_RANGE_BLOCK_BYTES,
) -> dict:
    if sample_index < 0:
        raise ValueError("sample_index 必须是非负整数")
    request = urllib.request.Request(metadata_url, headers={"User-Agent": USER_AGENT})
    with opener(request, timeout=30.0) as response:
        metadata = json.load(response)
    key = str(sample_index)
    required_columns = ("split", "duration", "audio_filename", "midi_filename")
    if any(key not in metadata.get(column, {}) for column in required_columns):
        raise ValueError(f"MAESTRO v{MAESTRO_VERSION} 不存在样本索引 {sample_index}")
    if metadata["split"][key] not in {"train", "validation", "test"}:
        raise ValueError("MAESTRO metadata split 无效")
    duration = metadata["duration"][key]
    if not isinstance(duration, (int, float)) or duration <= 0:
        raise ValueError("MAESTRO metadata duration 无效")

    sample_id = f"maestro-v3-{metadata['split'][key]}-{sample_index:04d}"
    sample_root = Path(output_root).resolve() / sample_id
    sample_root.mkdir(parents=True, exist_ok=True)
    audio_output = sample_root / "audio.wav"
    midi_output = sample_root / "reference.mid"
    manifest_output = sample_root / "manifest.json"
    for path in (audio_output, midi_output, manifest_output):
        if path.exists():
            raise FileExistsError(f"为避免覆盖，目标必须不存在: {path}")

    remote = HttpRangeReader(
        archive_url,
        opener=opener,
        block_bytes=range_block_bytes,
    )
    try:
        with zipfile.ZipFile(remote) as archive:
            audio_digest = _extract_asset(
                archive, metadata["audio_filename"][key], audio_output
            )
            midi_digest = _extract_asset(
                archive, metadata["midi_filename"][key], midi_output
            )
    except zipfile.BadZipFile as error:
        raise ValueError(f"远程 MAESTRO ZIP 无效: {error}") from error
    finally:
        remote.close()

    manifest = {
        "schema_version": 1,
        "samples": [{
            "id": sample_id,
            "source": f"MAESTRO v{MAESTRO_VERSION}",
            "split": metadata["split"][key],
            "audio": {"path": audio_output.name, "sha256": audio_digest},
            "reference_midi": {"path": midi_output.name, "sha256": midi_digest},
            "segment": {"start_seconds": 0.0, "end_seconds": duration},
            "noise": None,
        }],
        "provenance": {
            "sample_index": sample_index,
            "archive_url": archive_url,
            "archive_sha256": MAESTRO_ARCHIVE_SHA256,
            "metadata_url": metadata_url,
        },
    }
    manifest_output.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return {
        "sample_id": sample_id,
        "sample_root": str(sample_root),
        "audio": str(audio_output),
        "reference_midi": str(midi_output),
        "manifest": str(manifest_output),
        "duration_seconds": duration,
    }


def _extract_asset(archive: zipfile.ZipFile, relative_name: str, output: Path) -> str:
    normalized = relative_name.replace("\\", "/")
    if normalized.startswith("/") or ".." in normalized.split("/"):
        raise ValueError(f"MAESTRO metadata 路径不安全: {relative_name}")
    suffix_matches = [
        info for info in archive.infolist()
        if not info.is_dir() and info.filename.replace("\\", "/").endswith(f"/{normalized}")
    ]
    if len(suffix_matches) != 1:
        raise ValueError(f"MAESTRO ZIP 条目必须唯一: {relative_name}")
    info = suffix_matches[0]
    if info.file_size <= 0 or info.file_size > MAX_ASSET_BYTES:
        raise ValueError(f"MAESTRO 资产大小超限: {info.file_size}")
    temporary = output.with_suffix(output.suffix + ".part")
    if temporary.exists():
        raise FileExistsError(f"临时目标已存在: {temporary}")
    digest = hashlib.sha256()
    try:
        with archive.open(info) as source, temporary.open("xb") as target:
            total = 0
            while chunk := source.read(1024 * 1024):
                total += len(chunk)
                if total > MAX_ASSET_BYTES:
                    raise ValueError("MAESTRO 资产解压后超过上限")
                digest.update(chunk)
                target.write(chunk)
            target.flush()
            os.fsync(target.fileno())
        if total != info.file_size:
            raise ValueError(f"MAESTRO 资产长度错误: {total} != {info.file_size}")
        os.replace(temporary, output)
    except Exception:
        temporary.unlink(missing_ok=True)
        raise
    return digest.hexdigest()

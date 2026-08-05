from __future__ import annotations

import io
import json
import re
import tempfile
import unittest
import urllib.request
import zipfile
from pathlib import Path

from scoreleap_transcription_eval.maestro import fetch_maestro_sample
from scoreleap_transcription_eval.manifest import load_and_validate_manifest


class FakeResponse(io.BytesIO):
    def __init__(self, payload: bytes, *, status: int, headers: dict[str, str]) -> None:
        super().__init__(payload)
        self.status = status
        self.headers = headers

    def getcode(self) -> int:
        return self.status

    def __enter__(self) -> "FakeResponse":
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()


class FakeMaestroServer:
    archive_url = "https://example.invalid/maestro.zip"
    metadata_url = "https://example.invalid/maestro.json"

    def __init__(self) -> None:
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            archive.writestr("maestro-v3.0.0/2008/sample.wav", b"RIFF-generated-audio")
            archive.writestr("maestro-v3.0.0/2008/sample.midi", b"MThd-generated-midi")
        self.archive = buffer.getvalue()
        self.metadata = json.dumps({
            "split": {"92": "test"},
            "duration": {"92": 65.95},
            "audio_filename": {"92": "2008/sample.wav"},
            "midi_filename": {"92": "2008/sample.midi"},
        }).encode()
        self.archive_ranges: list[tuple[int, int]] = []

    def open(self, request: urllib.request.Request, **_kwargs: object) -> FakeResponse:
        if request.full_url == self.metadata_url:
            return FakeResponse(
                self.metadata,
                status=200,
                headers={"Content-Length": str(len(self.metadata))},
            )
        if request.full_url != self.archive_url:
            raise AssertionError(f"unexpected URL: {request.full_url}")
        if request.get_method() == "HEAD":
            return FakeResponse(
                b"",
                status=200,
                headers={
                    "Content-Length": str(len(self.archive)),
                    "Accept-Ranges": "bytes",
                },
            )
        range_header = request.get_header("Range")
        if range_header is None:
            raise AssertionError("archive GET 必须携带 Range")
        match = re.fullmatch(r"bytes=(\d+)-(\d+)", range_header)
        if match is None:
            raise AssertionError(f"invalid range: {range_header}")
        start, end = map(int, match.groups())
        self.archive_ranges.append((start, end))
        return FakeResponse(
            self.archive[start : end + 1],
            status=206,
            headers={"Content-Range": f"bytes {start}-{end}/{len(self.archive)}"},
        )


class MaestroFetchTests(unittest.TestCase):
    def test_fetches_only_ranges_and_builds_verified_manifest(self) -> None:
        server = FakeMaestroServer()
        with tempfile.TemporaryDirectory() as directory:
            result = fetch_maestro_sample(
                92,
                directory,
                archive_url=server.archive_url,
                metadata_url=server.metadata_url,
                opener=server.open,
                range_block_bytes=32,
            )
            self.assertEqual(Path(result["audio"]).read_bytes(), b"RIFF-generated-audio")
            self.assertEqual(Path(result["reference_midi"]).read_bytes(), b"MThd-generated-midi")
            manifest = load_and_validate_manifest(result["manifest"], verify_files=True)
            self.assertEqual(manifest["samples"][0]["id"], "maestro-v3-test-0092")
            self.assertTrue(server.archive_ranges)
            self.assertLessEqual(max(end - start + 1 for start, end in server.archive_ranges), 32)

            with self.assertRaisesRegex(FileExistsError, "避免覆盖"):
                fetch_maestro_sample(
                    92,
                    directory,
                    archive_url=server.archive_url,
                    metadata_url=server.metadata_url,
                    opener=server.open,
                    range_block_bytes=32,
                )

    def test_rejects_unknown_sample_index_before_opening_archive(self) -> None:
        server = FakeMaestroServer()
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "不存在样本索引"):
                fetch_maestro_sample(
                    9999,
                    directory,
                    archive_url=server.archive_url,
                    metadata_url=server.metadata_url,
                    opener=server.open,
                )
        self.assertFalse(server.archive_ranges)


if __name__ == "__main__":
    unittest.main()

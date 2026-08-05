from __future__ import annotations

import contextlib
import hashlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import mido

from scoreleap_transcription_eval.cli import main
from scoreleap_transcription_eval.gates import apply_quality_gates
from scoreleap_transcription_eval.manifest import load_and_validate_manifest
from scoreleap_transcription_eval.metrics import Note, evaluate_notes


def write_midi(path: Path, notes: list[tuple[int, float, float]]) -> None:
    """测试夹具完全由程序生成，不携带任何第三方音频或 MIDI。"""
    ticks_per_beat = 480
    tempo = 500_000
    events: list[tuple[int, int, mido.Message]] = []
    for pitch, onset, offset in notes:
        start_tick = round(mido.second2tick(onset, ticks_per_beat, tempo))
        end_tick = round(mido.second2tick(offset, ticks_per_beat, tempo))
        events.append((start_tick, 1, mido.Message("note_on", note=pitch, velocity=80)))
        events.append((end_tick, 0, mido.Message("note_off", note=pitch, velocity=0)))
    events.sort(key=lambda event: (event[0], event[1]))
    track = mido.MidiTrack([mido.MetaMessage("set_tempo", tempo=tempo, time=0)])
    previous_tick = 0
    for tick, _, message in events:
        message.time = tick - previous_tick
        previous_tick = tick
        track.append(message)
    midi = mido.MidiFile(ticks_per_beat=ticks_per_beat)
    midi.tracks.append(track)
    midi.save(path)


class MetricsTests(unittest.TestCase):
    def test_evaluate_uses_distinct_onset_and_offset_matches(self) -> None:
        refs = [Note(60, 0.0, 1.0), Note(62, 2.0, 3.0), Note(64, 4.0, 5.0)]
        preds = [Note(60, 0.03, 1.04), Note(62, 2.04, 3.50), Note(65, 4.0, 5.0)]
        result = evaluate_notes(refs, preds, 5.0)
        self.assertEqual(result.onset_match_count, 2)
        self.assertEqual(result.onset_offset_match_count, 1)
        self.assertAlmostEqual(result.onset_f1, 2 / 3)
        self.assertAlmostEqual(result.onset_offset_f1, 1 / 3)
        self.assertAlmostEqual(result.false_notes_per_minute, 12.0)
        self.assertAlmostEqual(result.median_onset_error_ms, 35.0)
        self.assertAlmostEqual(result.drift_ms_per_minute, 300.0)

    def test_one_reference_cannot_match_duplicate_predictions(self) -> None:
        result = evaluate_notes([Note(60, 1.0, 2.0)], [Note(60, 0.98, 2.0), Note(60, 1.02, 2.0)], 2.0)
        self.assertEqual(result.onset_match_count, 1)
        self.assertEqual(result.prediction_note_count - result.onset_match_count, 1)

    def test_offset_matching_supports_crossing_pairs(self) -> None:
        refs = [Note(60, 0.00, 1.00), Note(60, 0.04, 2.00)]
        preds = [Note(60, 0.01, 2.00), Note(60, 0.03, 1.00)]
        result = evaluate_notes(refs, preds, 2.0)
        self.assertEqual(result.onset_offset_match_count, 2)
        self.assertEqual(result.onset_offset_f1, 1.0)

    def test_invalid_explicit_duration_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "有限正数"):
            evaluate_notes([], [], 0.0)

    def test_release_gate_accepts_metrics_at_the_boundary(self) -> None:
        result = apply_quality_gates({
            "precision": 0.93,
            "recall": 0.87,
            "onset_f1": 0.90,
            "onset_offset_f1": 0.75,
            "false_notes_per_minute": 3.0,
        })
        self.assertTrue(result["passed"])
        self.assertTrue(all(result["checks"].values()))

    def test_release_gate_rejects_missing_or_regressed_metrics(self) -> None:
        result = apply_quality_gates({
            "precision": 0.929,
            "recall": 0.90,
            "onset_f1": 0.95,
            "onset_offset_f1": 0.80,
            "false_notes_per_minute": None,
        })
        self.assertFalse(result["passed"])
        self.assertFalse(result["checks"]["precision"])
        self.assertFalse(result["checks"]["false_notes_per_minute"])


class CliAndManifestTests(unittest.TestCase):
    def test_compare_formats_reports_identical_f1(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            first = Path(directory) / "first.mid"
            second = Path(directory) / "second.mid"
            notes = [(60, 0.0, 1.0), (64, 1.0, 2.0)]
            write_midi(first, notes)
            write_midi(second, notes)
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                exit_code = main(["compare-formats", str(first), str(second)])
            self.assertEqual(exit_code, 0)
            result = json.loads(stdout.getvalue())
            self.assertEqual(result["note_consistency_f1"], 1.0)
            self.assertIsNone(result["false_notes_per_minute"])

    def test_evaluate_uses_manifest_duration_for_empty_reference(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            audio = root / "sample.wav"
            reference = root / "empty.mid"
            prediction = root / "prediction.mid"
            audio.write_bytes(b"generated")
            write_midi(reference, [])
            write_midi(prediction, [(60, 0.0, 1.0)])
            sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({
                "schema_version": 1,
                "samples": [{
                    "id": "empty", "source": "generated", "split": "test",
                    "audio": {"path": audio.name, "sha256": sha(audio)},
                    "reference_midi": {"path": reference.name, "sha256": sha(reference)},
                    "segment": {"start_seconds": 5, "end_seconds": 15},
                    "noise": None,
                }],
            }), encoding="utf-8")
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                exit_code = main([
                    "evaluate", str(reference), str(prediction),
                    "--manifest", str(manifest), "--sample-id", "empty",
                ])
            self.assertEqual(exit_code, 0)
            self.assertEqual(json.loads(stdout.getvalue())["false_notes_per_minute"], 6.0)

    def test_gate_returns_three_when_quality_regresses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            audio = root / "sample.wav"
            reference = root / "reference.mid"
            prediction = root / "prediction.mid"
            audio.write_bytes(b"generated")
            write_midi(reference, [(60, 0.0, 1.0), (64, 1.0, 2.0)])
            write_midi(prediction, [(60, 0.0, 1.0)])
            sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({
                "schema_version": 1,
                "samples": [{
                    "id": "regression", "source": "generated", "split": "test",
                    "audio": {"path": audio.name, "sha256": sha(audio)},
                    "reference_midi": {"path": reference.name, "sha256": sha(reference)},
                    "segment": {"start_seconds": 0, "end_seconds": 2},
                    "noise": None,
                }],
            }), encoding="utf-8")
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                exit_code = main([
                    "gate", str(reference), str(prediction),
                    "--manifest", str(manifest), "--sample-id", "regression",
                ])
            self.assertEqual(exit_code, 3)
            self.assertFalse(json.loads(stdout.getvalue())["quality_gate"]["passed"])

    def test_manifest_validates_schema_and_hashes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            audio = root / "sample.wav"
            midi = root / "reference.mid"
            audio.write_bytes(b"generated audio placeholder")
            write_midi(midi, [(60, 0.0, 1.0)])
            sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
            manifest = {
                "schema_version": 1,
                "samples": [{
                    "id": "generated-1", "source": "generated", "split": "test",
                    "audio": {"path": audio.name, "sha256": sha(audio)},
                    "reference_midi": {"path": midi.name, "sha256": sha(midi)},
                    "segment": {"start_seconds": 0, "end_seconds": 1},
                    "noise": {"seed": 42, "snr_db": 20},
                }],
            }
            manifest_path = root / "manifest.json"
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with mock.patch.object(Path, "read_bytes", side_effect=AssertionError("SHA 必须流式读取")):
                loaded = load_and_validate_manifest(manifest_path, verify_files=True)
            self.assertEqual(loaded["samples"][0]["id"], "generated-1")

    def test_manifest_rejects_asset_path_outside_dataset_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "dataset"
            root.mkdir()
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({
                "schema_version": 1,
                "samples": [{
                    "id": "escape", "source": "generated", "split": "test",
                    "audio": {"path": "../outside.wav", "sha256": "0" * 64},
                    "reference_midi": {"path": "reference.mid", "sha256": "0" * 64},
                    "segment": {"start_seconds": 0, "end_seconds": 1}, "noise": None,
                }],
            }), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "越过数据集根目录"):
                load_and_validate_manifest(manifest)

    def test_manifest_rejects_symlink_that_escapes_dataset_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / "dataset"
            root.mkdir()
            outside = base / "outside.wav"
            outside.write_bytes(b"outside")
            link = root / "linked.wav"
            try:
                link.symlink_to(outside)
            except OSError as error:
                self.skipTest(f"当前系统不允许创建符号链接: {error}")
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({
                "schema_version": 1,
                "samples": [{
                    "id": "link", "source": "generated", "split": "test",
                    "audio": {"path": link.name, "sha256": "0" * 64},
                    "reference_midi": {"path": "reference.mid", "sha256": "0" * 64},
                    "segment": {"start_seconds": 0, "end_seconds": 1}, "noise": None,
                }],
            }), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "越过数据集根目录"):
                load_and_validate_manifest(manifest)

    def test_manifest_limits_asset_size_before_hashing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            audio = root / "large.wav"
            reference = root / "reference.mid"
            audio.write_bytes(b"123456789")
            write_midi(reference, [(60, 0.0, 1.0)])
            sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({
                "schema_version": 1,
                "samples": [{
                    "id": "large", "source": "generated", "split": "test",
                    "audio": {"path": audio.name, "sha256": sha(audio)},
                    "reference_midi": {"path": reference.name, "sha256": sha(reference)},
                    "segment": {"start_seconds": 0, "end_seconds": 1}, "noise": None,
                }],
            }), encoding="utf-8")
            with mock.patch("scoreleap_transcription_eval.manifest.MAX_ASSET_BYTES", 8):
                with self.assertRaisesRegex(ValueError, "资产超过"):
                    load_and_validate_manifest(manifest, verify_files=True)

    def test_manifest_requires_noise_seed_and_snr_as_pair(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            path.write_text(json.dumps({
                "schema_version": 1,
                "samples": [{
                    "id": "bad", "source": "generated", "split": "test",
                    "audio": {"path": "x", "sha256": "0" * 64},
                    "reference_midi": {"path": "y", "sha256": "0" * 64},
                    "segment": {"start_seconds": 0, "end_seconds": 1},
                    "noise": {"seed": 1},
                }],
            }), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "snr_db"):
                load_and_validate_manifest(path)

    def test_manifest_requires_explicit_clean_noise_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            path.write_text(json.dumps({
                "schema_version": 1,
                "samples": [{
                    "id": "bad", "source": "generated", "split": "test",
                    "audio": {"path": "x", "sha256": "0" * 64},
                    "reference_midi": {"path": "y", "sha256": "0" * 64},
                    "segment": {"start_seconds": 0, "end_seconds": 1},
                }],
            }), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "noise"):
                load_and_validate_manifest(path)

    def test_manifest_rejects_missing_segment_duration(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            path.write_text(json.dumps({
                "schema_version": 1,
                "samples": [{
                    "id": "bad", "source": "generated", "split": "test",
                    "audio": {"path": "x", "sha256": "0" * 64},
                    "reference_midi": {"path": "y", "sha256": "0" * 64},
                    "segment": {"start_seconds": 0}, "noise": None,
                }],
            }), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "end_seconds"):
                load_and_validate_manifest(path)

    def test_manifest_allows_pending_reference_midi(self) -> None:
        sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            audio = root / "audio.wav"
            audio.write_bytes(b"RIFF-pending-audio")
            path = root / "manifest.json"
            path.write_text(json.dumps({
                "schema_version": 1,
                "samples": [{
                    "id": "pending-ref", "source": "generated", "split": "hidden",
                    "audio": {"path": audio.name, "sha256": sha(audio)},
                    "reference_midi": None,
                    "segment": {"start_seconds": 0, "end_seconds": 60},
                    "noise": None,
                }],
            }), encoding="utf-8")
            loaded = load_and_validate_manifest(path, verify_files=True)
            self.assertIsNone(loaded["samples"][0]["reference_midi"])


if __name__ == "__main__":
    unittest.main()

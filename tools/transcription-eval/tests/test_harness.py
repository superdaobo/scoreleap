from __future__ import annotations

import contextlib
import hashlib
import io
import json
import tempfile
import unittest
from pathlib import Path

import mido

from scoreleap_transcription_eval.cli import main
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
            self.assertEqual(json.loads(stdout.getvalue())["note_consistency_f1"], 1.0)

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
            loaded = load_and_validate_manifest(manifest_path, verify_files=True)
            self.assertEqual(loaded["samples"][0]["id"], "generated-1")

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


if __name__ == "__main__":
    unittest.main()

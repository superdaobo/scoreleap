from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .manifest import load_and_validate_manifest, resolve_asset_path
from .metrics import ONSET_TOLERANCE_SECONDS, OFFSET_DURATION_RATIO, OFFSET_MIN_TOLERANCE_SECONDS, evaluate_notes
from .midi import read_midi_notes


def _metrics(reference: Path, prediction: Path, duration_seconds: float | None) -> dict:
    ref = read_midi_notes(reference)
    pred = read_midi_notes(prediction)
    result = evaluate_notes(ref.notes, pred.notes, duration_seconds).to_dict()
    result["thresholds"] = {
        "onset_tolerance_ms": ONSET_TOLERANCE_SECONDS * 1000,
        "offset_min_tolerance_ms": OFFSET_MIN_TOLERANCE_SECONDS * 1000,
        "offset_reference_duration_ratio": OFFSET_DURATION_RATIO,
    }
    return result


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="scoreleap-transcription-eval")
    subparsers = parser.add_subparsers(dest="command", required=True)
    evaluate = subparsers.add_parser("evaluate", help="比较参考 MIDI 与预测 MIDI")
    evaluate.add_argument("reference", type=Path)
    evaluate.add_argument("prediction", type=Path)
    evaluate.add_argument("--manifest", type=Path, required=True)
    evaluate.add_argument("--sample-id", required=True)
    compare = subparsers.add_parser("compare-formats", help="比较同源格式转换后的音符一致性")
    compare.add_argument("reference", type=Path)
    compare.add_argument("candidate", type=Path)
    manifest = subparsers.add_parser("validate-manifest", help="校验评测数据清单")
    manifest.add_argument("manifest", type=Path)
    manifest.add_argument("--verify-files", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "validate-manifest":
            data = load_and_validate_manifest(args.manifest, args.verify_files)
            payload = {"valid": True, "sample_count": len(data["samples"]), "files_verified": args.verify_files}
        elif args.command == "evaluate":
            manifest = load_and_validate_manifest(args.manifest)
            sample = next((item for item in manifest["samples"] if item["id"] == args.sample_id), None)
            if sample is None:
                raise ValueError(f"manifest 中不存在样本: {args.sample_id}")
            expected_reference = resolve_asset_path(args.manifest, sample["reference_midi"]["path"])
            if args.reference.resolve(strict=True) != expected_reference:
                raise ValueError("reference MIDI 与 manifest 样本不一致")
            duration = sample["segment"]["end_seconds"] - sample["segment"]["start_seconds"]
            payload = _metrics(args.reference, args.prediction, duration)
        else:
            payload = _metrics(args.reference, args.candidate, None)
            payload["note_consistency_f1"] = payload["onset_f1"]
        print(json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True))
        return 0
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(json.dumps({"error": str(error)}, ensure_ascii=False), file=sys.stderr)
        return 2

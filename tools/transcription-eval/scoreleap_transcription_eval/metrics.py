from __future__ import annotations

from collections import defaultdict
from dataclasses import asdict, dataclass
from typing import Callable, Sequence

import numpy as np

ONSET_TOLERANCE_SECONDS = 0.050
OFFSET_MIN_TOLERANCE_SECONDS = 0.050
OFFSET_DURATION_RATIO = 0.20


@dataclass(frozen=True, slots=True)
class Note:
    pitch: int
    onset_seconds: float
    offset_seconds: float


@dataclass(frozen=True, slots=True)
class Evaluation:
    reference_note_count: int
    prediction_note_count: int
    onset_match_count: int
    onset_offset_match_count: int
    precision: float
    recall: float
    onset_f1: float
    onset_offset_precision: float
    onset_offset_recall: float
    onset_offset_f1: float
    false_notes_per_minute: float | None
    median_onset_error_ms: float | None
    drift_ms_per_minute: float | None
    drift_intercept_ms: float | None

    def to_dict(self) -> dict[str, int | float | None]:
        return asdict(self)


def _ratio(numerator: int, denominator: int, empty_value: float) -> float:
    return numerator / denominator if denominator else empty_value


def _f1(precision: float, recall: float) -> float:
    return 2 * precision * recall / (precision + recall) if precision + recall else 0.0


def _match_pitch_group(
    references: Sequence[Note],
    predictions: Sequence[Note],
    eligible: Callable[[Note, Note], bool],
    cost: Callable[[Note, Note], float],
) -> list[tuple[Note, Note]]:
    """动态规划先最大化一对一匹配数，再最小化时间误差。"""
    n, m = len(references), len(predictions)
    scores = [[(0, 0.0) for _ in range(m + 1)] for _ in range(n + 1)]
    choices = [["" for _ in range(m + 1)] for _ in range(n + 1)]
    for i in range(1, n + 1):
        choices[i][0] = "r"
    for j in range(1, m + 1):
        choices[0][j] = "p"

    for i in range(1, n + 1):
        for j in range(1, m + 1):
            candidates = [(scores[i - 1][j], "r"), (scores[i][j - 1], "p")]
            ref, pred = references[i - 1], predictions[j - 1]
            if eligible(ref, pred):
                count, total_cost = scores[i - 1][j - 1]
                candidates.append(((count + 1, total_cost + cost(ref, pred)), "m"))
            best_score, best_choice = max(candidates, key=lambda item: (item[0][0], -item[0][1], item[1] == "m"))
            scores[i][j], choices[i][j] = best_score, best_choice

    pairs: list[tuple[Note, Note]] = []
    i, j = n, m
    while i and j:
        choice = choices[i][j]
        if choice == "m":
            pairs.append((references[i - 1], predictions[j - 1]))
            i -= 1
            j -= 1
        elif choice == "r":
            i -= 1
        else:
            j -= 1
    pairs.reverse()
    return pairs


def _match(references: Sequence[Note], predictions: Sequence[Note], require_offset: bool) -> list[tuple[Note, Note]]:
    refs_by_pitch: dict[int, list[Note]] = defaultdict(list)
    preds_by_pitch: dict[int, list[Note]] = defaultdict(list)
    for note in references:
        refs_by_pitch[note.pitch].append(note)
    for note in predictions:
        preds_by_pitch[note.pitch].append(note)

    pairs: list[tuple[Note, Note]] = []
    for pitch in refs_by_pitch.keys() & preds_by_pitch.keys():
        refs = sorted(refs_by_pitch[pitch], key=lambda note: note.onset_seconds)
        preds = sorted(preds_by_pitch[pitch], key=lambda note: note.onset_seconds)

        def eligible(ref: Note, pred: Note) -> bool:
            if abs(ref.onset_seconds - pred.onset_seconds) > ONSET_TOLERANCE_SECONDS:
                return False
            offset_tolerance = max(OFFSET_MIN_TOLERANCE_SECONDS, (ref.offset_seconds - ref.onset_seconds) * OFFSET_DURATION_RATIO)
            return not require_offset or abs(ref.offset_seconds - pred.offset_seconds) <= offset_tolerance

        def cost(ref: Note, pred: Note) -> float:
            result = abs(ref.onset_seconds - pred.onset_seconds)
            return result + (abs(ref.offset_seconds - pred.offset_seconds) if require_offset else 0.0)

        pairs.extend(_match_pitch_group(refs, preds, eligible, cost))
    return pairs


def evaluate_notes(
    references: Sequence[Note], predictions: Sequence[Note], reference_duration_seconds: float
) -> Evaluation:
    onset_pairs = _match(references, predictions, require_offset=False)
    onset_offset_pairs = _match(references, predictions, require_offset=True)
    both_empty = not references and not predictions
    precision = _ratio(len(onset_pairs), len(predictions), 1.0 if both_empty else 0.0)
    recall = _ratio(len(onset_pairs), len(references), 1.0 if both_empty else 0.0)
    full_precision = _ratio(len(onset_offset_pairs), len(predictions), 1.0 if both_empty else 0.0)
    full_recall = _ratio(len(onset_offset_pairs), len(references), 1.0 if both_empty else 0.0)

    errors_ms = np.asarray(
        [(pred.onset_seconds - ref.onset_seconds) * 1000 for ref, pred in onset_pairs], dtype=np.float64
    )
    median_error = float(np.median(np.abs(errors_ms))) if errors_ms.size else None
    drift = intercept = None
    if errors_ms.size >= 2:
        onsets = np.asarray([ref.onset_seconds for ref, _ in onset_pairs], dtype=np.float64)
        if float(np.ptp(onsets)) > 0:
            slope, intercept_value = np.polyfit(onsets, errors_ms, 1)
            drift, intercept = float(slope * 60), float(intercept_value)

    false_notes = len(predictions) - len(onset_pairs)
    false_per_minute = false_notes * 60 / reference_duration_seconds if reference_duration_seconds > 0 else None
    return Evaluation(
        len(references), len(predictions), len(onset_pairs), len(onset_offset_pairs),
        precision, recall, _f1(precision, recall), full_precision, full_recall, _f1(full_precision, full_recall),
        false_per_minute, median_error, drift, intercept,
    )

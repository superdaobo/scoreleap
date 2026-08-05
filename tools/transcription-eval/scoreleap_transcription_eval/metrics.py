from __future__ import annotations

import math
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


@dataclass(slots=True)
class _FlowEdge:
    target: int
    reverse: int
    capacity: int
    cost: float


def _add_flow_edge(graph: list[list[_FlowEdge]], source: int, target: int, cost: float) -> _FlowEdge:
    forward = _FlowEdge(target, len(graph[target]), 1, cost)
    reverse = _FlowEdge(source, len(graph[source]), 0, -cost)
    graph[source].append(forward)
    graph[target].append(reverse)
    return forward


def _match_pitch_group(
    references: Sequence[Note],
    predictions: Sequence[Note],
    eligible: Callable[[Note, Note], bool],
    cost: Callable[[Note, Note], float],
) -> list[tuple[Note, Note]]:
    """最小费用最大流允许交叉配对，并按匹配数、总误差依次优化。"""
    n, m = len(references), len(predictions)
    source, reference_start, prediction_start, sink = 0, 1, 1 + n, 1 + n + m
    graph: list[list[_FlowEdge]] = [[] for _ in range(sink + 1)]
    candidate_edges: list[tuple[int, int, _FlowEdge]] = []
    for i in range(n):
        _add_flow_edge(graph, source, reference_start + i, 0.0)
    for j in range(m):
        _add_flow_edge(graph, prediction_start + j, sink, 0.0)
    for i, ref in enumerate(references):
        for j, pred in enumerate(predictions):
            if eligible(ref, pred):
                edge = _add_flow_edge(graph, reference_start + i, prediction_start + j, cost(ref, pred))
                candidate_edges.append((i, j, edge))

    # 每轮寻找残量网络中的最短增广路；无路可增时即达到最大基数。
    while True:
        distances = [float("inf")] * len(graph)
        previous: list[tuple[int, int] | None] = [None] * len(graph)
        distances[source] = 0.0
        for _ in range(len(graph) - 1):
            changed = False
            for node, edges in enumerate(graph):
                if distances[node] == float("inf"):
                    continue
                for edge_index, edge in enumerate(edges):
                    candidate = distances[node] + edge.cost
                    if edge.capacity and candidate < distances[edge.target] - 1e-12:
                        distances[edge.target] = candidate
                        previous[edge.target] = (node, edge_index)
                        changed = True
            if not changed:
                break
        if previous[sink] is None:
            break
        node = sink
        while node != source:
            parent_edge = previous[node]
            if parent_edge is None:
                raise RuntimeError("残量网络路径不完整")
            parent, edge_index = parent_edge
            edge = graph[parent][edge_index]
            edge.capacity -= 1
            graph[node][edge.reverse].capacity += 1
            node = parent

    return [(references[i], predictions[j]) for i, j, edge in candidate_edges if edge.capacity == 0]


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
    references: Sequence[Note], predictions: Sequence[Note], reference_duration_seconds: float | None
) -> Evaluation:
    if reference_duration_seconds is not None and (
        isinstance(reference_duration_seconds, bool)
        or not isinstance(reference_duration_seconds, (int, float))
        or not math.isfinite(reference_duration_seconds)
        or reference_duration_seconds <= 0
    ):
        raise ValueError("reference_duration_seconds 必须是有限正数或 None")
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
    false_per_minute = (
        false_notes * 60 / reference_duration_seconds
        if reference_duration_seconds is not None and reference_duration_seconds > 0
        else None
    )
    return Evaluation(
        len(references), len(predictions), len(onset_pairs), len(onset_offset_pairs),
        precision, recall, _f1(precision, recall), full_precision, full_recall, _f1(full_precision, full_recall),
        false_per_minute, median_error, drift, intercept,
    )

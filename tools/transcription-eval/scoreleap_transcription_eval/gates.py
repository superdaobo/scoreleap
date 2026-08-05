from __future__ import annotations

import math
from typing import Final


# 发布阈值必须与产品验收约定保持一致；任何一项失败都阻断模型发布。
QUALITY_GATES: Final[dict[str, tuple[str, float]]] = {
    "precision": ("minimum", 0.93),
    "recall": ("minimum", 0.87),
    "onset_f1": ("minimum", 0.90),
    "onset_offset_f1": ("minimum", 0.75),
    "false_notes_per_minute": ("maximum", 3.0),
}


def apply_quality_gates(metrics: dict) -> dict:
    checks: dict[str, bool] = {}
    thresholds: dict[str, dict[str, float]] = {}
    for name, (direction, threshold) in QUALITY_GATES.items():
        value = metrics.get(name)
        valid_value = (
            isinstance(value, (int, float))
            and not isinstance(value, bool)
            and math.isfinite(value)
        )
        checks[name] = bool(
            valid_value
            and (value >= threshold if direction == "minimum" else value <= threshold)
        )
        thresholds[name] = {direction: threshold}
    return {
        "passed": all(checks.values()),
        "checks": checks,
        "thresholds": thresholds,
    }

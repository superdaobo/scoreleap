"""Issue #57：Transkun Worker clean_notes() 高音保留测试。

验证契约：worker 必须保留 MIDI 21–108（A0–C8），不得提前裁成游戏 48–83；
20/109 与负 pitch 控制事件必须丢弃。
纯标准库运行：python tools/transkun-worker/test_clean_notes.py
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from scoreleap_transkun_worker import clean_notes


class Note:
    def __init__(self, pitch, start=0.0, end=1.0, velocity=100):
        self.pitch = pitch
        self.start = start
        self.end = end
        self.velocity = velocity


def main() -> None:
    notes = [
        Note(21),  # A0：边界保留
        Note(84),  # 高音区保留
        Note(96),  # C7 保留
        Note(108),  # C8 边界保留
        Note(20),  # 低于 A0：丢弃
        Note(109),  # 高于 C8：丢弃
        Note(-1),  # 负 pitch 控制事件：丢弃
        Note(60, 0.0, 0.0),  # 无效时值：丢弃
        Note(60, 0.0, 0.5),  # 正常音符
    ]
    cleaned = clean_notes(notes, None)
    pitches = sorted(int(n.pitch) for n in cleaned)
    assert pitches == [21, 60, 84, 96, 108], f"意外保留/丢弃: {pitches}"
    assert len(cleaned) == 5
    # 最短时值过滤（minimum_note_length_ms=100 → 0.1s）
    short = clean_notes([Note(60, 0.0, 0.05)], 100)
    assert short == [], "短于最短时值的音符应被过滤"
    print(f"clean_notes OK: 保留 {pitches}，丢弃 20/109/负 pitch/无效时值")


if __name__ == "__main__":
    main()

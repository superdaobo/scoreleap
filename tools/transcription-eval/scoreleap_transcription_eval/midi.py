from __future__ import annotations

from collections import defaultdict, deque
from dataclasses import dataclass
from pathlib import Path

import mido


@dataclass(frozen=True, slots=True)
class MidiNotes:
    notes: tuple["Note", ...]
    duration_seconds: float


def read_midi_notes(path: str | Path) -> MidiNotes:
    """按全局速度轨读取 MIDI，并严格配对每个通道上的 note_on/note_off。"""
    from .metrics import Note

    midi = mido.MidiFile(path)
    tempo = 500_000
    elapsed = 0.0
    active: dict[tuple[int, int], deque[float]] = defaultdict(deque)
    notes: list[Note] = []

    for message in mido.merge_tracks(midi.tracks):
        elapsed += mido.tick2second(message.time, midi.ticks_per_beat, tempo)
        if message.type == "set_tempo":
            tempo = message.tempo
            continue
        if message.type == "note_on" and message.velocity > 0:
            active[(message.channel, message.note)].append(elapsed)
            continue
        if message.type not in {"note_off", "note_on"}:
            continue

        key = (message.channel, message.note)
        if not active[key]:
            raise ValueError(f"MIDI 存在无对应 note_on 的 note_off: channel={key[0]}, pitch={key[1]}")
        onset = active[key].popleft()
        if elapsed <= onset:
            raise ValueError(f"MIDI 音符时长必须大于零: channel={key[0]}, pitch={key[1]}")
        notes.append(Note(pitch=message.note, onset_seconds=onset, offset_seconds=elapsed))

    unclosed = [(channel, pitch) for (channel, pitch), starts in active.items() if starts]
    if unclosed:
        raise ValueError(f"MIDI 存在未关闭音符: {unclosed[:5]}")
    notes.sort(key=lambda note: (note.onset_seconds, note.pitch, note.offset_seconds))
    duration = max([elapsed, *(note.offset_seconds for note in notes)], default=elapsed)
    return MidiNotes(tuple(notes), duration)

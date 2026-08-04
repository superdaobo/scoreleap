"""转录核心：Predictor 抽象（可注入，测试用 FakePredictor）与 Basic Pitch 实现。"""

from typing import Any, Callable, List, Optional, Protocol, Tuple

from .errors import inference_error, model_error

StageCallback = Callable[[str, str], None]  # (stage, message)


class NoteInfo:
    """一条转录音符（供统计；不承载演奏数据——演奏数据以 MIDI 为准）。"""

    def __init__(self, start_time_s: float, pitch: int):
        self.start_time_s = start_time_s
        self.pitch = pitch


class Predictor(Protocol):
    """可注入的预测器接口。"""

    def predict(self, audio_path: str) -> Tuple[Any, List[NoteInfo]]:
        """返回 (midi_data, notes)；midi_data 必须支持 save/write 写 MIDI。"""
        ...


class BasicPitchPredictor:
    """basic-pitch 官方推理实现（tensorflow 后端，模型随包分发）。"""

    def __init__(self):
        self._model = None
        self._engine_version = "0.4.0"

    def _ensure_loaded(self) -> None:
        if self._model is not None:
            return
        try:
            from basic_pitch.inference import predict

            self._model = predict
        except Exception as e:  # noqa: BLE001 - 统一转结构化错误
            raise model_error(str(e)) from e

    def predict(self, audio_path: str) -> Tuple[Any, List[NoteInfo]]:
        self._ensure_loaded()
        try:
            # predict(...) -> (model_output, midi_data, note_events)
            _, midi_data, note_events = self._model(
                audio_path,
                onset_threshold=0.5,
                frame_threshold=0.3,
                minimum_note_length=127.58,
                minimum_frequency=None,
                maximum_frequency=None,
            )
        except Exception as e:  # noqa: BLE001
            raise inference_error(str(e)) from e
        # basic-pitch note_events: List[(start_time, end_time, pitch, amplitude, pitch_bends)]
        notes: List[NoteInfo] = []
        for ne in note_events:
            try:
                notes.append(NoteInfo(float(ne[0]), int(ne[2])))
            except Exception:  # noqa: BLE001 - 单条音符异常不影响整体
                continue
        return midi_data, notes


class FakePredictor:
    """测试用：不加载真实模型，返回固定音符（mido 构造 MIDI）。"""

    def __init__(self, notes: Optional[List[NoteInfo]] = None):
        self._notes = notes or [
            NoteInfo(0.0, 60),
            NoteInfo(0.5, 62),
            NoteInfo(1.0, 64),
        ]
        self.calls = 0

    def predict(self, audio_path: str) -> Tuple[Any, List[NoteInfo]]:
        self.calls += 1
        import mido

        mid = mido.MidiFile()
        track = mido.MidiTrack()
        mid.tracks.append(track)
        for n in self._notes:
            track.append(
                mido.Message(
                    "note_on",
                    note=n.pitch,
                    velocity=80,
                    time=int(n.start_time_s * mid.ticks_per_beat),
                )
            )
            track.append(mido.Message("note_off", note=n.pitch, velocity=0, time=480))
        return mid, self._notes


class Transcriber:
    """编排验证 → 模型加载 → 推理 → MIDI 写入 → metadata 写入。"""

    def __init__(
        self,
        predictor: Predictor,
        writer,
        request_id: str,
        engine_version: str = "0.4.0",
        worker_version: str = "0.1.0",
        cancel: Optional[Callable[[], bool]] = None,
    ):
        self._predictor = predictor
        self._writer = writer
        self._request_id = request_id
        self._engine_version = engine_version
        self._worker_version = worker_version
        self._cancel = cancel or (lambda: False)

    def _stage(self, stage: str, message: str) -> None:
        self._writer.stage(self._request_id, stage, message)

    def _check_cancel(self) -> None:
        from .errors import cancelled_error

        if self._cancel():
            raise cancelled_error()

    def run(
        self,
        input_path: str,
        output_midi: str,
        output_metadata: str,
        source_size_bytes: int,
        source_duration_ms: int,
        warnings: List[str],
    ) -> int:
        import os
        import time

        from .metadata import build_metadata, write_metadata

        start = time.monotonic()
        self._stage("validating_input", "正在验证音频")
        self._check_cancel()

        self._stage("loading_model", "正在加载本地模型")
        self._check_cancel()
        midi_data, notes = self._predictor.predict(input_path)
        note_count = len(notes)

        self._stage("writing_midi", "正在生成 MIDI")
        self._check_cancel()
        try:
            if hasattr(midi_data, "save"):
                midi_data.save(output_midi)
            elif hasattr(midi_data, "write"):
                midi_data.write(output_midi)
            else:
                raise TypeError("midi_data 对象不支持 save/write")
        except Exception as e:  # noqa: BLE001
            from .errors import midi_write_error

            raise midi_write_error(str(e)) from e

        elapsed_ms = int((time.monotonic() - start) * 1000)
        metadata = build_metadata(
            request_id=self._request_id,
            source_path=input_path,
            source_size_bytes=source_size_bytes,
            source_duration_ms=source_duration_ms,
            engine_version=self._engine_version,
            worker_version=self._worker_version,
            midi_file=os.path.basename(output_midi),
            note_count=note_count,
            elapsed_ms=elapsed_ms,
            warnings=warnings,
        )
        write_metadata(output_metadata, metadata)
        self._writer.result(
            self._request_id,
            midi_path=output_midi,
            metadata_path=output_metadata,
            elapsed_ms=elapsed_ms,
            note_count=note_count,
        )
        return 0


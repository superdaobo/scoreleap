# scoreleap-transcriber — 本地音频转录 Worker

将 MP3 转录为 MIDI（Basic Pitch），供 ScoreLeap 复用现有 MIDI 管线。

## 开发

```powershell
python -m venv .venv
.venv\Scripts\python -m pip install -e ".[dev]"
.venv\Scripts\python -m pytest
```

## 使用

```powershell
.venv\Scripts\scoreleap-transcriber transcribe ^
  --request-id <uuid> --input <abs.mp3> ^
  --output-midi <abs.generated.mid> --output-metadata <abs.metadata.json>
```

stdout 仅输出 JSON Lines（ready/stage/result/error）；日志走 stderr；退出码见 `scoreleap_transcriber/errors.py`。

## 协议

- `{"type":"ready",...}` / `{"type":"stage","stage":"validating_input|loading_model|transcribing|writing_midi",...}` / `{"type":"result","midi_path":...,"note_count":...}` / `{"type":"error","code":...}`
- schema_version=1；未知字段向后兼容。
- 测试模式：`SCORELEAP_FAKE_PREDICTOR=1`（不加载模型）。

## 限制（MVP）

- 仅 .mp3、≤200MB、≤10 分钟；
- 完整歌曲会有杂音符（无音源分离）；钢琴独奏/旋律清晰效果最佳。

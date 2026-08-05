# ScoreLeap 转录质量评测工具

该工具比较参考 MIDI 与预测 MIDI，固定使用以下验收规则：音高完全一致、起音误差不超过 50ms、尾音误差不超过 `max(50ms, 参考音符时长 × 20%)`，并保证一对一匹配。

## 安装与命令

```powershell
python -m pip install -r requirements.txt
python -m scoreleap_transcription_eval evaluate reference.mid prediction.mid --manifest manifest.json --sample-id maestro-hidden-001
python -m scoreleap_transcription_eval compare-formats lossless.mid encoded.mid
python -m scoreleap_transcription_eval validate-manifest manifest.json --verify-files
python -m unittest discover -s tests -v
```

`evaluate` 输出起音 Precision、Recall、F1，带尾音 Precision、Recall、F1，误音数/分钟、起音绝对误差中位数，以及预测相对参考的线性漂移（ms/min 与截距）。误音数/分钟严格使用所选 manifest 片段时长，即使参考 MIDI 为空也不会回退到 MIDI 尾音时间。`compare-formats` 额外输出 `note_consistency_f1`，其未经 manifest 定义的误音率固定为 `null`。

## 清单格式

```json
{
  "schema_version": 1,
  "samples": [{
    "id": "maestro-hidden-001",
    "source": "MAESTRO v3",
    "split": "hidden",
    "audio": {"path": "audio/001.wav", "sha256": "<64 hex>"},
    "reference_midi": {"path": "midi/001.mid", "sha256": "<64 hex>"},
    "segment": {"start_seconds": 0, "end_seconds": 30},
    "noise": {"seed": 20260805, "snr_db": 20}
  }]
}
```

干净样本的 `noise` 必须为 `null`；加噪样本必须同时提供非负整数 `seed` 与有限数值 `snr_db`。相对路径以清单所在目录为数据集根；绝对路径、`..` 越界和指向根外的符号链接均会被拒绝。只有传入 `--verify-files` 时才以流式方式读取文件并复核 SHA-256，单个资产最大 200MiB。

## 数据边界

MAESTRO、MAPS 和 `8月4日.MP3` 及其人工参考 MIDI 只能保存在本机 `D:\tmp` 下。严禁将第三方数据、用户音频、切片、派生 MIDI 或本地清单提交到仓库。本目录测试夹具均在临时目录中由程序生成。

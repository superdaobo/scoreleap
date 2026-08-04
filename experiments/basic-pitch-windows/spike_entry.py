# Spike 打包入口：调用 basic_pitch.predict.main（等价 console script）
# Windows 中文控制台默认 GBK：必须显式 reconfigure 为 UTF-8，否则 emoji/中文输出崩溃
import sys
try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass
from basic_pitch.predict import main
if __name__ == "__main__":
    sys.exit(main())

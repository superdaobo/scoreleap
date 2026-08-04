# 打包入口：绝对导入（PyInstaller 冻结后 __main__.py 的相对导入失效）
from scoreleap_transcriber.cli import main
import sys

if __name__ == "__main__":
    sys.exit(main())

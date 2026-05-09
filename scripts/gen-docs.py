#!/usr/bin/env python3
"""
lspz 文档生成脚本

当前状态: 框架脚本，等待 lspz-core 实现后启用

使用方式:
    python scripts/gen-docs.py          # 生成所有文档
    python scripts/gen-docs.py --check  # 检查文档是否过时
"""

import argparse
import sys
from pathlib import Path


def check_mode() -> bool:
    """检查是否为 check 模式"""
    return "--check" in sys.argv


def main():
    parser = argparse.ArgumentParser(description="lspz 文档生成脚本")
    parser.add_argument(
        "--check",
        action="store_true",
        help="检查模式：只检查文档是否过时，不写入",
    )
    args = parser.parse_args()

    project_root = Path(__file__).parent.parent
    docs_dir = project_root / "docs"

    if args.check:
        print("🔍 检查文档漂移...")
        # TODO: 实现漂移检测逻辑
        print("ℹ️  当前项目处于规划阶段，文档生成功能待实现")
        print("   详见 docs/specs/004-ssot-rules.md")
        return 0
    else:
        print("📝 生成文档...")
        # TODO: 实现文档生成逻辑
        print("ℹ️  当前项目处于规划阶段，文档生成功能待实现")
        print("   详见 docs/specs/004-ssot-rules.md")
        return 0


if __name__ == "__main__":
    sys.exit(main())

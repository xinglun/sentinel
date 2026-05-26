#!/usr/bin/env python3
"""AI Cockpit status 生成の回帰テスト。"""
from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path


def main() -> int:
    script = Path(__file__).with_name("ai_generate_status.py")
    with tempfile.TemporaryDirectory() as tmp:
        output = Path(tmp) / "current_status.md"
        command = ["python3", str(script), "--no-active", "--output", str(output)]
        subprocess.run(command, check=True, capture_output=True, text=True)
        initial = output.read_text(encoding="utf-8")
        subprocess.run(command, check=True, capture_output=True, text=True)
        repeated = output.read_text(encoding="utf-8")
        assert initial == repeated, "no-active status は時刻差分のみで更新してはならない"
    print("✅ cockpit status generation tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""AI Work Item 操作前後の共通 preflight を実行する。"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
ACTIVE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "active"
ARCHIVE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "archive"
CURRENT_STATUS = PROJECT_ROOT / ".ai" / "cockpit" / "current_status.md"


def current_status_is_no_active() -> bool:
    if not CURRENT_STATUS.exists():
        return False
    return "- State: `no_active_work_item`" in CURRENT_STATUS.read_text(encoding="utf-8")


def archived_counterparts(active_path: Path) -> list[Path]:
    if not ARCHIVE_DIR.exists():
        return []
    return sorted(path for path in ARCHIVE_DIR.rglob(active_path.name) if path.is_file())


def equivalent_archived_residue(active_path: Path, archive_path: Path) -> bool:
    if active_path.name.endswith(".summary.json"):
        try:
            active_json = json.loads(active_path.read_text(encoding="utf-8"))
            archive_json = json.loads(archive_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return False
        active_json.pop("contractPath", None)
        archive_json.pop("contractPath", None)
        return active_json == archive_json
    return active_path.read_bytes() == archive_path.read_bytes()


def cleanup_archived_active_residue() -> int:
    if not current_status_is_no_active() or not ACTIVE_DIR.exists():
        return 0

    removed = 0
    for active_path in sorted(ACTIVE_DIR.glob("*.json")):
        if not active_path.name.endswith((".contract.json", ".summary.json", ".review.json")):
            continue
        counterparts = archived_counterparts(active_path)
        if len(counterparts) != 1:
            continue
        if not equivalent_archived_residue(active_path, counterparts[0]):
            continue
        active_path.unlink()
        removed += 1

    if removed:
        print(f"✅ ai-preflight cleaned archived active residue: {removed} file(s)")
    return removed


def active_contracts() -> list[Path]:
    if not ACTIVE_DIR.exists():
        return []
    return sorted(path for path in ACTIVE_DIR.glob("*.contract.json") if path.is_file())


def run_check(label: str, script: Path) -> int:
    result = subprocess.run([sys.executable, str(script)], cwd=PROJECT_ROOT, check=False)
    if result.returncode != 0:
        print(f"❌ ai-preflight failed: {label}", file=sys.stderr)
    return result.returncode


def run_make(label: str, target: str, *, contract: Path | None = None) -> int:
    command = ["make", target]
    if contract is not None:
        command.append(f"CONTRACT={contract.relative_to(PROJECT_ROOT).as_posix()}")
    result = subprocess.run(command, cwd=PROJECT_ROOT, check=False)
    if result.returncode != 0:
        print(f"❌ ai-preflight failed: {label}", file=sys.stderr)
    return result.returncode


def main() -> int:
    cleanup_archived_active_residue()
    checks = [
        ("lifecycle", PROJECT_ROOT / "scripts" / "ai_check_lifecycle.py"),
        ("status consistency", PROJECT_ROOT / "scripts" / "ai_check_status_consistency.py"),
    ]
    for label, script in checks:
        code = run_check(label, script)
        if code != 0:
            return code

    contracts = active_contracts()
    if len(contracts) == 1:
        contract = contracts[0]
        review_checks = [
            ("preflight review generation", "generate-ai-preflight-review"),
            ("preflight review validation", "check-ai-preflight-review"),
        ]
        for label, target in review_checks:
            code = run_make(label, target, contract=contract)
            if code != 0:
                return code
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

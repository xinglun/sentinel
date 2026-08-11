#!/usr/bin/env python3
"""Work Item の archive 後 lifecycle を fail-closed で確認する。"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


PROJECT_ROOT = Path(__file__).resolve().parents[1]
ACTIVE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "active"
ARCHIVE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "archive"


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError("JSON root must be an object")
    return value


def archived_paths(task: str) -> tuple[Path | None, Path | None]:
    contracts = sorted(ARCHIVE_DIR.rglob(f"{task}.contract.json")) if ARCHIVE_DIR.exists() else []
    summaries = sorted(ARCHIVE_DIR.rglob(f"{task}.summary.json")) if ARCHIVE_DIR.exists() else []
    return (contracts[0] if len(contracts) == 1 else None, summaries[0] if len(summaries) == 1 else None)


def validate_archived_work_item(task: str) -> list[str]:
    """finish/archive の前提を再実行せず、close の境界を検証する。"""
    issues: list[str] = []
    active_paths = [
        ACTIVE_DIR / f"{task}.contract.json",
        ACTIVE_DIR / f"{task}.summary.json",
        ACTIVE_DIR / f"{task}.review.json",
    ]
    residue = [path.name for path in active_paths if path.exists()]
    if residue:
        issues.append(
            "active Work Item residue remains; run ai-finish and archive before close: "
            + ", ".join(residue)
        )

    contract_path, summary_path = archived_paths(task)
    if contract_path is None:
        issues.append(f"archived Contract is missing or ambiguous for task: {task}")
        return issues
    if summary_path is None:
        issues.append(f"archived Summary is missing or ambiguous for task: {task}")
        return issues

    try:
        contract = load_json(contract_path)
        summary = load_json(summary_path)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        issues.append(f"archived Work Item evidence is unreadable: {exc}")
        return issues

    if contract.get("workItemId") != task:
        issues.append(f"archived Contract workItemId does not match task: {task}")
    expected_contract = contract_path.relative_to(PROJECT_ROOT).as_posix()
    if summary.get("contractPath") != expected_contract:
        issues.append(
            "archived Summary contractPath mismatch: "
            f"expected {expected_contract}, got {summary.get('contractPath')!r}"
        )
    return issues


def run_make_check(command: list[str]) -> int:
    """Make 経由の既存 lifecycle/status gate を実行する。"""
    print("$ " + " ".join(command))
    result = subprocess.run(command, cwd=PROJECT_ROOT, check=False)
    return result.returncode


def close_work_item(task: str) -> list[str]:
    """archive evidence と共有 gate を確認し、問題を列挙する。"""
    issues = validate_archived_work_item(task)
    if issues:
        return issues
    for command in (["make", "check-work-items-lifecycle"], ["make", "check-ai-status-consistency"]):
        if run_make_check(command) != 0:
            issues.append(f"required close check failed: {' '.join(command)}")
            break
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description="Close an archived AI Work Item fail-closed.")
    parser.add_argument("--task", required=True)
    args = parser.parse_args()
    issues = close_work_item(args.task)
    if issues:
        for issue in issues:
            print(f"❌ {issue}", file=sys.stderr)
        return 1
    print(f"✅ Work Item close checks passed: {args.task}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

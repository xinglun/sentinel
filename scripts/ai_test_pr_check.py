#!/usr/bin/env python3
"""ai_check_pr の回帰テスト。"""

from __future__ import annotations

import json
import shutil
from pathlib import Path
from unittest.mock import patch

import ai_check_pr


REPO_ROOT = Path(__file__).resolve().parents[1]


def assert_true(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def write_json(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def valid_contract() -> dict:
    return {
        "contractVersion": 1,
        "workItemId": "task",
        "mode": "investigate",
        "title": "task",
        "scope": ["README.md"],
        "outOfScope": [],
        "sources": [{"path": "README.md", "reason": "test"}],
        "unknowns": [],
        "notCodable": False,
        "acceptance": ["test acceptance"],
        "verification": [{"command": "make fmt-check", "required": True}],
        "rollbackNote": "test rollback",
    }


def valid_summary(contract_path: str) -> dict:
    return {
        "workItemId": "task",
        "contractPath": contract_path,
        "changedFiles": [{"path": "README.md", "reason": "test"}],
        "sourcesUsed": ["README.md"],
        "verification": [{"command": "make fmt-check", "result": "passed"}],
        "unknownsRemaining": [],
        "risk": {"level": "low", "detail": "test"},
        "generatedFiles": [],
        "destructiveChanges": [],
        "observedIssues": [],
    }


def test_validate_archive_bundle_accepts_paired_archive_move(root: Path) -> None:
    contract_rel = ".ai/work-items/archive/2026/task.contract.json"
    summary_rel = ".ai/work-items/archive/2026/task.summary.json"
    active_contract_rel = ".ai/work-items/active/task.contract.json"
    active_summary_rel = ".ai/work-items/active/task.summary.json"
    write_json(root / contract_rel, valid_contract())
    write_json(root / summary_rel, valid_summary(contract_rel))
    changes = [
        ("D", active_contract_rel),
        ("D", active_summary_rel),
        ("A", contract_rel),
        ("A", summary_rel),
    ]
    with patch.object(ai_check_pr, "PROJECT_ROOT", root):
        issues = ai_check_pr.validate_archive_bundle(changes)
    assert_true(not issues, f"paired archive move should pass: {issues}")


def test_validate_archive_bundle_rejects_missing_summary(root: Path) -> None:
    contract_rel = ".ai/work-items/archive/2026/task.contract.json"
    active_contract_rel = ".ai/work-items/active/task.contract.json"
    write_json(root / contract_rel, valid_contract())
    changes = [
        ("D", active_contract_rel),
        ("A", contract_rel),
    ]
    with patch.object(ai_check_pr, "PROJECT_ROOT", root):
        issues = ai_check_pr.validate_archive_bundle(changes)
    assert_true(
        any("archive summary" in issue or "paired" in issue for issue in issues),
        f"missing summary should be rejected: {issues}",
    )


def test_validate_archive_bundle_rejects_modified_archive_path(root: Path) -> None:
    contract_rel = ".ai/work-items/archive/2026/task.contract.json"
    summary_rel = ".ai/work-items/archive/2026/task.summary.json"
    write_json(root / contract_rel, valid_contract())
    write_json(root / summary_rel, valid_summary(contract_rel))
    changes = [("M", contract_rel), ("A", summary_rel)]
    with patch.object(ai_check_pr, "PROJECT_ROOT", root):
        issues = ai_check_pr.validate_archive_bundle(changes)
    assert_true(
        any("append-only" in issue for issue in issues),
        f"modified archive path should be rejected: {issues}",
    )


def main() -> int:
    root = REPO_ROOT / "target" / "ai_pr_check_test"
    shutil.rmtree(root, ignore_errors=True)
    root.mkdir(parents=True, exist_ok=True)
    try:
        test_validate_archive_bundle_accepts_paired_archive_move(root)
        print("✅ paired_archive_move")
        test_validate_archive_bundle_rejects_missing_summary(root)
        print("✅ rejects_missing_summary")
        test_validate_archive_bundle_rejects_modified_archive_path(root)
        print("✅ rejects_modified_archive_path")
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print("✅ ai_check_pr tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

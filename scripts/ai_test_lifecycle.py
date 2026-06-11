#!/usr/bin/env python3
"""Work Item lifecycle checker の lightweight regression tests。"""

from __future__ import annotations

import shutil
from pathlib import Path
from unittest.mock import patch

import ai_check_lifecycle


REPO_ROOT = Path(__file__).resolve().parents[1]


def write(path: Path, content: str = "{}\n") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def assert_true(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def test_active_archive_duplicate_is_rejected(root: Path) -> None:
    active = root / ".ai/work-items/active"
    archive = root / ".ai/work-items/archive/2026"
    write(active / "sample.contract.json")
    write(active / "sample.summary.json", '{"contractPath": ".ai/work-items/active/sample.contract.json"}\n')
    write(archive / "sample.contract.json")
    write(archive / "sample.summary.json", '{"contractPath": ".ai/work-items/archive/2026/sample.contract.json"}\n')

    with (
        patch.object(ai_check_lifecycle, "PROJECT_ROOT", root),
        patch.object(ai_check_lifecycle, "WORK_ITEMS_DIR", root / ".ai/work-items"),
        patch.object(ai_check_lifecycle, "ACTIVE_DIR", active),
        patch.object(ai_check_lifecycle, "ARCHIVE_DIR", root / ".ai/work-items/archive"),
        patch.object(ai_check_lifecycle, "CURRENT_STATUS", root / ".ai/cockpit/current_status.md"),
    ):
        issues = ai_check_lifecycle.check_active_archive_duplicates()

    assert_true(issues, "active と archive の重複は検出されるべき")


def test_no_active_status_rejects_active_files(root: Path) -> None:
    active = root / ".ai/work-items/active"
    write(active / "sample.contract.json")
    write(root / ".ai/cockpit/current_status.md", "- State: `no_active_work_item`\n")

    with (
        patch.object(ai_check_lifecycle, "PROJECT_ROOT", root),
        patch.object(ai_check_lifecycle, "ACTIVE_DIR", active),
        patch.object(ai_check_lifecycle, "CURRENT_STATUS", root / ".ai/cockpit/current_status.md"),
    ):
        issues = ai_check_lifecycle.check_current_status_consistency()

    assert_true(issues, "no_active_work_item で active file が残る場合は検出されるべき")


def test_multiple_active_contracts_are_rejected(root: Path) -> None:
    active = root / ".ai/work-items/active"
    write(active / "first.contract.json")
    write(active / "first.summary.json", '{"contractPath": ".ai/work-items/active/first.contract.json"}\n')
    write(active / "second.contract.json")
    write(active / "second.summary.json", '{"contractPath": ".ai/work-items/active/second.contract.json"}\n')
    write(root / ".ai/cockpit/current_status.md", "- Task: `first`\n- State: `ready_for_review`\n")

    with (
        patch.object(ai_check_lifecycle, "PROJECT_ROOT", root),
        patch.object(ai_check_lifecycle, "ACTIVE_DIR", active),
        patch.object(ai_check_lifecycle, "CURRENT_STATUS", root / ".ai/cockpit/current_status.md"),
    ):
        issues = ai_check_lifecycle.check_single_active_contract()

    assert_true(issues, "複数 active Contract は検出されるべき")
    assert_true(
        any("current_status.md points to a single task" in issue for issue in issues),
        "単一 current_status との不整合も検出されるべき",
    )


def main() -> int:
    cases = [
        ("active_archive_duplicate_is_rejected", test_active_archive_duplicate_is_rejected),
        ("no_active_status_rejects_active_files", test_no_active_status_rejects_active_files),
        ("multiple_active_contracts_are_rejected", test_multiple_active_contracts_are_rejected),
    ]
    root = REPO_ROOT / "target" / "ai_lifecycle_test"
    shutil.rmtree(root, ignore_errors=True)
    try:
        for name, case in cases:
            shutil.rmtree(root, ignore_errors=True)
            case(root)
            print(f"✅ {name}")
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print("✅ lifecycle tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

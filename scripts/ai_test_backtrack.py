#!/usr/bin/env python3
"""backtrack hard gate の破壊変更承認境界を検証する。"""

from __future__ import annotations

import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

import ai_check_backtrack
from ai_check_backtrack import DestructiveApproval, approvals_for_changes, detect_items


def assert_count(items: list[object], expected: int, message: str) -> None:
    if len(items) != expected:
        raise AssertionError(f"{message}: expected={expected}, actual={len(items)}")


def test_deleted_test_without_approval_fails() -> None:
    items = detect_items([("D", "tests/pipeline_integration.rs")])
    assert_count(items, 1, "test deletion without approval")


def test_deleted_snapshot_requires_documented_approval() -> None:
    path = "src/features/radar/interface/snapshots/report.txt"
    undocumented = DestructiveApproval(("src/**/snapshots/**",), False)
    items = detect_items([("D", path)], [undocumented])
    assert_count(items, 1, "undocumented deletion must fail")

    documented = DestructiveApproval(("src/**/snapshots/**",), True)
    items = detect_items([("D", path)], [documented])
    assert_count(items, 0, "documented allowed deletion must pass")


def test_work_item_evidence_deletion_fails_by_default() -> None:
    items = detect_items([("D", ".ai/work-items/archive/2026/example.contract.json")])
    assert_count(items, 1, "evidence deletion must fail")


def test_active_work_item_archive_move_is_allowed() -> None:
    with TemporaryDirectory() as tmp:
        root = Path(tmp)
        target = root / ".ai/work-items/archive/2026/example.contract.json"
        target.parent.mkdir(parents=True)
        content = '{"workItemId":"example"}\n'
        target.write_text(content, encoding="utf-8")

        with (
            patch.object(ai_check_backtrack, "PROJECT_ROOT", root),
            patch.object(
                ai_check_backtrack,
                "run_git",
                lambda _args: subprocess.CompletedProcess(_args, 0, stdout=content, stderr=""),
            ),
        ):
            items = detect_items(
                [
                    ("D", ".ai/work-items/active/example.contract.json"),
                    ("A", ".ai/work-items/archive/2026/example.contract.json"),
                ]
            )

    assert_count(items, 0, "active evidence moved to archive should pass")


def test_active_work_item_stale_archive_counterpart_still_fails() -> None:
    with TemporaryDirectory() as tmp:
        root = Path(tmp)
        target = root / ".ai/work-items/archive/2026/example.contract.json"
        target.parent.mkdir(parents=True)
        target.write_text('{"workItemId":"example"}\n', encoding="utf-8")

        with patch.object(ai_check_backtrack, "PROJECT_ROOT", root):
            items = detect_items([("D", ".ai/work-items/active/example.contract.json")])

    assert_count(items, 1, "stale archive counterpart without current diff add must fail")


def test_active_work_item_archive_move_requires_matching_content() -> None:
    with TemporaryDirectory() as tmp:
        root = Path(tmp)
        target = root / ".ai/work-items/archive/2026/example.contract.json"
        target.parent.mkdir(parents=True)
        target.write_text('{"workItemId":"different"}\n', encoding="utf-8")

        with (
            patch.object(ai_check_backtrack, "PROJECT_ROOT", root),
            patch.object(
                ai_check_backtrack,
                "run_git",
                lambda _args: subprocess.CompletedProcess(
                    _args,
                    0,
                    stdout='{"workItemId":"example"}\n',
                    stderr="",
                ),
            ),
        ):
            items = detect_items(
                [
                    ("D", ".ai/work-items/active/example.contract.json"),
                    ("A", ".ai/work-items/archive/2026/example.contract.json"),
                ]
            )

    assert_count(items, 1, "archive move with mismatched content must fail")


def test_archive_summary_destructive_cleanup_is_loaded_without_explicit_args() -> None:
    with TemporaryDirectory() as tmp:
        root = Path(tmp)
        contract = root / ".ai/work-items/archive/2026/example.contract.json"
        summary = root / ".ai/work-items/archive/2026/example.summary.json"
        contract.parent.mkdir(parents=True)
        contract.write_text(
            """
{
  "destructiveChangePolicy": {
    "allowed": true,
    "allowPatterns": [
      ".ai/work-items/active/example.summary.json"
    ]
  }
}
""".lstrip(),
            encoding="utf-8",
        )
        summary.write_text(
            """
{
  "destructiveChanges": [
    {
      "path": ".ai/work-items/active/example.summary.json",
      "reason": "Work Item 完了に伴い archive/2026 へ移動した。"
    }
  ]
}
""".lstrip(),
            encoding="utf-8",
        )

        with patch.object(ai_check_backtrack, "PROJECT_ROOT", root):
            changes = [
                ("D", ".ai/work-items/active/example.summary.json"),
                ("A", ".ai/work-items/archive/2026/example.summary.json"),
            ]
            approvals = approvals_for_changes(changes, None, None)
            items = detect_items(changes, approvals)

    assert_count(items, 0, "archive summary cleanup approval should load without explicit args")


def test_archive_summary_cleanup_still_requires_documentation() -> None:
    with TemporaryDirectory() as tmp:
        root = Path(tmp)
        contract = root / ".ai/work-items/archive/2026/example.contract.json"
        summary = root / ".ai/work-items/archive/2026/example.summary.json"
        contract.parent.mkdir(parents=True)
        contract.write_text(
            """
{
  "destructiveChangePolicy": {
    "allowed": true,
    "allowPatterns": [
      ".ai/work-items/active/example.summary.json"
    ]
  }
}
""".lstrip(),
            encoding="utf-8",
        )
        summary.write_text('{"destructiveChanges":[]}\n', encoding="utf-8")

        with patch.object(ai_check_backtrack, "PROJECT_ROOT", root):
            changes = [
                ("D", ".ai/work-items/active/example.summary.json"),
                ("A", ".ai/work-items/archive/2026/example.summary.json"),
            ]
            approvals = approvals_for_changes(changes, None, None)
            items = detect_items(changes, approvals)

    assert_count(items, 1, "archive summary cleanup approval must be documented")


def main() -> int:
    cases = [
        test_deleted_test_without_approval_fails,
        test_deleted_snapshot_requires_documented_approval,
        test_work_item_evidence_deletion_fails_by_default,
        test_active_work_item_archive_move_is_allowed,
        test_active_work_item_stale_archive_counterpart_still_fails,
        test_active_work_item_archive_move_requires_matching_content,
        test_archive_summary_destructive_cleanup_is_loaded_without_explicit_args,
        test_archive_summary_cleanup_still_requires_documentation,
    ]
    for case in cases:
        case()
        print(f"✅ {case.__name__}")
    print("✅ backtrack hard gate tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

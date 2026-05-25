#!/usr/bin/env python3
"""backtrack hard gate の破壊変更承認境界を検証する。"""

from __future__ import annotations

from ai_check_backtrack import DestructiveApproval, detect_items


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


def main() -> int:
    cases = [
        test_deleted_test_without_approval_fails,
        test_deleted_snapshot_requires_documented_approval,
        test_work_item_evidence_deletion_fails_by_default,
    ]
    for case in cases:
        case()
        print(f"✅ {case.__name__}")
    print("✅ backtrack hard gate tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

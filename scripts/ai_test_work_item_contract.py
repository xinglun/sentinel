#!/usr/bin/env python3
"""Work Item Contract の必須 verification を検証する lightweight test。"""

from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path
from unittest.mock import patch

import ai_check_work_item
import ai_start


REPO_ROOT = Path(__file__).resolve().parents[1]


def assert_true(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def assert_equal(actual: object, expected: object, message: str) -> None:
    if actual != expected:
        raise AssertionError(f"{message}: expected={expected!r}, actual={actual!r}")


def minimal_contract(*, include_fmt: bool) -> dict:
    verification = [
        {"command": "make check-ai-backtrack", "required": True},
    ]
    if include_fmt:
        verification.insert(0, {"command": "make fmt-check", "required": True})
    return {
        "contractVersion": 1,
        "workItemId": "fmt_contract_test",
        "mode": "investigate",
        "title": "fmt contract test",
        "scope": ["README.md"],
        "outOfScope": [],
        "sources": [{"path": "README.md", "reason": "test source"}],
        "unknowns": [],
        "notCodable": False,
        "acceptance": ["test acceptance"],
        "verification": verification,
        "rollbackNote": "test rollback",
    }


def test_contract_rejects_missing_fmt() -> None:
    issues = ai_check_work_item.validate_contract(minimal_contract(include_fmt=False))
    assert_true(
        any("make fmt-check" in issue for issue in issues),
        "missing make fmt-check should be rejected",
    )


def test_contract_accepts_required_fmt() -> None:
    issues = ai_check_work_item.validate_contract(minimal_contract(include_fmt=True))
    assert_equal(issues, [], "required make fmt-check should pass")


def test_ai_start_generates_required_fmt(active: Path) -> None:
    task = "fmt_generated_contract"
    with (
        patch.object(ai_start, "ACTIVE_DIR", active),
        patch.object(sys, "argv", ["ai_start.py", "--task", task, "--mode", "investigate"]),
    ):
        code = ai_start.main()
    assert_equal(code, 0, "ai_start should succeed")
    contract = json.loads((active / f"{task}.contract.json").read_text(encoding="utf-8"))
    required = {
        item["command"]
        for item in contract["verification"]
        if item.get("required") is True
    }
    assert_true("make fmt-check" in required, "generated contract should include make fmt-check")


def main() -> int:
    active = REPO_ROOT / "target" / "ai_work_item_contract_test_active"
    shutil.rmtree(active, ignore_errors=True)
    try:
        test_contract_rejects_missing_fmt()
        print("✅ rejects_missing_fmt")
        test_contract_accepts_required_fmt()
        print("✅ accepts_required_fmt")
        test_ai_start_generates_required_fmt(active)
        print("✅ ai_start_generates_required_fmt")
    finally:
        shutil.rmtree(active, ignore_errors=True)
    print("✅ work item contract tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Contract / Summary verification command の make-only policy を検証する。"""

from __future__ import annotations

import ai_check_summary
import ai_check_work_item


def assert_true(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def assert_equal(actual: object, expected: object, message: str) -> None:
    if actual != expected:
        raise AssertionError(f"{message}: expected={expected!r}, actual={actual!r}")


def base_contract(command: str) -> dict:
    return {
        "contractVersion": 1,
        "workItemId": "make_only_test",
        "mode": "investigate",
        "title": "make only test",
        "scope": ["README.md"],
        "outOfScope": [],
        "sources": [{"path": "README.md", "reason": "test source"}],
        "unknowns": [],
        "notCodable": False,
        "acceptance": ["test acceptance"],
        "verification": [
            {"command": "make fmt-check", "required": True},
            {"command": command, "required": True},
        ],
        "rollbackNote": "test rollback",
    }


def base_summary(command: str) -> dict:
    return {
        "workItemId": "make_only_test",
        "contractPath": ".ai/work-items/active/make_only_test.contract.json",
        "changedFiles": [{"path": "README.md", "reason": "test change"}],
        "sourcesUsed": ["README.md"],
        "verification": [
            {"command": "make fmt-check", "result": "passed"},
            {"command": command, "result": "passed"},
        ],
        "unknownsRemaining": [],
        "risk": {"level": "low", "detail": "test risk"},
        "generatedFiles": [],
        "destructiveChanges": [],
        "observedIssues": [],
    }


def assert_rejects_contract(command: str) -> None:
    issues = ai_check_work_item.validate_contract(base_contract(command))
    assert_true(
        any("make entrypoint" in issue and command in issue for issue in issues),
        f"contract should reject raw command: {command}",
    )


def assert_rejects_summary(command: str) -> None:
    summary = base_summary(command)
    contract = base_contract("make quality")
    # Summary command policy は Contract required の照合以前に独立して検証する。
    contract["verification"] = [
        {"command": item["command"], "required": True}
        for item in summary["verification"]
    ]
    issues = ai_check_summary.validate_summary(summary, contract)
    assert_true(
        any("make entrypoint" in issue and command in issue for issue in issues),
        f"summary should reject raw command: {command}",
    )


def test_contract_accepts_make_command() -> None:
    assert_equal(
        ai_check_work_item.validate_contract(base_contract("make quality")),
        [],
        "make command should pass",
    )


def test_contract_rejects_raw_commands() -> None:
    for command in (
        "python3 scripts/check_architecture_boundaries.py",
        "cargo test",
        "bash scripts/check_audit_docs.sh",
        "git diff --check",
    ):
        assert_rejects_contract(command)


def test_summary_accepts_make_command() -> None:
    summary = base_summary("make quality")
    contract = base_contract("make quality")
    assert_equal(
        ai_check_summary.validate_summary(summary, contract),
        [],
        "make summary command should pass",
    )


def test_summary_rejects_raw_commands() -> None:
    for command in (
        "python3 scripts/ai_test_architecture_boundaries.py",
        "cargo clippy --all-targets -- -D warnings",
        "bash scripts/check_doc_forbidden_terms.sh",
        "git diff --check",
    ):
        assert_rejects_summary(command)


def main() -> int:
    test_contract_accepts_make_command()
    print("✅ contract_accepts_make_command")
    test_contract_rejects_raw_commands()
    print("✅ contract_rejects_raw_commands")
    test_summary_accepts_make_command()
    print("✅ summary_accepts_make_command")
    test_summary_rejects_raw_commands()
    print("✅ summary_rejects_raw_commands")
    print("✅ verification command make-only tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

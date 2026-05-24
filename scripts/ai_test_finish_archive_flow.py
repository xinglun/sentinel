#!/usr/bin/env python3
"""ai_finish の archive flow を検証する lightweight test。"""

from __future__ import annotations

import sys
import shutil
from pathlib import Path
from unittest.mock import patch

import ai_finish


REPO_ROOT = Path(__file__).resolve().parents[1]


def write_active_task(active: Path, task: str) -> None:
    active.mkdir(parents=True, exist_ok=True)
    (active / f"{task}.contract.json").write_text("{}\n", encoding="utf-8")
    (active / f"{task}.summary.json").write_text("{}\n", encoding="utf-8")


def run_finish(active: Path, args: list[str], failures: set[str] | None = None) -> tuple[int, list[list[str]]]:
    failures = failures or set()
    calls: list[list[str]] = []

    def fake_run(command: list[str]) -> tuple[int, int]:
        calls.append(command)
        command_key = " ".join(command)
        if any(marker in command_key for marker in failures):
            return 1, 1
        return 0, 1

    with (
        patch.object(ai_finish, "PROJECT_ROOT", REPO_ROOT),
        patch.object(ai_finish, "ACTIVE_DIR", active),
        patch.object(ai_finish, "run", fake_run),
        patch.object(sys, "argv", ["ai_finish.py", *args]),
    ):
        code = ai_finish.main()
    return code, calls


def assert_equal(actual: object, expected: object, message: str) -> None:
    if actual != expected:
        raise AssertionError(f"{message}: expected={expected!r}, actual={actual!r}")


def assert_true(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def test_success_archives(active: Path) -> None:
    task = "finish_archive_success"
    write_active_task(active, task)
    code, calls = run_finish(active, ["--task", task, "--skip-quality"])
    assert_equal(code, 0, "success code")
    assert_true(any(call[:2] == ["make", "archive-work-item"] for call in calls), "archive command should run")
    assert_true(
        any(call[:2] == ["make", "check-work-items-lifecycle"] for call in calls),
        "lifecycle check should run after archive",
    )
    assert_true(
        any(call[:2] == ["make", "check-ai-status-consistency"] for call in calls),
        "status consistency check should run after archive",
    )


def test_failure_does_not_archive(active: Path) -> None:
    task = "finish_archive_failure"
    write_active_task(active, task)
    code, calls = run_finish(active, ["--task", task, "--skip-quality"], failures={"check-ai-status"})
    assert_equal(code, 1, "failure code")
    assert_true(not any(call[:2] == ["make", "archive-work-item"] for call in calls), "archive command should not run")
    assert_true(
        not any(call[:2] == ["make", "check-work-items-lifecycle"] for call in calls),
        "lifecycle check should not run when archive is skipped by failed checks",
    )
    assert_true(
        not any(call[:2] == ["make", "check-ai-status-consistency"] for call in calls),
        "status consistency check should not run when archive is skipped by failed checks",
    )


def test_no_archive_keeps_checks_only(active: Path) -> None:
    task = "finish_archive_disabled"
    write_active_task(active, task)
    code, calls = run_finish(active, ["--task", task, "--skip-quality", "--no-archive"])
    assert_equal(code, 0, "no archive code")
    assert_true(not any(call[:2] == ["make", "archive-work-item"] for call in calls), "archive command should be skipped")
    assert_true(
        not any(call[:2] == ["make", "check-work-items-lifecycle"] for call in calls),
        "lifecycle check should be skipped when archive is disabled",
    )
    assert_true(
        not any(call[:2] == ["make", "check-ai-status-consistency"] for call in calls),
        "status consistency check should be skipped when archive is disabled",
    )


def main() -> int:
    cases = [
        ("success_archives", test_success_archives),
        ("failure_does_not_archive", test_failure_does_not_archive),
        ("no_archive_keeps_checks_only", test_no_archive_keeps_checks_only),
    ]
    active = REPO_ROOT / "target" / "ai_finish_test_active"
    shutil.rmtree(active, ignore_errors=True)
    try:
        for name, case in cases:
            case(active)
            print(f"✅ {name}")
    finally:
        shutil.rmtree(active, ignore_errors=True)
    print("✅ finish archive flow tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

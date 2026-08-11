#!/usr/bin/env python3
"""ai-close-work-item の lifecycle 回帰テスト。"""

from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path
from unittest.mock import patch

import ai_close_work_item


RESOLVER = Path.home() / ".codex" / "skills" / "ai-cockpit" / "scripts" / "resolve_make_entrypoint.py"


def assert_true(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def test_resolver_accepts_repository_makefile_with_close_target() -> None:
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        (root / "Makefile").write_text(
            "ai-start:\n\t@true\n"
            "ai-finish:\n\t@true\n"
            "ai-close-work-item:\n\t@true\n",
            encoding="utf-8",
        )
        result = subprocess.run(
            ["python3", str(RESOLVER), str(root)],
            text=True,
            capture_output=True,
            check=False,
        )
        assert_true(result.returncode == 0, result.stderr)
        assert_true(result.stdout.strip() == "make", result.stdout)


def test_close_rejects_active_item_instead_of_archiving_it(tmp_path: Path) -> None:
    active = tmp_path / ".ai/work-items/active"
    archive = tmp_path / ".ai/work-items/archive/2026"
    active.mkdir(parents=True)
    archive.mkdir(parents=True)
    (active / "sample.contract.json").write_text('{"workItemId":"sample"}\n', encoding="utf-8")
    (active / "sample.summary.json").write_text('{}\n', encoding="utf-8")
    (archive / "sample.contract.json").write_text('{"workItemId":"sample"}\n', encoding="utf-8")
    (archive / "sample.summary.json").write_text('{}\n', encoding="utf-8")

    with (
        patch.object(ai_close_work_item, "PROJECT_ROOT", tmp_path),
        patch.object(ai_close_work_item, "ACTIVE_DIR", active),
        patch.object(ai_close_work_item, "ARCHIVE_DIR", tmp_path / ".ai/work-items/archive"),
    ):
        result = ai_close_work_item.validate_archived_work_item("sample")

    assert_true(result, "active residue must block close and preserve finish/archive boundary")
    assert_true(any("active" in issue.lower() for issue in result), str(result))


def test_close_runs_lifecycle_and_status_checks(tmp_path: Path) -> None:
    archive = tmp_path / ".ai/work-items/archive/2026"
    archive.mkdir(parents=True)
    contract = archive / "sample.contract.json"
    summary = archive / "sample.summary.json"
    contract.write_text('{"workItemId":"sample","mode":"code"}\n', encoding="utf-8")
    summary.write_text(
        '{"contractPath":".ai/work-items/archive/2026/sample.contract.json"}\n',
        encoding="utf-8",
    )
    calls: list[list[str]] = []

    def fake_run(command: list[str]) -> int:
        calls.append(command)
        return 0

    with (
        patch.object(ai_close_work_item, "PROJECT_ROOT", tmp_path),
        patch.object(ai_close_work_item, "ACTIVE_DIR", tmp_path / ".ai/work-items/active"),
        patch.object(ai_close_work_item, "ARCHIVE_DIR", tmp_path / ".ai/work-items/archive"),
        patch.object(ai_close_work_item, "run_make_check", side_effect=fake_run),
    ):
        issues = ai_close_work_item.close_work_item("sample")

    assert_true(not issues, str(issues))
    assert_true(calls == [["make", "check-work-items-lifecycle"], ["make", "check-ai-status-consistency"]], str(calls))


def main() -> int:
    test_resolver_accepts_repository_makefile_with_close_target()
    print("✅ resolver_accepts_close_target")
    with tempfile.TemporaryDirectory() as temporary:
        test_close_rejects_active_item_instead_of_archiving_it(Path(temporary))
    print("✅ close_rejects_active_item")
    with tempfile.TemporaryDirectory() as temporary:
        test_close_runs_lifecycle_and_status_checks(Path(temporary))
    print("✅ close_runs_lifecycle_and_status_checks")
    print("✅ ai_close_work_item tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

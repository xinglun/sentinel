#!/usr/bin/env python3
"""ai_start の status 同期を検証する lightweight test。"""

from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path
from unittest.mock import patch

import ai_start


REPO_ROOT = Path(__file__).resolve().parents[1]


def assert_true(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def test_start_generates_active_status(root: Path) -> None:
    active = root / ".ai/work-items/active"
    calls: list[list[str]] = []

    def fake_run(command: list[str], **_: object) -> object:
        calls.append(command)

        class Result:
            returncode = 0

        return Result()

    with (
        patch.object(ai_start, "PROJECT_ROOT", root),
        patch.object(ai_start, "ACTIVE_DIR", active),
        patch.object(ai_start, "current_head", return_value="abc123"),
        patch.object(ai_start, "baseline_dirty_paths", return_value=["scripts/ai_start.py"]),
        patch.object(ai_start.subprocess, "run", fake_run),
        patch.object(
            sys,
            "argv",
            [
                "ai_start.py",
                "--task",
                "sample-start-status",
                "--title",
                "Sample Start Status",
                "--mode",
                "code",
            ],
        ),
    ):
        code = ai_start.main()

    assert_true(code == 0, "ai_start should succeed")
    contract = json.loads((active / "sample-start-status.contract.json").read_text(encoding="utf-8"))
    summary = json.loads((active / "sample-start-status.summary.json").read_text(encoding="utf-8"))
    assert_true(contract["contractVersion"] == 2, "contract should be v2")
    assert_true(contract["baseCommit"] == "abc123", "contract should record baseCommit")
    assert_true(contract["baselineDirtyPaths"] == ["scripts/ai_start.py"], "contract should record baselineDirtyPaths")
    assert_true("checkpointEvidence" in summary, "summary should contain checkpointEvidence")
    assert_true(
        [item["stage"] for item in summary["checkpointEvidence"]] == [
            "contract_start",
            "before_edit",
            "before_ready",
            "after_verification",
        ],
        "summary should prefill checkpointEvidence chain",
    )
    assert_true(
        any("scripts/ai_generate_status.py" in call for call in calls),
        "ai_start should generate active cockpit status after skeleton creation",
    )


def main() -> int:
    root = REPO_ROOT / "target" / "ai_start_test"
    shutil.rmtree(root, ignore_errors=True)
    try:
        test_start_generates_active_status(root)
        print("✅ start_generates_active_status")
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print("✅ ai_start tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

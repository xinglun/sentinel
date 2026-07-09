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


def write_preflight_review(root: Path, status: str) -> None:
    report_path = root / "target/ai_preflight_review.json"
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        json.dumps(
            {
                "status": status,
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )


def test_start_generates_active_status(root: Path) -> None:
    active = root / ".ai/work-items/active"
    calls: list[list[str]] = []

    def fake_run(command: list[str], **_: object) -> object:
        calls.append(command)
        if command[:2] == ["make", "ai-preflight"]:
            write_preflight_review(root, "ready")

        class Result:
            returncode = 0

        return Result()

    with (
        patch.object(ai_start, "PROJECT_ROOT", root),
        patch.object(ai_start, "ACTIVE_DIR", active),
        patch.object(ai_start, "current_head", return_value="abc123"),
        patch.object(
            ai_start,
            "baseline_dirty_paths",
            return_value=[
                {
                    "path": "scripts/ai_start.py",
                    "status": "M",
                    "fingerprint": "deadbeef",
                }
            ],
        ),
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
    assert_true(
        contract["baselineDirtyPaths"]
        == [
            {
                "path": "scripts/ai_start.py",
                "status": "M",
                "fingerprint": "deadbeef",
            }
        ],
        "contract should record baselineDirtyPaths",
    )
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
        sum(1 for call in calls if any(part.endswith("scripts/ai_generate_status.py") for part in call)) >= 2,
        "ai_start should sync active cockpit status before and after preflight",
    )
    preflight_indices = [
        index for index, call in enumerate(calls) if call[:2] == ["make", "ai-preflight"]
    ]
    assert_true(len(preflight_indices) >= 2, "ai_start should run preflight twice in code mode")
    preflight_index = preflight_indices[1]
    status_indices = [
        index
        for index, call in enumerate(calls)
        if any(part.endswith("scripts/ai_generate_status.py") for part in call)
    ]
    assert_true(
        status_indices[0] < preflight_index,
        "ai_start should generate cockpit status before preflight",
    )
    assert_true(
        status_indices[-1] > preflight_index,
        "ai_start should refresh cockpit status after preflight",
    )
    makefile = (REPO_ROOT / "Makefile").read_text(encoding="utf-8")
    assert_true(
        "make check-docs-metadata" in makefile,
        "Makefile should expose the docs metadata compatibility target",
    )
    assert_true(
        "check-rust: fmt-check check-docs-metadata" in makefile,
        "check-rust should route through the docs metadata target",
    )


def test_start_pauses_on_non_ready_preflight(root: Path) -> None:
    active = root / ".ai/work-items/active"
    calls: list[list[str]] = []

    def fake_run(command: list[str], **_: object) -> object:
        calls.append(command)
        if command[:2] == ["make", "ai-preflight"]:
            write_preflight_review(root, "needs_human_confirmation")

        class Result:
            returncode = 0

        return Result()

    with (
        patch.object(ai_start, "PROJECT_ROOT", root),
        patch.object(ai_start, "ACTIVE_DIR", active),
        patch.object(ai_start, "PREFLIGHT_REVIEW_PATH", root / "target" / "ai_preflight_review.json"),
        patch.object(ai_start, "current_head", return_value="abc123"),
        patch.object(
            ai_start,
            "baseline_dirty_paths",
            return_value=[
                {
                    "path": "scripts/ai_start.py",
                    "status": "M",
                    "fingerprint": "deadbeef",
                }
            ],
        ),
        patch.object(ai_start.subprocess, "run", fake_run),
        patch.object(
            sys,
            "argv",
            [
                "ai_start.py",
                "--task",
                "sample-start-pause",
                "--title",
                "Sample Start Pause",
                "--mode",
                "code",
            ],
        ),
    ):
        code = ai_start.main()

    assert_true(code == 1, "ai_start should pause when preflight is not ready")
    assert_true(
        any(command[:2] == ["make", "ai-preflight"] for command in calls),
        "ai_start should still invoke ai-preflight before pausing",
    )


def main() -> int:
    root = REPO_ROOT / "target" / "ai_start_test"
    shutil.rmtree(root, ignore_errors=True)
    try:
        test_start_generates_active_status(root)
        print("✅ start_generates_active_status")
        shutil.rmtree(root, ignore_errors=True)
        test_start_pauses_on_non_ready_preflight(root)
        print("✅ start_pauses_on_non_ready_preflight")
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print("✅ ai_start tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Retry Circuit Breaker の境界条件を検証する lightweight test。"""

from __future__ import annotations

import json
import tempfile
from pathlib import Path
from typing import Any

from ai_generate_status import consecutive_failure_count, status_for


WORK_ITEM_ID = "loop_task"


def write_event(log_path: Path, event_type: str, work_item_id: str = WORK_ITEM_ID) -> None:
    """観測 JSONL に最小イベントを書き込む。"""
    event: dict[str, Any] = {
        "timestamp": "2026-05-08T00:00:00+00:00",
        "eventType": event_type,
        "level": "info",
        "message": event_type,
        "workItemId": work_item_id,
    }
    with log_path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(event, separators=(",", ":")) + "\n")


def ready_contract() -> dict[str, Any]:
    """status_for 用の最小 Contract を返す。"""
    return {
        "workItemId": WORK_ITEM_ID,
        "mode": "code",
        "notCodable": False,
        "unknowns": [],
        "verification": [],
    }


def ready_summary() -> dict[str, Any]:
    """status_for 用の最小 Summary を返す。"""
    return {
        "workItemId": WORK_ITEM_ID,
        "verification": [],
    }


def assert_equal(actual: object, expected: object, message: str) -> None:
    if actual != expected:
        raise AssertionError(f"{message}: expected={expected!r}, actual={actual!r}")


def test_below_threshold_keeps_ready(log_path: Path) -> None:
    for _ in range(4):
        write_event(log_path, "check_failed")
    state, blockers = status_for(
        ready_contract(),
        ready_summary(),
        retry_threshold=5,
        observability_log=log_path,
    )
    assert_equal(consecutive_failure_count(WORK_ITEM_ID, log_path), 4, "below threshold count")
    assert_equal(state, "ready_for_review", "below threshold state")
    assert_equal(blockers, [], "below threshold blockers")


def test_threshold_blocks(log_path: Path) -> None:
    for _ in range(5):
        write_event(log_path, "check_failed")
    state, blockers = status_for(
        ready_contract(),
        ready_summary(),
        retry_threshold=5,
        observability_log=log_path,
    )
    assert_equal(consecutive_failure_count(WORK_ITEM_ID, log_path), 5, "threshold count")
    assert_equal(state, "blocked_by_ai_loop", "threshold state")
    if not blockers or "consecutive check failures 5/5" not in blockers[0]:
        raise AssertionError(f"unexpected blockers: {blockers!r}")


def test_status_generated_does_not_reset(log_path: Path) -> None:
    for _ in range(3):
        write_event(log_path, "check_failed")
    write_event(log_path, "status_generated")
    for _ in range(2):
        write_event(log_path, "check_failed")
    assert_equal(consecutive_failure_count(WORK_ITEM_ID, log_path), 5, "status event should not reset")


def test_check_passed_resets(log_path: Path) -> None:
    for _ in range(5):
        write_event(log_path, "check_failed")
    write_event(log_path, "check_passed")
    assert_equal(consecutive_failure_count(WORK_ITEM_ID, log_path), 0, "passed should reset")
    state, blockers = status_for(
        ready_contract(),
        ready_summary(),
        retry_threshold=5,
        observability_log=log_path,
    )
    assert_equal(state, "ready_for_review", "state after reset")
    assert_equal(blockers, [], "blockers after reset")


def test_other_work_item_is_ignored(log_path: Path) -> None:
    for _ in range(5):
        write_event(log_path, "check_failed", work_item_id="other_task")
    assert_equal(consecutive_failure_count(WORK_ITEM_ID, log_path), 0, "other Work Item should be ignored")


def run_case(name: str, case) -> None:  # type: ignore[no-untyped-def]
    with tempfile.TemporaryDirectory() as tmp:
        log_path = Path(tmp) / "ai_observability.jsonl"
        case(log_path)
    print(f"✅ {name}")


def main() -> int:
    cases = [
        ("below_threshold_keeps_ready", test_below_threshold_keeps_ready),
        ("threshold_blocks", test_threshold_blocks),
        ("status_generated_does_not_reset", test_status_generated_does_not_reset),
        ("check_passed_resets", test_check_passed_resets),
        ("other_work_item_is_ignored", test_other_work_item_is_ignored),
    ]
    for name, case in cases:
        run_case(name, case)
    print("✅ retry circuit breaker tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

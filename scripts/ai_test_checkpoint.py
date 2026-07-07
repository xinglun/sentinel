#!/usr/bin/env python3
"""ai_checkpoint の checkpoint 表示回帰テスト。"""

from __future__ import annotations

import ai_checkpoint


def assert_equal(actual: object, expected: object, message: str) -> None:
    if actual != expected:
        raise AssertionError(f"{message}: expected={expected!r}, actual={actual!r}")


def test_intent_context_defaults_when_intent_is_missing() -> None:
    assert_equal(
        ai_checkpoint.intent_context({"workItemId": "task"}),
        [
            "problem: not provided",
            "constraint: not provided",
            "rationale: not provided",
        ],
        "missing intent should fall back to placeholders",
    )


def test_intent_context_keeps_values_and_default_placeholders() -> None:
    assert_equal(
        ai_checkpoint.intent_context(
            {
                "intent": {
                    "problem": "Resolve optional intent compatibility.",
                    "constraints": ["Keep V2 backward compatible."],
                }
            }
        ),
        [
            "problem: Resolve optional intent compatibility.",
            "constraint: Keep V2 backward compatible.",
            "rationale: not provided",
        ],
        "intent context should keep explicit values",
    )


def test_checkpoint_snapshot_uses_required_counts() -> None:
    contract = {
        "acceptance": ["a", "b"],
        "unknowns": ["u"],
        "verification": [
            {"command": "make fmt-check", "required": True},
            {"command": "make test", "required": True},
        ],
    }
    summary = {
        "contractPath": ".ai/work-items/active/sample.contract.json",
        "verification": [
            {"command": "make fmt-check", "result": "passed"},
            {"command": "make test", "result": "failed"},
        ],
    }
    snapshot = ai_checkpoint.checkpoint_snapshot(contract, summary, stage="before_ready")
    assert_equal(snapshot["stage"], "before_ready", "stage should be echoed")
    assert_equal(snapshot["acceptanceCount"], 2, "acceptance count should be derived from contract")
    assert_equal(snapshot["unknownCount"], 1, "unknown count should be derived from contract")
    assert_equal(snapshot["requiredChecks"], 2, "required check count should be derived from contract")
    assert_equal(snapshot["requiredChecksPassed"], 1, "passed required checks should be counted from summary")


def main() -> int:
    test_intent_context_defaults_when_intent_is_missing()
    print("✅ intent_context_defaults_when_intent_is_missing")
    test_intent_context_keeps_values_and_default_placeholders()
    print("✅ intent_context_keeps_values_and_default_placeholders")
    test_checkpoint_snapshot_uses_required_counts()
    print("✅ checkpoint_snapshot_uses_required_counts")
    print("✅ ai_checkpoint tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Scenario Coverage と duplicate key rejection の回帰テスト。"""

from __future__ import annotations

import shutil
import tempfile
from pathlib import Path

import ai_check_scenario_coverage
import ai_json


def assert_equal(actual: object, expected: object, message: str) -> None:
    if actual != expected:
        raise AssertionError(f"{message}: expected={expected!r}, actual={actual!r}")


def assert_true(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def sample_contract(level: str) -> dict:
    return {
        "contractVersion": 2,
        "workItemId": "scenario_coverage_test",
        "mode": "code",
        "title": "scenario coverage test",
        "baseCommit": "deadbeefdeadbeef",
        "baselineDirtyPaths": [],
        "problemStatement": "test problem",
        "intent": {},
        "scope": ["README.md"],
        "outOfScope": [],
        "sources": [{"path": "README.md", "reason": "test source"}],
        "unknowns": [],
        "notCodable": False,
        "riskAssessment": {
            "level": level,
            "riskTypes": ["governance_process"],
            "reason": "test risk",
        },
        "agentCapability": {
            "canImplement": True,
            "canVerify": True,
            "needsHumanDecision": False,
            "blockedReason": "",
        },
        "executionDecision": {
            "status": "continue",
            "reason": "test",
        },
        "preReviewWarnings": [],
        "checkpointPolicy": {
            "requiredCheckpoints": ["contract_start", "before_edit", "before_ready", "after_verification"],
            "reminder": "test",
        },
        "acceptance": ["test acceptance"],
        "verification": [{"command": "make fmt-check", "required": True}],
        "destructiveChangePolicy": {
            "allowed": False,
            "requiresHumanApproval": True,
            "allowPatterns": [],
        },
        "rollbackNote": "test rollback",
    }


def sample_summary(scenario_coverage: list[dict] | None) -> dict:
    summary = {
        "workItemId": "scenario_coverage_test",
        "contractPath": ".ai/work-items/active/scenario_coverage_test.contract.json",
        "changedFiles": [{"path": "README.md", "reason": "test change"}],
        "sourcesUsed": ["README.md"],
        "verification": [{"command": "make fmt-check", "result": "passed"}],
        "unknownsRemaining": [],
        "risk": {"level": "low", "detail": "test risk"},
        "generatedFiles": [],
        "destructiveChanges": [],
        "observedIssues": [],
    }
    if scenario_coverage is not None:
        summary["scenarioCoverage"] = scenario_coverage
    return summary


def test_duplicate_key_rejection(root: Path) -> None:
    path = root / "duplicate.json"
    path.write_text('{"key": 1, "key": 2}\n', encoding="utf-8")
    try:
        ai_json.load_json(path)
    except ValueError as exc:
        assert_true("duplicate key" in str(exc), "duplicate key should be rejected")
    else:
        raise AssertionError("duplicate key JSON should fail")


def test_scenario_coverage_validation() -> None:
    coverage = [
        {
            "scenario": "verified scenario",
            "required": True,
            "status": "verified",
            "evidence": ["make check-ai-scenario-coverage"],
        },
        {
            "scenario": "not applicable scenario",
            "required": False,
            "status": "not_applicable",
            "evidence": [],
            "reason": "not relevant",
        },
    ]
    assert_equal(ai_check_scenario_coverage.detect(sample_contract("low"), sample_summary(coverage)), [], "valid coverage should pass")
    assert_equal(
        ai_check_scenario_coverage.detect(sample_contract("low"), sample_summary(None))[0].kind,
        "not_required",
        "low risk without coverage should be not_required",
    )


def test_scenario_coverage_state() -> None:
    coverage = [
        {
            "scenario": "verified scenario",
            "required": True,
            "status": "verified",
            "evidence": ["make check-ai-scenario-coverage"],
        }
    ]
    assert_equal(
        ai_check_scenario_coverage.scenario_coverage_state(sample_contract("medium"), sample_summary(coverage)),
        "complete",
        "verified required coverage should be complete",
    )
    assert_equal(
        ai_check_scenario_coverage.scenario_coverage_state(sample_contract("medium"), sample_summary(None)),
        "incomplete",
        "medium risk without coverage should be incomplete",
    )
    assert_equal(
        ai_check_scenario_coverage.scenario_coverage_state(sample_contract("low"), sample_summary(None)),
        "not_required",
        "low risk without coverage should be not_required",
    )
    assert_equal(
        ai_check_scenario_coverage.scenario_coverage_state(sample_contract("low"), None),
        "unknown",
        "missing summary should be unknown",
    )


def main() -> int:
    root = Path(tempfile.mkdtemp(prefix="scenario_coverage_test_"))
    try:
        test_duplicate_key_rejection(root)
        print("✅ duplicate_key_rejection")
        test_scenario_coverage_validation()
        print("✅ scenario_coverage_validation")
        test_scenario_coverage_state()
        print("✅ scenario_coverage_state")
    finally:
        shutil.rmtree(root, ignore_errors=True)
    print("✅ scenario coverage tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

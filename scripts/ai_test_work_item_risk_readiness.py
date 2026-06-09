#!/usr/bin/env python3
"""Work Item risk / review readiness field の回帰テスト。"""

from __future__ import annotations

from ai_check_summary import validate_summary
from ai_check_work_item import validate_contract
from ai_generate_status import status_for


def base_contract() -> dict:
    return {
        "contractVersion": 1,
        "workItemId": "risk-readiness-test",
        "mode": "code",
        "title": "Risk readiness test",
        "scope": ["src/example.rs"],
        "outOfScope": [],
        "sources": [{"path": "docs/spec.md", "reason": "仕様根拠。"}],
        "unknowns": [],
        "notCodable": False,
        "riskAssessment": {
            "level": "medium",
            "riskTypes": ["review_debt"],
            "reason": "review で確認すべき残余リスクがある。",
        },
        "agentCapability": {
            "canImplement": True,
            "canVerify": True,
            "needsHumanDecision": False,
            "blockedReason": "",
        },
        "executionDecision": {
            "status": "continue",
            "reason": "Contract が確定している。",
        },
        "preReviewWarnings": ["review focus を Summary に残す。"],
        "acceptance": ["risk readiness を検証できる。"],
        "verification": [{"command": "make fmt-check", "required": True}],
        "rollbackNote": "この test fixture を戻す。",
    }


def base_summary() -> dict:
    return {
        "workItemId": "risk-readiness-test",
        "contractPath": ".ai/work-items/active/risk-readiness-test.contract.json",
        "changedFiles": [{"path": "src/example.rs", "reason": "test fixture。"}],
        "sourcesUsed": ["docs/spec.md"],
        "verification": [{"command": "make fmt-check", "result": "passed"}],
        "unknownsRemaining": [],
        "risk": {"level": "medium", "detail": "review focus が残る。"},
        "generatedFiles": [],
        "destructiveChanges": [],
        "observedIssues": [],
        "residualRisks": [
            {
                "level": "medium",
                "area": "review_debt",
                "detail": "review で境界を再確認する。",
                "reviewRecommended": True,
                "followUpCandidate": True,
            }
        ],
        "reviewReadiness": {
            "status": "ready_with_risks",
            "reason": "required checks は通過したが review focus が残る。",
            "expectedReviewFocus": ["scope boundary"],
        },
        "userCorrectionsCaptured": ["risk channel を合法化する。"],
        "userCorrectionSolidification": [
            {
                "correction": "risk channel を合法化する。",
                "solidifiedTo": "guard",
                "reason": "Summary guard で固化する。",
            }
        ],
    }


def main() -> int:
    contract = base_contract()
    summary = base_summary()

    assert validate_contract(contract) == []
    assert validate_summary(summary, contract) == []
    state, blockers = status_for(contract, summary, retry_threshold=0)
    assert state == "ready_with_risks"
    assert blockers == []

    blocked_contract = base_contract()
    blocked_contract["executionDecision"] = {"status": "blocked", "reason": "human decision required"}
    assert any("executionDecision.status" in issue for issue in validate_contract(blocked_contract))

    incomplete_summary = base_summary()
    del incomplete_summary["userCorrectionSolidification"]
    assert any("userCorrectionSolidification" in issue for issue in validate_summary(incomplete_summary, contract))

    print("✅ Work Item risk readiness tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Work Item risk / review readiness field の回帰テスト。"""

from __future__ import annotations

from ai_check_summary import validate_summary
from ai_check_work_item import validate_contract
from ai_generate_status import status_for


def base_contract() -> dict:
    return {
        "contractVersion": 2,
        "workItemId": "risk-readiness-test",
        "mode": "code",
        "title": "Risk readiness test",
        "baseCommit": "deadbeefdeadbeef",
        "baselineDirtyPaths": [],
        "scope": ["src/example.rs"],
        "outOfScope": [],
        "sources": [{"path": "docs/spec.md", "reason": "仕様根拠。"}],
        "problemStatement": "review で確認すべき残余リスクを明示する。",
        "intent": {
            "problem": "risk readiness を検証する。",
            "constraints": ["summary の checkpointEvidence を記録する。"],
            "rationale": "checkpoint chain と review readiness の両方を保護する。",
        },
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
        "checkpointPolicy": {
            "requiredCheckpoints": [
                "contract_start",
                "before_edit",
                "before_ready",
                "after_verification",
            ],
            "reminder": "Contract と Summary を更新してから進める。",
        },
        "acceptance": ["risk readiness を検証できる。"],
        "verification": [
            {"command": "make check-ai-contract CONTRACT=.ai/work-items/active/risk-readiness-test.contract.json", "required": True},
            {"command": "make check-ai-scope CONTRACT=.ai/work-items/active/risk-readiness-test.contract.json", "required": True},
            {"command": "make fmt-check", "required": True},
            {"command": "make check-ai-guards CONTRACT=.ai/work-items/active/risk-readiness-test.contract.json", "required": True},
            {
                "command": "make check-ai-backtrack CONTRACT=.ai/work-items/active/risk-readiness-test.contract.json SUMMARY=.ai/work-items/active/risk-readiness-test.summary.json",
                "required": True,
            },
            {
                "command": "make check-ai-change-summary SUMMARY=.ai/work-items/active/risk-readiness-test.summary.json CONTRACT=.ai/work-items/active/risk-readiness-test.contract.json",
                "required": True,
            },
            {
                "command": "make generate-cockpit-status CONTRACT=.ai/work-items/active/risk-readiness-test.contract.json SUMMARY=.ai/work-items/active/risk-readiness-test.summary.json",
                "required": True,
            },
            {
                "command": "make check-ai-status CONTRACT=.ai/work-items/active/risk-readiness-test.contract.json SUMMARY=.ai/work-items/active/risk-readiness-test.summary.json",
                "required": True,
            },
        ],
        "rollbackNote": "この test fixture を戻す。",
    }


def base_summary() -> dict:
    return {
        "workItemId": "risk-readiness-test",
        "contractPath": ".ai/work-items/active/risk-readiness-test.contract.json",
        "changedFiles": [{"path": "src/example.rs", "reason": "test fixture。"}],
        "sourcesUsed": ["docs/spec.md"],
        "verification": [
            {"command": item["command"], "result": "passed"}
            for item in base_contract()["verification"]
        ],
        "unknownsRemaining": [],
        "risk": {"level": "medium", "detail": "review focus が残る。"},
        "generatedFiles": [],
        "destructiveChanges": [],
        "observedIssues": [],
        "checkpointEvidence": [
            {
                "stage": stage,
                "recorded": True,
                "detail": "fixture checkpoint",
                "contractHash": "deadbeefdeadbeef",
                "acceptanceCount": 1,
                "unknownCount": 0,
                "requiredChecks": len(base_contract()["verification"]),
                "requiredChecksPassed": len(base_contract()["verification"]),
            }
            for stage in [
                "contract_start",
                "before_edit",
                "before_ready",
                "after_verification",
            ]
        ],
        "checkpointReview": [
            {
                "checkpoint": "contract_start",
                "status": "confirmed",
                "note": "Contract を確認した。",
            },
            {
                "checkpoint": "before_edit",
                "status": "confirmed",
                "note": "scope を確認した。",
            },
            {
                "checkpoint": "before_ready",
                "status": "confirmed",
                "note": "Summary を更新した。",
            },
            {
                "checkpoint": "after_verification",
                "status": "confirmed",
                "note": "verification を同期した。",
            },
        ],
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

    missing_gate_contract = base_contract()
    missing_gate_contract["verification"] = [{"command": "make fmt-check", "required": True}]
    assert any("AI gate" in issue for issue in validate_contract(missing_gate_contract))

    missing_checkpoint_contract = base_contract()
    del missing_checkpoint_contract["checkpointPolicy"]
    assert any("checkpointPolicy" in issue for issue in validate_contract(missing_checkpoint_contract))

    incapable_contract = base_contract()
    incapable_contract["agentCapability"]["canVerify"] = False
    assert any("canVerify" in issue for issue in validate_contract(incapable_contract))

    incomplete_summary = base_summary()
    del incomplete_summary["userCorrectionSolidification"]
    assert any("userCorrectionSolidification" in issue for issue in validate_summary(incomplete_summary, contract))

    missing_checkpoint_summary = base_summary()
    del missing_checkpoint_summary["checkpointReview"]
    assert any("checkpointReview" in issue for issue in validate_summary(missing_checkpoint_summary, contract))

    print("✅ Work Item risk readiness tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

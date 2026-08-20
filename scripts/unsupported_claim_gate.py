"""Deterministic gate for unsupported external claims, not model-internal states."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

POLICY = "unsupported-claim-evidence-policy"
NEGATIVE_CASES = (
    "confident_without_evidence",
    "not_run_as_passed",
    "failed_as_passed",
    "inference_as_fact",
    "missing_file",
    "approval_without_decision",
    "simulation_as_real",
)
POSITIVE_CASES = ("supported_claim", "approved_claim")


def evaluate_claim(claim: dict[str, Any], *, root: Path) -> dict[str, Any]:
    evidence = claim.get("evidence")
    reasons: list[str] = []
    if not isinstance(evidence, list) or not evidence:
        reasons.append("claim has no evidence")
    else:
        for item in evidence:
            if not isinstance(item, dict):
                reasons.append("evidence item is not structured")
                continue
            if item.get("status") == "not_run":
                reasons.append("claim cites a check that was not_run")
            if item.get("status") in {"failed", "blocked", "error"}:
                reasons.append("claim cites a non-passing check")
            if item.get("kind") == "inference":
                reasons.append("inference is presented as fact")
            if item.get("kind") == "simulation":
                reasons.append("simulation is presented as real execution")
            path = item.get("path")
            if path is not None and not (root / str(path)).exists():
                reasons.append("cited evidence file does not exist")
            if item.get("kind") == "approval" and item.get("approved") is not True:
                reasons.append("approval evidence is absent")
    blocked = bool(reasons)
    return {
        "state": "blocked" if blocked else "allowed",
        "reason": "; ".join(reasons) if reasons else "claim is supported by structured evidence",
        "evidence": evidence if isinstance(evidence, list) else [],
        "resumeCondition": "Provide fresh, independently verifiable evidence before claiming completion."
        if blocked
        else "none",
        "policyReference": POLICY,
    }


def run_regression(root: Path) -> dict[str, Any]:
    cases: list[dict[str, Any]] = [
        {"name": "confident_without_evidence", "claim": {}},
        {"name": "not_run_as_passed", "claim": {"evidence": [{"status": "not_run"}]}},
        {"name": "failed_as_passed", "claim": {"evidence": [{"status": "failed"}]}},
        {"name": "inference_as_fact", "claim": {"evidence": [{"kind": "inference"}]}},
        {"name": "missing_file", "claim": {"evidence": [{"path": "missing-evidence.json"}]}},
        {
            "name": "approval_without_decision",
            "claim": {"evidence": [{"kind": "approval", "approved": False}]},
        },
        {"name": "simulation_as_real", "claim": {"evidence": [{"kind": "simulation"}]}},
        {
            "name": "supported_claim",
            "claim": {
                "evidence": [{"path": "scripts/unsupported_claim_gate.py", "status": "passed"}]
            },
        },
        {
            "name": "approved_claim",
            "claim": {
                "evidence": [
                    {
                        "kind": "approval",
                        "approved": True,
                        "path": "scripts/unsupported_claim_gate.py",
                        "status": "passed",
                    }
                ]
            },
        },
    ]
    results = [{"name": case["name"], **evaluate_claim(case["claim"], root=root)} for case in cases]
    return {
        "gate": "Unsupported Claim Regression Gate",
        "policyReference": POLICY,
        "results": results,
    }


def main() -> int:
    report = run_regression(Path(__file__).resolve().parents[1])
    expected = {item["name"]: item["state"] for item in report["results"]}
    if any(expected.get(name) != "blocked" for name in NEGATIVE_CASES) or any(
        expected.get(name) != "allowed" for name in POSITIVE_CASES
    ):
        print(json.dumps(report, indent=2, sort_keys=True))
        return 1
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

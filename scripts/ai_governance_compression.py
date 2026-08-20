#!/usr/bin/env python3
"""Derive Cockpit governance-compression status from Contract and Summary.

The module is intentionally pure: it consumes already-loaded Contract and
Summary dictionaries and returns a structured status model without any file I/O.
Rendering helpers are kept separate so the model can be tested directly.
"""

from __future__ import annotations

from datetime import UTC, datetime
from typing import Any

from ai_acceptance_policy import acceptance_signal
from ai_calibration_inventory import STATUS_VALUES
from ai_common import PROJECT_ROOT, non_empty_string, simple_yaml_lists, verification_key
from ai_intent_policy import intent_alignment_signal
from ai_review_readiness_policy import review_readiness_signal
from ai_risk_policy import residual_risk_signal
from ai_scenario_policy import has_risk_ack, is_hard_risk, scenario_items
from ai_verification_policy import verification_signal

RECOMMENDATIONS = {
    "ready_for_review",
    "ready_with_risks",
    "needs_investigation",
    "blocked",
}

SCENARIO_COVERAGE_POLICY = PROJECT_ROOT / ".ai" / "guards" / "scenario_coverage_policy.yaml"
DEFAULT_HARD_SCENARIO_RISK_TYPES = {
    "release",
    "release_distribution",
    "installer",
    "auth",
    "ci",
    "migration",
    "security",
    "api_change",
}

SIGNAL_ORDER = (
    "Intent",
    "Acceptance",
    "Unknowns",
    "Verification",
    "Scenario Coverage",
    "Guidelines",
    "Checkpoints",
    "Residual Risk",
)

VALID_SIGNAL_VALUES = {
    "Intent": {"resolved", "unresolved", "unknown", "not_applicable"},
    "Acceptance": {"complete", "incomplete", "unknown"},
    "Unknowns": {"resolved", "open", "unknown"},
    "Verification": {"passed", "failed", "incomplete"},
    "Scenario Coverage": {"complete", "incomplete", "not_required", "unknown"},
    "Guidelines": {"satisfied", "violated", "unknown"},
    "Checkpoints": {"complete", "incomplete", "not_required"},
    "Residual Risk": {"low", "medium", "high", "unknown"},
}

HUMAN_SIGNAL_COLORS = {
    "ready_for_review": "Green",
    "ready_with_risks": "Yellow",
    "needs_investigation": "Red",
    "blocked": "Red",
}


def human_signal(model: dict[str, Any]) -> dict[str, Any]:
    """Return a short, evidence-derived reviewer conclusion.

    This is deliberately a semantic color, not a score or confidence estimate.
    The recommendation remains the canonical decision value; this view only
    makes its meaning and next action easier to find.
    """
    recommendation = model.get("recommendation", "needs_investigation")
    color = HUMAN_SIGNAL_COLORS.get(recommendation, "Red")
    conclusions = {
        "ready_for_review": "Evidence is sufficient for human review.",
        "ready_with_risks": "Review may continue only with the recorded residual risks understood.",
        "needs_investigation": "Evidence is incomplete or ambiguous; investigate before proceeding.",
        "blocked": "A hard blocker prevents progression; stop until it is resolved.",
    }
    next_actions = {
        "ready_for_review": "Review the evidence and make the human commit or merge decision.",
        "ready_with_risks": "Read Residual Risk and Decision Drivers, then explicitly decide whether to proceed.",
        "needs_investigation": "Resolve the Decision Drivers and rerun the required checks.",
        "blocked": "Stop; resolve the blocker and regenerate the evidence-backed status.",
    }
    evidence = model.get("evidence", {})
    evidence_keys = [
        EVIDENCE_LABELS.get(key, key)
        for key in (
            "contract",
            "summary",
            "verification",
            "scenarioCoverage",
            "intentAlignment",
            "guidelines",
            "checkpoints",
            "residualRisk",
            "reviewReadiness",
        )
        if isinstance(evidence, dict) and evidence.get(key)
    ]
    return {
        "color": color,
        "recommendation": recommendation,
        "conclusion": conclusions.get(recommendation, conclusions["needs_investigation"]),
        "evidenceBasis": evidence_keys,
        "nextAction": next_actions.get(recommendation, next_actions["needs_investigation"]),
    }


EVIDENCE_LABELS = {
    "contract": "Contract",
    "summary": "Summary",
    "verification": "Verification",
    "scenarioCoverage": "Scenario Coverage",
    "intentAlignment": "Intent Alignment",
    "guidelines": "Guidelines",
    "checkpoints": "Checkpoints",
    "residualRisk": "Residual Risk",
    "reviewReadiness": "Review Readiness",
}


def _string_list(values: Any) -> list[str]:
    if not isinstance(values, list):
        return []
    return [item.strip() for item in values if isinstance(item, str) and item.strip()]


def _dict(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _summary_or_empty(summary: dict[str, Any] | None) -> dict[str, Any]:
    return summary if isinstance(summary, dict) else {}


def _has_meaningful_intent(contract: dict[str, Any]) -> bool:
    intent = contract.get("intent")
    if not isinstance(intent, dict):
        return False
    for key in ("businessGoal", "userGoal", "problem", "rationale"):
        if non_empty_string(intent.get(key)):
            return True
    for key in ("constraints", "nonGoals"):
        if _string_list(intent.get(key)):
            return True
    return False


def _required_checks(contract: dict[str, Any]) -> list[str]:
    return [
        verification_key(item)
        for item in contract.get("verification", [])
        if isinstance(item, dict) and item.get("required") is True and verification_key(item)
    ]


def _verification_index(summary: dict[str, Any]) -> dict[str, str]:
    index: dict[str, str] = {}
    for item in summary.get("verification", []):
        if isinstance(item, dict):
            key = verification_key(item)
            result = item.get("result")
            if key and isinstance(result, str):
                index[key] = result
    return index


def _guidelines_index(summary: dict[str, Any]) -> dict[str, bool]:
    index: dict[str, bool] = {}
    for item in summary.get("guidelinesCompliance", []):
        if (
            isinstance(item, dict)
            and non_empty_string(item.get("guideline"))
            and isinstance(item.get("compliant"), bool)
        ):
            index[str(item["guideline"])] = bool(item["compliant"])
    return index


def _checkpoint_stages(summary: dict[str, Any]) -> set[str]:
    stages: set[str] = set()
    for item in summary.get("checkpointEvidence", []):
        if (
            isinstance(item, dict)
            and non_empty_string(item.get("stage"))
            and item.get("recorded") is True
        ):
            stages.add(str(item["stage"]))
    return stages


def _max_risk_level(levels: list[str]) -> str:
    if "high" in levels:
        return "high"
    if "medium" in levels:
        return "medium"
    if "low" in levels:
        return "low"
    return "unknown"


def _risk_levels(summary: dict[str, Any]) -> list[str]:
    levels: list[str] = []
    risk = summary.get("risk")
    if isinstance(risk, dict) and risk.get("level") in {"low", "medium", "high"}:
        levels.append(str(risk["level"]))
    for item in summary.get("residualRisks", []):
        if isinstance(item, dict) and item.get("level") in {"low", "medium", "high"}:
            levels.append(str(item["level"]))
    return levels


def _scenario_coverage_hard_risk(contract: dict[str, Any]) -> bool:
    policy_hard_types = set(simple_yaml_lists(SCENARIO_COVERAGE_POLICY).get("hardRiskTypes", []))
    return is_hard_risk(contract, policy_hard_types or DEFAULT_HARD_SCENARIO_RISK_TYPES)


def _scenario_coverage_explicit_risk_ack(summary: dict[str, Any] | None) -> bool:
    return has_risk_ack(summary)


def _scenario_coverage_signal(
    contract: dict[str, Any], summary: dict[str, Any] | None
) -> dict[str, Any]:
    risk = _dict(contract.get("riskAssessment"))
    level = risk.get("level")
    if summary is None:
        return {
            "value": "unknown",
            "evidence": ["summary is missing"],
            "sources": ["contract.riskAssessment", "summary.scenarioCoverage"],
            "required": [],
            "verified": [],
            "unverified": [],
            "not_applicable": [],
            "hardRisk": _scenario_coverage_hard_risk(contract),
        }
    items = scenario_items(summary)
    hard_risk = _scenario_coverage_hard_risk(contract)

    if not items:
        if level == "low":
            return {
                "value": "not_required",
                "evidence": ["scenario coverage is not required for low-risk Work Items"],
                "sources": ["contract.riskAssessment", "summary.scenarioCoverage"],
                "required": [],
                "verified": [],
                "unverified": [],
                "not_applicable": [],
                "hardRisk": hard_risk,
            }
        return {
            "value": "incomplete",
            "evidence": ["scenario coverage is missing for medium/high risk"],
            "sources": ["contract.riskAssessment", "summary.scenarioCoverage"],
            "required": [],
            "verified": [],
            "unverified": [],
            "not_applicable": [],
            "hardRisk": hard_risk,
        }

    required: list[str] = []
    verified: list[str] = []
    unverified: list[str] = []
    not_applicable: list[str] = []
    invalid: list[str] = []

    for item in items:
        scenario = item.get("scenario")
        scenario_name = str(scenario).strip() if isinstance(scenario, str) else ""
        if not scenario_name or item.get("required") is not True:
            if scenario_name and item.get("required") is False:
                continue
            if not scenario_name:
                invalid.append("<missing scenario>")
            continue
        required.append(scenario_name)
        status = item.get("status")
        evidence = item.get("evidence")
        reason = item.get("reason")
        if status == "verified":
            if isinstance(evidence, list) and evidence:
                verified.append(scenario_name)
            else:
                invalid.append(scenario_name)
        elif status == "unverified":
            unverified.append(scenario_name)
        elif status == "not_applicable":
            if isinstance(reason, str) and reason.strip():
                not_applicable.append(scenario_name)
            else:
                invalid.append(scenario_name)
        else:
            invalid.append(scenario_name)

    if invalid:
        return {
            "value": "incomplete",
            "evidence": [f"scenario coverage has invalid required item(s): {', '.join(invalid)}"],
            "sources": [
                "contract.riskAssessment",
                "summary.scenarioCoverage",
                "summary.reviewReadiness",
            ],
            "required": required,
            "verified": verified,
            "unverified": unverified,
            "not_applicable": not_applicable,
            "hardRisk": hard_risk,
        }

    if not required:
        if level == "low":
            return {
                "value": "not_required",
                "evidence": [
                    "scenario coverage is optional because no required scenarios were declared"
                ],
                "sources": ["contract.riskAssessment", "summary.scenarioCoverage"],
                "required": [],
                "verified": [],
                "unverified": [],
                "not_applicable": [],
                "hardRisk": hard_risk,
            }
        return {
            "value": "incomplete",
            "evidence": [
                "scenario coverage has no required scenarios declared for medium/high risk"
            ],
            "sources": ["contract.riskAssessment", "summary.scenarioCoverage"],
            "required": [],
            "verified": verified,
            "unverified": unverified,
            "not_applicable": not_applicable,
            "hardRisk": hard_risk,
        }

    if unverified:
        return {
            "value": "incomplete",
            "evidence": [f"required scenario unverified: {', '.join(unverified)}"],
            "sources": [
                "contract.riskAssessment",
                "summary.scenarioCoverage",
                "summary.followUps",
                "summary.unverifiedScenarios",
            ],
            "required": required,
            "verified": verified,
            "unverified": unverified,
            "not_applicable": not_applicable,
            "hardRisk": hard_risk,
        }

    return {
        "value": "complete",
        "evidence": [
            f"scenario coverage complete: {len(verified)} verified, {len(not_applicable)} not_applicable"
        ],
        "sources": ["contract.riskAssessment", "summary.scenarioCoverage"],
        "required": required,
        "verified": verified,
        "unverified": [],
        "not_applicable": not_applicable,
        "hardRisk": hard_risk,
    }


def _unknowns_signal(contract: dict[str, Any], summary: dict[str, Any] | None) -> dict[str, Any]:
    contract_unknowns = _string_list(contract.get("unknowns"))
    summary_unknowns = _string_list(_summary_or_empty(summary).get("unknownsRemaining"))
    if summary is None:
        return {
            "value": "unknown" if not contract_unknowns else "open",
            "evidence": ["summary is missing"],
            "sources": ["contract.unknowns", "summary.unknownsRemaining"],
        }
    if contract_unknowns or summary_unknowns:
        return {
            "value": "open",
            "evidence": [
                f"contract unknowns: {len(contract_unknowns)}",
                f"summary unknownsRemaining: {len(summary_unknowns)}",
            ],
            "sources": ["contract.unknowns", "summary.unknownsRemaining"],
        }
    return {
        "value": "resolved",
        "evidence": ["no open unknowns recorded"],
        "sources": ["contract.unknowns", "summary.unknownsRemaining"],
    }


def _guidelines_signal(contract: dict[str, Any], summary: dict[str, Any] | None) -> dict[str, Any]:
    guidelines = _string_list(contract.get("guidelines"))
    if not guidelines:
        return {
            "value": "satisfied",
            "evidence": ["no contract guidelines declared"],
            "sources": ["contract.guidelines", "summary.guidelinesCompliance"],
        }

    if summary is None:
        return {
            "value": "unknown",
            "evidence": ["summary is missing"],
            "sources": ["contract.guidelines", "summary.guidelinesCompliance"],
        }

    index = _guidelines_index(summary)
    missing = [item for item in guidelines if item not in index]
    violated = [
        item for item, compliant in index.items() if item in guidelines and compliant is False
    ]
    if violated:
        return {
            "value": "violated",
            "evidence": [f"guideline violated: {', '.join(violated)}"],
            "sources": ["contract.guidelines", "summary.guidelinesCompliance"],
        }
    if missing:
        return {
            "value": "unknown",
            "evidence": [f"guideline compliance missing for: {', '.join(missing)}"],
            "sources": ["contract.guidelines", "summary.guidelinesCompliance"],
        }
    return {
        "value": "satisfied",
        "evidence": [f"guideline compliance satisfied for {len(guidelines)} guideline(s)"],
        "sources": ["contract.guidelines", "summary.guidelinesCompliance"],
    }


def _checkpoint_signal(contract: dict[str, Any], summary: dict[str, Any] | None) -> dict[str, Any]:
    policy = _dict(contract.get("checkpointPolicy"))
    if not policy.get("requiredBeforeFinish"):
        return {
            "value": "not_required",
            "evidence": ["checkpoint policy not required"],
            "sources": ["contract.checkpointPolicy", "summary.checkpointEvidence"],
        }

    required_stages = _string_list(policy.get("requiredStages"))
    if summary is None:
        return {
            "value": "incomplete",
            "evidence": ["summary is missing"],
            "sources": ["contract.checkpointPolicy", "summary.checkpointEvidence"],
        }

    recorded = _checkpoint_stages(summary)
    missing = [stage for stage in required_stages if stage not in recorded]
    if missing:
        return {
            "value": "incomplete",
            "evidence": [f"checkpoint evidence missing for: {', '.join(missing)}"],
            "sources": ["contract.checkpointPolicy", "summary.checkpointEvidence"],
        }
    return {
        "value": "complete",
        "evidence": [f"checkpoint evidence recorded for: {', '.join(required_stages)}"],
        "sources": ["contract.checkpointPolicy", "summary.checkpointEvidence"],
    }


def _destructive_change_violation(contract: dict[str, Any], summary: dict[str, Any] | None) -> bool:
    policy = _dict(contract.get("destructiveChangePolicy"))
    if policy.get("allowed") is True:
        return False
    return bool(_string_list(_summary_or_empty(summary).get("destructiveChanges")))


def _status_signals(
    contract: dict[str, Any], summary: dict[str, Any] | None
) -> tuple[dict[str, dict[str, Any]], dict[str, Any], dict[str, Any]]:
    summary_dict = _summary_or_empty(summary)
    verification = verification_signal(
        _required_checks(contract), _verification_index(summary_dict)
    )
    scenario_coverage = _scenario_coverage_signal(contract, summary)
    signals: dict[str, dict[str, Any]] = {
        "Intent": intent_alignment_signal(contract, summary_dict),
        "Verification": {
            "value": verification["value"],
            "evidence": verification["evidence"],
            "sources": verification["sources"],
        },
        "Scenario Coverage": {
            "value": scenario_coverage["value"],
            "evidence": scenario_coverage["evidence"],
            "sources": scenario_coverage["sources"],
        },
        "Unknowns": _unknowns_signal(contract, summary),
        "Acceptance": acceptance_signal(contract, summary, verification),
        "Guidelines": _guidelines_signal(contract, summary),
        "Checkpoints": _checkpoint_signal(contract, summary),
        "Residual Risk": residual_risk_signal(summary),
    }
    return signals, verification, scenario_coverage


def _decision_drivers(
    contract: dict[str, Any],
    summary: dict[str, Any] | None,
    signals: dict[str, dict[str, Any]],
    verification: dict[str, Any],
    scenario_coverage: dict[str, Any],
    review: dict[str, Any],
) -> list[str]:
    decision_drivers: list[str] = []

    if contract.get("mode") == "code" and contract.get("notCodable") is True:
        decision_drivers.append("notCodable is true")
    execution_status = _dict(contract.get("executionDecision")).get("status")
    if execution_status in {"block", "blocked"}:
        decision_drivers.append(f"executionDecision is {execution_status}")
    if execution_status in {"defer", "needs_human_decision"}:
        decision_drivers.append(f"executionDecision is {execution_status}")
    if verification["value"] == "failed":
        decision_drivers.extend(verification["evidence"])
    if signals["Guidelines"]["value"] == "violated":
        decision_drivers.extend(signals["Guidelines"]["evidence"])
    if _destructive_change_violation(contract, summary):
        decision_drivers.append("destructive changes are not allowed by the Contract")
    if signals["Verification"]["value"] == "incomplete":
        decision_drivers.extend(verification["evidence"])
    if signals["Scenario Coverage"]["value"] == "incomplete":
        decision_drivers.extend(scenario_coverage["evidence"])
    if signals["Unknowns"]["value"] == "open":
        decision_drivers.extend(signals["Unknowns"]["evidence"])
    if signals["Intent"]["value"] in {"unknown", "unresolved"}:
        decision_drivers.extend(signals["Intent"]["evidence"])
    if signals["Checkpoints"]["value"] == "incomplete":
        decision_drivers.extend(signals["Checkpoints"]["evidence"])
    if signals["Acceptance"]["value"] == "incomplete":
        decision_drivers.extend(signals["Acceptance"]["evidence"])
    if signals["Residual Risk"]["value"] == "unknown":
        decision_drivers.extend(signals["Residual Risk"]["evidence"])
    if review["status"] == "blocked":
        decision_drivers.append("reviewReadiness.status is blocked")
    if review["status"] == "not_ready":
        decision_drivers.append("reviewReadiness.status is not_ready")
    return decision_drivers


def _recommendation(
    contract: dict[str, Any],
    summary: dict[str, Any] | None,
    signals: dict[str, dict[str, Any]],
    verification: dict[str, Any],
    decision_drivers: list[str],
    review: dict[str, Any],
) -> str:
    execution_status = _dict(contract.get("executionDecision")).get("status")
    if any(
        reason
        for reason in decision_drivers
        if reason
        in {
            "notCodable is true",
            "executionDecision is block",
            "executionDecision is blocked",
            "reviewReadiness.status is blocked",
            "destructive changes are not allowed by the Contract",
        }
    ):
        recommendation = "blocked"
    elif execution_status in {"defer", "needs_human_decision"}:
        recommendation = "needs_investigation"
    elif (
        signals["Verification"]["value"] == "failed" or signals["Guidelines"]["value"] == "violated"
    ):
        recommendation = "blocked"
    elif (
        any(
            signal["value"] in {"incomplete", "unknown", "open"}
            for name, signal in signals.items()
            if name
            in {"Acceptance", "Unknowns", "Verification", "Checkpoints", "Residual Risk", "Intent"}
        )
        or review["status"] in {"unknown", "not_ready"}
        or (
            signals["Scenario Coverage"]["value"] == "incomplete"
            and _scenario_coverage_hard_risk(contract)
            and not _scenario_coverage_explicit_risk_ack(summary)
        )
    ):
        recommendation = "needs_investigation"
    elif (
        signals["Scenario Coverage"]["value"] == "incomplete"
        or signals["Residual Risk"]["value"] in {"medium", "high"}
        or review["status"] == "ready_with_risks"
    ):
        recommendation = "ready_with_risks"
    else:
        recommendation = "ready_for_review"

    if recommendation not in RECOMMENDATIONS:
        recommendation = "needs_investigation"
    return recommendation


def _status_evidence(
    contract: dict[str, Any],
    summary: dict[str, Any] | None,
    signals: dict[str, dict[str, Any]],
    verification: dict[str, Any],
    scenario_coverage: dict[str, Any],
    review: dict[str, Any],
) -> dict[str, list[str]]:
    # Keep the evidence compact and explainable.
    contract_evidence = [
        f"intent={'present' if _has_meaningful_intent(contract) else 'absent'}",
        f"acceptance={len(_string_list(contract.get('acceptance')))}",
        f"unknowns={len(_string_list(contract.get('unknowns')))}",
        f"guidelines={len(_string_list(contract.get('guidelines')))}",
        f"scenarioCoverage={'present' if scenario_items(_summary_or_empty(summary)) else 'absent'}",
        f"checkpointPolicy={'required' if _dict(contract.get('checkpointPolicy')).get('requiredBeforeFinish') else 'not_required'}",
    ]
    summary_evidence = [
        f"verification={len(verification['passed'])}/{len(verification['required'])} passed",
        f"scenarioCoverage={scenario_coverage['value']}; required={len(scenario_coverage['required'])}; unverified={len(scenario_coverage['unverified'])}",
        f"unknownsRemaining={len(_string_list(_summary_or_empty(summary).get('unknownsRemaining')))}",
        f"reviewReadiness={review['status']}",
        f"residualRisk={signals['Residual Risk']['value']}",
    ]
    verification_index = _verification_index(_summary_or_empty(summary))
    verification_evidence = [
        f"{check}: {verification_index.get(check, verification['value'])}"
        for check in verification["required"]
    ]
    return {
        "contract": contract_evidence,
        "summary": summary_evidence,
        "verification": verification_evidence,
        "intentAlignment": signals["Intent"]["evidence"],
        "scenarioCoverage": scenario_coverage["evidence"],
        "guidelines": signals["Guidelines"]["evidence"],
        "checkpoints": signals["Checkpoints"]["evidence"],
        "residualRisk": signals["Residual Risk"]["evidence"],
        "reviewReadiness": [f"status={review['status']}"]
        + ([f"focus={', '.join(review['focus'])}"] if review["focus"] else []),
    }


def derive_governance_status(
    contract: dict[str, Any], summary: dict[str, Any] | None
) -> dict[str, Any]:
    """Return a structured, recommendation-oriented status model."""

    contract = contract if isinstance(contract, dict) else {}
    summary_dict = summary if isinstance(summary, dict) else None
    signals, verification, scenario_coverage = _status_signals(contract, summary_dict)
    review = review_readiness_signal(summary_dict)
    decision_drivers = _decision_drivers(
        contract, summary_dict, signals, verification, scenario_coverage, review
    )
    recommendation = _recommendation(
        contract, summary_dict, signals, verification, decision_drivers, review
    )
    evidence = _status_evidence(
        contract, summary_dict, signals, verification, scenario_coverage, review
    )

    return {
        "recommendation": recommendation,
        "signals": [
            {"name": name, "value": signals[name]["value"], "sources": signals[name]["sources"]}
            for name in SIGNAL_ORDER
        ],
        "evidence": evidence,
        "decisionDrivers": decision_drivers,
        "reviewReadiness": review,
        "sources": {
            "contract": [
                "contract.intent",
                "contract.acceptance",
                "contract.unknowns",
                "contract.guidelines",
                "contract.checkpointPolicy",
                "contract.riskAssessment",
            ],
            "summary": [
                "summary.intentAlignment",
                "summary.verification",
                "summary.scenarioCoverage",
                "summary.followUps",
                "summary.unverifiedScenarios",
                "summary.unknownsRemaining",
                "summary.guidelinesCompliance",
                "summary.checkpointEvidence",
                "summary.risk",
                "summary.residualRisks",
                "summary.reviewReadiness",
            ],
        },
    }


def render_active_status(
    model: dict[str, Any],
    *,
    work_item_id: str,
    mode: str,
    contract_path: str,
    summary_path: str,
    generated_at: str | None = None,
    backtrack_report: str | None = None,
    backtrack_status: str | None = None,
    backtrack_items: list[dict[str, Any]] | None = None,
    preflight_review: dict[str, Any] | None = None,
    ownership_counts: dict[str, int] | None = None,
    calibration_inventory: dict[str, Any] | None = None,
    task_outcome: dict[str, Any] | None = None,
    calibration_corrective: dict[str, Any] | None = None,
) -> str:
    """Render compressed governance signals and optional Outcome link metadata."""
    timestamp = generated_at or datetime.now(UTC).isoformat()
    signal = human_signal(model)
    lines = [
        "---",
        "title: AI Cockpit Current Status",
        "generated: true",
        "---",
        "",
        "# AI Cockpit Current Status",
        "",
        "This file is generated by `scripts/ai_generate_status.py`. Do not update it by hand.",
        "",
        f"- Generated At: `{timestamp}`",
        f"- Task: `{work_item_id}`",
        f"- Mode: `{mode}`",
        "- Scope: `current_worktree`; one active Work Item and this generated report are local to this worktree.",
        "- An external Agent queries each worktree separately and owns any aggregate scheduling view.",
        f"- State: `{model['recommendation']}`",
        f"- Recommendation: `{model['recommendation']}`",
        f"- Contract Path: `{contract_path}`",
        f"- Summary Path: `{summary_path}`",
        "",
        "## Key Conclusion",
        "",
        "- Signal Domain: `governance_review`",
        f"- Color: `{signal['color']}`",
        f"- Conclusion: {signal['conclusion']}",
        f"- Evidence Basis: `{'; '.join(signal['evidenceBasis']) or 'none'}`",
        f"- Next Action: {signal['nextAction']}",
        "",
        "## Governance Signals",
        "",
    ]
    for signal in model["signals"]:
        lines.append(f"- {signal['name']}: {signal['value']}")

    review_status = preflight_review.get("status") if isinstance(preflight_review, dict) else None
    review_recommendation = (
        preflight_review.get("recommendation") if isinstance(preflight_review, dict) else None
    )
    review_drivers = (
        preflight_review.get("decisionDrivers") if isinstance(preflight_review, dict) else None
    )
    if non_empty_string(review_status) and non_empty_string(review_recommendation):
        lines.extend(["", "## Preflight Review", ""])
        lines.append(f"- Status: `{review_status}`")
        lines.append(f"- Recommendation: `{review_recommendation}`")
        lines.append("- Decision Drivers:")
        if isinstance(review_drivers, list) and any(
            non_empty_string(item) for item in review_drivers
        ):
            lines.extend(f"  - {item}" for item in review_drivers if non_empty_string(item))
        else:
            lines.append("  - none")
        lines.append(
            "- Pause Rule: `Cockpit Status keeps the Preflight Review visible for reviewers, but it does not replace the pre-implementation pause.`"
        )
        request = (
            preflight_review.get("humanDecisionRequest")
            if isinstance(preflight_review, dict)
            else None
        )
        if isinstance(request, dict):
            lines.extend(["", "## Human Decision Request", ""])
            lines.append(f"- Decision ID: `{request['decisionId']}`")
            lines.append("- What Happened:")
            lines.extend(f"  - {item}" for item in request["whatHappened"])
            lines.append(f"- Why It Matters: {request['whyItMatters']}")
            lines.append("- Options:")
            for option in request["options"]:
                lines.append(f"  - `{option['id']}` {option['label']}: {option['effect']}")
            lines.append(f"- Recommended Option: `{request['recommendedOption']}`")
            lines.append(f"- Recommendation Reason: {request['recommendationReason']}")
            lines.append(f"- Decision Needed: {request['question']}")
            lines.append(f"- Resume Condition: {request['resumeCondition']}")

    lines.extend(["", "## Diff Ownership", ""])
    if ownership_counts is None:
        lines.append("- Status: `not_generated`")
    else:
        for state in (
            "active_owned",
            "archived_owned",
            "unowned",
            "ambiguous",
            "out_of_scope",
            "approval_required",
        ):
            lines.append(f"- {state}: `{ownership_counts.get(state, 0)}`")
        unresolved = sum(
            ownership_counts.get(state, 0)
            for state in ("unowned", "ambiguous", "out_of_scope", "approval_required")
        )
        lines.append(f"- Unresolved: `{unresolved}`")

    lines.extend(["", "## Evidence", ""])
    for key in (
        "contract",
        "summary",
        "verification",
        "intentAlignment",
        "scenarioCoverage",
        "guidelines",
        "checkpoints",
        "residualRisk",
        "reviewReadiness",
    ):
        entries = model["evidence"].get(key, [])
        if not entries:
            lines.append(f"- {EVIDENCE_LABELS.get(key, key)}: none")
            continue
        if len(entries) == 1:
            lines.append(f"- {EVIDENCE_LABELS.get(key, key)}: `{entries[0]}`")
        else:
            lines.append(f"- {EVIDENCE_LABELS.get(key, key)}: `{'; '.join(entries)}`")

    if isinstance(task_outcome, dict):
        lines.extend(["", "## Task Outcome", ""])
        lines.append("- Signal Domain: `work_item_lifecycle`")
        lines.append("- Presence: `present`")
        outcome_path = task_outcome.get("markdownPath", "")
        outcome_count = task_outcome.get("evidenceCount", 0)
        outcome_status = task_outcome.get("status", "unknown")
        lines.append(f"- Status: `{outcome_status}`")
        color = task_outcome.get("humanStatusColor", "unknown")
        lines.append(f"- Traffic Light: `{color}`")
        failed_gate = task_outcome.get("failedCheck", task_outcome.get("failedGate", ""))
        if failed_gate:
            lines.append(f"- Failed Gate: `{failed_gate}`")
        recovery = task_outcome.get("recoveryCondition", "")
        if recovery:
            lines.append(f"- Recovery Condition: `{recovery}`")
        lines.append(f"- Report: `{outcome_path}`")
        lines.append(f"- Evidence Count: `{outcome_count}`")
    else:
        lines.extend(["", "## Task Outcome", ""])
        lines.append("- Signal Domain: `work_item_lifecycle`")
        lines.append("- Presence: `absent`")
        lines.append("- Status: `in_progress`")
        lines.append("- Traffic Light: `yellow`")
        lines.append("- Next Action: `continue verification or run make ai-finish`")

    if isinstance(calibration_corrective, dict):
        lines.extend(["", "## Calibration Corrective Route", ""])
        lines.append("- Traffic Light: `yellow`")
        lines.append(
            "- Authority: `bounded corrective exception; not calibration completion authority`"
        )
        lines.append(
            "- Session: `"
            + str(calibration_corrective.get("sessionId", "unknown"))
            + "` (`"
            + str(calibration_corrective.get("sessionState", "unknown"))
            + "`)"
        )
        lines.append(f"- Finding: `{calibration_corrective.get('findingId', 'unknown')}`")
        repair_paths = calibration_corrective.get("repairPaths")
        if isinstance(repair_paths, list):
            lines.append("- Repair Paths: `" + "; ".join(map(str, repair_paths)) + "`")
        lines.append(
            "- Resume Condition: `"
            + str(calibration_corrective.get("resumeCondition", "unknown"))
            + "`"
        )

    if isinstance(calibration_inventory, dict):
        lines.extend(["", "## Calibration Inventory", ""])
        lines.append(f"- Schema Version: `{calibration_inventory.get('schemaVersion', 'unknown')}`")
        summary = calibration_inventory.get("summary", {})
        if isinstance(summary, dict):
            lines.append(
                "- Summary: `"
                + ", ".join(f"{status}={summary.get(status, 0)}" for status in STATUS_VALUES)
                + "`"
            )
        items = calibration_inventory.get("items", {})
        if isinstance(items, dict):
            for key, item in items.items():
                if not isinstance(item, dict):
                    continue
                lines.append(
                    f"- {key}: `{item.get('status', 'unknown')}` "
                    f"(confirmation=`{item.get('confirmation', 'none')}`, source=`{item.get('source', '')}`)"
                )

    lines.extend(["", "## Decision Drivers", ""])
    if model["decisionDrivers"]:
        lines.extend(f"- {driver}" for driver in model["decisionDrivers"])
    else:
        lines.append("- none")

    lines.extend(["", "## Backtrack", ""])
    if backtrack_status:
        lines.append(f"- Status: `{backtrack_status}`")
    elif backtrack_report:
        lines.append("- Status: `unknown`")
    else:
        lines.append("- Status: `not_run`")
    if backtrack_report:
        lines.append(f"- Report: `{backtrack_report}`")
    if backtrack_items:
        for item in backtrack_items:
            if isinstance(item, dict):
                lines.append(f"- {item.get('kind')}: `{item.get('path')}` - {item.get('detail')}")
    elif backtrack_report:
        lines.append("- Items: none")

    lines.extend(
        [
            "",
            "## Next Action",
            "",
            "- resolve Diff Ownership Preview findings, then complete human review / commit decision",
        ]
    )
    return "\n".join(lines) + "\n"

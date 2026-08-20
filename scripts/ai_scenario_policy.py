"""Small, pure Scenario Coverage policy helpers."""
# mypy: disable-error-code=union-attr

from __future__ import annotations

from typing import Any

DEFAULT_HARD_RISK_TYPES = {
    "release",
    "release_distribution",
    "installer",
    "auth",
    "ci",
    "migration",
    "security",
    "api_change",
}


def scenario_items(summary: dict[str, Any]) -> list[dict[str, Any]]:
    values = summary.get("scenarioCoverage")
    return [item for item in values if isinstance(item, dict)] if isinstance(values, list) else []


def is_hard_risk(contract: dict[str, Any], configured: set[str] | None = None) -> bool:
    risk = (
        contract.get("riskAssessment") if isinstance(contract.get("riskAssessment"), dict) else {}
    )
    values = {item for item in risk.get("riskTypes", []) if isinstance(item, str)}
    return bool(values & (configured or DEFAULT_HARD_RISK_TYPES))


def has_risk_ack(summary: dict[str, Any] | None) -> bool:
    if not isinstance(summary, dict):
        return False
    review = (
        summary.get("reviewReadiness") if isinstance(summary.get("reviewReadiness"), dict) else {}
    )
    residual = summary.get("residualRisks")
    followups = summary.get("followUps")
    unverified = summary.get("unverifiedScenarios")
    return (
        review.get("status") == "ready_with_risks"
        and isinstance(residual, list)
        and any(isinstance(x, dict) for x in residual)
        and bool(
            (isinstance(followups, list) and followups)
            or (isinstance(unverified, list) and unverified)
        )
    )

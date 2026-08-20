"""Pure acceptance signal policy."""
# mypy: ignore-errors

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

SOURCES = ["contract.acceptance", "summary.verification", "summary.reviewReadiness"]
ACCEPTANCE_ID = re.compile(r"^(A[0-9]+):\s+.+")


def acceptance_ids(contract: dict[str, Any]) -> tuple[list[str], list[str]]:
    acceptance = contract.get("acceptance")
    if not isinstance(acceptance, list) or not acceptance:
        return [], ["contract.acceptance is missing"]
    # Existing v2 records may use unnumbered Acceptance prose. Strict evidence
    # mapping is opt-in when the Contract declares the stable A<n>: convention.
    if not any(
        isinstance(item, str) and re.match(r"^A[0-9]+:", item.strip()) for item in acceptance
    ):
        return [], []
    ids: list[str] = []
    issues: list[str] = []
    for index, item in enumerate(acceptance):
        if not isinstance(item, str) or ACCEPTANCE_ID.match(item.strip()) is None:
            issues.append(f"contract.acceptance[{index}] must start with a stable A<n>: identifier")
            continue
        identifier = item.split(":", 1)[0]
        if identifier in ids:
            issues.append(f"contract.acceptance contains duplicate identifier {identifier}")
        ids.append(identifier)
    return ids, issues


def _human_review_completed(mapping: dict[str, Any]) -> bool:
    review = mapping.get("humanReview")
    return mapping.get("humanReviewed") is True or (
        isinstance(review, dict) and review.get("completed") is True
    )


def validate_acceptance_evidence(
    contract: dict[str, Any],
    summary: dict[str, Any] | None,
    verification: list[dict[str, Any]],
    *,
    project_root: Path | None = None,
) -> list[str]:
    """Validate v2 Acceptance IDs and their executed evidence mappings.

    Contract v1 and legacy-shaped contracts remain readable without this strict
    mapping; active v2 Work Items must provide it before finish readiness.
    """
    if contract.get("contractVersion") != 2:
        return []
    if "acceptance" not in contract:
        return []
    ids, issues = acceptance_ids(contract)
    if not ids and not issues:
        return []
    if summary is None:
        return issues + ["summary is missing acceptanceEvidence"]
    mappings = summary.get("acceptanceEvidence")
    if not isinstance(mappings, list):
        return issues + ["summary.acceptanceEvidence must be a list"]
    mapped_ids: list[str] = []
    passed = {
        item.get("check")
        for item in verification
        if isinstance(item, dict) and item.get("result") == "passed"
    }
    root = project_root or Path(__file__).resolve().parents[1]
    risk = contract.get("riskAssessment", {})
    high_risk = isinstance(risk, dict) and risk.get("level") == "high"
    for index, mapping in enumerate(mappings):
        prefix = f"acceptanceEvidence[{index}]"
        if not isinstance(mapping, dict):
            issues.append(f"{prefix} must be an object")
            continue
        identifier = mapping.get("acceptanceId")
        if not isinstance(identifier, str) or not identifier.strip():
            issues.append(f"{prefix}.acceptanceId is required")
            continue
        if identifier not in ids:
            issues.append(f"{prefix}.acceptanceId does not reference a Contract Acceptance")
        if identifier in mapped_ids:
            issues.append(f"{prefix}.acceptanceId is duplicated")
        mapped_ids.append(identifier)
        evidence = mapping.get("evidence")
        if not isinstance(evidence, list) or not evidence:
            issues.append(f"{prefix}.evidence must be a non-empty list")
            continue
        if high_risk and not _human_review_completed(mapping):
            issues.append("high-risk Acceptance evidence requires humanReview.completed true")
        is_bug_fix = mapping.get("kind") == "bug_fix" or mapping.get("bugFix") is True
        if is_bug_fix and not isinstance(mapping.get("failureScenario"), str):
            issues.append("bug-fix Acceptance evidence requires failureScenario")
        for evidence_index, item in enumerate(evidence):
            evidence_prefix = f"{prefix}.evidence[{evidence_index}]"
            if not isinstance(item, dict):
                issues.append(f"{evidence_prefix} must be an object")
                continue
            for field in ("type", "path", "locator", "verification"):
                if not isinstance(item.get(field), (str, list)) or not item.get(field):
                    issues.append(f"{evidence_prefix}.{field} is required")
            path = item.get("path")
            if isinstance(path, str):
                candidate = root / path
                if Path(path).is_absolute() or not candidate.is_file():
                    issues.append(f"{evidence_prefix}.path does not exist")
            checks = item.get("verification")
            checks = [checks] if isinstance(checks, str) else checks
            if isinstance(checks, list):
                for check in checks:
                    if check not in passed:
                        issues.append(f"{evidence_prefix}.verification {check} was not passed")
            if (
                is_bug_fix
                and not item.get("failureScenario")
                and not mapping.get("failureScenario")
            ):
                issues.append("bug-fix Acceptance evidence requires failureScenario")
    missing = [identifier for identifier in ids if identifier not in mapped_ids]
    if missing:
        issues.append(f"missing Acceptance evidence mapping: {', '.join(missing)}")
    return issues


def acceptance_signal(
    contract: dict[str, Any], summary: dict[str, Any] | None, verification: dict[str, Any]
) -> dict[str, Any]:
    if summary is None:
        return {"value": "unknown", "evidence": ["summary is missing"], "sources": SOURCES}
    acceptance = contract.get("acceptance")
    if not isinstance(acceptance, list) or not acceptance:
        return {
            "value": "unknown",
            "evidence": ["contract.acceptance is missing"],
            "sources": SOURCES,
        }
    review = (
        summary.get("reviewReadiness") if isinstance(summary.get("reviewReadiness"), dict) else {}
    )
    status = (
        review.get("status")
        if review.get("status") in {"ready", "ready_with_risks", "not_ready", "blocked"}
        else "unknown"
    )
    if verification.get("value") != "passed":
        return {
            "value": "incomplete",
            "evidence": [f"required verification is {verification.get('value')}"],
            "sources": SOURCES,
        }
    if summary.get("unknownsRemaining"):
        return {
            "value": "incomplete",
            "evidence": ["summary.unknownsRemaining is not empty"],
            "sources": SOURCES,
        }
    if status == "unknown":
        return {
            "value": "unknown",
            "evidence": ["summary.reviewReadiness is missing"],
            "sources": SOURCES,
        }
    if status in {"not_ready", "blocked"}:
        return {
            "value": "incomplete",
            "evidence": [f"reviewReadiness.status is {status}"],
            "sources": SOURCES,
        }
    summary_verification = summary.get("verification", [])
    evidence_issues = validate_acceptance_evidence(
        contract,
        summary,
        summary_verification if isinstance(summary_verification, list) else [],
    )
    if evidence_issues:
        return {
            "value": "incomplete",
            "evidence": evidence_issues[:3],
            "sources": SOURCES,
        }
    return {
        "value": "complete",
        "evidence": [f"reviewReadiness.status is {status}"],
        "sources": SOURCES,
    }

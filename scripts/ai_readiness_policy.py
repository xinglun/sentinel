"""Fail-closed readiness policy shared by Preflight callers."""

# mypy: ignore-errors
from __future__ import annotations

from pathlib import Path
from typing import Any

from ai_calibration_inventory import build_inventory
from ai_common import parse_yaml


def readiness_evidence(root: Path) -> dict[str, Any]:
    """Aggregate static adoption evidence without executing project commands."""
    profile = root / ".ai" / "project_profile.yaml"
    policy = root / ".ai" / "guards" / "governance_complexity_policy.yaml"
    evidence: dict[str, Any] = {
        "profile": {"status": "present" if profile.is_file() else "missing"},
        "complexityPolicy": {"status": "missing"},
        "qualityCommands": {
            "status": "not_run",
            "reason": "Static readiness does not execute project commands.",
        },
        "criticalDomains": {
            "status": "not_run",
            "reason": "Critical-domain review remains project-owned.",
        },
        "unknowns": {"status": "unknown"},
    }
    # The compatibility fields below remain for existing callers, while the
    # shared Inventory is the authoritative source for cross-consumer status.
    inventory = build_inventory(root)
    evidence["inventory"] = inventory
    if policy.is_file():
        try:
            raw = parse_yaml(policy)
            proposal = raw.get("proposal", {}) if isinstance(raw, dict) else {}
            status = proposal.get("status") if isinstance(proposal, dict) else None
            evidence["complexityPolicy"] = {"status": status or "unavailable"}
        except (OSError, ValueError):
            evidence["complexityPolicy"] = {"status": "unavailable"}
    return evidence


def readiness_state(root: Path) -> dict[str, Any]:
    """Derive installed, calibrated, and production-ready states separately."""
    installed = (root / ".ai" / "cockpit" / "version.json").is_file()
    profile = root / ".ai" / "project_profile.yaml"
    guards = root / ".ai" / "guards" / "coverage_policy.yaml"
    inventory = build_inventory(root)
    items = inventory["items"]
    calibrated = (
        installed
        and items["profile"]["status"] == "complete"
        and items["guards"]["status"] not in {"incomplete", "unknown"}
    )
    ci = (root / ".github" / "workflows").is_dir() or (root / ".gitlab-ci.yml").is_file()
    review = (root / ".ai" / "guards" / "ai_review_policy.yaml").is_file()
    production_ready = calibrated and ci and review
    return {
        "state": "production_ready"
        if production_ready
        else (
            "calibration_complete"
            if calibrated
            else ("adoption_installed" if installed else "not_installed")
        ),
        "adoptionInstalled": installed,
        "calibrationComplete": calibrated,
        "productionReady": production_ready,
        "readinessEvidence": readiness_evidence(root),
        "calibrationInventory": inventory,
        "evidence": {
            "profile": profile.is_file(),
            "guards": guards.is_file(),
            "ci": ci,
            "reviewPolicy": review,
        },
    }


def has_explicit_blocker(contract: dict[str, Any]) -> bool:
    decision = (
        contract.get("executionDecision")
        if isinstance(contract.get("executionDecision"), dict)
        else {}
    )
    capability = (
        contract.get("agentCapability") if isinstance(contract.get("agentCapability"), dict) else {}
    )
    if contract.get("notCodable") is True or decision.get("status") in {
        "block",
        "defer",
        "needs_human_decision",
    }:
        return True
    return bool(
        capability
        and (
            capability.get("canImplement") is False
            or capability.get("canVerify") is False
            or capability.get("needsHumanDecision") is True
        )
    )

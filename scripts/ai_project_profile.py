#!/usr/bin/env python3
"""Load and validate the AI Cockpit project boundary profile."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from ai_calibration_profiles import load_policy, validate_selection
from ai_common import non_empty_string, parse_yaml

FACT_KEYS = ("languages", "frameworks", "buildSystems", "infrastructure")
BOUNDARY_KEYS = ("productionRoots", "featureRoots", "testRoots", "generatedPaths", "criticalPaths")
REQUIRED_KEYS = {
    "version",
    "detectedFacts",
    "suggestedBoundaries",
    "approvedBoundaries",
    "reviewRequirements",
    "unknowns",
    "evidence",
    "approval",
}


def string_list(container: dict[str, Any], key: str) -> list[str] | None:
    value = container.get(key)
    if not isinstance(value, list) or any(not non_empty_string(item) for item in value):
        return None
    return [str(item).strip() for item in value]


def validate_profile(data: dict[str, Any], *, require_approval: bool) -> list[str]:
    issues: list[str] = []
    missing = sorted(REQUIRED_KEYS - data.keys())
    issues.extend(f"missing field: {key}" for key in missing)
    if data.get("version") not in {1, "1"}:
        issues.append("version must be 1")

    facts = data.get("detectedFacts")
    if not isinstance(facts, dict):
        issues.append("detectedFacts must be an object")
    else:
        for key in FACT_KEYS:
            if string_list(facts, key) is None:
                issues.append(f"detectedFacts.{key} must be a list of non-empty strings")

    for section_name in ("suggestedBoundaries", "approvedBoundaries"):
        section = data.get(section_name)
        if not isinstance(section, dict):
            issues.append(f"{section_name} must be an object")
            continue
        for key in BOUNDARY_KEYS:
            if string_list(section, key) is None:
                issues.append(f"{section_name}.{key} must be a list of non-empty strings")

    for key in ("reviewRequirements", "unknowns", "evidence"):
        if string_list(data, key) is None:
            issues.append(f"{key} must be a list of non-empty strings")

    approval = data.get("approval")
    if not isinstance(approval, dict):
        issues.append("approval must be an object")
    else:
        reviewed = approval.get("reviewed")
        if reviewed not in {True, False, "true", "false"}:
            issues.append("approval.reviewed must be boolean")
        if require_approval:
            if reviewed not in {True, "true"}:
                issues.append("approval.reviewed must be true for the confirmed Profile")
            if not non_empty_string(approval.get("reviewedBy")):
                issues.append("approval.reviewedBy is required for the confirmed Profile")
            if not non_empty_string(approval.get("reason")):
                issues.append("approval.reason is required for the confirmed Profile")

    if require_approval:
        approved = data.get("approvedBoundaries")
        if isinstance(approved, dict):
            if not string_list(approved, "productionRoots"):
                issues.append("approvedBoundaries.productionRoots must not be empty")
            if not string_list(approved, "testRoots"):
                issues.append("approvedBoundaries.testRoots must not be empty")
        unknowns = data.get("unknowns")
        if isinstance(unknowns, list) and any(
            str(item).lower().startswith("blocking:") for item in unknowns
        ):
            issues.append("blocking unknowns must be resolved before Profile approval")

    calibration_profile = data.get("calibrationProfile")
    if calibration_profile is not None:
        try:
            policy = load_policy()
        except ValueError as exc:
            issues.append(str(exc))
        else:
            issues.extend(
                validate_selection(
                    calibration_profile,
                    policy,
                    require_human=require_approval,
                )
            )
    return issues


def load_profile(path: Path, *, require_approval: bool) -> tuple[dict[str, Any], list[str]]:
    if not path.is_file():
        return {}, [f"Profile not found: {path}"]
    try:
        data = parse_yaml(path)
    except (OSError, ValueError) as exc:
        return {}, [f"failed to parse Profile: {exc}"]
    if not isinstance(data, dict):
        return {}, ["Profile root must be an object"]
    return data, validate_profile(data, require_approval=require_approval)

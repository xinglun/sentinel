#!/usr/bin/env python3
"""Derive objective operation-impact facts from declarations and changed paths.

This module intentionally does not classify people or request intent.  It
compares Contract declarations with observable change facts so callers can
require evidence or stop on a contradiction.
"""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from typing import Any

_STATUS_ACTIONS = {"A": "create", "M": "modify", "D": "delete", "R": "move"}
_IMPACT_SUFFIXES = {".yaml", ".yml", ".toml", ".ini", ".env", ".json"}
_API_MARKERS = ("api", "public", "interface", "contract")
_TEST_MARKERS = ("test", "tests", "snapshot", "coverage")
_CI_MARKERS = (".github/workflows/", "ci/", "pipeline", "makefile")


def _declared_action(contract: Mapping[str, Any]) -> str:
    operation = contract.get("requestedOperation")
    if not isinstance(operation, Mapping):
        return ""
    action = operation.get("action")
    return action.strip().lower() if isinstance(action, str) else ""


def _impact_classes(path: str, action: str) -> set[str]:
    lowered = path.casefold()
    suffix = "." + lowered.rsplit(".", 1)[-1] if "." in lowered else ""
    classes: set[str] = set()
    if lowered.startswith((".ai/", "docs/", "scripts/", "tests/")):
        return classes
    if action in {"delete", "move"}:
        classes.add("compatibility_affecting")
    if any(marker in lowered for marker in _API_MARKERS):
        classes.add("compatibility_affecting")
    if suffix in _IMPACT_SUFFIXES and not lowered.startswith("docs/"):
        classes.add("configuration_affecting")
    if action in {"delete", "move"} and (
        any(marker in lowered for marker in _TEST_MARKERS)
        or any(marker in lowered for marker in _CI_MARKERS)
    ):
        classes.add("test_evidence_affecting")
    return classes


def derive_operation_impact(
    contract: Mapping[str, Any], changed_paths: Iterable[tuple[str, str]]
) -> dict[str, Any]:
    """Return deterministic impact facts for already-observed name-status data."""
    normalized = [(str(status)[:1].upper(), str(path)) for status, path in changed_paths]
    observed_actions = sorted({_STATUS_ACTIONS.get(status, "modify") for status, _ in normalized})
    impacts: set[str] = set()
    targets: list[str] = []
    for status, path in normalized:
        action = _STATUS_ACTIONS.get(status, "modify")
        path_impacts = _impact_classes(path, action)
        if path_impacts:
            targets.append(path)
            impacts.update(path_impacts)
    declared = _declared_action(contract)
    destructive = "delete" in observed_actions
    declaration_conflict = destructive and declared not in {"delete", "remove", "retire"}
    if declaration_conflict:
        decision = "block"
    elif impacts:
        decision = "evidence_required"
    else:
        decision = "not_applicable"
    return {
        "version": 1,
        "declaredAction": declared or None,
        "observedActions": observed_actions,
        "targets": sorted(targets),
        "impactClasses": sorted(impacts),
        "riskProperties": {
            "destructive": destructive,
            "compatibilityAffecting": "compatibility_affecting" in impacts,
            "evidenceWeakening": "test_evidence_affecting" in impacts,
        },
        "declarationConflict": declaration_conflict,
        "decisionStates": {
            "requestTrustDecision": "not_assessed",
            "authorityBindingDecision": "not_assessed",
            "safetyEvidenceDecision": "evidence_required" if impacts else "not_applicable",
            "scopeConsistencyDecision": "inconsistent" if declaration_conflict else "consistent",
            "effectiveDecision": "block" if declaration_conflict else decision,
        },
        "decision": decision,
    }


def impact_requires_reference_record(report: Mapping[str, Any]) -> bool:
    """Return whether a report has an impact class requiring an evidence record."""
    return bool(report.get("impactClasses"))

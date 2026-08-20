#!/usr/bin/env python3
"""Deterministic Trust Layer capability, intent, constraint, and criteria guards."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, cast

from ai_common import parse_yaml
from ai_critical_domain_guards import critical_domain_signals
from ai_trust_schema import ValidationError, validate_payload

PROJECT_ROOT = Path(__file__).resolve().parents[1]
CAPABILITIES_PATH = PROJECT_ROOT / ".ai" / "project" / "capabilities.json"
REQUESTED_OPERATION_POLICY_PATH = PROJECT_ROOT / ".ai" / "policies" / "requested-operation.yaml"
SUCCESS_CRITERIA_PATH = PROJECT_ROOT / ".ai" / "project" / "success_criteria.json"
CANONICAL_STATES = {"allow", "review", "confirm", "defer", "block", "error", "not_applicable"}
LEGACY_TO_CANONICAL = {
    "Ready": "allow",
    "Partial": "review",
    "Missing": "block",
    "Inconsistent": "block",
    "Not Applicable": "not_applicable",
}


def display_path(path: Path) -> str:
    """Keep evidence portable and free of machine-specific absolute paths."""
    try:
        return path.relative_to(PROJECT_ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def _signal(name: str, value: str, evidence: list[str], sources: list[str]) -> dict[str, Any]:
    state = LEGACY_TO_CANONICAL.get(value, "error")
    return {
        "name": name,
        "value": value,
        "signalId": f"guard.{name.lower().replace(' ', '_')}",
        "state": state,
        "confidence": "deterministic",
        "evidence": evidence,
        "sources": sources,
        "policyReference": sources[0] if sources else "guard.default",
        "humanDecisionAllowed": value == "Partial",
        "safeAlternatives": [],
    }


def validate_signal_state(signal: dict[str, Any]) -> bool:
    """Ensure canonical state is derived from the legacy value, never independently authored."""
    value = signal.get("value")
    value = value if isinstance(value, str) else ""
    expected = LEGACY_TO_CANONICAL.get(value, "error")
    return signal.get("state") == expected and signal.get("state") in CANONICAL_STATES


def _load_json(path: Path) -> tuple[Any | None, list[str]]:
    try:
        return json.loads(path.read_text(encoding="utf-8")), []
    except (OSError, json.JSONDecodeError) as exc:
        return None, [f"cannot load {path}: {exc}"]


def capability_signal(contract: dict[str, Any], path: Path = CAPABILITIES_PATH) -> dict[str, Any]:
    payload, issues = _load_json(path)
    if issues:
        return _signal("Capability", "Missing", issues, [display_path(path)])
    if not isinstance(payload, dict):
        return _signal(
            "Capability", "Inconsistent", ["declaration must be an object"], [display_path(path)]
        )
    try:
        validate_payload("repository_capabilities", payload)
    except ValidationError as exc:
        return _signal("Capability", "Inconsistent", [str(exc)], [display_path(path)])

    capabilities = set(payload["capabilities"])
    required: set[str] = set()
    for raw_path in contract.get("scope", []):
        value = str(raw_path).lower()
        if "test" in value or value.startswith("tests/"):
            required.add("test_automation")
        if "doc" in value:
            required.add("documentation")
        if value.startswith("scripts/") or value == "makefile":
            required.add("software_design")
        if value.startswith(".ai/"):
            required.add("ai_governance")
    missing = sorted(required - capabilities)
    if missing:
        return _signal(
            "Capability",
            "Partial",
            [f"required capability is not declared: {item}" for item in missing],
            [display_path(path), "contract.scope"],
        )
    return _signal(
        "Capability",
        "Ready",
        [f"declared capabilities cover: {', '.join(sorted(required)) or 'the requested scope'}"],
        [display_path(path), "contract.scope"],
    )


def intent_capability_signal(
    contract: dict[str, Any], path: Path = CAPABILITIES_PATH
) -> dict[str, Any]:
    """Derive requested capability from a repository-owned operation policy."""
    operation = contract.get("requestedOperation")
    if operation is None:
        return _signal(
            "Intent Capability",
            "Not Applicable",
            ["no requestedOperation is declared"],
            ["contract.requestedOperation"],
        )
    if not isinstance(operation, dict):
        return _signal(
            "Intent Capability",
            "Inconsistent",
            ["requestedOperation must be an object"],
            ["contract.requestedOperation"],
        )
    fields = ("target", "action", "environment", "effect")
    if any(
        not isinstance(operation.get(field), str) or not operation[field].strip()
        for field in fields
    ) or not isinstance(operation.get("authorityRequired"), bool):
        return _signal(
            "Intent Capability",
            "Inconsistent",
            [
                "requestedOperation must contain target/action/environment/effect and boolean authorityRequired"
            ],
            ["contract.requestedOperation"],
        )
    payload, issues = _load_json(path)
    if issues or not isinstance(payload, dict):
        return _signal(
            "Intent Capability",
            "Inconsistent",
            issues or ["capability declaration must be an object"],
            [display_path(path)],
        )
    try:
        validate_payload("repository_capabilities", payload)
    except ValidationError as exc:
        return _signal("Intent Capability", "Inconsistent", [str(exc)], [display_path(path)])
    mappings = payload.get("operationMappings")
    key = f"{operation['target']}.{operation['action']}"
    try:
        operation_policy = parse_yaml(REQUESTED_OPERATION_POLICY_PATH)
    except ValueError as exc:
        return _signal(
            "Intent Capability",
            "Inconsistent",
            [str(exc)],
            [display_path(REQUESTED_OPERATION_POLICY_PATH)],
        )
    operations = operation_policy.get("operations")
    policy = operations.get(key) if isinstance(operations, dict) else None
    if (
        not isinstance(policy, dict)
        or operation["environment"] not in policy.get("environments", [])
        or operation["effect"] not in policy.get("effects", [])
        or str(policy.get("authorityRequired", "false")).lower() == "true"
        and not operation["authorityRequired"]
    ):
        return _signal(
            "Intent Capability",
            "Inconsistent",
            [f"requestedOperation combination is not allowed by policy: {key}"],
            [display_path(REQUESTED_OPERATION_POLICY_PATH), "contract.requestedOperation"],
        )
    if operation["authorityRequired"] and not isinstance(contract.get("authorityEvidence"), dict):
        return _signal(
            "Intent Capability",
            "Inconsistent",
            ["authorityEvidence is required when authorityRequired is true"],
            ["contract.requestedOperation", "contract.authorityEvidence"],
        )
    required = mappings.get(key) if isinstance(mappings, dict) else None
    if not isinstance(required, list) or not required:
        return _signal(
            "Intent Capability",
            "Inconsistent",
            [f"requestedOperation is not mapped by repository policy: {key}"],
            [display_path(path), "contract.requestedOperation"],
        )
    missing = sorted(set(required) - set(payload["capabilities"]))
    if missing:
        return _signal(
            "Intent Capability",
            "Partial",
            [f"operation policy requires undeclared capability: {item}" for item in missing],
            [display_path(path), f"operationMappings.{key}"],
        )
    return _signal(
        "Intent Capability",
        "Ready",
        [f"operation policy {key} derives required capabilities: {', '.join(required)}"],
        [
            display_path(REQUESTED_OPERATION_POLICY_PATH),
            display_path(path),
            f"operationMappings.{key}",
            "contract.requestedOperation",
        ],
    )


_AMBIGUOUS_TERMS = re.compile(
    r"\b(?:something|maybe|somehow|as appropriate|if needed|etc)\b", re.IGNORECASE
)
_UNDERSPECIFIED_TERMS = re.compile(
    r"(?:\bmake\s+(?:it|this)\s+better\b|\bimprove\s+(?:it|this)\b|随便改改|大概改一下)",
    re.IGNORECASE,
)

# This is deliberately a small boundary vocabulary.  Broad multilingual and hidden-risk
# interpretation is a separate Work Item; these explicit examples prevent scope-path
# relabeling from turning an obviously unsupported physical request into documentation work.
_UNSUPPORTED_OPERATION_TERMS = (
    "build a rocket",
    "make a rocket",
    "manufacture a rocket",
    "造一枚火箭",
    "制造火箭",
    "帮我制造武器",
    "制作炸弹",
    "build a bomb",
    "make a weapon",
    "ロケットを作って",
    "爆弾を作って",
    "ロケットを作成してください",
    "ロケット製造を手伝ってください",
    "请帮我造一枚火箭",
    "帮我设计并制造火箭",
    "please help me build a rocket",
    "could you build a rocket for me",
    "make every transaction succeed",
    "approve every transaction automatically",
    "让所有交易都成功",
    "すべての取引を必ず成功させて",
)

_RAW_REQUEST_EXEMPTIONS = {
    "system_maintenance",
    "dependency_upgrade",
    "release_metadata",
    "internal_governance",
}
_RAW_REQUEST_SOURCE_TYPES = {"human", "issue", "pr_comment", "system"}
_RAW_REQUEST_EXEMPTION_FIELDS = {
    "exemption",
    "policyRef",
    "triggerRef",
    "applicability",
    "approvedBy",
}
_RAW_REQUEST_TRIGGER_REFS = {
    "scheduled-maintenance",
    "automated-dependency-update",
    "release-automation",
    "internal-governance",
}
_RAW_REQUEST_APPLICABILITY = {"repository", "sandbox", "test"}


def _structured_exemption(value: Any, contract: dict[str, Any] | None = None) -> bool:
    return (
        isinstance(value, dict)
        and value.get("exemption") in _RAW_REQUEST_EXEMPTIONS
        and set(value) == _RAW_REQUEST_EXEMPTION_FIELDS
        and value.get("policyRef") == "raw-request-exemptions.v1"
        and isinstance(value.get("triggerRef"), str)
        and value["triggerRef"] in _RAW_REQUEST_TRIGGER_REFS
        and isinstance(value.get("applicability"), list)
        and bool(value["applicability"])
        and set(value["applicability"]).issubset(_RAW_REQUEST_APPLICABILITY)
        and isinstance(value.get("approvedBy"), str)
        and bool(value["approvedBy"].strip())
        and (contract is None or contract.get("riskAssessment", {}).get("level") != "high")
    )


def _requires_raw_request(contract: dict[str, Any]) -> bool:
    scope = contract.get("scope")
    return (
        contract.get("contractVersion") == 2
        and contract.get("mode") == "code"
        and isinstance(scope, list)
        and any(isinstance(item, str) and ".ai/work-items/active/" in item for item in scope)
    )


def _raw_request_source_issues(contract: dict[str, Any]) -> list[str]:
    source = contract.get("rawRequestSource")
    if not isinstance(source, dict):
        return ["rawRequestSource must be declared when rawUserRequest is present"]
    issues: list[str] = []
    if source.get("type") not in _RAW_REQUEST_SOURCE_TYPES:
        issues.append("rawRequestSource.type must be human, issue, pr_comment, or system")
    for field in ("reference", "capturedAt", "digest"):
        if not isinstance(source.get(field), str) or not source[field].strip():
            issues.append(f"rawRequestSource.{field} must be a non-empty string")
    return issues


def raw_request_signal(contract: dict[str, Any], path: Path = CAPABILITIES_PATH) -> dict[str, Any]:
    """Bind raw request evidence to declared intent and repository boundaries."""
    raw = contract.get("rawUserRequest")
    if raw is None:
        exemption = contract.get("rawRequestExemption")
        if _requires_raw_request(contract) and not _structured_exemption(exemption, contract):
            return _signal(
                "Raw Request",
                "Inconsistent",
                ["rawUserRequest is required for MODE=code Work Items"],
                ["contract.rawUserRequest", "contract.rawRequestExemption"],
            )
        if _structured_exemption(exemption, contract):
            exemption_name = cast(dict[str, Any], exemption)["exemption"]
            return _signal(
                "Raw Request",
                "Not Applicable",
                [f"rawUserRequest is exempted for {exemption_name}"],
                ["contract.rawRequestExemption"],
            )
        return _signal(
            "Raw Request",
            "Not Applicable",
            ["no rawUserRequest is declared"],
            ["contract.rawUserRequest"],
        )
    if not isinstance(raw, str) or not raw.strip():
        return _signal(
            "Raw Request",
            "Inconsistent",
            ["rawUserRequest must be a non-empty string"],
            ["contract.rawUserRequest"],
        )
    source_issues = _raw_request_source_issues(contract)
    if source_issues:
        return _signal("Raw Request", "Inconsistent", source_issues, ["contract.rawRequestSource"])
    payload, issues = _load_json(path)
    if issues or not isinstance(payload, dict):
        return _signal(
            "Raw Request",
            "Inconsistent",
            issues or ["capability declaration must be an object"],
            [display_path(path)],
        )
    try:
        validate_payload("repository_capabilities", payload)
    except ValidationError as exc:
        return _signal("Raw Request", "Inconsistent", [str(exc)], [display_path(path)])
    declared = contract.get("declaredIntent")
    if not isinstance(declared, dict) or not isinstance(
        declared.get("requestedCapabilities"), list
    ):
        return _signal(
            "Raw Request",
            "Inconsistent",
            ["declaredIntent.requestedCapabilities must be declared before capability inference"],
            ["contract.declaredIntent"],
        )
    requested = {item for item in declared["requestedCapabilities"] if isinstance(item, str)}
    capabilities = set(payload["capabilities"])
    missing = sorted(requested - capabilities)
    if missing:
        return _signal(
            "Raw Request",
            "Inconsistent",
            [f"raw request declares unsupported capability: {item}" for item in missing],
            [display_path(path), "contract.declaredIntent"],
        )
    normalized = raw.casefold()
    matches = [term for term in _UNSUPPORTED_OPERATION_TERMS if term.casefold() in normalized]
    if matches:
        return _signal(
            "Raw Request",
            "Inconsistent",
            [
                f"unsupported operation must not be reframed as repository work: {', '.join(matches)}"
            ],
            ["contract.rawUserRequest", display_path(path)],
        )
    return _signal(
        "Raw Request",
        "Ready",
        ["raw request, declared intent, and repository capabilities are aligned"],
        ["contract.rawUserRequest", "contract.declaredIntent", display_path(path)],
    )


def intent_guard_signal(contract: dict[str, Any]) -> dict[str, Any]:
    intent = contract.get("intent")
    if not isinstance(intent, dict):
        return _signal(
            "Intent Guard", "Missing", ["contract.intent is missing"], ["contract.intent"]
        )
    values = [intent.get("problem"), intent.get("rationale")]
    constraints = intent.get("constraints", [])
    text = " ".join(str(item) for item in values if isinstance(item, str))
    text += " " + " ".join(str(item) for item in constraints if isinstance(item, str))
    ambiguous = _AMBIGUOUS_TERMS.search(text)
    underspecified = _UNDERSPECIFIED_TERMS.search(text)
    if ambiguous or underspecified:
        evidence = [
            "intent contains ambiguous wording that cannot determine implementation behavior"
        ]
        if underspecified:
            missing = [
                label
                for field, label in (
                    ("target", "target"),
                    ("expectedOutcome", "expected outcome"),
                    ("successEvidence", "measurable success evidence"),
                )
                if not isinstance(intent.get(field), str) or not intent[field].strip()
            ]
            if missing:
                evidence.append(
                    "underspecified request is missing evidence categories: " + ", ".join(missing)
                )
            else:
                evidence.append(
                    "legacy ambiguity remains reviewable but requires explicit reviewer attention"
                )
        return _signal(
            "Intent Guard",
            "Partial",
            evidence,
            ["contract.intent"],
        )
    return _signal(
        "Intent Guard",
        "Ready",
        ["intent is specific enough for deterministic review"],
        ["contract.intent"],
    )


def constraint_conflict_signal(contract: dict[str, Any]) -> dict[str, Any]:
    constraints = contract.get("intent", {}).get("constraints", [])
    if not isinstance(constraints, list):
        return _signal(
            "Constraint Guard",
            "Inconsistent",
            ["intent.constraints must be a list"],
            ["contract.intent.constraints"],
        )
    normalized = [
        str(item).strip().lower() for item in constraints if isinstance(item, str) and item.strip()
    ]
    conflicts: list[str] = []
    for left in normalized:
        for right in normalized:
            if left == right:
                continue
            left_tokens = set(re.findall(r"[a-z0-9_]+", left))
            right_tokens = set(re.findall(r"[a-z0-9_]+", right))
            shared = left_tokens & right_tokens
            if shared and (
                ("must not" in left and "must" in right) or ("never" in left and "always" in right)
            ):
                conflicts.append(
                    f"conflicting constraints share target terms: {', '.join(sorted(shared))}"
                )
    if conflicts:
        return _signal(
            "Constraint Guard",
            "Inconsistent",
            sorted(set(conflicts)),
            ["contract.intent.constraints"],
        )
    return _signal(
        "Constraint Guard",
        "Ready",
        ["declared constraints do not conflict"],
        ["contract.intent.constraints"],
    )


def success_criteria_signal(
    contract: dict[str, Any], path: Path = SUCCESS_CRITERIA_PATH
) -> dict[str, Any]:
    work_item_id = contract.get("workItemId")
    if path == SUCCESS_CRITERIA_PATH and isinstance(work_item_id, str):
        owned_path = PROJECT_ROOT / ".ai" / "work-items" / "active" / f"{work_item_id}.success.json"
    else:
        owned_path = (
            path.parent / f"{work_item_id}.success.json" if isinstance(work_item_id, str) else path
        )
    criteria_path = owned_path if owned_path.is_file() else path
    payload, issues = _load_json(criteria_path)
    if issues:
        return _signal("Success Criteria", "Missing", issues, [display_path(criteria_path)])
    if not isinstance(payload, dict):
        return _signal(
            "Success Criteria",
            "Inconsistent",
            ["declaration must be an object"],
            [display_path(criteria_path)],
        )
    try:
        validate_payload("success_criteria", payload)
    except ValidationError as exc:
        return _signal(
            "Success Criteria", "Inconsistent", [str(exc)], [display_path(criteria_path)]
        )
    if payload["workItemId"] != contract.get("workItemId"):
        return _signal(
            "Success Criteria",
            "Not Applicable",
            [f"no project-owned criteria are assigned to {contract.get('workItemId', '')}"],
            [display_path(criteria_path)],
        )
    criteria = payload["criteria"]
    incomplete = [item["id"] for item in criteria if not item.get("evidenceHints")]
    if incomplete:
        return _signal(
            "Success Criteria",
            "Partial",
            [f"criteria lack evidence hints: {', '.join(incomplete)}"],
            [display_path(criteria_path)],
        )
    return _signal(
        "Success Criteria",
        "Ready",
        [f"{len(criteria)} criteria have statements and evidence hints"],
        [display_path(criteria_path)],
    )


def trust_signals(contract: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        raw_request_signal(contract),
        intent_capability_signal(contract),
        capability_signal(contract),
        intent_guard_signal(contract),
        constraint_conflict_signal(contract),
        success_criteria_signal(contract),
        *critical_domain_signals(contract),
    ]

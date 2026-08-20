"""Generate an evidence-derived Task Outcome JSON and Markdown view.

The generator is intentionally a small, deterministic transformation. It does
not validate schema bindings; the validator Work Item owns that responsibility.
"""

from __future__ import annotations

import argparse
import json
import re
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

GENERATOR_VERSION = "1.2"
FINAL_STATUSES = {
    "completed",
    "completed_with_warnings",
    "needs_human_confirmation",
    "blocked",
    "cancelled",
}
SECTION_TITLES = {
    "outcomeSummary": "Outcome Summary",
    "taskOverview": "Task Overview",
    "deliveredChanges": "Delivered Changes",
    "findings": "Findings",
    "risks": "Risks",
    "warnings": "Warnings",
    "limitations": "Limitations",
    "nonRiskExplanations": "Non-Risk Explanations",
    "forbiddenClaims": "Forbidden Claims",
    "interventions": "Interventions",
    "forcedStops": "Forced Stops",
    "resolutions": "Resolutions",
    "recurrencePrevention": "Recurrence Prevention",
    "avoidedImpact": "Avoided Impact",
    "residualRisks": "Residual Risks",
    "humanDecisions": "Human Decisions",
    "evidence": "Evidence",
    "implementationApproach": "Implementation Approach",
}
SECRET_KEY = re.compile(
    r"(password|passwd|secret|token|api[_-]?key|private[_-]?key)", re.IGNORECASE
)
SUPPORTED_LOCALES = {"en", "ja", "zh-CN"}


def _safe_text(value: Any, default: str = "") -> str:
    return value.strip() if isinstance(value, str) and value.strip() else default


def _evidence_refs(value: Any, fallback: str) -> list[dict[str, str]]:
    if not isinstance(value, list):
        return []
    refs: list[dict[str, str]] = []
    for item in value:
        if isinstance(item, dict):
            source = _safe_text(item.get("source"), fallback)
            subject = _safe_text(item.get("subject"), "evidence")
            ref = {"source": source, "subject": subject}
            digest = item.get("digest")
            if isinstance(digest, str) and re.fullmatch(r"[a-f0-9]{64}", digest):
                ref["digest"] = digest
            refs.append(ref)
        elif isinstance(item, str) and item.strip():
            refs.append({"source": fallback, "subject": item.strip()})
    return refs


def _load_summary_approach(evidence: Mapping[str, Any]) -> Mapping[str, Any] | None:
    """Read the approach only from an explicitly supplied Summary evidence source."""

    for item in evidence.get("sources", []) if isinstance(evidence.get("sources"), list) else []:
        if not isinstance(item, Mapping):
            continue
        source = item.get("source")
        if not isinstance(source, str) or not source.endswith(".summary.json"):
            continue
        try:
            summary = json.loads(Path(source).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        for key in ("implementationApproach", "configurationApproach"):
            candidate = summary.get(key)
            if isinstance(candidate, Mapping):
                return dict(candidate)
    return None


def _summary_source_present(evidence: Mapping[str, Any]) -> bool:
    sources = evidence.get("sources")
    if not isinstance(sources, list):
        return False
    return any(
        isinstance(item, Mapping)
        and isinstance(item.get("source"), str)
        and item["source"].endswith(".summary.json")
        for item in sources
    )


def _legacy_summary_contract_has_no_approach_signal(evidence: Mapping[str, Any]) -> bool:
    """Keep pre-approach Contracts readable without hiding new applicability gaps."""

    sources = evidence.get("sources")
    if not isinstance(sources, list):
        return False
    for item in sources:
        if not isinstance(item, Mapping):
            continue
        source = item.get("source")
        if not isinstance(source, str) or not source.endswith(".contract.json"):
            continue
        try:
            contract = json.loads(Path(source).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            # Some archived projection tests intentionally bind relative
            # sources to an isolated repository root outside the process cwd.
            # Keep those historic records readable; ai_finish resolves and
            # validates its current Contract before projecting new records.
            return True
        raw_request = contract.get("rawUserRequest") if isinstance(contract, Mapping) else None
        return not isinstance(raw_request, str) or not raw_request.strip()
    return False


def _incomplete_approach() -> dict[str, Any]:
    return {
        "approachType": "implementation",
        "status": "incomplete",
        "summary": {
            "text": "Implementation Approach was not recorded; the customer-facing explanation remains incomplete.",
            "status": "unknown",
            "evidence": [],
        },
        "mechanism": {
            "text": "The mechanism is unknown until the Summary records an evidence-bound approach.",
            "status": "unknown",
            "evidence": [],
        },
        "affectedComponents": [],
        "designDecisions": [],
        "technicalDetails": [],
        "evidence": [],
    }


def _not_applicable_approach() -> dict[str, Any]:
    approach = _incomplete_approach()
    approach["status"] = "not_applicable"
    approach["summary"]["text"] = (
        "No Summary approach source was supplied for this standalone projection."
    )
    approach["mechanism"]["text"] = (
        "Implementation Approach applicability is not determined by this standalone input."
    )
    return approach


def _approach_claim_statuses(value: Any) -> list[str]:
    statuses: list[str] = []
    if isinstance(value, Mapping):
        if value.get("status") in {"verified", "unverified", "unknown"}:
            statuses.append(str(value["status"]))
        for child in value.values():
            statuses.extend(_approach_claim_statuses(child))
    elif isinstance(value, list):
        for child in value:
            statuses.extend(_approach_claim_statuses(child))
    return statuses


def _render_implementation_approach(approach: Mapping[str, Any]) -> list[str]:
    def claim_text(value: Any, fallback: str) -> str:
        if not isinstance(value, Mapping):
            return fallback
        text = _safe_text(value.get("text"))
        if text:
            return text
        return _safe_text(value.get("detail"), _safe_text(value.get("decision"), fallback))

    def status(value: Any) -> str:
        return (
            _safe_text(value.get("status"), "unknown") if isinstance(value, Mapping) else "unknown"
        )

    lines = [
        f"Status: `{_safe_text(approach.get('status'), 'incomplete')}`",
        f"Customer summary ({status(approach.get('summary'))}): {claim_text(approach.get('summary'), 'None recorded.')}",
        f"Mechanism ({status(approach.get('mechanism'))}): {claim_text(approach.get('mechanism'), 'None recorded.')}",
        "",
        "Affected components",
    ]
    components = approach.get("affectedComponents", [])
    lines.extend(
        f"- {_safe_text(item.get('component'), 'Component')}: {_safe_text(item.get('detail'), 'None recorded.')} ({status(item)})"
        for item in components
        if isinstance(item, Mapping)
    ) if isinstance(components, list) and components else lines.append("- None recorded.")
    lines.extend(["", "Design decisions"])
    decisions = approach.get("designDecisions", [])
    lines.extend(
        f"- {_safe_text(item.get('decision'), 'Decision')}: {_safe_text(item.get('reason'), 'None recorded.')} ({status(item)})"
        for item in decisions
        if isinstance(item, Mapping)
    ) if isinstance(decisions, list) and decisions else lines.append("- None recorded.")
    lines.extend(["", "### Technical details"])
    details = approach.get("technicalDetails", [])
    lines.extend(
        f"- {_safe_text(item.get('topic'), 'Detail')}: {_safe_text(item.get('detail'), 'None recorded.')} ({status(item)})"
        for item in details
        if isinstance(item, Mapping)
    ) if isinstance(details, list) and details else lines.append("- None recorded.")
    lines.extend(["", "### Evidence"])
    evidence = approach.get("evidence", [])
    lines.extend(
        f"- {_safe_text(item.get('claim'), 'Claim')}: {_safe_text(item.get('source'), 'source')}#{_safe_text(item.get('subject'), 'subject')} ({status(item)})"
        for item in evidence
        if isinstance(item, Mapping)
    ) if isinstance(evidence, list) and evidence else lines.append("- None recorded.")
    return lines


def _event_sort_key(event: Mapping[str, Any]) -> tuple[str, str]:
    return (_safe_text(event.get("occurredAt")), _safe_text(event.get("eventId")))


def _event_description(event: Mapping[str, Any]) -> str:
    for key in ("description", "message", "reason", "title", "decision"):
        text = _safe_text(event.get(key))
        if text:
            return text
    return _safe_text(event.get("eventType"), "event")


def _state(event: Mapping[str, Any], default: str = "unresolved") -> str:
    value = _safe_text(event.get("state"), default)
    return (
        value
        if value in {"resolved", "mitigated", "accepted", "unresolved", "not_applicable"}
        else default
    )


def _risk(event: Mapping[str, Any], *, accepted: bool = False) -> dict[str, Any]:
    kind = _safe_text(event.get("kind"), "potential_risk")
    if kind not in {"observed_problem", "potential_risk", "prevented_event"}:
        kind = "potential_risk"
    severity = _safe_text(event.get("severity"), "medium")
    if severity not in {"informational", "low", "medium", "high", "critical"}:
        severity = "medium"
    risk = {
        "kind": kind,
        "severity": severity,
        "title": _safe_text(event.get("title"), _event_description(event)),
        "state": "accepted" if accepted else _state(event),
        "description": _event_description(event),
        "evidence": _evidence_refs(event.get("evidence"), "task-event-log"),
    }
    for key in (
        "sourceWarning",
        "affectedClaims",
        "requiredEvidence",
        "decisionOwner",
        "mitigation",
        "acceptanceStatus",
        "blockingFor",
    ):
        if key in event:
            risk[key] = event[key]
    return risk


def _conditional_impact(value: Any) -> str | None:
    impact = _safe_text(value)
    if not impact:
        return None
    if impact.lower().startswith(("if not detected", "could have", "如果未被发现")):
        return impact.rstrip(".") + "."
    return f"If not detected, could have led to {impact.rstrip('.')}."


def _text_values(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [item.strip() for item in value if isinstance(item, str) and item.strip()]


def _handoff_items(
    value: Any, fallback_source: str, *, fallback_title: str
) -> list[dict[str, Any]]:
    """Normalize handoff entries so every human item has detail and evidence."""

    if not isinstance(value, list):
        return []
    result: list[dict[str, Any]] = []
    for index, item in enumerate(value, start=1):
        if isinstance(item, Mapping):
            title = _safe_text(item.get("title"), f"{fallback_title} {index}")
            detail = _safe_text(
                item.get("detail"),
                _safe_text(item.get("description"), "Evidence-backed detail is recorded."),
            )
            refs = _evidence_refs(item.get("evidence"), fallback_source)
            result.append(
                {
                    "claim": title,
                    "title": title,
                    "detail": detail,
                    "evidenceRefs": refs,
                    "evidence": refs,
                    "inference": not bool(refs),
                }
            )
        elif isinstance(item, str) and item.strip():
            result.append(
                {
                    "claim": item.strip(),
                    "title": f"{fallback_title} {index}",
                    "detail": item.strip(),
                    "evidenceRefs": [{"source": fallback_source, "subject": item.strip()}],
                    "evidence": [{"source": fallback_source, "subject": item.strip()}],
                    "inference": False,
                }
            )
    return result


def _handoff_risks(value: Any, fallback_source: str) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    for item in value if isinstance(value, list) else []:
        if not isinstance(item, Mapping):
            continue
        severity = _safe_text(item.get("severity"), "medium")
        if severity not in {"informational", "low", "medium", "high", "critical"}:
            severity = "medium"
        state = _safe_text(item.get("state"), "unresolved")
        if state not in {"resolved", "mitigated", "accepted", "unresolved", "not_applicable"}:
            state = "unresolved"
        refs = _evidence_refs(item.get("evidenceRefs", item.get("evidence")), fallback_source)
        normalized = {
            "claim": _safe_text(item.get("claim"), _safe_text(item.get("title"), "Residual risk")),
            "severity": severity,
            "title": _safe_text(item.get("title"), "Residual risk"),
            "detail": _safe_text(
                item.get("detail"),
                _safe_text(item.get("description"), "Evidence-backed risk requires review."),
            ),
            "state": state,
            "evidenceRefs": refs,
            "evidence": refs,
            "inference": not bool(refs),
        }
        for key in (
            "sourceWarning",
            "affectedClaims",
            "requiredEvidence",
            "decisionOwner",
            "mitigation",
            "acceptanceStatus",
            "blockingFor",
        ):
            if key in item:
                normalized[key] = item[key]
        result.append(normalized)
    return result


def _structured_resolutions(value: Any, fallback_source: str) -> list[dict[str, Any]]:
    """Normalize evidence-bound resolution records for the top-level Outcome section."""
    result: list[dict[str, Any]] = []
    for item in value if isinstance(value, list) else []:
        if not isinstance(item, Mapping):
            continue
        refs = _evidence_refs(item.get("evidenceRefs", item.get("evidence")), fallback_source)
        result.append(
            {
                "problem": _safe_text(item.get("problem"), "Recorded problem"),
                "action": _safe_text(item.get("action"), "Recorded corrective action"),
                "verification": _safe_text(item.get("verification"), "Evidence review"),
                "result": _state(item, "resolved"),
                "evidenceRefs": refs,
                "evidence": refs,
                "inference": not bool(refs),
            }
        )
    return result


def _handoff_red_reasons(
    value: Any,
    *,
    status: str,
    failed_gate: str,
    recovery: str,
    fallback_source: str,
    fallback_cause: str,
) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    if isinstance(value, list):
        for item in value:
            if not isinstance(item, Mapping):
                continue
            refs = _evidence_refs(item.get("evidenceRefs", item.get("evidence")), fallback_source)
            result.append(
                {
                    "claim": _safe_text(item.get("cause"), fallback_cause),
                    "gate": _safe_text(item.get("gate"), failed_gate or "unknown"),
                    "cause": _safe_text(item.get("cause"), fallback_cause),
                    "location": _safe_text(item.get("location"), "finish pipeline"),
                    "recovery": _safe_text(
                        item.get("recovery"), recovery or "Review the failed evidence and retry."
                    ),
                    "evidenceRefs": refs,
                    "evidence": refs,
                    "inference": not bool(refs),
                }
            )
    if not result and status == "blocked":
        refs = [{"source": fallback_source, "subject": failed_gate or "blocked"}]
        result.append(
            {
                "claim": fallback_cause or "A required gate did not pass.",
                "gate": failed_gate or "unknown",
                "cause": fallback_cause or "A required gate did not pass.",
                "location": "finish pipeline",
                "recovery": recovery or "Review the failed gate and retry.",
                "evidenceRefs": refs,
                "evidence": refs,
                "inference": False,
            }
        )
    return result


def _handoff_questions(
    evidence: Mapping[str, Any],
    *,
    findings: Sequence[Mapping[str, Any]],
    risks: Sequence[Mapping[str, Any]],
    forced_stops: Sequence[Mapping[str, Any]],
    resolutions: Sequence[Mapping[str, Any]],
    avoided: Sequence[str],
    residual: Sequence[Mapping[str, Any]],
    human_decisions: Sequence[str],
    prevention: Sequence[Mapping[str, Any]],
    red_reasons: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    supplied = evidence.get("handoffQuestions")
    supplied = supplied if isinstance(supplied, Mapping) else {}
    blocked = _text_values(supplied.get("blockedProblems")) or [
        _safe_text(item.get("reason"), "Blocked by a forced stop.")
        for item in forced_stops
        if _safe_text(item.get("result")) not in {"resolved", "mitigated", "accepted"}
    ]
    if not blocked:
        blocked = [_safe_text(item.get("cause"), "Blocked by a red gate.") for item in red_reasons]
    resolved = _text_values(supplied.get("resolvedProblems")) or [
        _safe_text(item.get("problem"), "Recorded problem") for item in resolutions
    ]
    approach = _text_values(supplied.get("resolutionApproach")) or [
        _safe_text(item.get("action"), "Recorded corrective action") for item in resolutions
    ]
    remaining = _text_values(supplied.get("remainingRisks")) or [
        _safe_text(item.get("detail"), "Residual risk requires review.") for item in residual
    ]

    def claims(values: Sequence[str], source: str) -> list[dict[str, Any]]:
        return [
            {
                "claim": value,
                "evidenceRefs": [],
                "inference": True,
            }
            for value in values
            if isinstance(value, str) and value.strip()
        ]

    def supplied_claims(key: str, fallback: list[str], source: str) -> list[dict[str, Any]]:
        raw = supplied.get(key)
        if not isinstance(raw, list):
            return claims(fallback, source)
        result: list[dict[str, Any]] = []
        for item in raw:
            if isinstance(item, Mapping):
                value = _safe_text(item.get("claim"), _safe_text(item.get("detail")))
                refs = _evidence_refs(item.get("evidenceRefs", item.get("evidence")), source)
                if value:
                    result.append(
                        {"claim": value, "evidenceRefs": refs, "inference": not bool(refs)}
                    )
            elif isinstance(item, str) and item.strip():
                result.append({"claim": item.strip(), "evidenceRefs": [], "inference": True})
        return result or claims(fallback, source)

    supplied_problem_count = supplied.get("problemCount")
    if isinstance(supplied_problem_count, int) and supplied_problem_count >= 0:
        problem_count = supplied_problem_count
    else:
        problem_count = (
            len(findings) + len(risks) + len(forced_stops) + len(evidence.get("warnings", []))
        )
    recurrence = _safe_text(
        supplied.get("recurrenceLikelihood"),
        "low" if prevention else "unknown: no recurrence-prevention evidence was recorded.",
    )
    next_time = _safe_text(
        supplied.get("nextTime"),
        "Bind the conversation locale and preserve evidence details from the start of the Work Item.",
    )
    return {
        "problemCount": problem_count,
        "problemCountEvidenceRefs": _evidence_refs(
            supplied.get("problemCountEvidenceRefs"), "structured-evidence"
        ),
        "blockedProblems": supplied_claims("blockedProblems", blocked, "task-outcome"),
        "resolvedProblems": supplied_claims("resolvedProblems", resolved, "task-outcome"),
        "resolutionApproach": supplied_claims("resolutionApproach", approach, "task-outcome"),
        "avoidedRisks": supplied_claims("avoidedRisks", list(avoided), "task-outcome"),
        "remainingRisks": supplied_claims("remainingRisks", remaining, "task-outcome"),
        "agentUnknowns": supplied_claims(
            "agentUnknowns", _text_values(evidence.get("unknowns")), "contract"
        ),
        "humanConfirmations": supplied_claims(
            "humanConfirmations", list(human_decisions), "task-outcome"
        ),
        "recurrenceLikelihood": {
            "claim": recurrence,
            "evidenceRefs": _evidence_refs(supplied.get("recurrenceEvidenceRefs"), "task-outcome"),
            "inference": not bool(
                _evidence_refs(supplied.get("recurrenceEvidenceRefs"), "task-outcome")
            ),
        },
        "nextTime": {
            "claim": next_time,
            "evidenceRefs": _evidence_refs(supplied.get("nextTimeEvidenceRefs"), "task-outcome"),
            "inference": not bool(
                _evidence_refs(supplied.get("nextTimeEvidenceRefs"), "task-outcome")
            ),
        },
    }


def _status(
    evidence: Mapping[str, Any], events: Sequence[Mapping[str, Any]], warnings: list[str]
) -> str:
    requested = _safe_text(evidence.get("status"))
    if requested in FINAL_STATUSES:
        return requested
    types = {event.get("eventType") for event in events}
    if "cancelled" in types:
        return "cancelled"
    if "external_handoff_timeout" in types:
        return "blocked"
    if any(
        event.get("eventType") == "stop"
        and _state(event) not in {"resolved", "mitigated", "accepted", "not_applicable"}
        for event in events
    ):
        return "needs_human_confirmation"
    return "completed_with_warnings" if warnings else "completed"


def _human_status_color(status: str) -> str:
    """Return the canonical human diagnostic color for an Outcome status."""

    if status == "completed":
        return "green"
    if status in {"completed_with_warnings", "needs_human_confirmation"}:
        return "yellow"
    return "red"


def generate_outcome(
    task_id: str,
    bindings: Mapping[str, Any],
    *,
    events: Sequence[Mapping[str, Any]] = (),
    evidence: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Build one deterministic Outcome object from structured evidence/events."""

    evidence = evidence or {}
    approach = evidence.get("implementationApproach")
    if not isinstance(approach, Mapping):
        approach = evidence.get("configurationApproach")
    if not isinstance(approach, Mapping):
        approach = _load_summary_approach(evidence)
    if not isinstance(approach, Mapping):
        approach = (
            _not_applicable_approach()
            if _legacy_summary_contract_has_no_approach_signal(evidence)
            else _incomplete_approach()
            if _summary_source_present(evidence)
            else _not_applicable_approach()
        )
    else:
        approach = dict(approach)
    ordered = sorted((dict(event) for event in events), key=_event_sort_key)
    findings: list[dict[str, Any]] = []
    finding_keys: set[str] = set()
    risks: list[dict[str, Any]] = []
    warnings: list[str] = [
        item.strip()
        for item in evidence.get("warnings", [])
        if isinstance(item, str) and item.strip()
    ]
    interventions: list[dict[str, Any]] = []
    forced_stops: list[dict[str, Any]] = []
    resolutions: list[dict[str, Any]] = _structured_resolutions(
        evidence.get("resolutions"), "structured-evidence"
    )
    prevention: list[dict[str, Any]] = []
    avoided: list[str] = []
    human_decisions: list[str] = [
        item.strip()
        for item in evidence.get("humanDecisions", [])
        if isinstance(item, str) and item.strip()
    ]
    limitations = [
        dict(item) for item in evidence.get("limitations", []) if isinstance(item, Mapping)
    ]
    non_risk_explanations = [
        dict(item) for item in evidence.get("nonRiskExplanations", []) if isinstance(item, Mapping)
    ]
    forbidden_claims = [
        item.strip()
        for item in evidence.get("forbiddenClaims", [])
        if isinstance(item, str) and item.strip()
    ]
    approach_statuses = _approach_claim_statuses(approach)
    approach_warning = None
    if approach.get("status") == "incomplete":
        approach_warning = "Implementation Approach is incomplete; do not claim the implementation is fully explained."
    elif approach.get("status") == "complete" and any(
        status in {"unverified", "unknown"} for status in approach_statuses
    ):
        approach_warning = "Implementation Approach contains unverified or unknown claims; do not treat them as confirmed facts."
    if approach_warning:
        warnings.append(approach_warning)
        limitations.append(
            {
                "sourceWarning": approach_warning,
                "title": "Implementation Approach evidence is incomplete",
                "affectedClaims": ["implementation_mechanism"],
                "requiredEvidence": ["code, configuration, dependency, or test evidence"],
                "forbiddenClaims": ["Do not claim the Implementation Approach is fully verified."],
            }
        )
        forbidden_claims.append("Do not claim the Implementation Approach is fully verified.")
        non_risk_explanations.append(
            {
                "sourceWarning": approach_warning,
                "reason": "The approach record is retained as an explicit knowledge-completeness warning.",
                "evidence": [],
            }
        )
    all_evidence: list[dict[str, str]] = _evidence_refs(
        evidence.get("evidence"), "structured-evidence"
    )
    all_evidence.extend(_evidence_refs(evidence.get("sources"), "structured-evidence"))
    all_evidence.extend(_evidence_refs(approach.get("evidence"), "implementation-approach"))
    publication = evidence.get("publication")
    if isinstance(publication, dict):
        tag = _safe_text(publication.get("tag"), "published-release")
        digest = publication.get("assetDigest")
        ref = {
            "source": "release-workflow",
            "subject": tag,
        }
        if isinstance(digest, str) and re.fullmatch(r"[a-f0-9]{64}", digest):
            ref["digest"] = digest
        all_evidence.append(ref)

    for event in ordered:
        event_type = event.get("eventType")
        refs = _evidence_refs(event.get("evidence"), "task-event-log")
        all_evidence.extend(refs)
        if event_type == "finding":
            fingerprint = _safe_text(
                event.get("findingFingerprint"), _safe_text(event.get("eventId"), "finding")
            )
            key = (
                fingerprint
                if event.get("recurrence") != "post_fix"
                else f"{fingerprint}:{event.get('eventId')}"
            )
            if key in finding_keys:
                continue
            finding_keys.add(key)
            category = _safe_text(event.get("category"), "other")
            if category not in {
                "gap",
                "defect",
                "evidence",
                "security",
                "installer",
                "release",
                "process",
                "other",
            }:
                category = "other"
            severity = _safe_text(event.get("severity"), "medium")
            if severity not in {"informational", "low", "medium", "high", "critical"}:
                severity = "medium"
            findings.append(
                {
                    "findingFingerprint": fingerprint,
                    "category": category,
                    "severity": severity,
                    "title": _safe_text(event.get("title"), _event_description(event)),
                    "state": _state(event),
                    "description": _event_description(event),
                    "evidence": refs,
                }
            )
        elif event_type == "risk":
            risks.append(_risk(event))
        elif event_type == "risk-accepted":
            risks.append(_risk(event, accepted=True))
        elif event_type == "warning":
            warnings.append(_event_description(event))
        elif event_type in {"confirmation", "resume"}:
            decision = _safe_text(event.get("decision"), _event_description(event))
            if decision:
                human_decisions.append(decision)
        elif event_type == "stop":
            forced_stops.append(
                {
                    "stage": _safe_text(event.get("stage"), "unknown"),
                    "reason": _safe_text(event.get("reason"), _event_description(event)),
                    "policyOrGuard": _safe_text(event.get("policyOrGuard"), "governance guard"),
                    "attemptedAction": _safe_text(
                        event.get("attemptedAction"), "continue execution"
                    ),
                    "conditionalImpact": _safe_text(
                        _conditional_impact(event.get("avoidedImpact"))
                    ),
                    "handoff": _safe_text(event.get("handoff")),
                    "humanDecision": _safe_text(event.get("humanDecision")),
                    "recovery": _safe_text(event.get("recovery")),
                    "result": _state(event),
                    "evidence": refs,
                }
            )
        elif event_type == "resolution":
            resolutions.append(
                {
                    "problem": _safe_text(event.get("problem"), _event_description(event)),
                    "action": _safe_text(event.get("action"), "Recorded corrective action"),
                    "verification": _safe_text(event.get("verification"), "Evidence review"),
                    "result": _state(event, "resolved"),
                    "evidence": refs,
                }
            )
        elif event_type == "prevention":
            kind = _safe_text(event.get("kind"), "None")
            if kind not in {
                "None",
                "Documentation",
                "Test",
                "Automated Check",
                "Structural Prevention",
            }:
                kind = "None"
            prevention.append(
                {
                    "kind": kind,
                    "coverage": _safe_text(event.get("coverage"), "No coverage claim recorded"),
                    "limits": _safe_text(event.get("limits")),
                    "humanDependency": _safe_text(
                        event.get("humanDependency"), "Human review remains required"
                    ),
                }
            )
        if event_type in {"stop", "intervention", "risk", "resolution"}:
            impact = _conditional_impact(event.get("avoidedImpact"))
            if impact and refs:
                avoided.append(impact)
        if event_type == "intervention":
            kind = _safe_text(event.get("kind"), "observed")
            if kind not in {"observed", "warned", "intervened", "prevented"}:
                kind = "observed"
            interventions.append(
                {
                    "kind": kind,
                    "title": _safe_text(event.get("title"), _event_description(event)),
                    "description": _event_description(event),
                    "evidence": refs,
                }
            )

    human_decisions = list(dict.fromkeys(human_decisions))
    residual = [risk for risk in risks if risk["state"] in {"accepted", "unresolved"}]
    structured_risks = _handoff_risks(evidence.get("handoffRisks"), "summary")
    if structured_risks:
        residual.extend(
            risk for risk in structured_risks if risk["state"] in {"accepted", "unresolved"}
        )

    def unique_refs(refs: list[dict[str, str]]) -> list[dict[str, str]]:
        return list({json.dumps(ref, sort_keys=True): ref for ref in refs}.values())

    status = _status(evidence, ordered, warnings)
    delivered = [
        item
        for item in evidence.get("deliveredChanges", evidence.get("changedFiles", []))
        if isinstance(item, str)
    ]
    sections: dict[str, Any] = {
        "outcomeSummary": _safe_text(
            evidence.get("outcomeSummary"),
            f"Task {task_id} generated an evidence-derived outcome with status {status}.",
        ),
        "taskOverview": _safe_text(evidence.get("taskOverview"), f"Governed Work Item: {task_id}"),
        "deliveredChanges": delivered,
        "findings": findings,
        "risks": risks,
        "warnings": sorted(set(warnings)),
        "limitations": limitations,
        "nonRiskExplanations": non_risk_explanations,
        "forbiddenClaims": sorted(set(forbidden_claims)),
        "interventions": interventions,
        "forcedStops": forced_stops,
        "resolutions": resolutions,
        "recurrencePrevention": prevention,
        "avoidedImpact": sorted(set(avoided)),
        "residualRisks": residual,
        "humanDecisions": human_decisions,
        "evidence": unique_refs(all_evidence),
        "implementationApproach": approach,
    }
    locale = _safe_text(evidence.get("locale"), _safe_text(bindings.get("locale"), "en"))
    if locale not in SUPPORTED_LOCALES:
        raise ValueError(f"unsupported Outcome locale: {locale}")
    completed = _handoff_items(
        evidence.get("completed"), "structured-evidence", fallback_title="Completed item"
    )
    if not completed:
        refs = unique_refs(all_evidence)
        completed = [
            {
                "claim": "Governed Outcome recorded",
                "title": "Governed Outcome recorded",
                "detail": _safe_text(
                    evidence.get("outcomeSummary"),
                    "No file change was declared; the recorded governance evidence is the bounded result.",
                ),
                "evidenceRefs": refs,
                "evidence": refs,
                "inference": not bool(refs),
            }
        ]
    passed = _handoff_items(
        evidence.get("passedChecks"), "verification", fallback_title="Passed check"
    )
    if not passed:
        refs = unique_refs(all_evidence)
        passed = [
            {
                "claim": "Outcome validation",
                "title": "Outcome validation",
                "detail": "The evidence-derived Outcome was generated for review.",
                "evidenceRefs": refs,
                "evidence": refs,
                "inference": not bool(refs),
            }
        ]
    retained = _handoff_items(evidence.get("retained"), "summary", fallback_title="Retained item")
    if not retained:
        retained = _handoff_items(
            [
                {
                    "title": "Warning retained",
                    "detail": warning,
                    "evidence": [{"source": "summary", "subject": "knownGaps"}],
                }
                for warning in sorted(set(warnings))
            ],
            "summary",
            fallback_title="Retained item",
        )
    handoff_risks = _handoff_risks(evidence.get("handoffRisks"), "summary")
    if not handoff_risks:
        handoff_risks = _handoff_risks(residual, "task-outcome")
    failed_gate = _safe_text(evidence.get("failedGate"))
    recovery_condition = _safe_text(evidence.get("recoveryCondition"))
    red_reasons = _handoff_red_reasons(
        evidence.get("redReasons"),
        status=status,
        failed_gate=failed_gate,
        recovery=recovery_condition,
        fallback_source="task-outcome",
        fallback_cause=warnings[-1] if warnings else "A required gate did not pass.",
    )
    handoff = {
        "locale": locale,
        "completed": completed,
        "passed": passed,
        "retained": retained,
        "risks": handoff_risks,
        "redReasons": red_reasons,
        "questions": _handoff_questions(
            evidence,
            findings=findings,
            risks=risks,
            forced_stops=forced_stops,
            resolutions=resolutions,
            avoided=avoided,
            residual=residual,
            human_decisions=human_decisions,
            prevention=prevention,
            red_reasons=red_reasons,
        ),
    }
    canonical_bindings = dict(bindings)
    canonical_bindings["generatorVersion"] = GENERATOR_VERSION
    return {
        "format": "ai-cockpit-task-outcome",
        "schemaVersion": 1,
        "workItemId": task_id,
        "status": status,
        "humanStatusColor": _human_status_color(status),
        "failedGate": failed_gate,
        "recoveryCondition": recovery_condition,
        "bindings": canonical_bindings,
        "sections": sections,
        "humanHandoff": handoff,
    }


def render_markdown(outcome: Mapping[str, Any]) -> str:
    """Render Markdown as a derived view; empty sections are explicitly None."""

    sections = outcome["sections"]
    lines = [
        f"# Task Outcome: {outcome['workItemId']}",
        "",
        f"Status: `{outcome['status']}`",
        f"Human Status: `{outcome.get('humanStatusColor', 'unknown')}`",
    ]
    failed_gate = _safe_text(outcome.get("failedGate"))
    recovery = _safe_text(outcome.get("recoveryCondition"))
    if failed_gate:
        lines.append(f"Failed Gate: `{failed_gate}`")
    if recovery:
        lines.append(f"Recovery Condition: {recovery}")
    handoff = outcome.get("humanHandoff")
    if isinstance(handoff, Mapping):
        lines.extend(["", "## Human Handoff", f"Locale: `{handoff.get('locale', 'unknown')}`"])
        for key, title in (
            ("completed", "What was completed"),
            ("passed", "What passed"),
            ("retained", "What was retained"),
            ("risks", "Risks"),
            ("redReasons", "Red reasons"),
        ):
            lines.extend([f"### {title}"])
            values = handoff.get(key, [])
            if not isinstance(values, list) or not values:
                lines.append("None")
            else:
                for item in values:
                    if isinstance(item, Mapping):
                        detail = _safe_text(
                            item.get("detail"), json.dumps(item, ensure_ascii=False, sort_keys=True)
                        )
                        title_value = _safe_text(
                            item.get("title"), _safe_text(item.get("gate"), title)
                        )
                        lines.append(f"- {title_value}: {detail}")
                    else:
                        lines.append(f"- {item}")
        questions = handoff.get("questions")
        if isinstance(questions, Mapping):
            lines.extend(["### Human questions"])
            for key in (
                "problemCount",
                "blockedProblems",
                "resolvedProblems",
                "resolutionApproach",
                "avoidedRisks",
                "remainingRisks",
                "agentUnknowns",
                "humanConfirmations",
                "recurrenceLikelihood",
                "nextTime",
            ):
                value = questions.get(key)
                if isinstance(value, list):
                    rendered = "; ".join(str(item) for item in value) if value else "None"
                else:
                    rendered = str(value) if value not in (None, "") else "None"
                lines.append(f"- {key}: {rendered}")
        lines.append("")
    lines.append("")
    for key, title in SECTION_TITLES.items():
        lines.extend([f"## {title}"])
        value = sections[key]
        if key == "implementationApproach":
            lines.extend(
                _render_implementation_approach(value if isinstance(value, Mapping) else {})
            )
            lines.append("")
            continue
        if isinstance(value, list):
            if not value:
                lines.append("None")
            else:
                for item in value:
                    if isinstance(item, dict):
                        if key == "resolutions":
                            problem = _safe_text(item.get("problem"), "Recorded problem")
                            action = _safe_text(item.get("action"), "Recorded corrective action")
                            verification = _safe_text(item.get("verification"), "Evidence review")
                            lines.append(f"- {problem}: {action} (Verification: {verification})")
                            continue
                        if key == "residualRisks":
                            title = _safe_text(item.get("title"), "Residual risk")
                            detail = _safe_text(
                                item.get("detail"), "Evidence-backed risk requires review."
                            )
                            lines.append(f"- {title}: {detail}")
                            continue
                        lines.append(
                            f"- {item.get('title', item.get('subject', item.get('kind', json.dumps(item, sort_keys=True))))}"
                        )
                    else:
                        lines.append(f"- {item}")
        else:
            lines.append(value or "None")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def _main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "input",
        type=Path,
        help="Evidence JSON containing taskId, bindings, events, and optional evidence",
    )
    parser.add_argument("json_output", type=Path)
    parser.add_argument("markdown_output", type=Path)
    args = parser.parse_args()
    payload = json.loads(args.input.read_text(encoding="utf-8"))
    result = generate_outcome(
        payload["taskId"],
        payload["bindings"],
        events=payload.get("events", []),
        evidence=payload.get("evidence"),
    )
    args.json_output.write_text(
        json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )
    args.markdown_output.write_text(render_markdown(result), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())

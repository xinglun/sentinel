"""Pure Intent Alignment signal policy."""
# mypy: ignore-errors

from __future__ import annotations

from typing import Any


def intent_alignment_signal(contract: dict[str, Any], summary: dict[str, Any]) -> dict[str, Any]:
    intent = contract.get("intent") if isinstance(contract.get("intent"), dict) else {}
    meaningful = any(
        isinstance(intent.get(k), str) and intent[k].strip() for k in ("problem", "rationale")
    ) or any(isinstance(x, str) and x.strip() for x in intent.get("constraints", []))
    if not meaningful:
        return {
            "value": "not_applicable",
            "evidence": ["contract.intent has no meaningful content"],
            "sources": ["contract.intent"],
        }
    alignment = summary.get("intentAlignment")
    if not isinstance(alignment, dict) or not alignment:
        return {
            "value": "unknown",
            "evidence": ["summary.intentAlignment is missing"],
            "sources": ["contract.intent", "summary.intentAlignment"],
        }
    fields = [
        ("problem", "problemResolved", "problemResolutionEvidence", bool(intent.get("problem"))),
        (
            "constraints",
            "constraintsRespected",
            "constraintsRespectEvidence",
            bool(intent.get("constraints")),
        ),
        ("nonGoals", "nonGoalsAvoided", None, bool(intent.get("nonGoals"))),
        ("rationale", "rationaleValidated", "rationaleValidated", bool(intent.get("rationale"))),
    ]
    unresolved = []
    unknown = []
    applicable = []
    for name, key, legacy, present in fields:
        if not present:
            continue
        applicable.append(name)
        value = alignment.get(key)
        if isinstance(value, bool):
            if not value:
                unresolved.append(name)
        elif legacy and isinstance(alignment.get(legacy), str) and alignment[legacy].strip():
            pass
        else:
            unknown.append(name)
    if unresolved:
        value, evidence = (
            "unresolved",
            [f"intent alignment unresolved for: {', '.join(unresolved)}"],
        )
    elif unknown:
        value, evidence = (
            "unknown",
            [f"intent alignment missing evidence for: {', '.join(unknown)}"],
        )
    else:
        value, evidence = "resolved", [f"intent alignment validated for: {', '.join(applicable)}"]
    return {
        "value": value,
        "evidence": evidence,
        "sources": ["contract.intent", "summary.intentAlignment"],
    }

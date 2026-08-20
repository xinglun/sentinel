#!/usr/bin/env python3
"""Persist and validate Trust Layer Human Decision Requests and Evidence."""

from __future__ import annotations

import json
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from ai_common import InvalidDataShapeError
from ai_trust_schema import ValidationError, validate_payload

PROJECT_ROOT = Path(__file__).resolve().parents[1]


def decisions_dir(root: Path = PROJECT_ROOT) -> Path:
    return root / ".ai" / "decisions"


def request_id(report: dict[str, Any]) -> str:
    """Return a stable ID derived from the contract and fresh Preflight hashes."""
    return f"HDR-{report.get('contractHash', '')}-{str(report.get('preflightHash', ''))[:8]}"


def _request_payload(report: dict[str, Any]) -> dict[str, Any]:
    request = report.get("humanDecisionRequest")
    if not isinstance(request, dict):
        raise InvalidDataShapeError("a structured human decision request is required")
    signals = report.get("signals", [])
    evidence = [
        {"type": "preflight_signal", "reference": f"{item.get('name')}: {evidence_item}"}
        for item in signals
        if isinstance(item, dict)
        for evidence_item in item.get("evidence", [])
        if isinstance(evidence_item, str) and evidence_item
    ]
    options = [
        {
            "id": str(item["id"]),
            "label": str(item["label"]),
            "risk": "unknown",
            "consequence": str(item.get("effect", "Option consequence is recorded in Preflight.")),
        }
        for item in request.get("options", [])
        if isinstance(item, dict) and item.get("id") and item.get("label")
    ]
    return {
        "schemaVersion": 1,
        "decisionId": request_id(report),
        "workItemId": str(report.get("workItemId", "")),
        "status": "needs_human_confirmation",
        "category": "preflight_review",
        "severity": "medium",
        "whatHappened": list(request.get("whatHappened", [])),
        "whyItMatters": str(request.get("whyItMatters", "")),
        "evidence": evidence
        or [{"type": "preflight", "reference": "target/ai_preflight_review.json"}],
        "options": options,
        "recommendedOption": str(request.get("recommendedOption", "")),
        "recommendationReason": str(request.get("recommendationReason", "")),
        "question": str(request.get("question", "")),
        "resumeCondition": str(request.get("resumeCondition", "")),
    }


def persist_request(report: dict[str, Any], root: Path = PROJECT_ROOT) -> Path | None:
    if report.get("status") != "needs_human_confirmation":
        return None
    payload = _request_payload(report)
    validate_payload("human_decision_request", payload)
    directory = decisions_dir(root)
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / f"{payload['decisionId']}.request.json"
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return path


def load_request(decision_id: str, root: Path = PROJECT_ROOT) -> dict[str, Any]:
    path = decisions_dir(root) / f"{decision_id}.request.json"
    payload = json.loads(path.read_text(encoding="utf-8"))
    validate_payload("human_decision_request", payload)
    return payload


def record_evidence(
    request: dict[str, Any],
    *,
    selected_option: str,
    decision: str,
    decided_by: str,
    rationale: str,
    source: str = "human_confirmation",
    root: Path = PROJECT_ROOT,
) -> Path:
    if selected_option not in {item["id"] for item in request["options"]}:
        raise ValueError("selected option is not present in the persisted Decision Request")
    payload = {
        "schemaVersion": 1,
        "decisionId": request["decisionId"],
        "selectedOption": selected_option,
        "decision": decision,
        "decidedBy": decided_by,
        "decidedAt": datetime.now(UTC).isoformat(),
        "rationale": rationale,
        "source": source,
        "supersedes": None,
    }
    try:
        validate_payload("human_decision_evidence", payload)
    except ValidationError as exc:
        raise ValueError(str(exc)) from exc
    directory = decisions_dir(root)
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / f"{request['decisionId']}.evidence.json"
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return path

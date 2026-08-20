"""Bounded, local external-execution handoff and receipt validation.

This module never contacts an external system.  It records only the exact
evidence an external actor must later return before local work may resume.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from collections.abc import Mapping
from datetime import datetime
from pathlib import Path
from typing import Any

from ai_task_event_log import append_event


class HandoffError(ValueError):
    """Raised when handoff or receipt evidence is absent, stale, or unbound."""


_BINDING_FIELDS = ("workItemId", "branch", "headCommit", "tree", "contractDigest", "summaryDigest")
_FULFILLERS = {"hosted_ci", "provider_release", "human", "adopter"}
_IDENTIFIER = re.compile(r"^[a-z][a-z0-9_.-]{2,127}$")
_DIGEST = re.compile(r"^[a-f0-9]{40,64}$")


def _time(value: str) -> datetime:
    if not isinstance(value, str) or not value.endswith("Z"):
        raise HandoffError("deadline must be a UTC Z timestamp")
    try:
        return datetime.fromisoformat(value)
    except ValueError as exc:
        raise HandoffError("deadline must be an ISO-8601 timestamp") from exc


def _bindings(value: Mapping[str, Any]) -> dict[str, str]:
    result: dict[str, str] = {}
    for field in _BINDING_FIELDS:
        item = value.get(field)
        if not isinstance(item, str) or not item:
            raise HandoffError(f"binding {field} is required")
        if field in {
            "headCommit",
            "tree",
            "contractDigest",
            "summaryDigest",
        } and not _DIGEST.fullmatch(item):
            raise HandoffError(f"binding {field} must be a hexadecimal digest")
        result[field] = item
    return result


def validate_handoff(handoff: Mapping[str, Any]) -> dict[str, Any]:
    if handoff.get("handoffVersion") != 1:
        raise HandoffError("unsupported handoff version")
    action = handoff.get("action")
    if not isinstance(action, str) or not _IDENTIFIER.fullmatch(action):
        raise HandoffError("action must be a bounded identifier")
    fulfiller = handoff.get("fulfiller")
    if fulfiller not in _FULFILLERS:
        raise HandoffError("fulfiller is not authorized")
    receipt_kind = handoff.get("receiptKind")
    if not isinstance(receipt_kind, str) or not _IDENTIFIER.fullmatch(receipt_kind):
        raise HandoffError("receipt kind must be a bounded identifier")
    return {
        "bindings": _bindings(handoff.get("bindings", {})),
        "deadline": _time(handoff.get("deadline", "")),
    }


def build_handoff(
    bindings: Mapping[str, Any], *, action: str, fulfiller: str, receipt_kind: str, deadline: str
) -> dict[str, Any]:
    handoff = {
        "handoffVersion": 1,
        "state": "awaiting_external_receipt",
        "bindings": dict(bindings),
        "action": action,
        "fulfiller": fulfiller,
        "receiptKind": receipt_kind,
        "deadline": deadline,
    }
    validate_handoff(handoff)
    return handoff


def project_handoff(handoff: Mapping[str, Any], *, now: str) -> dict[str, str]:
    validated = validate_handoff(handoff)
    facts = {
        "action": str(handoff["action"]),
        "fulfiller": str(handoff["fulfiller"]),
        "receiptKind": str(handoff["receiptKind"]),
        "deadline": str(handoff["deadline"]),
    }
    if _time(now) > validated["deadline"]:
        return {
            "state": "blocked",
            "humanStatusColor": "red",
            "recoveryCondition": "Obtain a new bound external receipt through the canonical receipt-ingest command; timeout alone cannot resume work.",
            **facts,
        }
    return {
        "state": "awaiting_external_receipt",
        "humanStatusColor": "yellow",
        "recoveryCondition": "Wait for the declared external actor to provide the exact bound receipt; do not poll or resume locally.",
        **facts,
    }


def ingest_receipt(
    handoff: Mapping[str, Any], receipt: Mapping[str, Any], *, now: str
) -> dict[str, Any]:
    validated = validate_handoff(handoff)
    if _time(now) > validated["deadline"]:
        raise HandoffError("handoff expired; a timeout cannot resume work")
    if receipt.get("receiptVersion") != 1 or receipt.get("kind") != handoff.get("receiptKind"):
        raise HandoffError("receipt shape or kind does not match handoff")
    if receipt.get("fulfilledBy") != handoff.get("fulfiller"):
        raise HandoffError("receipt fulfiller is not authorized")
    receipt_bindings = _bindings(receipt.get("bindings", {}))
    for field, expected in validated["bindings"].items():
        if receipt_bindings[field] != expected:
            raise HandoffError(f"receipt binding mismatch: {field}")
    return {
        "eventType": "external_receipt_ingested",
        "state": "resolved",
        "handoff": dict(handoff),
        "receipt": dict(receipt),
    }


def _event_id(prefix: str, payload: Mapping[str, Any]) -> str:
    return (
        f"{prefix}-" + hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest()[:16]
    )


def record_handoff(events: Path, handoff: Mapping[str, Any]) -> dict[str, Any]:
    """Append the sole durable record of a validated external handoff."""
    validated = validate_handoff(handoff)
    payload = dict(handoff)
    return append_event(
        events,
        {
            "eventId": _event_id("handoff", payload),
            "eventType": "external_handoff",
            "workItemId": validated["bindings"]["workItemId"],
            "evidence": [{"source": "external-handoff", "subject": str(handoff["action"])}],
            "handoff": payload,
        },
    )


def ingest_and_record(
    events: Path, handoff: Mapping[str, Any], receipt: Mapping[str, Any], *, now: str
) -> dict[str, Any]:
    """Validate and append a receipt resolution; no receipt means no resume event."""
    result = ingest_receipt(handoff, receipt, now=now)
    payload = {"handoff": dict(handoff), "receipt": dict(receipt)}
    return append_event(
        events,
        {
            "eventId": _event_id("receipt", payload),
            "eventType": "external_receipt_ingested",
            "workItemId": result["handoff"]["bindings"]["workItemId"],
            "evidence": [{"source": "external-receipt", "subject": str(receipt["kind"])}],
            **result,
        },
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("handoff", help="handoff JSON file")
    parser.add_argument("--receipt", help="receipt JSON file to validate")
    parser.add_argument(
        "--events", type=Path, help="append-only event log for durable handoff/receipt record"
    )
    parser.add_argument("--now", required=True, help="UTC ISO-8601 timestamp")
    args = parser.parse_args()
    try:
        handoff = json.loads(Path(args.handoff).read_text(encoding="utf-8"))
        if args.receipt:
            receipt = json.loads(Path(args.receipt).read_text(encoding="utf-8"))
            result = (
                ingest_and_record(args.events, handoff, receipt, now=args.now)
                if args.events
                else ingest_receipt(handoff, receipt, now=args.now)
            )
        else:
            result = (
                record_handoff(args.events, handoff)
                if args.events
                else project_handoff(handoff, now=args.now)
            )
    except (HandoffError, OSError, json.JSONDecodeError) as exc:
        parser.error(str(exc))
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

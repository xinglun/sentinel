"""Append-only Task Outcome event log primitives.

This module deliberately owns evidence recording only. Outcome generation and
validation consume the resulting JSONL stream in later Work Items.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

EVENT_TYPES = {
    "finding",
    "risk",
    "warning",
    "confirmation",
    "stop",
    "resume",
    "resolution",
    "risk-accepted",
    "check-pass-after-fix",
    "prevention",
    "completed",
    "cancelled",
    "external_handoff",
    "external_receipt_ingested",
    "external_handoff_timeout",
}
IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$")
SECRET_KEY = re.compile(
    r"(password|passwd|secret|token|api[_-]?key|private[_-]?key)", re.IGNORECASE
)


class EventLogError(ValueError):
    """Raised when an event would violate the append-only evidence contract."""


def _utc_now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def finding_fingerprint(
    checker_id: str, reason_code: str, affected_resource: str, evidence_subject: str
) -> str:
    """Return a stable fingerprint for one observed finding identity."""

    parts = (checker_id, reason_code, affected_resource, evidence_subject)
    if not all(isinstance(part, str) and part.strip() for part in parts):
        raise EventLogError("finding fingerprint inputs must be non-empty strings")
    return hashlib.sha256("\x1f".join(part.strip() for part in parts).encode()).hexdigest()


def _reject_secrets(value: Any, path: str = "event") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if SECRET_KEY.search(str(key)):
                raise EventLogError(f"secret-like field is not allowed: {path}.{key}")
            _reject_secrets(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _reject_secrets(child, f"{path}[{index}]")


def validate_event(event: dict[str, Any], existing: list[dict[str, Any]] | None = None) -> None:
    """Validate an event and its relationship to existing append-only records."""

    existing = existing or []
    required = {"eventId", "eventType", "workItemId", "occurredAt", "evidence"}
    missing = required - event.keys()
    if missing:
        raise EventLogError(f"missing event fields: {', '.join(sorted(missing))}")
    if not isinstance(event["eventId"], str) or not IDENTIFIER.fullmatch(event["eventId"]):
        raise EventLogError("eventId must be a bounded identifier")
    if any(row.get("eventId") == event["eventId"] for row in existing):
        raise EventLogError(f"duplicate eventId: {event['eventId']}")
    if event["eventType"] not in EVENT_TYPES:
        raise EventLogError(f"unsupported eventType: {event['eventType']}")
    if not isinstance(event["workItemId"], str) or not re.fullmatch(
        r"[a-z0-9][a-z0-9-]{2,127}", event["workItemId"]
    ):
        raise EventLogError("workItemId must be a task identifier")
    if not isinstance(event["occurredAt"], str) or not event["occurredAt"].strip():
        raise EventLogError("occurredAt must be non-empty")
    if not isinstance(event["evidence"], list):
        raise EventLogError("evidence must be an array")
    _reject_secrets(event)
    if event["eventType"] == "finding":
        if not isinstance(event.get("findingFingerprint"), str) or not event["findingFingerprint"]:
            raise EventLogError("finding events require findingFingerprint")
        if not isinstance(event.get("checkerId"), str) or not isinstance(
            event.get("reasonCode"), str
        ):
            raise EventLogError("finding events require checkerId and reasonCode")
        if not isinstance(event.get("affectedResource"), str) or not isinstance(
            event.get("evidenceSubject"), str
        ):
            raise EventLogError("finding events require affectedResource and evidenceSubject")
    if event["eventType"] in {"event_corrected", "event_superseded"}:
        raise EventLogError("correction uses a normal event with explicit relationship fields")
    relation = event.get("correctsEventId") or event.get("supersedesEventId")
    if relation is not None and (
        not isinstance(relation, str) or not any(row.get("eventId") == relation for row in existing)
    ):
        raise EventLogError("correction/supersession must reference an existing eventId")


def read_events(path: Path) -> list[dict[str, Any]]:
    """Read and validate every JSONL record without modifying the file."""

    if not path.exists():
        return []
    events: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            raise EventLogError(f"blank event line at {line_number}")
        try:
            event = json.loads(line)
        except json.JSONDecodeError as exc:
            raise EventLogError(f"invalid JSON at line {line_number}") from exc
        if not isinstance(event, dict):
            raise EventLogError(f"event at line {line_number} must be an object")
        validate_event(event, events)
        events.append(event)
    return events


def append_event(path: Path, event: dict[str, Any]) -> dict[str, Any]:
    """Validate and append exactly one event, preserving all existing bytes."""

    events = read_events(path)
    candidate = dict(event)
    candidate.setdefault("occurredAt", _utc_now())
    validate_event(candidate, events)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(
            json.dumps(candidate, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        )
        handle.write("\n")
    return candidate


def _main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", type=Path)
    parser.add_argument("--event", type=Path, help="JSON object to append")
    parser.add_argument("--validate", action="store_true")
    args = parser.parse_args()
    try:
        if args.event:
            append_event(args.path, json.loads(args.event.read_text(encoding="utf-8")))
        else:
            read_events(args.path)
    except (EventLogError, OSError, json.JSONDecodeError) as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())

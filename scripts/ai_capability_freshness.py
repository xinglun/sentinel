"""Evaluate deterministic, local freshness for capability evidence records."""

from __future__ import annotations

import sys
from datetime import UTC, datetime, timedelta
from typing import Any


def _iso(value: datetime) -> str:
    return value.astimezone(UTC).isoformat()


def make_record(
    *,
    environment: dict[str, Any],
    scope: list[str],
    now: datetime,
    ttl: timedelta = timedelta(days=30),
) -> dict[str, Any]:
    """Create a local evidence record; provider state is descriptive, never verified."""
    return {
        "verifiedAt": _iso(now),
        "validUntil": _iso(now + ttl),
        "environment": environment,
        "scope": scope,
        "evidenceFreshness": "fresh",
    }


def current_environment() -> dict[str, Any]:
    """Return the portable repository-evidence identity used for validation."""
    return {
        "runtime": f"python-{sys.version_info.major}",
        "toolVersions": ["capability-truth-schema-1"],
        "provider": "not_configured",
    }


def evaluate_freshness(
    record: dict[str, Any], *, environment: dict[str, Any], now: datetime
) -> dict[str, list[str] | str]:
    """Return ``fresh`` only for a complete, unexpired matching local snapshot."""
    required = ("verifiedAt", "validUntil", "environment", "scope", "evidenceFreshness")
    if any(field not in record for field in required):
        return {"state": "stale", "reasons": ["freshness_record_incomplete"]}
    try:
        valid_until = datetime.fromisoformat(str(record["validUntil"]))
    except ValueError:
        return {"state": "stale", "reasons": ["valid_until_invalid"]}
    if valid_until.tzinfo is None:
        return {"state": "stale", "reasons": ["valid_until_invalid"]}
    if now.astimezone(UTC) > valid_until.astimezone(UTC):
        return {"state": "stale", "reasons": ["valid_until_expired"]}
    if record["environment"] != environment:
        return {"state": "stale", "reasons": ["environment_mismatch"]}
    if record["evidenceFreshness"] != "fresh":
        return {"state": "stale", "reasons": ["record_marked_stale"]}
    return {"state": "fresh", "reasons": []}

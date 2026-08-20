"""Pure review-readiness signal policy."""
# mypy: ignore-errors

from __future__ import annotations

from typing import Any

STATUSES = {"not_ready", "ready", "ready_with_risks", "blocked"}


def review_readiness_signal(summary: dict[str, Any] | None) -> dict[str, Any]:
    if summary is None:
        return {"status": "unknown", "focus": [], "sources": ["summary.reviewReadiness"]}
    readiness = (
        summary.get("reviewReadiness") if isinstance(summary.get("reviewReadiness"), dict) else {}
    )
    status = readiness.get("status") if readiness.get("status") in STATUSES else "unknown"
    focus = readiness.get("expectedReviewFocus")
    focus = (
        [item.strip() for item in focus if isinstance(item, str) and item.strip()]
        if isinstance(focus, list)
        else []
    )
    return {"status": status, "focus": focus, "sources": ["summary.reviewReadiness"]}

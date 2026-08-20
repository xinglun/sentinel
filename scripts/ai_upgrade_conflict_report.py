#!/usr/bin/env python3
"""Build and validate the human-readable upgrade conflict report."""

from __future__ import annotations

from typing import Any

REPORT_VERSION = 1
CLASSIFICATIONS = {"Template-owned", "Project-owned", "Diverged", "Human Confirmation Required"}


def build_report(
    entries: list[dict[str, Any]], *, source_version: Any = None, target_version: Any = None
) -> dict[str, Any]:
    """Return a stable report that can be reviewed before an upgrade is applied."""
    normalized: list[dict[str, Any]] = []
    for entry in entries:
        item = dict(entry)
        item.setdefault("classification", "Human Confirmation Required")
        item.setdefault("summary", item.get("reason", "Review this path before continuing."))
        item.setdefault("recommendation", "Confirm explicitly or keep the target file unchanged.")
        normalized.append(item)
    requires_confirmation = any(
        item["classification"] == "Human Confirmation Required" for item in normalized
    )
    return {
        "reportVersion": REPORT_VERSION,
        "sourceVersion": source_version,
        "targetVersion": target_version,
        "entries": normalized,
        "requiresHumanConfirmation": requires_confirmation,
        "status": "needs_human_confirmation" if requires_confirmation else "ready",
    }


def validate_report(report: Any) -> list[str]:
    if not isinstance(report, dict):
        return ["report must be an object"]
    issues: list[str] = []
    if report.get("reportVersion") != REPORT_VERSION:
        issues.append("reportVersion is unsupported")
    entries = report.get("entries")
    if not isinstance(entries, list):
        return [*issues, "entries must be a list"]
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
            issues.append(f"entries[{index}] must contain a path")
        elif entry.get("classification") not in CLASSIFICATIONS:
            issues.append(f"entries[{index}] has an invalid classification")
    return issues

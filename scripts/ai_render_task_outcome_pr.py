"""Render an explicitly approved, sanitized Task Outcome PR fragment.

This module is presentation-only.  It never edits the Outcome JSON and it
never includes machine evidence, provenance, stop details, or unapproved
sections in a pull-request fragment.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from ai_common import parse_yaml
from ai_render_task_outcome_multilingual import normalize_locale

SAFE_FIELDS = (
    "status",
    "outcomeSummary",
    "taskOverview",
    "deliveredChanges",
    "findings",
    "risks",
    "warnings",
    "residualRisks",
)
DEFAULT_FIELDS = ("status", "outcomeSummary")
PR_CHROME: dict[str, dict[str, Any]] = {
    "en": {
        "title": "Task Outcome Summary",
        "status": "Status",
        "none": "None",
        "fields": {
            "outcomeSummary": "Outcome",
            "taskOverview": "Overview",
            "deliveredChanges": "Delivered Changes",
            "findings": "Findings",
            "risks": "Risks",
            "warnings": "Warnings",
            "residualRisks": "Residual Risks",
        },
    },
    "ja": {
        "title": "タスク結果の概要",
        "status": "状態",
        "none": "なし",
        "fields": {
            "outcomeSummary": "結果",
            "taskOverview": "タスク概要",
            "deliveredChanges": "変更内容",
            "findings": "検出事項",
            "risks": "リスク",
            "warnings": "警告",
            "residualRisks": "残存リスク",
        },
    },
    "zh-CN": {
        "title": "任务结果摘要",
        "status": "状态",
        "none": "无",
        "fields": {
            "outcomeSummary": "结果",
            "taskOverview": "任务概览",
            "deliveredChanges": "交付变更",
            "findings": "发现",
            "risks": "风险",
            "warnings": "警告",
            "residualRisks": "剩余风险",
        },
    },
}
SECRET = re.compile(
    r"(?i)(?:password|passwd|secret|token|api[_-]?key|private[_-]?key)\s*[:=]\s*[^\s,;]+"
)
ABSOLUTE_PATH = re.compile(r"(?<![A-Za-z0-9_])/(?:Users|home|private|tmp|var)/[^\s`]+")
UNSUPPORTED_CLAIM = re.compile(
    r"(?i)\b(?:score|productivity|hours? saved|money saved|percent(?:age)?|roi)\b"
)


def _policy(profile: Mapping[str, Any]) -> Mapping[str, Any]:
    reporting = profile.get("reporting", {})
    if not isinstance(reporting, Mapping):
        return {}
    policy = reporting.get("pullRequestSummary", {})
    return policy if isinstance(policy, Mapping) else {}


def _language(profile: Mapping[str, Any], requested: str | None) -> str:
    """Resolve explicit, PR-policy, or reporting locale without silent fallback."""
    if requested is not None:
        return normalize_locale(requested)
    reporting = profile.get("reporting", {})
    if not isinstance(reporting, Mapping):
        return "en"
    policy = _policy(profile)
    configured = policy.get("language", reporting.get("defaultLanguage", "en"))
    return normalize_locale(configured)


def _enabled(policy: Mapping[str, Any]) -> bool:
    value = policy.get("enabled")
    return value is True or (isinstance(value, str) and value.strip().lower() == "true")


def _safe_text(value: Any) -> str:
    if not isinstance(value, str):
        return ""
    text = SECRET.sub("[redacted]", value.strip())
    text = ABSOLUTE_PATH.sub("[path redacted]", text)
    if UNSUPPORTED_CLAIM.search(text):
        return "[redacted unsupported quantitative claim]"
    return text


def _items(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    result: list[str] = []
    for item in value:
        if isinstance(item, str):
            text = _safe_text(item)
        elif isinstance(item, Mapping):
            text = ""
            for key in ("title", "subject", "problem", "description", "kind", "stage"):
                text = _safe_text(item.get(key))
                if text:
                    break
        else:
            text = ""
        if text:
            result.append(text)
    return result


def render_pr_summary(
    outcome: Mapping[str, Any],
    profile: Mapping[str, Any],
    *,
    language: str | None = None,
) -> str:
    """Return a PR-safe Markdown fragment, or empty string when not approved."""

    policy = _policy(profile)
    if not _enabled(policy):
        return ""
    locale = _language(profile, language)
    chrome = PR_CHROME[locale]
    requested = policy.get("fields", DEFAULT_FIELDS)
    fields = (
        tuple(field for field in requested if field in SAFE_FIELDS)
        if isinstance(requested, list)
        else DEFAULT_FIELDS
    )
    sections = outcome.get("sections", {})
    if not isinstance(sections, Mapping):
        sections = {}
    lines = [f"## {chrome['title']}", ""]
    for field in fields:
        if field == "status":
            status = _safe_text(outcome.get("status")) or "unknown"
            lines.append(f"- {chrome['status']}: `{status}`")
        elif field in {"outcomeSummary", "taskOverview"}:
            value = _safe_text(sections.get(field)) or chrome["none"]
            lines.append(f"- {chrome['fields'][field]}: {value}")
        else:
            values = _items(sections.get(field))
            label = chrome["fields"][field]
            if values:
                lines.append(f"- {label}:")
                lines.extend(f"  - {value}" for value in values)
            else:
                lines.append(f"- {label}: {chrome['none']}")
    return "\n".join(lines) + "\n"


def _main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("outcome", type=Path)
    parser.add_argument("profile", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--language")
    args = parser.parse_args()
    try:
        outcome = json.loads(args.outcome.read_text(encoding="utf-8"))
        profile = parse_yaml(args.profile) if args.profile.exists() else {}
        rendered = render_pr_summary(outcome, profile, language=args.language)
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"Failed to render Task Outcome PR summary: {exc}", file=sys.stderr)
        return 2
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())

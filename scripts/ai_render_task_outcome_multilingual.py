"""Render Task Outcome Markdown views from Project Profile locale policy.

The Outcome JSON remains the sole fact source.  This module localizes the
derived Markdown chrome and never invents or translates arbitrary evidence
prose.
"""

from __future__ import annotations

import argparse
import json
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from ai_common import parse_yaml

LOCALES = ("ja", "en", "zh-CN")
ALIASES = {
    "ja": "ja",
    "ja-jp": "ja",
    "japanese": "ja",
    "en": "en",
    "en-us": "en",
    "en_us": "en",
    "english": "en",
    "zh": "zh-CN",
    "zh-cn": "zh-CN",
    "zh_cn": "zh-CN",
    "simplified-chinese": "zh-CN",
}
CHROME = {
    "ja": {
        "title": "Task Outcome",
        "status": "状態",
        "summary": "概要",
        "overview": "タスク概要",
        "none": "None",
        "sections": {
            "deliveredChanges": "変更内容",
            "findings": "検出事項",
            "risks": "リスク",
            "warnings": "警告",
            "interventions": "介入",
            "forcedStops": "強制停止",
            "resolutions": "解決",
            "recurrencePrevention": "再発防止",
            "avoidedImpact": "回避された影響",
            "residualRisks": "残存リスク",
            "humanDecisions": "人間の判断",
            "evidence": "証拠",
        },
        "handoff": {
            "title": "人間への引き継ぎ",
            "completed": "完了した内容",
            "passed": "合格した確認",
            "retained": "保留・残存事項",
            "risks": "リスク",
            "redReasons": "🔴の理由",
            "questions": "人間向け確認事項",
        },
    },
    "en": {
        "title": "Task Outcome",
        "status": "Status",
        "summary": "Outcome Summary",
        "overview": "Task Overview",
        "none": "None",
        "sections": {
            "deliveredChanges": "Delivered Changes",
            "findings": "Findings",
            "risks": "Risks",
            "warnings": "Warnings",
            "interventions": "Interventions",
            "forcedStops": "Forced Stops",
            "resolutions": "Resolutions",
            "recurrencePrevention": "Recurrence Prevention",
            "avoidedImpact": "Avoided Impact",
            "residualRisks": "Residual Risks",
            "humanDecisions": "Human Decisions",
            "evidence": "Evidence",
        },
        "handoff": {
            "title": "Human Handoff",
            "completed": "What was completed",
            "passed": "What passed",
            "retained": "What was retained",
            "risks": "Risks",
            "redReasons": "Red reasons",
            "questions": "Human questions",
        },
    },
    "zh-CN": {
        "title": "任务结果",
        "status": "状态",
        "summary": "结果摘要",
        "overview": "任务概览",
        "none": "None",
        "sections": {
            "deliveredChanges": "交付变更",
            "findings": "发现",
            "risks": "风险",
            "warnings": "警告",
            "interventions": "干预",
            "forcedStops": "强制停止",
            "resolutions": "解决方案",
            "recurrencePrevention": "防止复发",
            "avoidedImpact": "避免的影响",
            "residualRisks": "剩余风险",
            "humanDecisions": "人的决定",
            "evidence": "证据",
        },
        "handoff": {
            "title": "面向人的交接",
            "completed": "已完成",
            "passed": "已通过",
            "retained": "保留事项",
            "risks": "风险",
            "redReasons": "🔴原因",
            "questions": "需要人了解和确认",
        },
    },
}
SECTION_ORDER = (
    "deliveredChanges",
    "findings",
    "risks",
    "warnings",
    "interventions",
    "forcedStops",
    "resolutions",
    "recurrencePrevention",
    "avoidedImpact",
    "residualRisks",
    "humanDecisions",
    "evidence",
)
STATUS_TRAFFIC_LIGHTS = {
    "completed": "🟢",
    "completed_with_warnings": "🟡",
    "needs_human_confirmation": "🟡",
    "blocked": "🔴",
    "cancelled": "🔴",
}


def normalize_locale(value: Any) -> str:
    """Normalize one supported locale alias or raise instead of falling back."""

    if not isinstance(value, str) or not value.strip():
        raise ValueError("locale must be a non-empty string")
    key = value.strip().lower()
    try:
        return ALIASES[key]
    except KeyError as exc:
        raise ValueError(f"unsupported Outcome locale: {value}") from exc


def _reporting(profile: Mapping[str, Any]) -> Mapping[str, Any]:
    reporting = profile.get("reporting", {})
    return reporting if isinstance(reporting, Mapping) else {}


def selected_locales(profile: Mapping[str, Any]) -> tuple[str, ...]:
    """Return the exact configured generation set, defaulting to defaultLanguage."""

    reporting = _reporting(profile)
    default = normalize_locale(reporting.get("defaultLanguage", "ja"))
    policy = reporting.get("taskOutcome", {})
    if not isinstance(policy, Mapping):
        policy = {}
    configured = policy.get("languages")
    if configured is None:
        configured = policy.get("locales")
    if configured is None:
        return (default,)
    if not isinstance(configured, list) or not configured:
        raise ValueError("reporting.taskOutcome.languages must be a non-empty list")
    result: list[str] = []
    for value in configured:
        locale = normalize_locale(value)
        if locale not in result:
            result.append(locale)
    return tuple(result)


def _item_text(item: Any, none: str) -> str:
    if isinstance(item, str) and item.strip():
        return item.strip()
    if isinstance(item, Mapping):
        for key in ("claim", "title", "subject", "problem", "description", "kind", "stage"):
            value = item.get(key)
            if isinstance(value, str) and value.strip():
                return value.strip()
    return none


def _handoff_item_text(item: Any, none: str) -> str:
    if isinstance(item, Mapping):
        claim = _item_text(item, none)
        detail = item.get("detail")
        text = (
            f"{claim}: {detail}"
            if isinstance(detail, str) and detail.strip() and detail.strip() != claim
            else claim
        )
        if item.get("inference") is True:
            text += " (inference)"
        return text
    return _item_text(item, none)


def render_localized_outcome(outcome: Mapping[str, Any], locale: str) -> str:
    """Render one locale without mutating or changing Outcome machine keys."""

    locale = normalize_locale(locale)
    chrome: dict[str, Any] = CHROME[locale]
    task_id = outcome.get("workItemId", "unknown-task")
    status = outcome.get("status", "unknown")
    status_text = str(status)
    traffic_light = STATUS_TRAFFIC_LIGHTS.get(status_text, "🔴")
    sections = outcome.get("sections", {})
    if not isinstance(sections, Mapping):
        sections = {}
    lines = [
        f"# {chrome['title']}: {task_id}",
        "",
        f"{chrome['status']}: {traffic_light} `{status_text}`",
        "",
        f"## {chrome['summary']}",
        str(sections.get("outcomeSummary") or chrome["none"]),
        "",
        f"## {chrome['overview']}",
        str(sections.get("taskOverview") or chrome["none"]),
        "",
    ]
    for key in SECTION_ORDER:
        lines.append(f"## {chrome['sections'][key]}")
        values = sections.get(key, [])
        if isinstance(values, list) and values:
            lines.extend(f"- {_item_text(item, chrome['none'])}" for item in values)
        else:
            lines.append(chrome["none"])
        lines.append("")
    handoff = outcome.get("humanHandoff")
    if isinstance(handoff, Mapping):
        labels = chrome["handoff"]
        lines.extend([f"## {labels['title']}", f"Locale: `{handoff.get('locale', 'unknown')}`", ""])
        for key in ("completed", "passed", "retained", "risks", "redReasons"):
            lines.append(f"### {labels[key]}")
            values = handoff.get(key, [])
            if isinstance(values, list) and values:
                lines.extend(f"- {_handoff_item_text(item, chrome['none'])}" for item in values)
            else:
                lines.append(chrome["none"])
            lines.append("")
        questions = handoff.get("questions")
        lines.append(f"### {labels['questions']}")
        if isinstance(questions, Mapping):
            for key, value in questions.items():
                if key == "problemCountEvidenceRefs":
                    continue
                if isinstance(value, list):
                    rendered = (
                        "; ".join(_handoff_item_text(item, chrome["none"]) for item in value)
                        or chrome["none"]
                    )
                elif isinstance(value, Mapping):
                    rendered = _handoff_item_text(value, chrome["none"])
                else:
                    rendered = str(value)
                lines.append(f"- {key}: {rendered}")
        else:
            lines.append(chrome["none"])
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def render_outcome_files(
    outcome: Mapping[str, Any], profile: Mapping[str, Any], output_dir: Path
) -> list[Path]:
    """Write exactly the Profile-approved locale files and return sorted paths."""

    output_dir.mkdir(parents=True, exist_ok=True)
    task_id = outcome.get("workItemId", "unknown-task")
    paths: list[Path] = []
    for locale in selected_locales(profile):
        path = output_dir / f"{task_id}.outcome.{locale}.md"
        path.write_text(render_localized_outcome(outcome, locale), encoding="utf-8")
        paths.append(path)
    return paths


def _main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("outcome", type=Path)
    parser.add_argument("profile", type=Path)
    parser.add_argument("output_dir", type=Path)
    args = parser.parse_args()
    outcome = json.loads(args.outcome.read_text(encoding="utf-8"))
    profile = parse_yaml(args.profile) if args.profile.exists() else {}
    for path in render_outcome_files(outcome, profile, args.output_dir):
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())

"""Render a Task Outcome JSON object as a deterministic Markdown report."""

from __future__ import annotations

import argparse
import json
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from ai_generate_task_outcome import _render_implementation_approach

SECTION_TITLES = (
    ("outcomeSummary", "Outcome Summary"),
    ("taskOverview", "Task Overview"),
    ("deliveredChanges", "Delivered Changes"),
    ("findings", "Findings"),
    ("risks", "Risks"),
    ("warnings", "Warnings"),
    ("limitations", "Limitations"),
    ("nonRiskExplanations", "Non-Risk Explanations"),
    ("forbiddenClaims", "Forbidden Claims"),
    ("interventions", "Interventions"),
    ("forcedStops", "Forced Stops"),
    ("resolutions", "Resolutions"),
    ("recurrencePrevention", "Recurrence Prevention"),
    ("avoidedImpact", "Avoided Impact"),
    ("residualRisks", "Residual Risks"),
    ("humanDecisions", "Human Decisions"),
    ("evidence", "Evidence"),
    ("implementationApproach", "Implementation Approach"),
)


def _item_text(item: Any) -> str:
    if isinstance(item, str):
        return item
    if isinstance(item, Mapping):
        for key in ("claim", "title", "subject", "problem", "kind", "stage", "source"):
            value = item.get(key)
            if isinstance(value, str) and value.strip():
                return value
        return json.dumps(dict(item), ensure_ascii=False, sort_keys=True)
    return str(item)


def render_task_outcome(outcome: Mapping[str, Any]) -> str:
    """Render all machine sections without modifying the input mapping."""

    task_id = outcome.get("workItemId", "unknown-task")
    status = outcome.get("status", "unknown")
    sections = outcome.get("sections", {})
    human_status = outcome.get("humanStatusColor", "unknown")
    lines = [
        f"# Task Outcome: {task_id}",
        "",
        f"Status: `{status}`",
        f"Human Status: `{human_status}`",
    ]
    failed_gate = outcome.get("failedGate")
    recovery = outcome.get("recoveryCondition")
    if isinstance(failed_gate, str) and failed_gate.strip():
        lines.append(f"Failed Gate: `{failed_gate.strip()}`")
    if isinstance(recovery, str) and recovery.strip():
        lines.append(f"Recovery Condition: {recovery.strip()}")
    lines.append("")
    for key, title in SECTION_TITLES:
        lines.append(f"## {title}")
        value = sections.get(key, []) if isinstance(sections, Mapping) else []
        if key == "implementationApproach":
            lines.extend(
                _render_implementation_approach(value if isinstance(value, Mapping) else {})
            )
            lines.append("")
            continue
        if key in {"outcomeSummary", "taskOverview"}:
            lines.append(value if isinstance(value, str) and value else "None")
        elif isinstance(value, list) and value:
            lines.extend(f"- {_item_text(item)}" for item in value)
        else:
            lines.append("None")
        lines.append("")
    handoff = outcome.get("humanHandoff")
    if isinstance(handoff, Mapping):
        lines.extend(["## Human Handoff", f"Locale: `{handoff.get('locale', 'unknown')}`", ""])
        for key, title in (
            ("completed", "What was completed"),
            ("passed", "What passed"),
            ("retained", "What was retained"),
            ("risks", "Risks"),
            ("redReasons", "Red reasons"),
        ):
            lines.append(f"### {title}")
            values = handoff.get(key, [])
            if isinstance(values, list) and values:
                for item in values:
                    detail = item.get("detail") if isinstance(item, Mapping) else None
                    text = _item_text(item)
                    if isinstance(detail, str) and detail.strip() and detail.strip() != text:
                        text = f"{text}: {detail.strip()}"
                    if isinstance(item, Mapping) and item.get("inference") is True:
                        text += " (inference)"
                    lines.append(f"- {text}")
            else:
                lines.append("None")
            lines.append("")
        lines.append("### Human questions")
        questions = handoff.get("questions")
        if isinstance(questions, Mapping):
            for key, value in questions.items():
                if key == "problemCountEvidenceRefs":
                    continue
                if isinstance(value, list):
                    text = "; ".join(_item_text(item) for item in value) or "None"
                elif isinstance(value, Mapping):
                    text = _item_text(value)
                else:
                    text = str(value)
                lines.append(f"- {key}: {text}")
        else:
            lines.append("None")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def _main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    outcome = json.loads(args.input.read_text(encoding="utf-8"))
    args.output.write_text(render_task_outcome(outcome), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())

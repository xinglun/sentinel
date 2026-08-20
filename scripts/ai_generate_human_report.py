#!/usr/bin/env python3
"""Generate and validate a compact Human Benefit Report from Task Outcome truth."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from ai_check_task_outcome import validate_outcome

REPORT_VERSION = 1
PHASES = {"review", "final"}
RESOLVED_STATES = {"resolved", "mitigated", "accepted", "not_applicable"}


def _canonical_digest(value: Mapping[str, Any]) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def _mapping_list(value: Any) -> list[dict[str, Any]]:
    return (
        [dict(item) for item in value if isinstance(item, Mapping)]
        if isinstance(value, list)
        else []
    )


def _string_list(value: Any) -> list[str]:
    return (
        [item.strip() for item in value if isinstance(item, str) and item.strip()]
        if isinstance(value, list)
        else []
    )


def _severity(item: Mapping[str, Any]) -> str:
    value = item.get("severity")
    return value if value in {"informational", "low", "medium", "high", "critical"} else "medium"


def _description(item: Mapping[str, Any]) -> str:
    for key in ("description", "detail", "title", "reason", "conditionalImpact"):
        value = item.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return "Evidence-backed risk requires review."


def _claim_text(item: Any) -> str:
    if isinstance(item, Mapping):
        value = item.get("claim") or item.get("detail") or item.get("title")
        if isinstance(value, str) and value.strip():
            text = value.strip()
            if item.get("inference") is True:
                text += " (inference)"
            return text
    return str(item).strip() if isinstance(item, str) and item.strip() else "None recorded."


def _claim_lines(items: Any) -> list[str]:
    return [_claim_text(item) for item in items] if isinstance(items, list) else []


def _evidence_suffix(item: Any) -> str:
    if not isinstance(item, Mapping):
        return ""
    refs = item.get("evidenceRefs") or item.get("evidence")
    if not isinstance(refs, list) or not refs:
        return ""
    subjects = [
        str(ref.get("subject"))
        for ref in refs
        if isinstance(ref, Mapping) and isinstance(ref.get("subject"), str)
    ]
    return f" [evidence: {', '.join(subjects)}]" if subjects else ""


def _remaining_risks(sections: Mapping[str, Any]) -> list[dict[str, Any]]:
    candidates = _mapping_list(sections.get("residualRisks"))
    candidates.extend(
        item
        for key in ("findings", "risks")
        for item in _mapping_list(sections.get(key))
        if item.get("state") not in RESOLVED_STATES
    )
    result: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for item in candidates:
        detail = _description(item)
        key = (_severity(item), detail)
        if key in seen:
            continue
        seen.add(key)
        risk: dict[str, Any] = {"severity": key[0], "detail": detail}
        evidence = item.get("evidence")
        if isinstance(evidence, list) and evidence:
            risk["evidence"] = evidence
        result.append(risk)
    return result


def _prevented_risks(sections: Mapping[str, Any]) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    seen: set[str] = set()
    for stop in _mapping_list(sections.get("forcedStops")):
        impact = stop.get("conditionalImpact")
        if not isinstance(impact, str) or not impact.strip():
            continue
        item: dict[str, Any] = {
            "risk": impact.strip(),
            "severity": _severity(stop),
            "detectedBy": str(stop.get("policyOrGuard") or "governance_guard"),
            "action": "blocked",
            "resolution": str(stop.get("recovery") or stop.get("result") or "unresolved"),
        }
        evidence = stop.get("evidence")
        if isinstance(evidence, list) and evidence:
            item["evidence"] = evidence
        seen.add(item["risk"])
        result.append(item)
    for risk in _mapping_list(sections.get("risks")):
        if risk.get("kind") != "prevented_event":
            continue
        detail = _description(risk)
        if detail in seen:
            continue
        seen.add(detail)
        result.append(
            {
                "risk": detail,
                "severity": _severity(risk),
                "detectedBy": "task_outcome_evidence",
                "action": "prevented",
                "resolution": str(risk.get("state") or "unresolved"),
                "evidence": risk.get("evidence", []),
            }
        )
    for impact in _string_list(sections.get("avoidedImpact")):
        if impact in seen:
            continue
        seen.add(impact)
        result.append(
            {
                "risk": impact,
                "severity": "medium",
                "detectedBy": "task_outcome_evidence",
                "action": "prevented",
                "resolution": "See Task Outcome evidence.",
            }
        )
    return result


def _next_safe_action(
    phase: str,
    sections: Mapping[str, Any],
    remaining: Sequence[Mapping[str, Any]],
    closure_facts: Mapping[str, Any] | None,
) -> str:
    for stop in reversed(_mapping_list(sections.get("forcedStops"))):
        # Historical stops remain in the Outcome as evidence, but a resolved
        # stop is not an actionable recovery condition for the current task.
        # Only unresolved stops may drive the next action; otherwise a stale
        # retry instruction can be shown after a later passing verification.
        state = stop.get("state") or stop.get("result")
        if state in RESOLVED_STATES:
            continue
        recovery = stop.get("recovery")
        if isinstance(recovery, str) and recovery.strip():
            return recovery.strip()
    if phase == "final" and closure_facts is not None:
        return (
            f"Continue from {closure_facts['continueFrom']} on synchronized "
            f"{closure_facts['base']}."
        )
    if remaining:
        return "Review the remaining risks and obtain the named evidence before proceeding."
    return "Review the pull request and provider checks before merge."


def _implementation_approach_lines(approach: Mapping[str, Any]) -> list[str]:
    """Render the customer summary first and technical detail after it."""

    def text(value: Any, fallback: str) -> str:
        if not isinstance(value, Mapping):
            return fallback
        return str(value.get("text") or value.get("detail") or value.get("decision") or fallback)

    def claim_status(value: Any) -> str:
        return str(value.get("status", "unknown")) if isinstance(value, Mapping) else "unknown"

    lines = [
        f"Status: `{approach.get('status', 'incomplete')}`",
        f"Customer summary ({claim_status(approach.get('summary'))}): {text(approach.get('summary'), 'None recorded.')}",
        f"Mechanism ({claim_status(approach.get('mechanism'))}): {text(approach.get('mechanism'), 'None recorded.')}",
        "",
        "Affected components",
    ]
    components = approach.get("affectedComponents", [])
    lines.extend(
        f"- {item.get('component', 'Component')}: {item.get('detail', 'None recorded.')} ({claim_status(item)})"
        for item in components
        if isinstance(item, Mapping)
    ) if isinstance(components, list) and components else lines.append("- None recorded.")
    lines.extend(["", "Design decisions"])
    decisions = approach.get("designDecisions", [])
    lines.extend(
        f"- {item.get('decision', 'Decision')}: {item.get('reason', 'None recorded.')} ({claim_status(item)})"
        for item in decisions
        if isinstance(item, Mapping)
    ) if isinstance(decisions, list) and decisions else lines.append("- None recorded.")
    lines.extend(["", "### Technical details"])
    details = approach.get("technicalDetails", [])
    lines.extend(
        f"- {item.get('topic', 'Detail')}: {item.get('detail', 'None recorded.')} ({claim_status(item)})"
        for item in details
        if isinstance(item, Mapping)
    ) if isinstance(details, list) and details else lines.append("- None recorded.")
    lines.extend(["", "### Evidence"])
    evidence = approach.get("evidence", [])
    lines.extend(
        f"- {item.get('claim', 'Claim')}: {item.get('source', 'source')}#{item.get('subject', 'subject')} ({claim_status(item)})"
        for item in evidence
        if isinstance(item, Mapping)
    ) if isinstance(evidence, list) and evidence else lines.append("- None recorded.")
    return lines


def _validate_source(outcome: Mapping[str, Any], contract: Mapping[str, Any] | None = None) -> None:
    report = validate_outcome(
        outcome,
        expected_task_id=str(outcome.get("workItemId", "")),
        contract=contract,
    )
    if not report.valid:
        detail = "; ".join(f"{item.code}: {item.message}" for item in report.errors)
        raise ValueError(f"Task Outcome is invalid: {detail}")


def _validate_closure_facts(value: Mapping[str, Any] | None) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise TypeError("final Human Benefit Report requires closure facts")
    required = {
        "pullRequest",
        "mergeCommit",
        "base",
        "baseCommit",
        "workBranch",
        "cleanup",
        "continueFrom",
    }
    if any(not isinstance(value.get(key), str) or not str(value[key]).strip() for key in required):
        raise ValueError("final Human Benefit Report closure facts are incomplete")
    if not str(value["pullRequest"]).startswith("https://"):
        raise ValueError("final Human Benefit Report pull request must be provider-bound")
    if len(str(value["mergeCommit"])) != 40 or len(str(value["baseCommit"])) != 40:
        raise ValueError("final Human Benefit Report commit facts are malformed")
    return {key: str(value[key]) for key in sorted(required)}


def generate_human_report(
    outcome: Mapping[str, Any],
    *,
    phase: str = "review",
    closure_facts: Mapping[str, Any] | None = None,
    contract: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Project one validated Task Outcome into a concise human decision view."""

    if phase not in PHASES:
        raise ValueError(f"unsupported Human Benefit Report phase: {phase}")
    _validate_source(outcome, contract)
    sections = outcome["sections"]
    handoff = outcome.get("humanHandoff")
    findings = _mapping_list(sections.get("findings"))
    risks = _mapping_list(sections.get("risks"))
    warnings = _string_list(sections.get("warnings"))
    stops = _mapping_list(sections.get("forcedStops"))
    issue_records = [*findings, *risks, *stops]
    resolved = sum(item.get("state") in RESOLVED_STATES for item in [*findings, *risks])
    resolved += sum(item.get("result") in RESOLVED_STATES for item in stops)
    detected = len(issue_records) + len(warnings)
    remaining = _remaining_risks(sections)
    report: dict[str, Any] = {
        "reportVersion": REPORT_VERSION,
        "workItemId": outcome["workItemId"],
        "phase": phase,
        "result": outcome["status"],
        "sourceOutcome": {
            "format": outcome["format"],
            "schemaVersion": outcome["schemaVersion"],
            "digest": _canonical_digest(outcome),
        },
        "task": {
            "summary": sections["outcomeSummary"],
            "deliveredChanges": list(sections.get("deliveredChanges", [])),
        },
        "issues": {
            "detected": detected,
            "hardStops": len(stops),
            "warnings": len(warnings),
            "resolved": resolved,
            "unresolved": detected - resolved,
        },
        "preventedRisks": _prevented_risks(sections),
        "interventions": _string_list(sections.get("interventions")),
        "forcedStops": stops,
        "humanDecisions": list(sections.get("humanDecisions", [])),
        "limitations": _mapping_list(sections.get("limitations")),
        "forbiddenClaims": _string_list(sections.get("forbiddenClaims")),
        "remainingRisks": remaining,
    }
    approach = sections.get("implementationApproach")
    if isinstance(approach, Mapping):
        report["implementationApproach"] = dict(approach)
    if isinstance(handoff, Mapping):
        report["humanHandoff"] = dict(handoff)
    normalized_facts = _validate_closure_facts(closure_facts) if phase == "final" else None
    if normalized_facts is not None:
        report["closure"] = normalized_facts
        report["closureEvidenceState"] = "repository_recorded_only"
    report["nextSafeAction"] = _next_safe_action(phase, sections, remaining, normalized_facts)
    return report


def render_human_report(report: Mapping[str, Any]) -> str:
    """Render a concise English view without changing machine facts."""

    handoff = report.get("humanHandoff")
    if isinstance(handoff, Mapping):
        questions = handoff.get("questions", {})
        questions = questions if isinstance(questions, Mapping) else {}
        status_labels = {
            "completed": "Success",
            "completed_with_warnings": "Partial",
            "blocked": "Blocked",
            "needs_human_confirmation": "Blocked",
            "cancelled": "Failed",
        }
        result = report.get("result")
        lines = [
            "# AI Cockpit Task Report",
            "",
            "Task Result",
            f"Status: {status_labels.get(str(result), str(result))}",
            "",
            "What was completed",
        ]
        approach = report.get("implementationApproach")
        if isinstance(approach, Mapping):
            lines.extend(
                ["", "Implementation Approach", *_implementation_approach_lines(approach), ""]
            )
        completed = handoff.get("completed", [])
        lines.extend(
            f"- {_claim_text(item)}{_evidence_suffix(item)}" for item in completed
        ) if isinstance(completed, list) and completed else lines.append("- None recorded.")
        blocked = questions.get("blockedProblems", [])
        retained = handoff.get("retained", [])
        lines.extend(
            [
                "",
                "Problems found",
                f"- Total: {questions.get('problemCount', 0)}",
                f"- Blocking: {len(blocked) if isinstance(blocked, list) else 0}",
                f"- Warning: {len(retained) if isinstance(retained, list) else 0}",
                "",
                "Stops triggered",
            ]
        )
        stops = report.get("forcedStops", [])
        if isinstance(stops, list) and stops:
            for stop in stops:
                if isinstance(stop, Mapping):
                    lines.append(
                        f"- Reason: {stop.get('reason', 'None recorded.')} | Stage: {stop.get('stage', 'unknown')} | Resolution: {stop.get('recovery') or stop.get('result', 'None recorded.')}{_evidence_suffix(stop)}"
                    )
        else:
            lines.append("- None recorded.")
        lines.extend(["", "Problems resolved"])
        resolved = questions.get("resolvedProblems", [])
        approach = questions.get("resolutionApproach", [])
        if isinstance(resolved, list) and resolved:
            for index, item in enumerate(resolved):
                solution = (
                    approach[index]
                    if isinstance(approach, list) and index < len(approach)
                    else "See recorded resolution evidence."
                )
                lines.append(f"- Problem: {_claim_text(item)}")
                lines.append(f"  Solution: {_claim_text(solution)}")
                lines.append(
                    f"  Evidence: {_evidence_suffix(item).strip() or 'inference (no evidence reference)'}"
                )
        else:
            lines.append("- None recorded.")
        avoided = questions.get("avoidedRisks", [])
        remaining = questions.get("remainingRisks", [])
        unknowns = questions.get("agentUnknowns", [])
        decisions = questions.get("humanConfirmations", [])
        recurrence = questions.get("recurrenceLikelihood", {})
        next_time = questions.get("nextTime", {})
        lines.extend(["", "Risks avoided"])
        lines.extend(
            f"- {_claim_text(item)}{_evidence_suffix(item)}" for item in avoided
        ) if isinstance(avoided, list) and avoided else lines.append("- None recorded.")
        lines.extend(["", "Remaining risks"])
        lines.extend(
            f"- {_claim_text(item)}{_evidence_suffix(item)}" for item in remaining
        ) if isinstance(remaining, list) and remaining else lines.append("- None recorded.")
        lines.extend(["", "Unknowns"])
        lines.extend(
            f"- {_claim_text(item)}{_evidence_suffix(item)}" for item in unknowns
        ) if isinstance(unknowns, list) and unknowns else lines.append("- None recorded.")
        lines.extend(["", "Human decisions"])
        lines.extend(
            f"- {_claim_text(item)}{_evidence_suffix(item)}" for item in decisions
        ) if isinstance(decisions, list) and decisions else lines.append("- None recorded.")
        lines.extend(["", "Verification"])
        passed = handoff.get("passed", [])
        lines.extend(
            f"- {_claim_text(item)}{_evidence_suffix(item)}" for item in passed
        ) if isinstance(passed, list) and passed else lines.append("- None recorded.")
        lines.extend(
            [
                "",
                "Impact",
                f"- Rework avoided: {_claim_text(avoided[0]) if isinstance(avoided, list) and avoided else 'None recorded.'}",
                f"- Repeat correction prevented: {_claim_text(recurrence)}",
                f"- Major risk prevented: {_claim_text(avoided[0]) if isinstance(avoided, list) and avoided else 'None recorded.'}",
                "",
                "Next action",
                f"- {_claim_text(next_time)}",
            ]
        )
        return "\n".join(lines) + "\n"

    issues = report["issues"]
    lines = [
        "# AI Cockpit Task Report",
        "",
        f"Phase: `{report['phase']}`",
        f"Result: `{report['result']}`",
        "",
        "## 任务结论 / Task conclusion",
        f"Task Outcome status: `{report['result']}`.",
        "",
        "## 实际完成内容 / Completed work",
        str(report["task"]["summary"]),
        "",
    ]
    approach = report.get("implementationApproach")
    if isinstance(approach, Mapping):
        lines.extend(["## Implementation Approach", *_implementation_approach_lines(approach), ""])
    lines.extend(
        [
            "## 发现的问题 / Findings",
            f"- Detected issues: {issues['detected']}",
            f"- Hard stops: {issues['hardStops']}",
            f"- Warnings: {issues['warnings']}",
            f"- Resolved: {issues['resolved']}",
            f"- Unresolved: {issues['unresolved']}",
            "",
            "## AI Cockpit 的干预 / AI Cockpit interventions",
        ]
    )
    interventions = report.get("interventions", [])
    lines.extend(f"- {item}" for item in interventions) if interventions else lines.append(
        "None recorded."
    )
    lines.extend(["", "## 强制停止 / Forced stops"])
    stops = report.get("forcedStops", [])
    lines.extend(
        f"- {_description(item)}" for item in stops if isinstance(item, Mapping)
    ) if stops else lines.append("None recorded.")
    lines.extend(["", "## 已解决风险 / Resolved risks"])
    if issues["resolved"]:
        lines.append(f"- {issues['resolved']} resolved item(s) recorded in the Task Outcome.")
    else:
        lines.append("None recorded.")
    lines.extend(["", "## 避免的影响 / Avoided impact"])
    prevented = report.get("preventedRisks", [])
    lines.extend(
        f"- {item['risk']} ({item['action']}: {item['resolution']})" for item in prevented
    ) if prevented else lines.append("None recorded.")
    lines.extend(["", "## 未解决风险 / Unresolved risks"])
    remaining = report.get("remainingRisks", [])
    lines.extend(
        f"- [{item['severity']}] {item['detail']}" for item in remaining
    ) if remaining else lines.append("None recorded.")
    lines.extend(["", "## 未执行验证 / Not-run verification"])
    limitations = report.get("limitations", [])
    lines.extend(
        f"- {item.get('title', 'Limitation')}: {item.get('sourceWarning', 'Evidence is unavailable.')}"
        for item in limitations
    ) if limitations else lines.append("None recorded.")
    lines.extend(["", "## 当前禁止声明 / Forbidden claims"])
    claims = report.get("forbiddenClaims", [])
    lines.extend(f"- {item}" for item in claims) if claims else lines.append("None recorded.")
    lines.extend(["", "## 需要人的决定 / Human decisions"])
    decisions = report.get("humanDecisions", [])
    lines.extend(f"- {item}" for item in decisions) if decisions else lines.append("None recorded.")
    lines.extend(["", "## 下一步 / Next safe action", str(report["nextSafeAction"]), ""])
    return "\n".join(lines)


def validate_human_report(
    report: Any,
    outcome: Mapping[str, Any],
    *,
    phase: str | None = None,
    closure_facts: Mapping[str, Any] | None = None,
    markdown: str | None = None,
) -> list[str]:
    """Fail closed unless the report exactly matches its validated source."""

    if not isinstance(report, Mapping):
        return ["report must be an object"]
    selected_phase = phase or report.get("phase")
    try:
        expected = generate_human_report(
            outcome, phase=str(selected_phase), closure_facts=closure_facts
        )
    except (KeyError, TypeError, ValueError) as exc:
        return [str(exc)]
    issues: list[str] = []
    if dict(report) != expected:
        issues.append("report is stale or inconsistent with Task Outcome")
    if markdown is not None and markdown != render_human_report(expected):
        issues.append("Markdown is stale or inconsistent with Human Benefit Report")
    return issues


def _write_json(path: Path, value: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--phase", choices=sorted(PHASES), default="review")
    parser.add_argument("--closure-facts", type=Path)
    parser.add_argument("outcome", type=Path)
    parser.add_argument("json_output", type=Path)
    parser.add_argument("markdown_output", type=Path)
    args = parser.parse_args(argv)
    try:
        source = json.loads(args.outcome.read_text(encoding="utf-8"))
        facts = (
            json.loads(args.closure_facts.read_text(encoding="utf-8"))
            if args.closure_facts
            else None
        )
        if args.check:
            report = json.loads(args.json_output.read_text(encoding="utf-8"))
            markdown = args.markdown_output.read_text(encoding="utf-8")
            errors = validate_human_report(
                report,
                source,
                phase=args.phase,
                closure_facts=facts,
                markdown=markdown,
            )
            if errors:
                raise ValueError("; ".join(errors))
        else:
            report = generate_human_report(source, phase=args.phase, closure_facts=facts)
            _write_json(args.json_output, report)
            args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
            args.markdown_output.write_text(render_human_report(report), encoding="utf-8")
    except (OSError, KeyError, TypeError, ValueError, json.JSONDecodeError) as exc:
        print(f"Human Benefit Report failed: {exc}", file=sys.stderr)
        return 1
    print(
        f"Human Benefit Report {('check passed' if args.check else 'generated')}: {args.json_output}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

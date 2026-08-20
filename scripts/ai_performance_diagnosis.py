#!/usr/bin/env python3
"""Derive evidence-only Work Item governance cost and bottleneck reports."""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import defaultdict
from datetime import UTC, datetime
from pathlib import Path
from typing import Any


class PerformanceReportError(ValueError):
    """Raised when performance evidence cannot be interpreted safely."""


def _int(value: Any) -> int | None:
    return value if isinstance(value, int) and value >= 0 else None


def load_events(path: Path, *, work_item_id: str) -> tuple[list[dict[str, Any]], int]:
    """Load valid events for one Work Item; malformed evidence fails closed."""
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise PerformanceReportError(f"cannot read observability log: {exc}") from exc
    events: list[dict[str, Any]] = []
    ignored = 0
    for number, line in enumerate(lines, 1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            raise PerformanceReportError(f"malformed observability JSON at line {number}") from exc
        if not isinstance(value, dict):
            raise PerformanceReportError(f"observability event at line {number} is not an object")
        event_item = value.get("workItemId")
        if event_item != work_item_id:
            ignored += 1
            continue
        events.append(value)
    return events, ignored


def _event_duration(event: dict[str, Any]) -> int | None:
    direct = _int(event.get("durationMs"))
    if direct is not None:
        return direct
    fields = event.get("fields")
    return _int(fields.get("durationMs")) if isinstance(fields, dict) else None


def _wait_category(event: dict[str, Any]) -> str | None:
    """Return a wait category only when the event declares one explicitly."""
    event_type = event.get("eventType")
    fields = event.get("fields")
    raw_category = fields.get("category") if isinstance(fields, dict) else None
    if raw_category is None and isinstance(fields, dict):
        raw_category = fields.get("waitKind")
    category = str(raw_category).strip().lower() if raw_category is not None else ""
    if event_type in {"ci_wait", "ci_wait_finished"}:
        return "ci"
    if event_type in {
        "contention_wait",
        "contention_wait_finished",
        "resource_wait",
        "resource_wait_finished",
    }:
        return "resource"
    if category in {"ci", "ci_wait"}:
        return "ci"
    if category in {"resource", "resource_wait", "contention", "contention_wait"}:
        return "resource"
    return None


def _run_id(event: dict[str, Any]) -> str | None:
    """Read an explicit run identity without inventing one."""
    value = event.get("runId")
    if isinstance(value, str) and value.strip():
        return value
    fields = event.get("fields")
    nested = fields.get("runId") if isinstance(fields, dict) else None
    return nested if isinstance(nested, str) and nested.strip() else None


def _numeric(value: Any) -> int | float | None:
    return (
        value
        if isinstance(value, (int, float)) and not isinstance(value, bool) and value >= 0
        else None
    )


def _baseline_comparison(report: dict[str, Any], baseline: dict[str, Any] | None) -> dict[str, Any]:
    """Compare only numeric fields present in both reports."""
    if baseline is None:
        return {
            "status": "not_provided",
            "fields": {},
            "limitations": ["no baseline report supplied"],
        }
    if not isinstance(baseline, dict):
        raise PerformanceReportError("baseline report must be a JSON object")
    fields: dict[str, dict[str, int | float]] = {}
    for section, key in (
        ("time", "totalElapsedMs"),
        ("time", "agentActiveMs"),
        ("time", "verificationMs"),
        ("time", "ciWaitMs"),
        ("time", "resourceWaitMs"),
    ):
        current_section = report.get(section)
        baseline_section = baseline.get(section)
        current = _numeric(current_section.get(key)) if isinstance(current_section, dict) else None
        previous = (
            _numeric(baseline_section.get(key)) if isinstance(baseline_section, dict) else None
        )
        if current is None or previous is None:
            continue
        delta = current - previous
        percent = round((delta / previous) * 100, 3) if previous else None
        comparison: dict[str, int | float] = {
            "beforeMs": previous,
            "afterMs": current,
            "deltaMs": delta,
        }
        if percent is not None:
            comparison["deltaPercent"] = percent
        fields[f"{section}.{key}"] = comparison
    limitations: list[str] = []
    if not fields:
        limitations.append("baseline has no matching numeric timing fields")
    return {
        "status": "compared" if fields else "incomparable",
        "fields": fields,
        "limitations": limitations,
    }


def build_report(
    events: list[dict[str, Any]],
    *,
    work_item_id: str,
    ignored_cross_work_item_events: int = 0,
    source_path: str | None = None,
    source_digest: str | None = None,
    baseline: dict[str, Any] | None = None,
    baseline_report: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build deterministic metrics without estimating unavailable categories."""
    phases: dict[str, int] = defaultdict(int)
    gates: dict[str, int] = defaultdict(int)
    gate_runs = 0
    verification_runs = 0
    retries = 0
    backtracks = 0
    human_decisions = 0
    total_elapsed: int | None = None
    ci_wait = 0
    ci_wait_seen = False
    resource_wait = 0
    resource_wait_seen = False
    ambiguous_contention = False
    verification_occurrences: dict[tuple[str, str | None], int] = defaultdict(int)

    for event in events:
        event_type = event.get("eventType")
        duration = _event_duration(event)
        raw_fields = event.get("fields")
        fields: dict[str, Any] = raw_fields if isinstance(raw_fields, dict) else {}
        if event_type == "work_item_finished" and duration is not None:
            total_elapsed = duration if total_elapsed is None else max(total_elapsed, duration)
        if event_type == "lifecycle_phase_finished":
            phase = fields.get("phase") or event.get("phase") or "unknown"
            if isinstance(phase, str) and duration is not None:
                phases[phase] += duration
        if event_type in {"check_started", "check_passed", "check_failed"}:
            check = event.get("checkId")
            if isinstance(check, str):
                gate_runs += event_type == "check_started"
                if duration is not None and event_type != "check_started":
                    gates[check] += duration
                if check == "quality" or check.startswith("quality"):
                    verification_runs += event_type == "check_started"
                    if event_type == "check_started":
                        verification_occurrences[(check, _run_id(event))] += 1
        wait_category = _wait_category(event)
        if event_type in {"wait_finished", "wait_started"} and wait_category is None:
            fields_category = fields.get("category") or fields.get("waitKind")
            if fields_category is not None:
                ambiguous_contention = True
        if wait_category is not None and duration is not None:
            if wait_category == "ci":
                ci_wait += duration
                ci_wait_seen = True
            else:
                resource_wait += duration
                resource_wait_seen = True
        if event_type in {"retry", "work_item_retry"} or fields.get("retry") is True:
            retries += 1
        if (
            event_type in {"backtrack", "backtrack_recorded"}
            or event.get("checkId") == "aiBacktrack"
        ):
            backtracks += 1
        if event_type in {"human_decision_requested", "human_decision_recorded"}:
            human_decisions += 1

    candidates: list[dict[str, Any]] = [
        {"name": f"phase:{name}", "durationMs": duration, "source": "lifecycle_phase_finished"}
        for name, duration in phases.items()
    ] + [
        {"name": f"gate:{name}", "durationMs": duration, "source": "check_result"}
        for name, duration in gates.items()
    ]
    if ci_wait_seen:
        candidates.append({"name": "wait:ci", "durationMs": ci_wait, "source": "explicit_wait"})
    if resource_wait_seen:
        candidates.append(
            {"name": "wait:resource", "durationMs": resource_wait, "source": "explicit_wait"}
        )
    candidates.sort(key=lambda item: (-int(item["durationMs"]), str(item["name"])))
    report: dict[str, Any] = {
        "schemaVersion": 1,
        "workItemId": work_item_id,
        "generatedAt": datetime.now(UTC).isoformat(),
        "source": {
            "kind": "local_observability",
            "path": source_path,
            "sha256": source_digest,
            "ignoredCrossWorkItemEvents": ignored_cross_work_item_events,
        },
        "time": {
            "totalElapsedMs": total_elapsed if total_elapsed is not None else "unknown",
            "phaseDurationsMs": dict(sorted(phases.items())),
            "providerWaitMs": "unknown",
            "humanWaitMs": "unknown",
            "recoveryRetryMs": "unknown",
            "ciWaitMs": ci_wait if ci_wait_seen else "unknown",
            "resourceWaitMs": resource_wait if resource_wait_seen else "unknown",
        },
        "execution": {
            "gateRuns": gate_runs,
            "verificationRuns": verification_runs,
            "retries": retries,
            "backtracks": backtracks,
            "humanDecisions": human_decisions,
        },
        "tokenUsage": {"input": "unknown", "output": "unknown", "total": "unknown"},
        "topBottlenecks": candidates[:3],
        "advisory": True,
        "decisionImpact": "none",
    }
    repeated_verification = [
        {
            "checkId": check,
            "runId": run_id if run_id is not None else "unknown",
            "count": count,
        }
        for (check, run_id), count in sorted(verification_occurrences.items())
        if count > 1
    ]
    comparison = _baseline_comparison(
        report, baseline_report if baseline_report is not None else baseline
    )
    limitations = list(comparison["limitations"])
    if ambiguous_contention:
        limitations.append("ambiguous contention evidence was not attributed")
    report["repeatedVerification"] = repeated_verification
    report["contention"] = {
        "ciWaitMs": ci_wait if ci_wait_seen else "unknown",
        "resourceWaitMs": resource_wait if resource_wait_seen else "unknown",
    }
    report["baselineComparison"] = comparison
    report["diagnosis"] = {
        "status": "measured" if candidates or repeated_verification else "insufficient_evidence",
        "limitations": limitations,
        "causalClaims": [],
    }
    report["reportDigest"] = (
        "sha256:"
        + hashlib.sha256(
            json.dumps(
                {key: value for key, value in report.items() if key != "generatedAt"},
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
            ).encode()
        ).hexdigest()
    )
    return report


def render_markdown(report: dict[str, Any]) -> str:
    time_data = report["time"]
    execution = report["execution"]
    lines = [
        f"# Governance Cost: {report['workItemId']}",
        "",
        "Evidence-only report; unavailable provider and human timings remain `unknown`.",
        "",
        f"- Total elapsed: `{time_data['totalElapsedMs']}` ms",
        f"- Gate runs: `{execution['gateRuns']}`",
        f"- Verification runs: `{execution['verificationRuns']}`",
        f"- Retries / backtracks: `{execution['retries']}` / `{execution['backtracks']}`",
        f"- Human decisions: `{execution['humanDecisions']}`",
        "",
        "## Top bottlenecks",
        "",
    ]
    if report["topBottlenecks"]:
        lines.extend(
            f"{index}. `{item['name']}` — `{item['durationMs']}` ms"
            for index, item in enumerate(report["topBottlenecks"], 1)
        )
    diagnosis = report.get("diagnosis", {})
    lines.extend(["", "## Diagnosis", "", f"- Status: `{diagnosis.get('status', 'unknown')}`"])
    repeated = report.get("repeatedVerification", [])
    if repeated:
        lines.append(
            "- Repeated verification: "
            + ", ".join(
                f"`{item['checkId']}` x{item['count']} (run `{item['runId']}`)" for item in repeated
            )
        )
    else:
        lines.append("- Repeated verification: `none measured`")
    contention = report.get("contention", {})
    lines.append(
        f"- CI / resource contention wait: `{contention.get('ciWaitMs', 'unknown')}` / `"
        f"{contention.get('resourceWaitMs', 'unknown')}` ms"
    )
    comparison = report.get("baselineComparison", {})
    lines.append(f"- Baseline comparison: `{comparison.get('status', 'unknown')}`")
    for limitation in diagnosis.get("limitations", []):
        lines.append(f"- Limitation: {limitation}")
    lines.extend(["", "Advisory only: diagnosis does not change governance decisions."])
    if not report["topBottlenecks"]:
        lines.append("No measured bottlenecks.")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-item", required=True)
    parser.add_argument("--events", type=Path, default=Path("target/ai_observability.jsonl"))
    parser.add_argument("--json-output", type=Path, required=True)
    parser.add_argument("--markdown-output", type=Path, required=True)
    parser.add_argument("--baseline-report", type=Path)
    args = parser.parse_args()
    try:
        events, ignored = load_events(args.events, work_item_id=args.work_item)
        digest = "sha256:" + hashlib.sha256(args.events.read_bytes()).hexdigest()
        baseline = None
        if args.baseline_report is not None:
            try:
                baseline = json.loads(args.baseline_report.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError) as exc:
                raise PerformanceReportError(f"invalid baseline report: {exc}") from exc
        report = build_report(
            events,
            work_item_id=args.work_item,
            ignored_cross_work_item_events=ignored,
            source_path=args.events.as_posix(),
            source_digest=digest,
            baseline=baseline,
        )
    except (OSError, PerformanceReportError) as exc:
        parser.error(str(exc))
    args.json_output.parent.mkdir(parents=True, exist_ok=True)
    args.json_output.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
    args.markdown_output.write_text(render_markdown(report), encoding="utf-8")
    print(f"performance diagnosis written: {args.json_output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

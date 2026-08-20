#!/usr/bin/env python3
"""Build advisory governance-cost metrics from Work Item-scoped evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import defaultdict
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from ai_performance_diagnosis import PerformanceReportError, load_events


def _duration(event: dict[str, Any]) -> int | None:
    value = event.get("durationMs")
    if isinstance(value, int) and value >= 0:
        return value
    fields = event.get("fields")
    nested = fields.get("durationMs") if isinstance(fields, dict) else None
    return nested if isinstance(nested, int) and nested >= 0 else None


def _wait_category(event: dict[str, Any]) -> str | None:
    """Return a wait category only when the event declares one explicitly."""
    event_type = event.get("eventType")
    fields = event.get("fields")
    raw_category = fields.get("category") if isinstance(fields, dict) else None
    if raw_category is None and isinstance(fields, dict):
        raw_category = fields.get("waitKind")
    category = str(raw_category).strip().lower() if raw_category is not None else ""
    if event_type in {"ci_wait_finished", "ci_wait"}:
        category = "ci"
    elif event_type in {"human_wait_finished", "human_wait"}:
        category = "human"
    if category in {"ci", "ci_wait"}:
        return "ci"
    if category in {"human", "human_wait"}:
        return "human"
    return None


def _known(value: int, seen: bool) -> int | str:
    return value if seen else "unknown"


def build_report(
    events: list[dict[str, Any]],
    *,
    work_item_id: str,
    ignored_cross_work_item_events: int = 0,
    source_path: str | None = None,
    source_digest: str | None = None,
) -> dict[str, Any]:
    """Return observed counts and durations without estimating unavailable values."""
    phase_durations: dict[str, int] = defaultdict(int)
    gate_durations: dict[str, int] = defaultdict(int)
    local_compute = 0
    gate_duration = 0
    gate_runs = 0
    verification_runs = 0
    total_elapsed: int | None = None
    verification_duration = 0
    verification_duration_seen = False
    ci_wait = 0
    ci_wait_seen = False
    human_wait = 0
    human_wait_seen = False
    recovery_retry = 0
    recovery_retry_seen = False
    phase_duration_seen = False
    bottleneck_candidates: list[dict[str, Any]] = []
    retries = 0
    backtracks = 0
    human_decisions = 0
    for event in events:
        event_type = event.get("eventType")
        raw_fields = event.get("fields")
        fields: dict[str, Any] = raw_fields if isinstance(raw_fields, dict) else {}
        duration = _duration(event)
        if event_type == "work_item_finished" and duration is not None:
            total_elapsed = duration if total_elapsed is None else max(total_elapsed, duration)
        if event_type == "lifecycle_phase_finished" and duration is not None:
            phase = fields.get("phase") or event.get("phase") or "unknown"
            if isinstance(phase, str):
                phase_durations[phase] += duration
                local_compute += duration
                phase_duration_seen = True
        check_id = event.get("checkId")
        if event_type == "check_started" and isinstance(check_id, str):
            gate_runs += 1
            if check_id == "quality" or check_id.startswith("quality"):
                verification_runs += 1
        if event_type in {"check_passed", "check_failed"} and duration is not None:
            gate_duration += duration
            if isinstance(check_id, str):
                gate_durations[check_id] += duration
                if check_id == "quality" or check_id.startswith("quality"):
                    verification_duration += duration
                    verification_duration_seen = True
        wait_category = _wait_category(event)
        if wait_category is not None and duration is not None:
            if wait_category == "ci":
                ci_wait += duration
                ci_wait_seen = True
            else:
                human_wait += duration
                human_wait_seen = True
        if event_type in {"retry", "work_item_retry"} or fields.get("retry") is True:
            retries += 1
            if duration is not None:
                recovery_retry += duration
                recovery_retry_seen = True
        if event_type in {"backtrack", "backtrack_recorded"} or check_id == "aiBacktrack":
            backtracks += 1
        if event_type in {"human_decision_requested", "human_decision_recorded"}:
            human_decisions += 1

    bottleneck_candidates.extend(
        {
            "name": f"gate:{check_id}",
            "durationMs": duration,
            "source": "check_result",
        }
        for check_id, duration in gate_durations.items()
    )
    if ci_wait_seen:
        bottleneck_candidates.append(
            {"name": "wait:ci", "durationMs": ci_wait, "source": "explicit_wait"}
        )
    if human_wait_seen:
        bottleneck_candidates.append(
            {"name": "wait:human", "durationMs": human_wait, "source": "explicit_wait"}
        )
    if recovery_retry_seen:
        bottleneck_candidates.append(
            {"name": "recovery:retry", "durationMs": recovery_retry, "source": "explicit_recovery"}
        )
    bottleneck_candidates.extend(
        {
            "name": f"phase:{phase}",
            "durationMs": duration,
            "source": "lifecycle_phase_finished",
        }
        for phase, duration in phase_durations.items()
    )
    bottleneck_candidates.sort(key=lambda item: (-int(item["durationMs"]), str(item["name"])))

    observed = {
        "localComputeMs": local_compute if events and local_compute else "unknown",
        "gateDurationMs": gate_duration if gate_duration else "unknown",
        "phaseDurationsMs": dict(sorted(phase_durations.items())),
        "gateRuns": gate_runs,
        "verificationRuns": verification_runs,
        "retries": retries,
        "backtracks": backtracks,
        "humanDecisions": human_decisions,
    }
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
        "observed": observed,
        "time": {
            "totalElapsedMs": total_elapsed if total_elapsed is not None else "unknown",
            "agentActiveMs": _known(local_compute, phase_duration_seen),
            "verificationMs": _known(verification_duration, verification_duration_seen),
            "ciWaitMs": _known(ci_wait, ci_wait_seen),
            "humanWaitMs": _known(human_wait, human_wait_seen),
            "recoveryRetryMs": _known(recovery_retry, recovery_retry_seen),
            "phaseDurationsMs": dict(sorted(phase_durations.items())),
        },
        "execution": {
            "gateRuns": gate_runs,
            "verificationRuns": verification_runs,
            "retries": retries,
            "backtracks": backtracks,
            "humanDecisions": human_decisions,
        },
        "topBottlenecks": bottleneck_candidates[:3],
        "unknown": {
            "providerWaitMs": "unknown",
            "humanWaitMs": "unknown",
            "recoveryRetryMs": "unknown",
            "tokenUsage": {"input": "unknown", "output": "unknown", "total": "unknown"},
        },
        "advisory": True,
        "decisionImpact": "none",
    }
    digest_source = {key: value for key, value in report.items() if key != "generatedAt"}
    digest_payload = json.dumps(
        digest_source, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    )
    report["reportDigest"] = "sha256:" + hashlib.sha256(digest_payload.encode()).hexdigest()
    return report


def render_markdown(report: dict[str, Any]) -> str:
    observed = report["observed"]
    unknown = report["unknown"]
    lines = [
        f"# Governance cost: {report['workItemId']}",
        "",
        "Advisory, evidence-only metrics. Unknown values are not estimated.",
        "",
        f"- Local compute: `{observed['localComputeMs']}` ms",
        f"- Gate duration: `{observed['gateDurationMs']}` ms",
        f"- Gate / verification runs: `{observed['gateRuns']}` / `{observed['verificationRuns']}`",
        f"- Retries / backtracks: `{observed['retries']}` / `{observed['backtracks']}`",
        f"- Human decisions: `{observed['humanDecisions']}`",
        f"- Provider wait / human wait: `{unknown['providerWaitMs']}` / `{unknown['humanWaitMs']}`",
        f"- Total elapsed: `{report['time']['totalElapsedMs']}` ms",
        f"- Agent active / verification: `{report['time']['agentActiveMs']}` / `{report['time']['verificationMs']}` ms",
        f"- CI wait / human wait: `{report['time']['ciWaitMs']}` / `{report['time']['humanWaitMs']}` ms",
        f"- Recovery / retry: `{report['time']['recoveryRetryMs']}` ms",
        "",
        "## Top bottlenecks",
        "",
    ]
    if report["topBottlenecks"]:
        lines.extend(
            f"{index}. `{item['name']}` — `{item['durationMs']}` ms"
            for index, item in enumerate(report["topBottlenecks"], 1)
        )
    else:
        lines.append("No measured bottlenecks.")
    lines.extend(["", f"Report digest: `{report['reportDigest']}`", ""])
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-item", required=True)
    parser.add_argument("--events", type=Path, default=Path("target/ai_observability.jsonl"))
    parser.add_argument("--json-output", type=Path, required=True)
    parser.add_argument("--markdown-output", type=Path, required=True)
    args = parser.parse_args()
    try:
        events, ignored = load_events(args.events, work_item_id=args.work_item)
        source_digest = "sha256:" + hashlib.sha256(args.events.read_bytes()).hexdigest()
        report = build_report(
            events,
            work_item_id=args.work_item,
            ignored_cross_work_item_events=ignored,
            source_path=args.events.as_posix(),
            source_digest=source_digest,
        )
    except (OSError, PerformanceReportError) as exc:
        parser.error(str(exc))
    args.json_output.parent.mkdir(parents=True, exist_ok=True)
    args.json_output.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
    args.markdown_output.write_text(render_markdown(report), encoding="utf-8")
    print(f"governance cost report written: {args.json_output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Enforce per-file coverage regression floors for lifecycle-critical scripts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

CRITICAL_MINIMUMS = {
    "scripts/ai_checkpoint.py": 85.0,
    "scripts/ai_finish.py": 85.0,
    "scripts/ai_doctor.py": 85.0,
    "scripts/ai_check_scope.py": 75.0,
    "scripts/ai_generate_status.py": 80.0,
    "scripts/ai_check_summary.py": 70.0,
    "scripts/ai_archive_work_item.py": 80.0,
    "scripts/ai_check_status.py": 70.0,
    "scripts/ai_check_status_consistency.py": 75.0,
    "scripts/ai_check_review_policy.py": 80.0,
    "scripts/ai_project_doctor.py": 80.0,
    "scripts/ai_project_profile.py": 70.0,
    "scripts/ai_calibrate.py": 60.0,
    "scripts/ai_check_guard_calibration.py": 75.0,
    "scripts/ai_check_agent_risk.py": 75.0,
    "scripts/check_system_invariants.py": 75.0,
    "scripts/ai_check_backtrack.py": 85.0,
    "scripts/ai_check_coverage_guard.py": 85.0,
    "scripts/ai_check_guards.py": 85.0,
    "scripts/ai_check_work_item.py": 70.0,
    "scripts/ai_check_scenario_coverage.py": 75.0,
}


def coverage_failures(report: dict) -> list[str]:
    files = report.get("files", {})
    failures = []
    for path, minimum in CRITICAL_MINIMUMS.items():
        data = files.get(path)
        if not isinstance(data, dict):
            failures.append(f"{path}: missing from coverage report")
            continue
        summary = data.get("summary", {})
        covered = summary.get("percent_covered") if isinstance(summary, dict) else None
        if not isinstance(covered, (int, float)) or covered < minimum:
            actual = "invalid" if not isinstance(covered, (int, float)) else f"{covered:.2f}%"
            failures.append(f"{path}: {actual} is below {minimum:.0f}%")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", nargs="?", default="target/coverage.json")
    args = parser.parse_args()
    try:
        report = json.loads(Path(args.report).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"critical coverage check failed: {exc}", file=sys.stderr)
        return 1
    failures = coverage_failures(report)
    if failures:
        for failure in failures:
            print(f"[ERROR] {failure}", file=sys.stderr)
        return 1
    print(f"critical coverage floors passed: {len(CRITICAL_MINIMUMS)} file(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

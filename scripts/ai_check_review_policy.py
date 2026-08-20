#!/usr/bin/env python3
"""Report changes that should receive explicit AI review focus."""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

from ai_common import (
    PROJECT_ROOT,
    changed_paths,
    included,
    load_json,
    non_empty_string,
    simple_yaml_lists,
    simple_yaml_scalars,
)
from ai_observability import create_observability, elapsed_ms

POLICY = PROJECT_ROOT / ".ai" / "guards" / "ai_review_policy.yaml"
REPORT = PROJECT_ROOT / "target" / "ai_review_policy_report.json"


def review_patterns() -> tuple[list[str], list[str]]:
    lists = simple_yaml_lists(POLICY)
    return (
        lists.get("requiredReviewChecklist.include", []),
        lists.get("requiredReviewChecklist.exclude", []),
    )


def adapter_issues() -> list[str]:
    scalars = simple_yaml_scalars(POLICY)
    mode = scalars.get("adapter.mode", "")
    source = scalars.get("adapter.source", "")
    evidence = scalars.get("adapter.approvalEvidence", "")
    protection = scalars.get("adapter.highRiskProtection", "")
    if not any((mode, source, evidence, protection)):
        return []
    issues: list[str] = []
    if mode not in {"single_maintainer", "codeowners", "dual_approval", "protected_environment"}:
        issues.append("adapter.mode must identify a supported adopter policy")
    if source != "adopter_owned":
        issues.append("adapter.source must remain adopter_owned")
    if evidence not in {"external", "repository", "workflow"}:
        issues.append("adapter.approvalEvidence must identify external evidence")
    if protection not in {"adopter_configured", "required", "not_applicable"}:
        issues.append("adapter.highRiskProtection must identify adopter configuration")
    return issues


def review_focus(summary: dict[str, Any] | None) -> list[str]:
    if not isinstance(summary, dict):
        return []
    readiness = summary.get("reviewReadiness")
    if not isinstance(readiness, dict):
        return []
    focus = readiness.get("expectedReviewFocus")
    if not isinstance(focus, list):
        return []
    return [item for item in focus if non_empty_string(item)]


def detect(paths: list[str], *, include: list[str], exclude: list[str]) -> list[str]:
    return [path for path in paths if included(path, include) and not included(path, exclude)]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Report AI review policy matches.")
    parser.add_argument(
        "--summary", help="Optional AI Change Summary used to inspect reviewReadiness."
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    start = time.time()
    try:
        include, exclude = review_patterns()
        policy_issues = adapter_issues()
        paths = changed_paths()
        summary = load_json(Path(args.summary)) if args.summary else None
    except (OSError, json.JSONDecodeError, RuntimeError, ValueError) as exc:
        print(f"Failed to run review policy check: {exc}", file=sys.stderr)
        return 1

    if policy_issues:
        print("review policy adapter check failed: " + "; ".join(policy_issues), file=sys.stderr)
        return 1
    matched = detect(paths, include=include, exclude=exclude)
    focus = review_focus(summary)
    status = (
        "warning"
        if matched and args.summary and not focus
        else ("review_recommended" if matched else "none")
    )
    report = {
        "status": status,
        "matchedPaths": matched,
        "reviewFocus": focus,
        "summaryPath": args.summary or "",
        "policyPath": POLICY.relative_to(PROJECT_ROOT).as_posix(),
        "adapter": {
            "status": "adopter_owned",
            "mode": simple_yaml_scalars(POLICY).get("adapter.mode", ""),
        },
    }
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    obs = create_observability(
        work_item_id=summary.get("workItemId", "") if isinstance(summary, dict) else ""
    )
    duration = elapsed_ms(start)
    if matched:
        print(f"review policy matched {len(matched)} path(s)")
        for path in matched:
            print(f"[review] {path}")
        if focus:
            print(f"review focus recorded: {len(focus)} item(s)")
        elif args.summary:
            print(
                "[warning] review policy matched paths, but Summary reviewReadiness.expectedReviewFocus is empty"
            )
    else:
        print("review policy: no matched paths")
    print(f"report: {REPORT.relative_to(PROJECT_ROOT)}")
    obs.check_passed(
        check_id="aiReviewPolicy",
        duration_ms=duration,
        fields={"matchedPaths": len(matched), "reviewFocus": len(focus)},
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

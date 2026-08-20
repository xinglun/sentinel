#!/usr/bin/env python3
"""Check that confirmed project boundaries are represented by AI Cockpit Guards."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

from ai_common import simple_yaml_lists, simple_yaml_scalars
from ai_project_profile import load_profile, string_list


def calibration_issues(root: Path, profile: dict[str, Any]) -> list[str]:
    approved = profile.get("approvedBoundaries", {})
    if not isinstance(approved, dict):
        return ["approvedBoundaries must be an object"]
    coverage = simple_yaml_lists(root / ".ai" / "guards" / "coverage_policy.yaml")
    ownership = simple_yaml_scalars(root / ".ai" / "guards" / "file_ownership.yaml")
    review = simple_yaml_lists(root / ".ai" / "guards" / "ai_review_policy.yaml")
    boundary = simple_yaml_scalars(root / ".ai" / "guards" / "file_boundary.yaml")
    checks = simple_yaml_scalars(root / ".ai" / "cockpit" / "checks.yaml")
    issues: list[str] = []
    mappings = (
        (
            "productionRoots",
            coverage.get("production.include", []),
            "coverage_policy production.include",
        ),
        ("testRoots", coverage.get("tests.include", []), "coverage_policy tests.include"),
    )
    for key, configured, label in mappings:
        for pattern in string_list(approved, key) or []:
            if pattern not in configured:
                issues.append(f"{key} pattern is missing from {label}: {pattern}")
    for pattern in string_list(approved, "generatedPaths") or []:
        if f"{pattern}.boundary" not in boundary:
            issues.append(f"generated path is missing from file_boundary.yaml: {pattern}")
    review_patterns = review.get("requiredReviewChecklist.include", [])
    for pattern in string_list(approved, "criticalPaths") or []:
        if f"{pattern}.aiWrite" not in ownership and pattern not in review_patterns:
            issues.append(f"critical path is missing from ownership or review policy: {pattern}")
    for requirement in profile.get("reviewRequirements", []):
        if requirement == "quality" and "checks.quality.command" not in checks:
            issues.append("quality review requirement needs the quality Check ID in checks.yaml")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".")
    parser.add_argument("--profile", default=".ai/project_profile.yaml")
    args = parser.parse_args()
    root = Path(args.root).resolve()
    profile, issues = load_profile(root / args.profile, require_approval=True)
    if not issues:
        issues.extend(calibration_issues(root, profile))
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1
    print("project Profile and Guard calibration check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

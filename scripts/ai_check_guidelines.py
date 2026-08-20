#!/usr/bin/env python3
"""Validate Summary guidelinesCompliance against Contract guidelines."""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

from ai_common import load_json, non_empty_string
from ai_observability import create_observability, elapsed_ms


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate that guidelines declared in the Contract are complied with in the Summary."
    )
    parser.add_argument("--contract", required=True, help="Path to the Contract file")
    parser.add_argument("--summary", required=True, help="Path to the Summary file")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    start_time = time.time()

    contract_path = Path(args.contract)
    summary_path = Path(args.summary)

    if not contract_path.exists():
        print(f"[ERROR] Contract file does not exist: {contract_path}", file=sys.stderr)
        return 1
    if not summary_path.exists():
        print(f"[ERROR] Summary file does not exist: {summary_path}", file=sys.stderr)
        return 1

    try:
        contract = load_json(contract_path)
        summary = load_json(summary_path)
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"[ERROR] Failed to load files: {exc}", file=sys.stderr)
        return 1

    work_item_id = contract.get("workItemId", "")
    obs = create_observability(work_item_id=work_item_id)

    # Get guidelines from Contract
    guidelines = contract.get("guidelines", [])
    if not isinstance(guidelines, list):
        print("[ERROR] Contract guidelines must be a list.", file=sys.stderr)
        return 1

    # Get guidelinesCompliance from Summary
    compliance_list = summary.get("guidelinesCompliance", [])
    if not isinstance(compliance_list, list):
        print("[ERROR] Summary guidelinesCompliance must be a list.", file=sys.stderr)
        return 1

    issues: list[str] = []

    # Map compliance status
    compliance_map = {
        item.get("guideline"): item
        for item in compliance_list
        if isinstance(item, dict) and non_empty_string(item.get("guideline"))
    }

    for guideline in guidelines:
        if not isinstance(guideline, str):
            issues.append(f"Invalid guideline format in Contract: {guideline}")
            continue

        compliance = compliance_map.get(guideline)
        if not compliance:
            issues.append(f'Missing compliance details in Summary for guideline: "{guideline}"')
            continue

        if compliance.get("compliant") is not True:
            issues.append(
                f'Guideline compliance not confirmed (compliant is not true): "{guideline}"'
            )

        if not non_empty_string(compliance.get("evidence")):
            issues.append(f'Empty compliance evidence for guideline: "{guideline}"')

    duration = elapsed_ms(start_time)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        obs.check_failed(
            check_id="aiGuidelines", duration_ms=duration, detail=f"{len(issues)} issue(s)"
        )
        return 1

    print(f"guidelines compliance check passed: {len(guidelines)} guideline(s) verified")
    obs.check_passed(check_id="aiGuidelines", duration_ms=duration)
    return 0


if __name__ == "__main__":
    sys.exit(main())

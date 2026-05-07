#!/usr/bin/env python3
"""AI Change Summary と Work Item Contract の対応を検証する。"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

from ai_observability import create_observability, elapsed_ms


REQUIRED_FIELDS = (
    "workItemId",
    "contractPath",
    "changedFiles",
    "sourcesUsed",
    "verification",
    "unknownsRemaining",
    "risk",
    "generatedFiles",
    "destructiveChanges",
    "observedIssues",
)
RESULTS = {"passed", "failed", "not_run"}
RISK_LEVELS = {"low", "medium", "high"}


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data


def non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def validate_summary(summary: dict[str, Any], contract: dict[str, Any] | None) -> list[str]:
    issues: list[str] = []
    for key in REQUIRED_FIELDS:
        if key not in summary:
            issues.append(f"{key} が不足しています。")

    if contract is not None and summary.get("workItemId") != contract.get("workItemId"):
        issues.append("workItemId が Contract と一致しません。")

    changed = summary.get("changedFiles")
    if not isinstance(changed, list) or not changed:
        issues.append("changedFiles は 1 件以上必要です。")
    elif any(
        not isinstance(item, dict)
        or not non_empty_string(item.get("path"))
        or not non_empty_string(item.get("reason"))
        for item in changed
    ):
        issues.append("changedFiles は path/reason を持つ object list にしてください。")

    verification = summary.get("verification")
    if not isinstance(verification, list) or not verification:
        issues.append("verification は 1 件以上必要です。")
    else:
        for index, item in enumerate(verification):
            if not isinstance(item, dict):
                issues.append(f"verification[{index}] は object にしてください。")
                continue
            if not non_empty_string(item.get("command")):
                issues.append(f"verification[{index}].command は必須です。")
            if item.get("result") not in RESULTS:
                issues.append(f"verification[{index}].result は {sorted(RESULTS)} のいずれかにしてください。")

    risk = summary.get("risk")
    if not isinstance(risk, dict):
        issues.append("risk は object にしてください。")
    else:
        if risk.get("level") not in RISK_LEVELS:
            issues.append(f"risk.level は {sorted(RISK_LEVELS)} のいずれかにしてください。")
        if not non_empty_string(risk.get("detail")):
            issues.append("risk.detail は必須です。")

    for key in ("sourcesUsed", "unknownsRemaining", "generatedFiles", "destructiveChanges", "observedIssues"):
        if key in summary and not isinstance(summary.get(key), list):
            issues.append(f"{key} は list にしてください。")

    if contract is not None:
        required = [
            item.get("command")
            for item in contract.get("verification", [])
            if isinstance(item, dict) and item.get("required") is True and non_empty_string(item.get("command"))
        ]
        status = {
            item.get("command"): item.get("result")
            for item in summary.get("verification", [])
            if isinstance(item, dict)
        }
        missing = [command for command in required if command not in status]
        non_passed = [command for command in required if status.get(command) != "passed"]
        if missing:
            issues.append(f"Summary に required verification が不足しています: {', '.join(missing)}")
        if non_passed:
            issues.append(f"required verification が passed ではありません: {', '.join(non_passed)}")
    return issues


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="AI Change Summary を検証します。")
    parser.add_argument("summary", nargs="?")
    parser.add_argument("--contract")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.summary:
        print("ℹ️ Skipping summary check (no active summary provided)")
        return 0
    start = time.time()
    try:
        summary = load_json(Path(args.summary))
        contract = load_json(Path(args.contract)) if args.contract else None
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"❌ Summary / Contract を読めません: {exc}", file=sys.stderr)
        return 1

    work_item_id = summary.get("workItemId", "")
    obs = create_observability(work_item_id=work_item_id)

    issues = validate_summary(summary, contract)
    duration = elapsed_ms(start)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        print(f"❌ ai summary check failed: {len(issues)} issue(s)", file=sys.stderr)
        obs.check_failed(check_id="aiSummary", duration_ms=duration, detail=f"{len(issues)} issue(s)")
        return 1
    print(f"✅ ai summary check passed: {args.summary}")
    obs.check_passed(check_id="aiSummary", duration_ms=duration)
    return 0


if __name__ == "__main__":
    sys.exit(main())

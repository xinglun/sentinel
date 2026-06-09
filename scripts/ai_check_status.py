#!/usr/bin/env python3
"""Cockpit current_status.md が Contract / Summary と一致することを検証する。"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

from ai_observability import create_observability, elapsed_ms


REQUIRED_FIELDS = ("workItemId", "mode")


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data


def required_commands(contract: dict[str, Any]) -> list[str]:
    return [
        item.get("command")
        for item in contract.get("verification", [])
        if isinstance(item, dict) and item.get("required") is True and isinstance(item.get("command"), str)
    ]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Cockpit current_status.md を検証します。")
    parser.add_argument("status", nargs="?")
    parser.add_argument("--contract")
    parser.add_argument("--summary")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.contract or not args.summary:
        print("ℹ️ Skipping status check (no active contract/summary provided)")
        return 0
    start = time.time()
    try:
        contract = load_json(Path(args.contract))
        summary = load_json(Path(args.summary))
        status = Path(args.status).read_text(encoding="utf-8")
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"❌ Cockpit status を検証できません: {exc}", file=sys.stderr)
        return 1

    work_item_id = contract.get("workItemId", "")
    obs = create_observability(work_item_id=work_item_id)

    issues: list[str] = []
    for key in REQUIRED_FIELDS:
        value = contract.get(key)
        if isinstance(value, str) and f"`{value}`" not in status:
            issues.append(f"status に Contract の {key} がありません: {value}")

    if f"- Contract Path: `{args.contract}`" not in status:
        issues.append("status の Contract Path が一致しません。")
    if f"- Summary Path: `{args.summary}`" not in status:
        issues.append("status の Summary Path が一致しません。")
    ready_states = ("- State: `ready_for_review`", "- State: `ready_with_risks`")
    if not any(marker in status for marker in ready_states):
        issues.append("status が ready_for_review / ready_with_risks ではありません。")
    if "- none" not in status.split("## Required Checks", 1)[0]:
        issues.append("Blocking section が none ではありません。")

    verification_status = {
        item.get("command"): item.get("result")
        for item in summary.get("verification", [])
        if isinstance(item, dict)
    }
    for command in required_commands(contract):
        expected = f"- `{command}`: passed"
        if verification_status.get(command) != "passed":
            issues.append(f"Summary の required check が passed ではありません: {command}")
        if expected not in status:
            issues.append(f"status に required check の passed 表示がありません: {command}")

    duration = elapsed_ms(start)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        print(f"❌ cockpit status check failed: {len(issues)} issue(s)", file=sys.stderr)
        obs.check_failed(check_id="aiStatusCheck", duration_ms=duration, detail=f"{len(issues)} issue(s)")
        return 1
    print(f"✅ cockpit status check passed: {args.status}")
    obs.check_passed(check_id="aiStatusCheck", duration_ms=duration)
    return 0


if __name__ == "__main__":
    sys.exit(main())

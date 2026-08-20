#!/usr/bin/env python3
"""Enforce predecessor closure before a successor Work Item can run."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

from ai_common import load_json


def validate_predecessor(predecessor: Any) -> list[str]:
    if not isinstance(predecessor, dict):
        return ["predecessorWorkItem must be an evidence object"]
    issues: list[str] = []
    if predecessor.get("status") != "closed":
        issues.append("predecessor status must be closed")
    if predecessor.get("pr", {}).get("merged") is not True:
        issues.append("predecessor pr.merged must be true")
    closure = predecessor.get("closure")
    if not isinstance(closure, dict):
        return issues + ["predecessor closure must be an evidence object"]
    for field in ("succeeded", "localBranchDeleted", "remoteBranchDeleted", "baseSynchronized"):
        if closure.get(field) is not True:
            issues.append(f"closure.{field} must be true")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", required=True)
    args = parser.parse_args()
    try:
        contract = load_json(Path(args.contract))
    except (OSError, ValueError) as exc:
        print(f"serial order check failed: {exc}", file=sys.stderr)
        return 1
    predecessor = contract.get("predecessorWorkItem")
    if predecessor is None:
        print("serial order check passed: no predecessor declared")
        return 0
    issues = validate_predecessor(predecessor)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1
    print("serial order check passed: predecessor is fully closed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Fail-closed disable/enable state transitions for an installed Cockpit."""

from __future__ import annotations

import argparse
import json
from copy import deepcopy
from pathlib import Path
from typing import Any


def disable(state: dict[str, Any]) -> dict[str, Any]:
    """Set disabled state while preserving runtime, policy, evidence, and regions."""
    if state.get("state") == "purged":
        return {"state": "blocked", "reason": "purged_installation", "writes": []}
    if state.get("state") == "disabled":
        return {"state": "disabled", "idempotent": True, "writes": []}
    result = deepcopy(state)
    result["state"] = "disabled"
    result["disableEvidence"] = {
        "reason": "explicit_disable",
        "preserved": ["runtime", "policy", "evidence", "archive", "managedRegions"],
    }
    result["blockingEntry"] = True
    return {
        "state": "disabled",
        "writes": ["state", "disableEvidence", "blockingEntry"],
        "stateAfter": result,
    }


def enable(state: dict[str, Any], checks: dict[str, bool]) -> dict[str, Any]:
    """Restore active state only when every integrity/readiness check passes."""
    if state.get("state") == "active":
        return {"state": "active", "idempotent": True, "writes": []}
    required = ("runtimeIntegrity", "manifest", "projectProfile", "policy", "adoptionReadiness")
    failed = [name for name in required if checks.get(name) is not True]
    if failed:
        return {
            "state": "disabled",
            "reason": "readiness_failed",
            "failedChecks": failed,
            "writes": [],
            "resumeCondition": "resolve failed checks and retry",
        }
    result = deepcopy(state)
    result.update({"state": "active", "blockingEntry": False})
    return {"state": "active", "writes": ["state", "blockingEntry"], "stateAfter": result}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("disable", "enable"))
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--checks", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    state = json.loads(args.state.read_text(encoding="utf-8"))
    checks = {}
    if args.command == "enable":
        if args.checks is None:
            parser.error("--checks is required for enable")
        checks = json.loads(args.checks.read_text(encoding="utf-8"))
    result = disable(state) if args.command == "disable" else enable(state, checks)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    return 0 if result.get("state") != "blocked" else 2


if __name__ == "__main__":
    raise SystemExit(main())

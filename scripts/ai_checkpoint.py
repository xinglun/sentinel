#!/usr/bin/env python3
"""Work Item の checkpoint 状態を要約して表示する。"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data


def non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def required_verification(contract: dict[str, Any]) -> list[str]:
    values = contract.get("verification", [])
    if not isinstance(values, list):
        return []
    return [
        str(item["command"]).strip()
        for item in values
        if isinstance(item, dict) and item.get("required") is True and non_empty_string(item.get("command"))
    ]


def contract_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def verification_status(summary: dict[str, Any] | None) -> dict[str, str]:
    if not isinstance(summary, dict):
        return {}
    statuses: dict[str, str] = {}
    for item in summary.get("verification", []):
        if isinstance(item, dict) and non_empty_string(item.get("command")) and isinstance(item.get("result"), str):
            statuses[str(item["command"]).strip()] = str(item["result"])
    return statuses


def review_focus(summary: dict[str, Any] | None) -> list[str]:
    if not isinstance(summary, dict):
        return []
    readiness = summary.get("reviewReadiness")
    if not isinstance(readiness, dict):
        return []
    focus = readiness.get("expectedReviewFocus")
    if not isinstance(focus, list):
        return []
    return [item for item in focus if isinstance(item, str) and item.strip()]


def intent_context(contract: dict[str, Any]) -> list[str]:
    """intent が未設定でも checkpoint の説明を安定して出力する。"""
    intent = contract.get("intent")
    lines: list[str] = []
    if not isinstance(intent, dict):
        return [
            "problem: not provided",
            "constraint: not provided",
            "rationale: not provided",
        ]
    problem = intent.get("problem")
    if isinstance(problem, str) and problem.strip():
        lines.append(f"problem: {problem.strip()}")
    else:
        lines.append("problem: not provided")
    constraints = intent.get("constraints")
    if isinstance(constraints, list) and constraints:
        appended = False
        for item in constraints:
            if isinstance(item, str) and item.strip():
                lines.append(f"constraint: {item.strip()}")
                appended = True
        if not appended:
            lines.append("constraint: not provided")
    else:
        lines.append("constraint: not provided")
    rationale = intent.get("rationale")
    if isinstance(rationale, str) and rationale.strip():
        lines.append(f"rationale: {rationale.strip()}")
    else:
        lines.append("rationale: not provided")
    return lines


def checkpoint_snapshot(contract: dict[str, Any], summary: dict[str, Any] | None, *, stage: str) -> dict[str, Any]:
    required = required_verification(contract)
    status = verification_status(summary)
    passed_required = [command for command in required if status.get(command) == "passed"]
    contract_path = summary.get("contractPath") if isinstance(summary, dict) else ""
    hash_value = ""
    if isinstance(contract_path, str) and contract_path.strip():
        path = Path(contract_path)
        if path.exists():
            hash_value = contract_hash(path)
    if not hash_value and isinstance(contract, dict):
        hash_value = hashlib.sha256(json.dumps(contract, ensure_ascii=False, indent=2).encode("utf-8")).hexdigest()[:16]
    return {
        "stage": stage,
        "recorded": isinstance(summary, dict),
        "detail": "manual checkpoint" if not isinstance(summary, dict) else "summary checkpoint",
        "contractHash": hash_value,
        "acceptanceCount": len(contract.get("acceptance", [])) if isinstance(contract.get("acceptance"), list) else 0,
        "unknownCount": len(contract.get("unknowns", [])) if isinstance(contract.get("unknowns"), list) else 0,
        "requiredChecks": len(required),
        "requiredChecksPassed": len(passed_required),
    }


def next_action(contract: dict[str, Any], summary: dict[str, Any] | None) -> str:
    if contract.get("notCodable") is True:
        return "Stop coding. Resolve notCodable or record blocker/unknowns."
    unknowns = contract.get("unknowns")
    if isinstance(unknowns, list) and unknowns:
        return "Stop coding. Resolve unknowns or switch executionDecision away from continue."
    missing = [
        command
        for command in required_verification(contract)
        if verification_status(summary).get(command) != "passed"
    ]
    if missing:
        return f"Run or record required verification: {missing[0]}"
    return "Ready for final status generation and human review."


def print_list(title: str, values: list[Any]) -> None:
    print(f"\n## {title}")
    if not values:
        print("- none")
        return
    for value in values:
        print(f"- {value}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Print an AI Work Item checkpoint.")
    parser.add_argument("--contract", required=True)
    parser.add_argument("--summary")
    parser.add_argument("--stage", default="manual")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        contract = load_json(Path(args.contract))
        summary = load_json(Path(args.summary)) if args.summary and Path(args.summary).exists() else None
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"Failed to load checkpoint inputs: {exc}", file=sys.stderr)
        return 1

    snapshot = checkpoint_snapshot(contract, summary, stage=args.stage)

    print("# AI Work Item Checkpoint")
    print(f"- Stage: `{snapshot['stage']}`")
    print(f"- Work Item: `{contract.get('workItemId', '')}`")
    print(f"- Contract Hash: `{snapshot['contractHash']}`")
    print(f"- Mode: `{contract.get('mode', '')}`")
    print(f"- Contract Version: `{contract.get('contractVersion', '')}`")
    print(f"- notCodable: `{contract.get('notCodable')}`")
    print(f"- Execution Decision: `{contract.get('executionDecision', {}).get('status', '')}`")
    print(f"- Acceptance Count: `{snapshot['acceptanceCount']}`")
    print(f"- Unknown Count: `{snapshot['unknownCount']}`")
    print(f"- Required Checks: `{snapshot['requiredChecks']}`")
    print(f"- Required Checks Passed: `{snapshot['requiredChecksPassed']}`")
    print(f"- Recorded: `{snapshot['recorded']}`")
    print(f"- Detail: {snapshot['detail']}")

    print_list("Intent Context", intent_context(contract))
    print_list("Scope", contract.get("scope", []) if isinstance(contract.get("scope"), list) else [])
    print_list("Out Of Scope", contract.get("outOfScope", []) if isinstance(contract.get("outOfScope"), list) else [])
    print_list("Unknowns", contract.get("unknowns", []) if isinstance(contract.get("unknowns"), list) else [])
    print_list("Acceptance", contract.get("acceptance", []) if isinstance(contract.get("acceptance"), list) else [])
    print_list("Review Focus", review_focus(summary))

    print("\n## Required Verification")
    required = required_verification(contract)
    if not required:
        print("- none")
    else:
        status = verification_status(summary)
        for command in required:
            print(f"- `{command}`: {status.get(command, 'not_recorded')}")

    print(f"\n## Next Action\n- {next_action(contract, summary)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

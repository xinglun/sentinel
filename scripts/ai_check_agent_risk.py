#!/usr/bin/env python3
"""Validate hard controls for common agent execution risks."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import time
from pathlib import Path
from typing import Any

from ai_common import (
    PROJECT_ROOT,
    load_json,
    non_empty_string,
    simple_yaml_lists,
    verification_key,
    verification_status_for_generation,
)
from ai_observability import create_observability, elapsed_ms

POLICY = PROJECT_ROOT / ".ai" / "guards" / "agent_risk_policy.yaml"
REPORT = PROJECT_ROOT / "target" / "ai_agent_risk_report.json"
NON_CODING_STATUSES = {"defer", "needs_human_decision", "block"}


def command_prefixes(contract: dict[str, Any]) -> list[str]:
    values: list[str] = []
    for item in contract.get("verification", []):
        if isinstance(item, dict) and item.get("required") is True and verification_key(item):
            values.append(verification_key(item))
    return values


def has_required_gate(commands: list[str], required_prefix: str) -> bool:
    return required_prefix in commands


def matching_required_commands(commands: list[str], required_prefix: str) -> list[str]:
    return [command for command in commands if command == required_prefix]


def summary_status(
    summary: dict[str, Any] | None, contract: dict[str, Any] | None = None
) -> dict[str, str]:
    return verification_status_for_generation(summary, contract or {})


def checkpoint_evidence(summary: dict[str, Any] | None) -> list[dict[str, Any]]:
    if not isinstance(summary, dict):
        return []
    evidence = summary.get("checkpointEvidence")
    if not isinstance(evidence, list):
        return []
    return [item for item in evidence if isinstance(item, dict)]


def validate_checkpoint_bindings(
    contract: dict[str, Any],
    summary: dict[str, Any] | None,
    *,
    expected_contract_hash: str = "",
) -> list[str]:
    """Validate checkpoint-to-Contract bindings without requiring finished gates."""
    issues: list[str] = []
    policy = contract.get("checkpointPolicy")
    if not isinstance(policy, dict) or policy.get("requiredBeforeFinish") is not True:
        return issues
    required_stages = [
        item for item in policy.get("requiredStages", []) if isinstance(item, str) and item.strip()
    ]
    evidence = checkpoint_evidence(summary)
    evidence_stages = {
        item.get("stage")
        for item in evidence
        if non_empty_string(item.get("stage")) and item.get("recorded") is True
    }
    missing = [stage for stage in required_stages if stage not in evidence_stages]
    if missing:
        issues.append(f"missing checkpointEvidence for required stage(s): {', '.join(missing)}")
    expected_counts = {
        "acceptanceCount": len(contract.get("acceptance", []))
        if isinstance(contract.get("acceptance"), list)
        else 0,
        "unknownCount": len(contract.get("unknowns", []))
        if isinstance(contract.get("unknowns"), list)
        else 0,
        "requiredChecks": len(command_prefixes(contract)),
    }
    before_edit = next(
        (
            item
            for item in evidence
            if item.get("stage") == "before_edit" and item.get("recorded") is True
        ),
        None,
    )
    before_edit_hash = before_edit.get("contractHash") if isinstance(before_edit, dict) else None
    before_edit_is_stale = bool(
        expected_contract_hash
        and isinstance(before_edit_hash, str)
        and not (
            before_edit_hash == expected_contract_hash
            or before_edit_hash.startswith(expected_contract_hash)
            or expected_contract_hash.startswith(before_edit_hash)
        )
    )
    amendments = [
        item
        for item in evidence
        if item.get("stage") == "contract_amendment_revalidation" and item.get("recorded") is True
    ]
    valid_amendment = False
    amendment_started = False
    if before_edit_is_stale:
        if not amendments:
            issues.append("missing contract_amendment_revalidation for stale before_edit Contract")
        expected_previous_hash = before_edit_hash
        amendment_chain_is_valid = True
        for amendment in amendments:
            if (
                amendment.get("originalBeforeEditContractHash") != before_edit_hash
                or amendment.get("previousContractHash") != expected_previous_hash
                or not non_empty_string(amendment.get("contractHash"))
                or not non_empty_string(amendment.get("reason"))
            ):
                amendment_chain_is_valid = False
                break
            expected_previous_hash = amendment["contractHash"]
        final_amendment = amendments[-1] if amendments else {}
        if not isinstance(final_amendment, dict):
            final_amendment = {}
        hashes_match = isinstance(final_amendment.get("contractHash"), str) and (
            final_amendment["contractHash"] == expected_contract_hash
            or final_amendment["contractHash"].startswith(expected_contract_hash)
            or expected_contract_hash.startswith(final_amendment["contractHash"])
        )
        if amendments:
            amendment = final_amendment
            common_binding_is_valid = (
                amendment_chain_is_valid
                and hashes_match
                and all(amendment.get(key) == expected for key, expected in expected_counts.items())
                and amendment.get("requiredChecksPassed") == 0
            )
            if amendment.get("verificationStarted") is True:
                amendment_started = True
                invalidated = amendment.get("invalidatedRequiredChecks")
                passed_at_amendment = amendment.get("requiredChecksPassedAtAmendment")
                valid_amendment = (
                    common_binding_is_valid
                    and isinstance(invalidated, list)
                    and sorted(invalidated) == sorted(command_prefixes(contract))
                    and isinstance(passed_at_amendment, int)
                    and 0 <= passed_at_amendment <= len(command_prefixes(contract))
                )
            else:
                valid_amendment = common_binding_is_valid
        if amendment_started and not valid_amendment:
            issues.append("contract_amendment_revalidation cannot follow required verification")
        elif amendments and not valid_amendment:
            issues.append("contract_amendment_revalidation is stale or malformed")
    for item in evidence:
        if item.get("stage") not in required_stages or item.get("recorded") is not True:
            continue
        stage = item.get("stage")
        if not non_empty_string(item.get("contractHash")):
            issues.append(f"checkpointEvidence[{stage}].contractHash is required")
        for key in (
            "acceptanceCount",
            "unknownCount",
            "requiredChecks",
            "requiredChecksPassed",
        ):
            if not isinstance(item.get(key), int):
                issues.append(f"checkpointEvidence[{stage}].{key} must be integer")
        recorded_hash = item.get("contractHash")
        hashes_match = isinstance(recorded_hash, str) and (
            recorded_hash == expected_contract_hash
            or recorded_hash.startswith(expected_contract_hash)
            or expected_contract_hash.startswith(recorded_hash)
        )
        if (
            expected_contract_hash
            and not hashes_match
            and not (stage == "before_edit" and before_edit_is_stale and valid_amendment)
        ):
            issues.append(f"checkpointEvidence[{stage}] contractHash is stale")
        for key, expected in expected_counts.items():
            if item.get(key) != expected and not (
                stage == "before_edit" and before_edit_is_stale and valid_amendment
            ):
                issues.append(f"checkpointEvidence[{stage}].{key} is stale")
        if stage == "before_edit" and item.get("requiredChecksPassed") != 0:
            issues.append("before_edit checkpoint must be recorded before required verification")
    return issues


def validate_agent_risks(
    contract: dict[str, Any], summary: dict[str, Any] | None, *, expected_contract_hash: str = ""
) -> list[str]:
    issues: list[str] = []
    policy_lists = simple_yaml_lists(POLICY)
    required_gates = policy_lists.get("risks.promptIsAdvice.requiredVerification", [])
    commands = command_prefixes(contract)
    try:
        statuses = summary_status(summary, contract)
    except ValueError as exc:
        issues.append(str(exc))
        statuses = {}
    for required in required_gates:
        if not has_required_gate(commands, required):
            issues.append(f"missing required AI hard gate verification: {required}")
            continue
        if isinstance(summary, dict) and required != "aiAgentRisk":
            if os.environ.get("AI_FINISH_STABILIZING") == "1" and required in {
                "aiSummary",
                "aiStatus",
                "aiStatusCheck",
            }:
                continue
            passed = [
                command
                for command in matching_required_commands(commands, required)
                if statuses.get(command) == "passed"
            ]
            if not passed:
                issues.append(f"required AI hard gate is not passed in Summary: {required}")

    decision = contract.get("executionDecision")
    decision_status = decision.get("status") if isinstance(decision, dict) else ""
    has_unknowns = isinstance(contract.get("unknowns"), list) and bool(contract.get("unknowns"))
    not_codable = contract.get("notCodable") is True
    mode = contract.get("mode")
    raw_capability = contract.get("agentCapability")
    capability: dict[str, Any] = raw_capability if isinstance(raw_capability, dict) else {}

    if mode == "code" and (has_unknowns or not_codable):
        issues.append(
            "mode code cannot proceed with unknowns or notCodable; use investigate/author_todo/review/cleanup or clear blockers"
        )
    if has_unknowns or not_codable:
        if decision_status not in NON_CODING_STATUSES:
            issues.append(
                "unknowns/notCodable require executionDecision.status to be defer, needs_human_decision, or block"
            )
        if capability.get("canImplement") is True:
            issues.append("unknowns/notCodable require agentCapability.canImplement false")
    if decision_status == "continue" and capability.get("needsHumanDecision") is True:
        issues.append(
            "executionDecision continue conflicts with agentCapability.needsHumanDecision true"
        )

    issues.extend(
        validate_checkpoint_bindings(
            contract,
            summary,
            expected_contract_hash=expected_contract_hash,
        )
    )

    return issues


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate AI agent risk controls.")
    parser.add_argument("--contract")
    parser.add_argument("--summary")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.contract:
        print("Skipping agent risk check (no active contract provided)")
        return 0
    start = time.time()
    try:
        contract = load_json(Path(args.contract))
        summary = (
            load_json(Path(args.summary)) if args.summary and Path(args.summary).exists() else None
        )
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"Failed to run agent risk check: {exc}", file=sys.stderr)
        return 1

    expected_hash = hashlib.sha256(Path(args.contract).read_bytes()).hexdigest()[:16]
    issues = validate_agent_risks(contract, summary, expected_contract_hash=expected_hash)
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(
        json.dumps(
            {
                "status": "error" if issues else "none",
                "issues": issues,
                "contractPath": args.contract,
                "summaryPath": args.summary or "",
                "policyPath": POLICY.relative_to(PROJECT_ROOT).as_posix(),
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    obs = create_observability(work_item_id=contract.get("workItemId", ""))
    duration = elapsed_ms(start)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        print(f"report: {REPORT.relative_to(PROJECT_ROOT)}")
        obs.check_failed(
            check_id="aiAgentRisk", duration_ms=duration, detail=f"{len(issues)} issue(s)"
        )
        return 1
    print("agent risk check passed")
    print(f"report: {REPORT.relative_to(PROJECT_ROOT)}")
    obs.check_passed(check_id="aiAgentRisk", duration_ms=duration)
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""AI Work Item Contract の最低限の整合性を検証する。"""

from __future__ import annotations

import sys
import time
from pathlib import Path
from typing import Any

from ai_json import load_json
from ai_observability import create_observability, elapsed_ms
from ai_scenario_coverage import validate_scenario_coverage


REQUIRED_FIELDS = (
    "contractVersion",
    "workItemId",
    "mode",
    "title",
    "scope",
    "outOfScope",
    "sources",
    "unknowns",
    "notCodable",
    "acceptance",
    "verification",
    "rollbackNote",
)
ALLOWED_FIELDS = set(REQUIRED_FIELDS) | {
    "baseCommit",
    "baselineDirtyPaths",
    "problemStatement",
    "intent",
    "scenarioCoverage",
    "destructiveChangePolicy",
    "riskAssessment",
    "agentCapability",
    "executionDecision",
    "humanReview",
    "preReviewWarnings",
    "checkpointPolicy",
    "archiveRepair",
    "restrictedWriteApproval",
}
MODES = {"investigate", "author_todo", "code", "review", "cleanup"}
REQUIRED_VERIFICATION_COMMANDS = ("make fmt-check",)
REQUIRED_CODE_GATE_PREFIXES = (
    "make check-ai-contract",
    "make check-ai-scope",
    "make check-ai-guards",
    "make check-ai-backtrack",
    "make check-ai-scenario-coverage",
    "make check-ai-change-summary",
    "make generate-cockpit-status",
    "make check-ai-status",
)
RISK_LEVELS = {"low", "medium", "high", "blocked"}
RISK_TYPES = {
    "scope_unclear",
    "evidence_insufficient",
    "architecture_boundary",
    "data_integrity",
    "i18n_snapshot",
    "external_dependency",
    "destructive_change",
    "review_debt",
    "governance_process",
    "security",
    "ci",
    "migration",
    "api_change",
}
EXECUTION_STATUSES = {"continue", "contract_update_required", "blocked", "downgraded_to_investigation"}
REQUIRED_CHECKPOINTS = {"contract_start", "before_edit", "before_ready", "after_verification"}
ALLOWED_INTENT_FIELDS = {"problem", "constraints", "rationale"}


def non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def validate_string_list(data: dict[str, Any], key: str, *, allow_empty: bool) -> list[str]:
    issues: list[str] = []
    value = data.get(key)
    if not isinstance(value, list):
        return [f"{key} は list にしてください。"]
    if not allow_empty and not value:
        issues.append(f"{key} は 1 件以上必要です。")
    for index, item in enumerate(value):
        if not non_empty_string(item):
            issues.append(f"{key}[{index}] は空でない string にしてください。")
    return issues


def validate_sources(data: dict[str, Any]) -> list[str]:
    issues: list[str] = []
    sources = data.get("sources")
    if not isinstance(sources, list) or not sources:
        return ["sources は 1 件以上の list にしてください。"]
    for index, item in enumerate(sources):
        if non_empty_string(item):
            continue
        if isinstance(item, dict):
            if not non_empty_string(item.get("path")):
                issues.append(f"sources[{index}].path は必須です。")
            if not non_empty_string(item.get("reason")):
                issues.append(f"sources[{index}].reason は必須です。")
            continue
        issues.append(f"sources[{index}] は string または path/reason object にしてください。")
    return issues


def validate_optional_problem_statement(data: dict[str, Any]) -> list[str]:
    if "problemStatement" not in data:
        return []
    value = data.get("problemStatement")
    if not non_empty_string(value):
        return ["problemStatement は空でない string にしてください。"]
    return []


def validate_optional_intent(data: dict[str, Any]) -> list[str]:
    if "intent" not in data:
        return []
    issues: list[str] = []
    value = data.get("intent")
    if value is None:
        return []
    if not isinstance(value, dict):
        return ["intent は object にしてください。"]
    for key in value:
        if key not in ALLOWED_INTENT_FIELDS:
            issues.append(f"intent.{key} は許可されていない field です。")
    problem = value.get("problem")
    if problem is not None and not non_empty_string(problem):
        issues.append("intent.problem は空でない string にしてください。")
    constraints = value.get("constraints")
    if constraints is not None:
        issues.extend(validate_string_list({"constraints": constraints}, "constraints", allow_empty=True))
    rationale = value.get("rationale")
    if rationale is not None and not non_empty_string(rationale):
        issues.append("intent.rationale は空でない string にしてください。")
    return issues


def validate_archive_repair(data: dict[str, Any]) -> list[str]:
    if "archiveRepair" not in data:
        return []
    repair = data["archiveRepair"]
    if not isinstance(repair, dict):
        return ["archiveRepair は object にしてください。"]
    required = ("targetPath", "restoreFromCommit", "baseContentSha256", "restoredContentSha256", "reason")
    return [f"archiveRepair.{key} は空でない string にしてください。" for key in required if not isinstance(repair.get(key), str) or not repair[key].strip()]


def validate_optional_v2_fields(data: dict[str, Any]) -> list[str]:
    if data.get("contractVersion") != 2:
        return []
    issues: list[str] = []
    if not non_empty_string(data.get("baseCommit")):
        issues.append("contractVersion 2 では baseCommit が必要です。")
    baseline = data.get("baselineDirtyPaths")
    if not isinstance(baseline, list):
        issues.append("contractVersion 2 では baselineDirtyPaths は list にしてください。")
    else:
        for index, item in enumerate(baseline):
            if isinstance(item, str):
                issues.append(f"baselineDirtyPaths[{index}] は object にしてください。")
                continue
            if not isinstance(item, dict):
                issues.append(f"baselineDirtyPaths[{index}] は object にしてください。")
                continue
            if not non_empty_string(item.get("path")):
                issues.append(f"baselineDirtyPaths[{index}].path は空でない string にしてください。")
            if "status" in item and not non_empty_string(item.get("status")):
                issues.append(f"baselineDirtyPaths[{index}].status は空でない string にしてください。")
            if not non_empty_string(item.get("fingerprint")):
                issues.append(f"baselineDirtyPaths[{index}].fingerprint は空でない string にしてください。")
    issues.extend(validate_optional_problem_statement(data))
    issues.extend(validate_optional_intent(data))
    issues.extend(validate_archive_repair(data))
    issues.extend(validate_scenario_coverage(data.get("scenarioCoverage")))
    return issues


def is_make_command(command: str) -> bool:
    stripped = command.strip()
    return stripped == "make" or stripped.startswith("make ")


def validate_verification_command(command: str, index: int) -> list[str]:
    if is_make_command(command):
        return []
    return [
        f"verification[{index}].command は make entrypoint を使ってください: {command}"
    ]


def validate_verification(data: dict[str, Any]) -> list[str]:
    issues: list[str] = []
    values = data.get("verification")
    if not isinstance(values, list) or not values:
        return ["verification は 1 件以上の list にしてください。"]
    required_commands: set[str] = set()
    for index, item in enumerate(values):
        if not isinstance(item, dict):
            issues.append(f"verification[{index}] は object にしてください。")
            continue
        if not non_empty_string(item.get("command")):
            issues.append(f"verification[{index}].command は必須です。")
        else:
            issues.extend(validate_verification_command(item["command"], index))
        if not isinstance(item.get("required"), bool):
            issues.append(f"verification[{index}].required は boolean にしてください。")
        if item.get("required") is True and non_empty_string(item.get("command")):
            required_commands.add(item["command"].strip())
    for command in REQUIRED_VERIFICATION_COMMANDS:
        if command not in required_commands:
            issues.append(f"verification に required command が必要です: {command}")
    return issues


def required_verification_commands(data: dict[str, Any]) -> set[str]:
    commands: set[str] = set()
    values = data.get("verification")
    if not isinstance(values, list):
        return commands
    for item in values:
        if isinstance(item, dict) and item.get("required") is True and non_empty_string(item.get("command")):
            commands.add(item["command"].strip())
    return commands


def validate_code_gate_verification(data: dict[str, Any]) -> list[str]:
    if data.get("mode") != "code":
        return []
    execution = data.get("executionDecision")
    if isinstance(execution, dict) and execution.get("status") != "continue":
        return []
    required = required_verification_commands(data)
    issues: list[str] = []
    for prefix in REQUIRED_CODE_GATE_PREFIXES:
        if not any(command == prefix or command.startswith(f"{prefix} ") for command in required):
            issues.append(f"code mode の required verification に AI gate が必要です: {prefix}")
    return issues


def validate_optional_risk_assessment(data: dict[str, Any]) -> list[str]:
    if "riskAssessment" not in data:
        return []
    issues: list[str] = []
    value = data.get("riskAssessment")
    if not isinstance(value, dict):
        return ["riskAssessment は object にしてください。"]
    if value.get("level") not in RISK_LEVELS:
        issues.append(f"riskAssessment.level は {sorted(RISK_LEVELS)} のいずれかにしてください。")
    risk_types = value.get("riskTypes")
    if not isinstance(risk_types, list) or not risk_types:
        issues.append("riskAssessment.riskTypes は 1 件以上の list にしてください。")
    else:
        for index, risk_type in enumerate(risk_types):
            if risk_type not in RISK_TYPES:
                issues.append(f"riskAssessment.riskTypes[{index}] は {sorted(RISK_TYPES)} のいずれかにしてください。")
    if not non_empty_string(value.get("reason")):
        issues.append("riskAssessment.reason は必須です。")
    return issues


def validate_optional_agent_capability(data: dict[str, Any]) -> list[str]:
    if "agentCapability" not in data:
        return []
    issues: list[str] = []
    value = data.get("agentCapability")
    if not isinstance(value, dict):
        return ["agentCapability は object にしてください。"]
    for key in ("canImplement", "canVerify", "needsHumanDecision"):
        if not isinstance(value.get(key), bool):
            issues.append(f"agentCapability.{key} は boolean にしてください。")
    blocked_reason = value.get("blockedReason")
    if blocked_reason is not None and not isinstance(blocked_reason, str):
        issues.append("agentCapability.blockedReason は string にしてください。")
    if value.get("needsHumanDecision") is True and not non_empty_string(blocked_reason):
        issues.append("agentCapability.needsHumanDecision: true の場合は blockedReason を記録してください。")
    return issues


def validate_optional_execution_decision(data: dict[str, Any]) -> list[str]:
    if "executionDecision" not in data:
        return []
    issues: list[str] = []
    value = data.get("executionDecision")
    if not isinstance(value, dict):
        return ["executionDecision は object にしてください。"]
    if value.get("status") not in EXECUTION_STATUSES:
        issues.append(f"executionDecision.status は {sorted(EXECUTION_STATUSES)} のいずれかにしてください。")
    if not non_empty_string(value.get("reason")):
        issues.append("executionDecision.reason は必須です。")
    return issues


def validate_optional_human_review(data: dict[str, Any]) -> list[str]:
    if "humanReview" not in data:
        return []
    value = data.get("humanReview")
    if not isinstance(value, dict):
        return ["humanReview は object にしてください。"]
    issues: list[str] = []
    if value.get("status") not in {"pending", "confirmed"}:
        issues.append("humanReview.status は pending または confirmed にしてください。")
    if not non_empty_string(value.get("decision")):
        issues.append("humanReview.decision は必須です。")
    open_questions = value.get("openQuestions")
    if not isinstance(open_questions, list) or not open_questions or not all(non_empty_string(item) for item in open_questions):
        issues.append("humanReview.openQuestions は空でない string の list にしてください。")
    return issues


def validate_optional_checkpoint_policy(data: dict[str, Any]) -> list[str]:
    if "checkpointPolicy" not in data:
        if data.get("mode") == "code":
            return ["code mode では checkpointPolicy が必要です。"]
        return []
    issues: list[str] = []
    value = data.get("checkpointPolicy")
    if not isinstance(value, dict):
        return ["checkpointPolicy は object にしてください。"]
    checkpoints = value.get("requiredCheckpoints")
    if not isinstance(checkpoints, list):
        issues.append("checkpointPolicy.requiredCheckpoints は list にしてください。")
    else:
        actual: set[str] = set()
        for index, checkpoint in enumerate(checkpoints):
            if not non_empty_string(checkpoint):
                issues.append(f"checkpointPolicy.requiredCheckpoints[{index}] は空でない string にしてください。")
                continue
            actual.add(checkpoint)
        missing = REQUIRED_CHECKPOINTS - actual
        if missing:
            issues.append(f"checkpointPolicy.requiredCheckpoints が不足しています: {', '.join(sorted(missing))}")
    if not non_empty_string(value.get("reminder")):
        issues.append("checkpointPolicy.reminder は必須です。")
    return issues


def validate_optional_restricted_write_approval(data: dict[str, Any]) -> list[str]:
    """restricted path の変更に対する明示 approval evidence を検証する。"""
    if "restrictedWriteApproval" not in data:
        return []
    approval = data.get("restrictedWriteApproval")
    if not isinstance(approval, dict):
        return ["restrictedWriteApproval は object にしてください。"]
    issues: list[str] = []
    if not isinstance(approval.get("approved"), bool):
        issues.append("restrictedWriteApproval.approved は boolean にしてください。")
    if approval.get("approved") is True:
        for key in ("approvedBy", "reason"):
            if not non_empty_string(approval.get(key)):
                issues.append(f"restrictedWriteApproval.{key} は approved=true の場合に必須です。")
    return issues


def validate_contract(data: dict[str, Any]) -> list[str]:
    issues: list[str] = []
    for key in REQUIRED_FIELDS:
        if key not in data:
            issues.append(f"{key} が不足しています。")
    for key in data:
        if key not in ALLOWED_FIELDS:
            issues.append(f"未知の field です: {key}")

    if data.get("contractVersion") != 1:
        if data.get("contractVersion") != 2:
            issues.append("contractVersion は 1 または 2 にしてください。")
    if data.get("mode") not in MODES:
        issues.append(f"mode は {sorted(MODES)} のいずれかにしてください。")
    for key in ("workItemId", "title", "rollbackNote"):
        if key in data and not non_empty_string(data.get(key)):
            issues.append(f"{key} は空でない string にしてください。")

    issues.extend(validate_string_list(data, "scope", allow_empty=False))
    issues.extend(validate_string_list(data, "outOfScope", allow_empty=True))
    issues.extend(validate_string_list(data, "unknowns", allow_empty=True))
    issues.extend(validate_string_list(data, "acceptance", allow_empty=False))
    issues.extend(validate_sources(data))
    issues.extend(validate_optional_v2_fields(data))
    issues.extend(validate_verification(data))
    issues.extend(validate_optional_risk_assessment(data))
    issues.extend(validate_optional_agent_capability(data))
    issues.extend(validate_optional_execution_decision(data))
    issues.extend(validate_optional_human_review(data))
    issues.extend(validate_optional_checkpoint_policy(data))
    issues.extend(validate_optional_restricted_write_approval(data))
    issues.extend(validate_code_gate_verification(data))
    if "preReviewWarnings" in data:
        issues.extend(validate_string_list(data, "preReviewWarnings", allow_empty=True))

    if not isinstance(data.get("notCodable"), bool):
        issues.append("notCodable は boolean にしてください。")
    if data.get("mode") == "code" and data.get("notCodable"):
        issues.append("mode: code で notCodable: true の task は coding できません。")
    if data.get("mode") == "code" and data.get("unknowns"):
        issues.append("mode: code で unknowns が残っています。")
    execution_status = None
    execution_decision = data.get("executionDecision")
    if isinstance(execution_decision, dict):
        execution_status = execution_decision.get("status")
    elif data.get("mode") == "code":
        issues.append("mode: code では executionDecision が必要です。")
    if data.get("mode") == "code" and execution_status in {"contract_update_required", "blocked", "downgraded_to_investigation"}:
        issues.append(f"mode: code で executionDecision.status: {execution_status} の task は ready にできません。")
    if isinstance(data.get("agentCapability"), dict) and isinstance(execution_decision, dict):
        capability = data["agentCapability"]
        execution_status = execution_decision.get("status")
        if execution_status == "continue":
            if capability.get("canImplement") is not True:
                issues.append("agentCapability.canImplement: false の task は executionDecision: continue にできません。")
            if capability.get("canVerify") is not True:
                issues.append("agentCapability.canVerify: false の task は executionDecision: continue にできません。")
            if capability.get("needsHumanDecision") is True:
                issues.append("agentCapability.needsHumanDecision: true の task は executionDecision: continue にできません。")
    return issues


def main() -> int:
    if len(sys.argv) < 2 or not sys.argv[1]:
        print("ℹ️ Skipping work item check (no active contract provided)")
        return 0
    path = Path(sys.argv[1])
    start = time.time()
    try:
        data = load_json(path)
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"❌ Work Item Contract を読めません: {exc}", file=sys.stderr)
        return 1

    work_item_id = data.get("workItemId", "")
    obs = create_observability(work_item_id=work_item_id)

    issues = validate_contract(data)
    duration = elapsed_ms(start)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        print(f"❌ work item contract check failed: {len(issues)} issue(s)", file=sys.stderr)
        obs.check_failed(check_id="aiWorkItem", duration_ms=duration, detail=f"{len(issues)} issue(s)")
        return 1
    print(f"✅ work item contract check passed: {path}")
    obs.check_passed(check_id="aiWorkItem", duration_ms=duration)
    return 0


if __name__ == "__main__":
    sys.exit(main())

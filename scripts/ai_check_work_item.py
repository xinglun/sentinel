#!/usr/bin/env python3
"""AI Work Item Contract の最低限の整合性を検証する。"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path
from typing import Any

from ai_observability import create_observability, elapsed_ms


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
    "destructiveChangePolicy",
    "riskAssessment",
    "agentCapability",
    "executionDecision",
    "preReviewWarnings",
}
MODES = {"investigate", "author_todo", "code", "review", "cleanup"}
REQUIRED_VERIFICATION_COMMANDS = ("make fmt-check",)
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
}
EXECUTION_STATUSES = {"continue", "contract_update_required", "blocked", "downgraded_to_investigation"}


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data


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


def validate_contract(data: dict[str, Any]) -> list[str]:
    issues: list[str] = []
    for key in REQUIRED_FIELDS:
        if key not in data:
            issues.append(f"{key} が不足しています。")
    for key in data:
        if key not in ALLOWED_FIELDS:
            issues.append(f"未知の field です: {key}")

    if data.get("contractVersion") != 1:
        issues.append("contractVersion は 1 にしてください。")
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
    issues.extend(validate_verification(data))
    issues.extend(validate_optional_risk_assessment(data))
    issues.extend(validate_optional_agent_capability(data))
    issues.extend(validate_optional_execution_decision(data))
    if "preReviewWarnings" in data:
        issues.extend(validate_string_list(data, "preReviewWarnings", allow_empty=True))

    if not isinstance(data.get("notCodable"), bool):
        issues.append("notCodable は boolean にしてください。")
    if data.get("mode") == "code" and data.get("notCodable"):
        issues.append("mode: code で notCodable: true の task は coding できません。")
    if data.get("mode") == "code" and data.get("unknowns"):
        issues.append("mode: code で unknowns が残っています。")
    if isinstance(data.get("executionDecision"), dict):
        execution_status = data["executionDecision"].get("status")
        if data.get("mode") == "code" and execution_status in {"contract_update_required", "blocked", "downgraded_to_investigation"}:
            issues.append(f"mode: code で executionDecision.status: {execution_status} の task は ready にできません。")
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

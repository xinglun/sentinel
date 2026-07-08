#!/usr/bin/env python3
"""Scenario Coverage の検証と状態判定を共通化する。"""

from __future__ import annotations

from typing import Any


SCENARIO_COVERAGE_STATUSES = {"verified", "unverified", "not_applicable"}
SCENARIO_COVERAGE_STATES = {"complete", "incomplete", "not_required", "unknown"}


def non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def string_list(value: Any, *, allow_empty: bool = True) -> list[str]:
    if not isinstance(value, list):
        return []
    if not allow_empty and not value:
        return []
    return [item for item in value if non_empty_string(item)]


def validate_scenario_coverage(values: Any, *, field_name: str = "scenarioCoverage") -> list[str]:
    """Scenario Coverage の形を軽量に検証する。"""
    if values is None:
        return []
    issues: list[str] = []
    if not isinstance(values, list):
        return [f"{field_name} は list にしてください。"]
    for index, item in enumerate(values):
        if not isinstance(item, dict):
            issues.append(f"{field_name}[{index}] は object にしてください。")
            continue
        if not non_empty_string(item.get("scenario")):
            issues.append(f"{field_name}[{index}].scenario は必須です。")
        if not isinstance(item.get("required"), bool):
            issues.append(f"{field_name}[{index}].required は boolean にしてください。")
        status = item.get("status")
        if status not in SCENARIO_COVERAGE_STATUSES:
            issues.append(
                f"{field_name}[{index}].status は {sorted(SCENARIO_COVERAGE_STATUSES)} のいずれかにしてください。"
            )
        evidence = item.get("evidence")
        if not isinstance(evidence, list):
            issues.append(f"{field_name}[{index}].evidence は list にしてください。")
        elif any(not non_empty_string(entry) for entry in evidence):
            issues.append(f"{field_name}[{index}].evidence は空でない string list にしてください。")
        reason = item.get("reason")
        if status in {"unverified", "not_applicable"} and not non_empty_string(reason):
            issues.append(f"{field_name}[{index}].reason は必須です。")
        if status == "verified" and not evidence:
            issues.append(f"{field_name}[{index}].evidence は verified の場合に 1 件以上必要です。")
    return issues


def _risk_level(contract: dict[str, Any] | None) -> str:
    if not isinstance(contract, dict):
        return "unknown"
    risk = contract.get("riskAssessment")
    if isinstance(risk, dict) and risk.get("level") in {"low", "medium", "high"}:
        return str(risk["level"])
    return "unknown"


def _required_items(values: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [item for item in values if item.get("required") is True]


def scenario_coverage_state(contract: dict[str, Any] | None, summary: dict[str, Any] | None) -> str:
    """current_status 向けの Scenario Coverage 状態を返す。"""
    if not isinstance(summary, dict):
        return "unknown"

    values = summary.get("scenarioCoverage")
    if not isinstance(values, list) or not values:
        level = _risk_level(contract)
        if level == "low":
            return "not_required"
        if level in {"medium", "high"}:
            return "incomplete"
        return "unknown"

    items = [item for item in values if isinstance(item, dict)]
    required = _required_items(items)
    if not required:
        level = _risk_level(contract)
        if level == "low":
            return "not_required"
        if level in {"medium", "high"}:
            return "incomplete"
        return "unknown"

    for item in required:
        status = item.get("status")
        if status == "verified" and string_list(item.get("evidence"), allow_empty=False):
            continue
        if status == "not_applicable" and non_empty_string(item.get("reason")):
            continue
        return "incomplete"
    return "complete"

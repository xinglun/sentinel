#!/usr/bin/env python3
"""Check governance complexity budget and require repayment evidence on overrun."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

from ai_common import load_json, numeric_value, parse_yaml


def enforcement_mode(policy: dict[str, Any], metric: str) -> str:
    enforcement = policy.get("enforcement", {}) if isinstance(policy, dict) else {}
    mode = enforcement.get(metric) if isinstance(enforcement, dict) else None
    return mode if mode in {"error", "warning"} else "error"


def validate_budget_impact(
    contract: dict[str, Any], metrics: dict[str, Any], policy: dict[str, Any]
) -> list[str]:
    limits = policy.get("max", {}) if isinstance(policy, dict) else {}
    impact = contract.get("budgetImpact")
    issues: list[str] = []
    for metric, limit in limits.items():
        values = [metrics.get(metric)]
        if isinstance(impact, dict):
            expected = impact.get("expectedMetrics", {})
            if isinstance(expected, dict):
                values.append(expected.get(metric))
            future = impact.get("reservedFutureMetrics", {})
            if isinstance(future, dict):
                values.append(future.get(metric))
        normalized_limit = numeric_value(limit)
        if normalized_limit is None:
            continue
        for value in values:
            normalized_value = numeric_value(value)
            if normalized_value is None or normalized_value <= normalized_limit:
                continue
            if enforcement_mode(policy, metric) == "warning":
                continue
            approved = (
                isinstance(impact, dict)
                and impact.get("approved") is True
                and impact.get("repaymentWorkItem")
                and impact.get("repaymentRecords")
            )
            if approved:
                continue
            issues.append(f"{metric} exceeds policy max: {value} > {limit}")
            if not isinstance(impact, dict) or not impact.get("repaymentWorkItem"):
                issues.append(f"{metric} overrun requires budgetImpact.repaymentWorkItem")
            if not isinstance(impact, dict) or not impact.get("repaymentRecords"):
                issues.append(f"{metric} overrun requires budgetImpact.repaymentRecords")
    return issues


def budget_warnings(
    contract: dict[str, Any], metrics: dict[str, Any], policy: dict[str, Any]
) -> list[str]:
    limits = policy.get("max", {}) if isinstance(policy, dict) else {}
    warnings: list[str] = []
    impact = contract.get("budgetImpact") if isinstance(contract, dict) else None
    expected = impact.get("expectedMetrics", {}) if isinstance(impact, dict) else {}
    future = impact.get("reservedFutureMetrics", {}) if isinstance(impact, dict) else {}
    for metric, limit in limits.items():
        if enforcement_mode(policy, metric) != "warning":
            continue
        normalized_limit = numeric_value(limit)
        if normalized_limit is None:
            continue
        values = [metrics.get(metric)]
        if isinstance(expected, dict):
            values.append(expected.get(metric))
        if isinstance(future, dict):
            values.append(future.get(metric))
        overrun_values: list[tuple[int | float, Any]] = []
        for value in dict.fromkeys(values):
            normalized_value = numeric_value(value)
            if normalized_value is not None and normalized_value > normalized_limit:
                overrun_values.append((normalized_value, value))
        if overrun_values:
            _normalized, value = max(overrun_values, key=lambda item: item[0])
            warnings.append(f"{metric} exceeds policy max (warning): {value} > {limit}")
    return warnings


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", required=True)
    parser.add_argument("--report", default="target/governance_complexity_report.json")
    parser.add_argument("--policy", default=".ai/guards/governance_complexity_policy.yaml")
    args = parser.parse_args()
    try:
        contract = load_json(Path(args.contract))
        report_path = Path(args.report)
        metrics = load_json(report_path) if report_path.exists() else {}
        policy = parse_yaml(Path(args.policy))
    except (OSError, ValueError) as exc:
        print(f"budget impact check failed: {exc}", file=sys.stderr)
        return 1
    issues = validate_budget_impact(contract, metrics, policy)
    for warning in budget_warnings(contract, metrics, policy):
        print(f"[WARNING] {warning}", file=sys.stderr)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1
    print("budget impact check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

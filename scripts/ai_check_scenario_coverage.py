#!/usr/bin/env python3
"""Scenario Coverage の findings を報告する。"""

from __future__ import annotations

import argparse
import json
import sys
import time
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from ai_json import load_json
from ai_observability import create_observability, elapsed_ms
from ai_scenario_coverage import non_empty_string, scenario_coverage_state, validate_scenario_coverage


PROJECT_ROOT = Path(__file__).resolve().parents[1]
POLICY = PROJECT_ROOT / ".ai" / "guards" / "scenario_coverage_policy.yaml"
REPORT_PATH = PROJECT_ROOT / "target" / "ai_scenario_coverage_report.json"
DEFAULT_HARD_RISK_TYPES = {
    "governance_process",
    "data_integrity",
    "architecture_boundary",
    "i18n_snapshot",
    "external_dependency",
    "destructive_change",
    "security",
    "ci",
    "migration",
    "api_change",
}


@dataclass(frozen=True)
class ScenarioCoverageItem:
    severity: str
    kind: str
    scenario: str
    detail: str


def yaml_scalar(raw: str) -> str:
    return raw.strip().strip('"').strip("'")


def hard_risk_types() -> set[str]:
    if not POLICY.exists():
        return DEFAULT_HARD_RISK_TYPES
    values: set[str] = set()
    in_list = False
    for raw in POLICY.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line == "hardRiskTypes:":
            in_list = True
            continue
        if in_list and line.startswith("- "):
            values.add(yaml_scalar(line[2:]))
        elif in_list and not raw.startswith("  "):
            break
    return values or DEFAULT_HARD_RISK_TYPES


def risk_level(contract: dict[str, Any]) -> str:
    risk = contract.get("riskAssessment")
    if isinstance(risk, dict) and risk.get("level") in {"low", "medium", "high"}:
        return str(risk["level"])
    return "unknown"


def scenario_items(summary: dict[str, Any] | None) -> list[dict[str, Any]]:
    if not isinstance(summary, dict):
        return []
    values = summary.get("scenarioCoverage")
    if not isinstance(values, list):
        return []
    return [item for item in values if isinstance(item, dict)]


def explicit_risk_ack(summary: dict[str, Any] | None) -> bool:
    if not isinstance(summary, dict):
        return False
    readiness = summary.get("reviewReadiness")
    if not isinstance(readiness, dict) or readiness.get("status") != "ready_with_risks":
        return False
    residual_risks = summary.get("residualRisks")
    if not isinstance(residual_risks, list) or not residual_risks:
        return False
    follow_ups = summary.get("followUps")
    unverified = summary.get("unverifiedScenarios")
    return (
        isinstance(follow_ups, list)
        and any(isinstance(item, str) and item.strip() for item in follow_ups)
    ) or (
        isinstance(unverified, list)
        and any(isinstance(item, str) and item.strip() for item in unverified)
    )


def hard_risk(contract: dict[str, Any]) -> bool:
    risk = contract.get("riskAssessment")
    if not isinstance(risk, dict):
        return False
    values = {item for item in risk.get("riskTypes", []) if isinstance(item, str)}
    return any(item in hard_risk_types() for item in values)


def detect(contract: dict[str, Any], summary: dict[str, Any] | None) -> list[ScenarioCoverageItem]:
    items: list[ScenarioCoverageItem] = []
    level = risk_level(contract)
    hard = hard_risk(contract)
    coverage = scenario_items(summary)

    for issue in validate_scenario_coverage(coverage):
        items.append(ScenarioCoverageItem("error", "invalid_scenario_coverage", "", issue))

    if summary is None:
        return [ScenarioCoverageItem("warning", "missing_summary", "", "summary is missing")] + items

    if not coverage:
        if level in {"medium", "high"}:
            severity = "error" if hard else "warning"
            detail = "scenario coverage is missing for medium/high risk"
            items.append(ScenarioCoverageItem(severity, "missing_scenario_coverage", "", detail))
        else:
            items.append(ScenarioCoverageItem("warning", "not_required", "", "scenario coverage is not required for low-risk Work Items"))
        return items

    required_seen = False
    for entry in coverage:
        scenario = str(entry.get("scenario", "")).strip()
        required = entry.get("required") is True
        status = entry.get("status")
        evidence = entry.get("evidence")
        reason = entry.get("reason")

        if not required:
            continue
        required_seen = True

        if status == "verified" and isinstance(evidence, list) and evidence:
            continue

        if status == "verified":
            items.append(ScenarioCoverageItem("error", "missing_evidence", scenario, "required verified scenario has no evidence"))
            continue

        if status == "not_applicable":
            if not non_empty_string(reason):
                items.append(ScenarioCoverageItem("error", "missing_reason", scenario, "required not_applicable scenario is missing reason"))
            continue

        if status == "unverified":
            severity = "warning" if level == "low" or explicit_risk_ack(summary) else "error"
            items.append(ScenarioCoverageItem(severity, "required_scenario_unverified", scenario, "required scenario remains unverified"))
            continue

        items.append(ScenarioCoverageItem("error", "invalid_status", scenario, f"unsupported scenarioCoverage status: {status}"))

    if not required_seen and level in {"medium", "high"}:
        severity = "error" if hard else "warning"
        items.append(
            ScenarioCoverageItem(
                severity,
                "missing_required_scenarios",
                "",
                "scenario coverage has no required scenarios declared for medium/high risk",
            )
        )

    return items


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Scenario Coverage の findings を報告します。")
    parser.add_argument("--contract")
    parser.add_argument("--summary")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.contract:
        print("Skipping scenario coverage check (no active contract provided)")
        return 0

    start = time.time()
    try:
        contract = load_json(Path(args.contract))
        summary = load_json(Path(args.summary)) if args.summary and Path(args.summary).exists() else None
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"Failed to run scenario coverage check: {exc}", file=sys.stderr)
        return 1

    findings = detect(contract, summary)
    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "status": "error" if any(item.severity == "error" for item in findings) else ("warning" if findings else "none"),
        "contractPath": args.contract,
        "summaryPath": args.summary or "",
        "riskLevel": risk_level(contract),
        "hardRiskTypes": sorted(hard_risk_types()),
        "items": [asdict(item) for item in findings],
    }
    REPORT_PATH.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    obs = create_observability(work_item_id=contract.get("workItemId", ""))
    duration = elapsed_ms(start)
    if any(item.severity == "error" for item in findings):
        for item in findings:
            if item.severity == "error":
                print(f"[ERROR] {item.kind}: {item.scenario} - {item.detail}", file=sys.stderr)
        print(f"report: {REPORT_PATH.relative_to(PROJECT_ROOT)}")
        obs.check_failed(check_id="aiScenarioCoverage", duration_ms=duration, detail="scenario coverage findings")
        return 1

    for item in findings:
        print(f"[warning] {item.kind}: {item.scenario} - {item.detail}")
    print(f"report: {REPORT_PATH.relative_to(PROJECT_ROOT)}")
    obs.check_passed(check_id="aiScenarioCoverage", duration_ms=duration, fields={"warnings": len(findings)})
    return 0


if __name__ == "__main__":
    sys.exit(main())

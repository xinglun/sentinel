#!/usr/bin/env python3
"""AI Change Summary と Work Item Contract の対応を検証する。"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

from ai_observability import create_observability, elapsed_ms


REQUIRED_FIELDS = (
    "workItemId",
    "contractPath",
    "changedFiles",
    "sourcesUsed",
    "verification",
    "unknownsRemaining",
    "risk",
    "generatedFiles",
    "destructiveChanges",
    "observedIssues",
)
RESULTS = {"passed", "failed", "not_run"}
RISK_LEVELS = {"low", "medium", "high", "blocked"}
RESIDUAL_RISK_LEVELS = {"low", "medium", "high"}
REVIEW_READINESS_STATUSES = {"ready", "ready_with_risks", "not_ready"}
SOLIDIFICATION_TARGETS = {"contract", "summary", "doc", "template", "guard", "skill", "none_with_reason"}


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data


def non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def is_make_command(command: str) -> bool:
    stripped = command.strip()
    return stripped == "make" or stripped.startswith("make ")


def validate_verification_command(command: str, index: int) -> list[str]:
    if is_make_command(command):
        return []
    return [
        f"verification[{index}].command は make entrypoint を使ってください: {command}"
    ]


def validate_string_list(summary: dict[str, Any], key: str, *, allow_empty: bool = True) -> list[str]:
    issues: list[str] = []
    value = summary.get(key)
    if value is None:
        return issues
    if not isinstance(value, list):
        return [f"{key} は list にしてください。"]
    if not allow_empty and not value:
        issues.append(f"{key} は 1 件以上必要です。")
    for index, item in enumerate(value):
        if not non_empty_string(item):
            issues.append(f"{key}[{index}] は空でない string にしてください。")
    return issues


def validate_optional_residual_risks(summary: dict[str, Any]) -> list[str]:
    if "residualRisks" not in summary:
        return []
    issues: list[str] = []
    value = summary.get("residualRisks")
    if not isinstance(value, list):
        return ["residualRisks は list にしてください。"]
    for index, item in enumerate(value):
        if not isinstance(item, dict):
            issues.append(f"residualRisks[{index}] は object にしてください。")
            continue
        if item.get("level") not in RESIDUAL_RISK_LEVELS:
            issues.append(f"residualRisks[{index}].level は {sorted(RESIDUAL_RISK_LEVELS)} のいずれかにしてください。")
        for key in ("area", "detail"):
            if not non_empty_string(item.get(key)):
                issues.append(f"residualRisks[{index}].{key} は必須です。")
        for key in ("reviewRecommended", "followUpCandidate"):
            if not isinstance(item.get(key), bool):
                issues.append(f"residualRisks[{index}].{key} は boolean にしてください。")
    return issues


def validate_optional_review_readiness(summary: dict[str, Any]) -> list[str]:
    if "reviewReadiness" not in summary:
        return []
    issues: list[str] = []
    value = summary.get("reviewReadiness")
    if not isinstance(value, dict):
        return ["reviewReadiness は object にしてください。"]
    status = value.get("status")
    if status not in REVIEW_READINESS_STATUSES:
        issues.append(f"reviewReadiness.status は {sorted(REVIEW_READINESS_STATUSES)} のいずれかにしてください。")
    if not non_empty_string(value.get("reason")):
        issues.append("reviewReadiness.reason は必須です。")
    focus = value.get("expectedReviewFocus")
    if not isinstance(focus, list):
        issues.append("reviewReadiness.expectedReviewFocus は list にしてください。")
    elif status == "ready_with_risks" and not focus:
        issues.append("reviewReadiness.status: ready_with_risks の場合は expectedReviewFocus が必要です。")
    elif any(not non_empty_string(item) for item in focus):
        issues.append("reviewReadiness.expectedReviewFocus は空でない string list にしてください。")
    if status == "ready_with_risks" and not summary.get("residualRisks"):
        issues.append("reviewReadiness.status: ready_with_risks の場合は residualRisks を記録してください。")
    return issues


def validate_optional_user_correction_solidification(summary: dict[str, Any]) -> list[str]:
    if "userCorrectionSolidification" not in summary:
        return []
    issues: list[str] = []
    value = summary.get("userCorrectionSolidification")
    if not isinstance(value, list):
        return ["userCorrectionSolidification は list にしてください。"]
    for index, item in enumerate(value):
        if not isinstance(item, dict):
            issues.append(f"userCorrectionSolidification[{index}] は object にしてください。")
            continue
        if not non_empty_string(item.get("correction")):
            issues.append(f"userCorrectionSolidification[{index}].correction は必須です。")
        if item.get("solidifiedTo") not in SOLIDIFICATION_TARGETS:
            issues.append(f"userCorrectionSolidification[{index}].solidifiedTo は {sorted(SOLIDIFICATION_TARGETS)} のいずれかにしてください。")
        if not non_empty_string(item.get("reason")):
            issues.append(f"userCorrectionSolidification[{index}].reason は必須です。")
    return issues


def validate_summary(summary: dict[str, Any], contract: dict[str, Any] | None) -> list[str]:
    issues: list[str] = []
    for key in REQUIRED_FIELDS:
        if key not in summary:
            issues.append(f"{key} が不足しています。")

    if contract is not None and summary.get("workItemId") != contract.get("workItemId"):
        issues.append("workItemId が Contract と一致しません。")

    changed = summary.get("changedFiles")
    if not isinstance(changed, list) or not changed:
        issues.append("changedFiles は 1 件以上必要です。")
    elif any(
        not isinstance(item, dict)
        or not non_empty_string(item.get("path"))
        or not non_empty_string(item.get("reason"))
        for item in changed
    ):
        issues.append("changedFiles は path/reason を持つ object list にしてください。")

    verification = summary.get("verification")
    if not isinstance(verification, list) or not verification:
        issues.append("verification は 1 件以上必要です。")
    else:
        for index, item in enumerate(verification):
            if not isinstance(item, dict):
                issues.append(f"verification[{index}] は object にしてください。")
                continue
            if not non_empty_string(item.get("command")):
                issues.append(f"verification[{index}].command は必須です。")
            else:
                issues.extend(validate_verification_command(item["command"], index))
            if item.get("result") not in RESULTS:
                issues.append(f"verification[{index}].result は {sorted(RESULTS)} のいずれかにしてください。")

    risk = summary.get("risk")
    if not isinstance(risk, dict):
        issues.append("risk は object にしてください。")
    else:
        if risk.get("level") not in RISK_LEVELS:
            issues.append(f"risk.level は {sorted(RISK_LEVELS)} のいずれかにしてください。")
        if not non_empty_string(risk.get("detail")):
            issues.append("risk.detail は必須です。")

    for key in ("sourcesUsed", "unknownsRemaining", "generatedFiles", "destructiveChanges", "observedIssues"):
        if key in summary and not isinstance(summary.get(key), list):
            issues.append(f"{key} は list にしてください。")
    issues.extend(validate_string_list(summary, "expectedReviewFocus"))
    issues.extend(validate_string_list(summary, "userCorrectionsCaptured"))
    issues.extend(validate_string_list(summary, "knownGaps"))
    issues.extend(validate_optional_residual_risks(summary))
    issues.extend(validate_optional_review_readiness(summary))
    issues.extend(validate_optional_user_correction_solidification(summary))

    if summary.get("userCorrectionsCaptured") and "userCorrectionSolidification" not in summary:
        issues.append("userCorrectionsCaptured がある場合は userCorrectionSolidification で固化先を記録してください。")

    if contract is not None:
        required = [
            item.get("command")
            for item in contract.get("verification", [])
            if isinstance(item, dict) and item.get("required") is True and non_empty_string(item.get("command"))
        ]
        status = {
            item.get("command"): item.get("result")
            for item in summary.get("verification", [])
            if isinstance(item, dict)
        }
        missing = [command for command in required if command not in status]
        non_passed = [command for command in required if status.get(command) != "passed"]
        if missing:
            issues.append(f"Summary に required verification が不足しています: {', '.join(missing)}")
        if non_passed:
            issues.append(f"required verification が passed ではありません: {', '.join(non_passed)}")
    return issues


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="AI Change Summary を検証します。")
    parser.add_argument("summary", nargs="?")
    parser.add_argument("--contract")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.summary:
        print("ℹ️ Skipping summary check (no active summary provided)")
        return 0
    start = time.time()
    try:
        summary = load_json(Path(args.summary))
        contract = load_json(Path(args.contract)) if args.contract else None
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"❌ Summary / Contract を読めません: {exc}", file=sys.stderr)
        return 1

    work_item_id = summary.get("workItemId", "")
    obs = create_observability(work_item_id=work_item_id)

    issues = validate_summary(summary, contract)
    duration = elapsed_ms(start)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        print(f"❌ ai summary check failed: {len(issues)} issue(s)", file=sys.stderr)
        obs.check_failed(check_id="aiSummary", duration_ms=duration, detail=f"{len(issues)} issue(s)")
        return 1
    print(f"✅ ai summary check passed: {args.summary}")
    obs.check_passed(check_id="aiSummary", duration_ms=duration)
    return 0


if __name__ == "__main__":
    sys.exit(main())

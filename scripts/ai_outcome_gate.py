"""Shared fail-closed terminal Outcome gate."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from ai_check_task_outcome import validate_outcome
from ai_common import load_json


@dataclass(frozen=True)
class OutcomeGateResult:
    valid: bool
    issues: tuple[str, ...]
    outcome: dict[str, Any] | None = None


def validate_terminal_outcome(
    outcome_path: Path,
    markdown_path: Path,
    *,
    expected_task_id: str,
    contract_path: Path,
    summary_path: Path,
    expected_base_commit: str | None = None,
    expected_head_commit: str | None = None,
) -> OutcomeGateResult:
    """Validate the complete current Outcome required by terminal lifecycle steps."""
    issues: list[str] = []
    outcome: dict[str, Any] | None = None

    if not outcome_path.is_file():
        issues.append(f"Outcome JSON is missing: {outcome_path}")
    if not markdown_path.is_file():
        issues.append(f"Outcome Markdown is missing: {markdown_path}")
    if issues:
        return OutcomeGateResult(False, tuple(issues))

    try:
        loaded = load_json(outcome_path)
        markdown = markdown_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError, ValueError, TypeError) as exc:
        return OutcomeGateResult(False, (f"Outcome evidence cannot be loaded: {exc}",))
    if not isinstance(loaded, dict):
        return OutcomeGateResult(False, ("Outcome JSON must be an object",))
    outcome = loaded

    try:
        validation = validate_outcome(
            outcome,
            markdown,
            expected_task_id=expected_task_id,
            contract=load_json(contract_path),
        )
    except (KeyError, TypeError, ValueError) as exc:
        issues.append(f"Outcome schema validation raised an error: {exc}")
    else:
        issues.extend(f"{item.code}: {item.message}" for item in validation.errors)
    if outcome.get("status") != "completed":
        issues.append("terminal Outcome requires status=completed")
    if outcome.get("humanStatusColor") != "green":
        issues.append("terminal Outcome requires humanStatusColor=green")

    bindings = outcome.get("bindings")
    if not isinstance(bindings, dict):
        issues.append("terminal Outcome bindings are missing")
        return OutcomeGateResult(False, tuple(dict.fromkeys(issues)), outcome)

    try:
        contract_digest = _sha256_file(contract_path)
    except OSError as exc:
        issues.append(f"Contract cannot be read for Outcome binding: {exc}")
    else:
        if bindings.get("contractDigest") != contract_digest:
            issues.append("Outcome bindings.contractDigest is stale")

    try:
        summary = load_json(summary_path)
        summary_digest = _sha256_file(summary_path)
    except (OSError, ValueError, TypeError) as exc:
        issues.append(f"Summary cannot be read for Outcome binding: {exc}")
        summary = None
    else:
        if bindings.get("summaryDigest") != summary_digest:
            issues.append("Outcome bindings.summaryDigest is stale")
        if isinstance(summary, dict):
            verification_digest = _sha256_json(summary.get("verification", []))
            if bindings.get("verificationDigest") != verification_digest:
                issues.append("Outcome bindings.verificationDigest is stale")
            if summary.get("workItemId") not in {None, expected_task_id}:
                issues.append("Summary workItemId does not match the Outcome Work Item")

    if expected_base_commit is not None and bindings.get("baseCommit") != expected_base_commit:
        issues.append("Outcome bindings.baseCommit does not match the current Contract")
    if expected_head_commit is not None and bindings.get("headCommit") != expected_head_commit:
        issues.append("Outcome bindings.headCommit does not match the current candidate Head")

    contract_data = None
    try:
        contract_data = load_json(contract_path)
    except (OSError, ValueError, TypeError):
        contract_data = None
    if isinstance(contract_data, dict):
        if contract_data.get("workItemId") != expected_task_id:
            issues.append("Contract workItemId does not match the Outcome Work Item")
        base_commit = contract_data.get("baseCommit")
        if isinstance(base_commit, str) and bindings.get("baseCommit") != base_commit:
            issues.append("Outcome bindings.baseCommit does not match the Contract")

    return OutcomeGateResult(not issues, tuple(dict.fromkeys(issues)), outcome)


def _sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _sha256_json(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()

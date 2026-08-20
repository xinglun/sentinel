"""Single-source lifecycle truth primitives for governed Work Items.

The module deliberately records facts rather than inferring lifecycle progress
from a mutable Summary.  It is small enough to ship to adopters unchanged and
is used by source, installer, and diagnostic integration tests.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

RUNTIME_NAME = "ai_lifecycle_truth.py"
REQUIRED_RUNTIME = (RUNTIME_NAME,)
_REPOSITORY_ISSUE_PREFIX = "https://github.com/spirex-ds-dev/ai-cockpit-template/issues/"
_ARCHIVE_PROJECTION_SUFFIXES = (
    ".contract.json",
    ".summary.json",
    ".review.json",
    ".success.json",
    ".outcome.json",
    ".outcome.md",
    ".events.jsonl",
    ".successor-receipt.json",
)


def _digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _now() -> str:
    return datetime.now(UTC).isoformat()


def _write(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def _read(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise TypeError(f"{path} must contain an object")
    return value


def _nonempty(value: object) -> bool:
    return isinstance(value, str) and bool(value.strip())


def rewrite_exact_path_references(value: Any, replacements: dict[str, str]) -> Any:
    """Return a deep copy with only exact repository-path values replaced."""
    if isinstance(value, dict):
        return {
            key: rewrite_exact_path_references(item, replacements) for key, item in value.items()
        }
    if isinstance(value, list):
        return [rewrite_exact_path_references(item, replacements) for item in value]
    if isinstance(value, str):
        return replacements.get(value, value)
    return value


def archived_outcome_projection(
    outcome: dict[str, Any], *, root: Path, contract_path: Path, work_item_id: str
) -> dict[str, Any]:
    """Project active artifact paths to an existing archive transaction."""
    archive_dir = contract_path.parent
    active_dir = root / ".ai" / "work-items" / "active"
    replacements: dict[str, str] = {}
    for suffix in _ARCHIVE_PROJECTION_SUFFIXES:
        name = f"{work_item_id}{suffix}"
        archived_path = archive_dir / name
        if not archived_path.is_file():
            continue
        replacements[(active_dir / name).relative_to(root).as_posix()] = archived_path.relative_to(
            root
        ).as_posix()
    projected = rewrite_exact_path_references(outcome, replacements)
    if not isinstance(projected, dict):  # pragma: no cover - input type guarantees this
        raise TypeError("archived Outcome projection must remain an object")
    return projected


def _repository_issue(issue: object) -> bool:
    if not _nonempty(issue) or not str(issue).startswith(_REPOSITORY_ISSUE_PREFIX):
        return False
    number = str(issue).removeprefix(_REPOSITORY_ISSUE_PREFIX)
    return number.isdecimal()


def _successor_is_bound(predecessor: dict[str, str], successor: object) -> bool:
    if not isinstance(successor, dict):
        return False
    task = successor.get("workItemId")
    return (
        _nonempty(task)
        and task != predecessor.get("workItemId")
        and successor.get("branch") == f"codex/{task}"
        and isinstance(successor.get("baseCommit"), str)
        and len(successor["baseCommit"]) == 40
        and all(char in "0123456789abcdef" for char in successor["baseCommit"].lower())
    )


def validate_successor_receipt(
    *, predecessor_outcome: Path, predecessor_work_item_id: str, receipt: object
) -> str | None:
    """Return a fail-closed reason unless a receipt binds this blocked Outcome."""
    if not isinstance(receipt, dict) or receipt.get("schemaVersion") != 1:
        return "malformed_receipt"
    try:
        outcome = _read(predecessor_outcome)
    except (OSError, TypeError, ValueError):
        return "predecessor_outcome_unreadable"
    if outcome.get("workItemId") != predecessor_work_item_id or outcome.get("status") != "blocked":
        return "predecessor_not_blocked"
    predecessor = receipt.get("predecessor")
    successor = receipt.get("successor")
    if predecessor != {"workItemId": predecessor_work_item_id}:
        return "unbound_predecessor"
    if not _successor_is_bound({"workItemId": predecessor_work_item_id}, successor):
        return "unbound_successor"
    successor_work_item_id = successor.get("workItemId") if isinstance(successor, dict) else None
    if receipt.get("successorWorkItemId") != successor_work_item_id:
        return "unbound_successor"
    if receipt.get("predecessorOutcomeDigest") != _digest(predecessor_outcome.read_bytes()):
        return "outcome_digest_mismatch"
    if not _repository_issue(receipt.get("issue")):
        return "foreign_issue"
    if not _nonempty(receipt.get("authority")):
        return "missing_authority"
    if not _nonempty(receipt.get("reason")):
        return "missing_reason"
    if receipt.get("transition") not in {"superseded", "quarantined"}:
        return "invalid_transition_mode"
    return None


def is_valid_superseded_transition(*, contract_path: Path, work_item_id: str) -> bool:
    """Return whether exact successor evidence supersedes this blocked Outcome."""
    outcome_path = contract_path.with_name(f"{work_item_id}.outcome.json")
    receipt_path = contract_path.with_name(f"{work_item_id}.successor-receipt.json")
    try:
        receipt = _read(receipt_path)
    except (OSError, TypeError, ValueError):
        return False
    return (
        receipt.get("transition") == "superseded"
        and validate_successor_receipt(
            predecessor_outcome=outcome_path,
            predecessor_work_item_id=work_item_id,
            receipt=receipt,
        )
        is None
    )


def superseded_summary_validation_exception(
    *, contract_path: Path, work_item_id: str, summary_issues: list[str]
) -> bool:
    """Accept only failed-verification issues bound to a superseded blocked predecessor."""
    permitted_prefixes = (
        "Summary is missing required verification:",
        "required verification is not passed:",
    )
    if not summary_issues or any(
        not issue.startswith(permitted_prefixes) for issue in summary_issues
    ):
        return False
    return is_valid_superseded_transition(
        contract_path=contract_path,
        work_item_id=work_item_id,
    )


@dataclass(frozen=True)
class FinishFailure:
    exitCode: int
    archivePermitted: bool
    outcome: dict[str, Any]
    outcomePath: Path


@dataclass(frozen=True)
class CheckpointDecision:
    accepted: bool
    reason: str


@dataclass(frozen=True)
class QualityAttemptDecision:
    sourceBound: bool
    result: str
    verificationStarted: bool
    summarySpoofed: bool


@dataclass(frozen=True)
class RetryOrSuccessorDecision:
    """The lifecycle continuation boundary for a blocked active Work Item."""

    action: str
    successorRequired: bool
    reason: str


@dataclass(frozen=True)
class SuccessorTransition:
    accepted: bool
    reason: str
    receipt: dict[str, Any]


@dataclass(frozen=True)
class InstallerParity:
    ready: bool
    reason: str
    missing: list[str]


@dataclass(frozen=True)
class Projections:
    status: dict[str, Any]
    doctor: dict[str, Any]
    japanese: dict[str, Any]


def _outcome_path(root: Path, task: str) -> Path:
    return root / ".ai" / "work-items" / "active" / f"{task}.outcome.json"


def finish_failure(
    *,
    root: Path,
    identity: dict[str, str],
    failedGate: str,
    message: str,
    archiveRequested: bool,
) -> FinishFailure:
    """Write a red blocked Outcome before returning any finish failure."""
    task = identity["workItemId"]
    outcome = {
        "schemaVersion": 1,
        "workItemId": task,
        "status": "blocked",
        "humanStatusColor": "red",
        "failedGate": failedGate,
        "recoveryCondition": f"Resolve {failedGate} and record a new bound verification attempt before retrying.",
        "message": message,
        "archivePermitted": False,
        "archiveRequested": archiveRequested,
        "identity": dict(identity),
        "recordedAt": _now(),
    }
    path = _outcome_path(root, task)
    _write(path, outcome)
    return FinishFailure(exitCode=1, archivePermitted=False, outcome=outcome, outcomePath=path)


def record_before_edit(
    *, contract: Path, summary: Path, identity: dict[str, str]
) -> dict[str, Any] | CheckpointDecision:
    """Record the one immutable implementation boundary for an active Contract."""
    value = _read(summary)
    if "beforeEdit" in value:
        return CheckpointDecision(False, "before_edit_immutable")
    record = {
        "contractDigest": _digest(contract.read_bytes()),
        "identity": dict(identity),
        "recordedAt": _now(),
        "stage": "before_edit",
    }
    value["beforeEdit"] = record
    _write(summary, value)
    return record


def revalidate_contract(*, contract: Path, summary: Path, reason: str) -> dict[str, Any]:
    """Append a pre-verification Contract amendment without replacing before_edit."""
    value = _read(summary)
    baseline = value.get("beforeEdit")
    if not isinstance(baseline, dict):
        raise TypeError("before_edit_required")
    if value.get("qualityAttemptReceipt"):
        raise ValueError("contract_amendment_locked_after_quality_attempt")
    amended = _digest(contract.read_bytes())
    if baseline.get("contractDigest") == amended:
        raise ValueError("contract_unchanged")
    records = value.setdefault("contractAmendments", [])
    if not isinstance(records, list):
        raise TypeError("contract_amendments_invalid")
    record = {
        "schemaVersion": 1,
        "beforeEditContractDigest": baseline["contractDigest"],
        "newContractDigest": amended,
        "reason": reason,
        "recordedAt": _now(),
    }
    if any(item.get("newContractDigest") == amended for item in records if isinstance(item, dict)):
        raise ValueError("duplicate_contract_amendment")
    records.append(record)
    _write(summary, value)
    return record


def record_quality_attempt(
    *, root: Path, identity: dict[str, str], result: str, command: list[str], output: str
) -> Path:
    """Persist a source-bound passed or failed quality attempt, independent of Summary."""
    if result not in {"passed", "failed"}:
        raise ValueError("quality_result_invalid")
    receipt = {
        "schemaVersion": 1,
        "kind": "ai-cockpit-quality-attempt",
        "identity": dict(identity),
        "result": result,
        "command": list(command),
        "outputDigest": _digest(output.encode("utf-8")),
        "sourceBound": True,
        "recordedAt": _now(),
    }
    path = (
        root / ".ai" / "work-items" / "runtime" / f"{identity['workItemId']}.quality-attempt.json"
    )
    _write(path, receipt)
    return path


def quality_attempt_state(
    *, receipt: Path, identity: dict[str, str], summary: dict[str, Any]
) -> QualityAttemptDecision:
    value = _read(receipt)
    bound = value.get("identity") == identity and value.get("sourceBound") is True
    raw_result = value.get("result")
    result = raw_result if isinstance(raw_result, str) else "invalid"
    summary_spoofed = (
        any(
            isinstance(item, dict)
            and item.get("check") == "quality"
            and item.get("result") == "passed"
            for item in summary.get("verification", [])
            if isinstance(summary.get("verification", []), list)
        )
        and result != "passed"
    )
    return QualityAttemptDecision(
        sourceBound=bound,
        result=result,
        verificationStarted=bound and result in {"passed", "failed"},
        summarySpoofed=summary_spoofed,
    )


def can_amend_contract(decision: QualityAttemptDecision) -> bool:
    return not decision.verificationStarted


def retry_or_successor_decision(
    *,
    sameActiveContract: bool,
    sameScope: bool,
    baseChanged: bool,
    immutableDelivery: bool,
) -> RetryOrSuccessorDecision:
    """Choose in-place evidence retry or the existing governed successor route."""
    if baseChanged:
        return RetryOrSuccessorDecision(
            "governed_successor_or_quarantine",
            True,
            "the delivery must restart from a changed base",
        )
    if not sameActiveContract:
        return RetryOrSuccessorDecision(
            "governed_successor_or_quarantine",
            True,
            "the active Contract is no longer the same delivery boundary",
        )
    if not sameScope:
        return RetryOrSuccessorDecision(
            "governed_successor_or_quarantine",
            True,
            "the Contract scope is invalidated for this delivery",
        )
    if immutableDelivery:
        return RetryOrSuccessorDecision(
            "governed_successor_or_quarantine",
            True,
            "immutable failed-delivery evidence requires a new governed delivery",
        )
    return RetryOrSuccessorDecision(
        "retry_in_place",
        False,
        "same active Contract and scope permit an append-only evidence correction and retry",
    )


def transition_to_successor(
    *,
    predecessorOutcome: Path,
    predecessor: dict[str, str],
    successor: dict[str, str],
    issue: str,
    authority: str,
    mode: str,
    reason: str,
) -> SuccessorTransition:
    """Create the only legal continuation receipt for a blocked predecessor."""
    if not _nonempty(authority):
        return SuccessorTransition(False, "missing_authority", {})
    if not _nonempty(reason):
        return SuccessorTransition(False, "missing_reason", {})
    if mode not in {"superseded", "quarantined"}:
        return SuccessorTransition(False, "invalid_transition_mode", {})
    if not _repository_issue(issue):
        return SuccessorTransition(False, "foreign_issue", {})
    try:
        outcome = _read(predecessorOutcome)
    except (OSError, TypeError, ValueError):
        return SuccessorTransition(False, "predecessor_outcome_unreadable", {})
    if (
        outcome.get("workItemId") != predecessor.get("workItemId")
        or outcome.get("status") != "blocked"
    ):
        return SuccessorTransition(False, "predecessor_not_blocked", {})
    if not _successor_is_bound(predecessor, successor):
        return SuccessorTransition(False, "unbound_successor", {})
    receipt = {
        "schemaVersion": 1,
        "transition": mode,
        "predecessor": dict(predecessor),
        "predecessorOutcomeDigest": _digest(predecessorOutcome.read_bytes()),
        "successor": dict(successor),
        "successorWorkItemId": successor["workItemId"],
        "issue": issue,
        "authority": authority,
        "reason": reason,
        "recordedAt": _now(),
    }
    path = predecessorOutcome.parent / f"{predecessor['workItemId']}.successor-receipt.json"
    _write(path, receipt)
    return SuccessorTransition(True, "accepted", receipt)


def installer_parity(*, source: Path, catalog: dict[str, Any], adopter: Path) -> InstallerParity:
    """Check and stage the one lifecycle runtime used by an isolated adopter."""
    scripts = catalog.get("scripts")
    if not isinstance(scripts, list) or any(item not in scripts for item in REQUIRED_RUNTIME):
        return InstallerParity(False, "missing_catalog_runtime", list(REQUIRED_RUNTIME))
    template = source / "templates" / "make" / "Makefile.ai"
    if "check-ai-lifecycle-truth:" not in template.read_text(encoding="utf-8"):
        return InstallerParity(False, "missing_template_target", ["check-ai-lifecycle-truth"])
    missing = [name for name in REQUIRED_RUNTIME if not (source / "scripts" / name).is_file()]
    if missing:
        return InstallerParity(False, "missing_source_runtime", missing)
    target = adopter / "scripts"
    target.mkdir(parents=True, exist_ok=True)
    for name in REQUIRED_RUNTIME:
        shutil.copy2(source / "scripts" / name, target / name)
    copied = [name for name in REQUIRED_RUNTIME if not (target / name).is_file()]
    return InstallerParity(not copied, "ready" if not copied else "missing_adopter_runtime", copied)


def project_lifecycle_truth(*, outcome: dict[str, Any], languages: tuple[str, ...]) -> Projections:
    """Project the same traffic-light fact for status, doctor, and Japanese UI."""
    common: dict[str, Any] = {
        key: outcome[key]
        for key in ("workItemId", "status", "humanStatusColor", "failedGate", "recoveryCondition")
        if key in outcome
    }
    status = {**common, "projection": "status"}
    doctor = {**common, "projection": "doctor"}
    japanese = {**common, "projection": "status", "language": "ja" if "ja" in languages else "en"}
    return Projections(status=status, doctor=doctor, japanese=japanese)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate installed lifecycle-truth runtime availability."
    )
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--transition-to-successor", action="store_true")
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--predecessor-task")
    parser.add_argument("--successor-task")
    parser.add_argument("--successor-branch")
    parser.add_argument("--successor-base")
    parser.add_argument("--issue")
    parser.add_argument("--authority")
    parser.add_argument("--mode")
    parser.add_argument("--reason")
    args = parser.parse_args()
    if args.check:
        print("lifecycle truth runtime available")
    if args.transition_to_successor:
        required = (
            args.predecessor_task,
            args.successor_task,
            args.successor_branch,
            args.successor_base,
            args.issue,
            args.authority,
            args.mode,
            args.reason,
        )
        if not all(required):
            print("missing_transition_input", file=sys.stderr)
            return 1
        root = args.root.resolve()
        outcome = root / ".ai" / "work-items" / "active" / f"{args.predecessor_task}.outcome.json"
        if not outcome.is_file():
            print("predecessor_outcome_missing", file=sys.stderr)
            return 1
        predecessor = {"workItemId": args.predecessor_task}
        successor = {
            "workItemId": args.successor_task,
            "branch": args.successor_branch,
            "baseCommit": args.successor_base,
        }
        result = transition_to_successor(
            predecessorOutcome=outcome,
            predecessor=predecessor,
            successor=successor,
            issue=args.issue,
            authority=args.authority,
            mode=args.mode,
            reason=args.reason,
        )
        if not result.accepted:
            print(result.reason, file=sys.stderr)
            return 1
        print(json.dumps(result.receipt, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

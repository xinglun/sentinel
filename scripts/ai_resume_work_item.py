#!/usr/bin/env python3
"""Append a source-bound baseline transition to a paused Work Item Contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from ai_common import matches
from ai_start_receipt import (
    PROJECT_ROOT,
    RESUME_SCHEMA_VERSION,
    SYNCHRONIZATION_SCHEMA_VERSION,
    predecessor_closure_snapshot,
    receipt_path,
    validate_receipt,
    validate_resume_history,
    validate_synchronization_history_structure,
    work_branch_identifies_work_item,
)


class ResumeError(ValueError):
    """Raised when a Work Item baseline transition cannot be trusted."""


def governed_git_executable() -> str:
    """Resolve one absolute executable before any controlled local Git operation."""
    resolved = shutil.which("git")
    if not resolved or not Path(resolved).is_absolute():
        raise ResumeError("synchronization requires an absolute Git executable")
    executable = Path(resolved)
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise ResumeError("synchronization requires an executable Git binary")
    return str(executable)


def _load_json(path: Path, description: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        raise ResumeError(f"{description} cannot be read: {exc}") from exc
    if not isinstance(value, dict):
        raise ResumeError(f"{description} must be a JSON object")
    return value


def _git(project_root: Path, *args: str) -> str:
    """Run a controlled Git query through the validated executable boundary."""
    executable = governed_git_executable()
    result = subprocess.run(
        [executable, *args],  # nosec B603 - validated executable and fixed callers
        cwd=project_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "Git command failed"
        raise ResumeError(detail)
    return result.stdout.strip()


def _governed_git(project_root: Path, *args: str, preserve_output: bool = False) -> str:
    """Run a synchronization-only Git query through the absolute executable boundary."""
    executable = governed_git_executable()
    result = subprocess.run(
        [executable, *args],  # nosec B603
        cwd=project_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "Git command failed"
        raise ResumeError(detail)
    return result.stdout if preserve_output else result.stdout.strip()


def _atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    payload = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def _atomic_write_json_pair(
    contract_path: Path, contract: dict[str, Any], summary_path: Path, summary: dict[str, Any]
) -> None:
    """Write active evidence together, restoring both originals on a write failure."""
    originals = {contract_path: contract_path.read_bytes(), summary_path: summary_path.read_bytes()}
    try:
        _atomic_write_json(summary_path, summary)
        _atomic_write_json(contract_path, contract)
    except BaseException:
        for path, original in originals.items():
            path.write_bytes(original)
        raise


def _summary_after_synchronization(summary: dict[str, Any]) -> dict[str, Any]:
    """Invalidate completed verification without claiming it applies to the new base."""
    candidate = dict(summary)
    verification = summary.get("verification")
    if not isinstance(verification, list):
        raise ResumeError("Summary verification must be an array")
    candidate["verification"] = [
        {**item, "result": "not_run"} if isinstance(item, dict) else item for item in verification
    ]
    return candidate


def _clean_worktree(project_root: Path) -> bool:
    return not _git(project_root, "status", "--porcelain", "--untracked-files=all")


def _dirty_paths(project_root: Path) -> list[str]:
    """Return ordinary dirty paths, rejecting ambiguous porcelain records."""
    output = _governed_git(
        project_root,
        "status",
        "--porcelain",
        "--untracked-files=all",
        preserve_output=True,
    )
    paths: list[str] = []
    for line in output.splitlines():
        if len(line) < 4 or line[2] != " ":
            raise ResumeError("synchronization cannot checkpoint an ambiguous Git status record")
        path = line[3:]
        if not path or " -> " in path:
            raise ResumeError("synchronization cannot checkpoint renamed or ambiguous paths")
        paths.append(path)
    return sorted(set(paths))


def _checkpoint_authorized_owned_paths(
    contract: dict[str, Any], paths: list[str], work_item_id: str
) -> None:
    """Allow a checkpoint only for explicit Contract-owned active evidence."""
    checkpoint = contract.get("synchronizationCheckpoint")
    if not isinstance(checkpoint, dict) or checkpoint.get("authorized") is not True:
        raise ResumeError("dirty synchronization requires an explicitly authorized checkpoint")
    if not isinstance(checkpoint.get("reason"), str) or not checkpoint["reason"].strip():
        raise ResumeError("dirty synchronization checkpoint reason is required")
    scope = [item for item in contract.get("scope", []) if isinstance(item, str)]
    scope.extend(
        [
            f".ai/work-items/starts/{work_item_id}.json",
            f".ai/work-items/active/{work_item_id}.contract.json",
            f".ai/work-items/active/{work_item_id}.summary.json",
            f".ai/work-items/active/{work_item_id}.outcome.json",
            f".ai/work-items/active/{work_item_id}.outcome.md",
            ".ai/cockpit/current_status.md",
            ".ai/cockpit/task_report.json",
            ".ai/cockpit/task_report.md",
            "target/ai_*.json",
            "target/ai_*.jsonl",
        ]
    )
    out_of_scope = [item for item in contract.get("outOfScope", []) if isinstance(item, str)]
    for path in paths:
        if any(matches(pattern, path) for pattern in out_of_scope):
            raise ResumeError(f"dirty synchronization path is explicitly out of scope: {path}")
        if not any(matches(pattern, path) for pattern in scope):
            raise ResumeError(f"dirty synchronization path is not Contract-owned: {path}")


def _commit_synchronization_checkpoint(
    project_root: Path, contract: dict[str, Any], work_item_id: str
) -> tuple[str | None, str | None, list[str]]:
    """Commit only an explicitly authorized owned dirty Work Item checkpoint."""
    if _clean_worktree(project_root):
        return None, None, []
    paths = _dirty_paths(project_root)
    checkpoint = contract.get("synchronizationCheckpoint")
    if not isinstance(checkpoint, dict) or checkpoint.get("authorized") is not True:
        raise ResumeError("synchronization requires a clean dedicated Work Item worktree")
    _checkpoint_authorized_owned_paths(contract, paths, work_item_id)
    before = _governed_git(project_root, "rev-parse", "HEAD")
    _governed_git(project_root, "add", "--all", "--", *paths)
    _governed_git(
        project_root,
        "commit",
        "-m",
        f"chore(ai): synchronization checkpoint for {work_item_id}",
    )
    return before, _governed_git(project_root, "rev-parse", "HEAD"), paths


def _rebase_onto(project_root: Path, target: str) -> None:
    executable = governed_git_executable()
    result = subprocess.run(
        [executable, "rebase", target],  # nosec B603
        cwd=project_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return
    abort = subprocess.run(
        [executable, "rebase", "--abort"],  # nosec B603
        cwd=project_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if abort.returncode != 0:
        raise ResumeError("rebase failed and automatic abort failed")
    raise ResumeError("rebase conflicted and was aborted")


def _live_remote_head(project_root: Path, base_remote: str, base_branch: str) -> str:
    """Read the provider's advertised branch tip without changing local refs."""
    executable = governed_git_executable()
    result = subprocess.run(
        [
            executable,
            "ls-remote",
            "--exit-code",
            "--heads",
            base_remote,
            f"refs/heads/{base_branch}",
        ],  # nosec B603
        cwd=project_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise ResumeError("remote default branch cannot be verified")
    fields = result.stdout.strip().split()
    if len(fields) != 2 or len(fields[0]) != 40:
        raise ResumeError("remote default branch returned an invalid revision")
    return fields[0]


def _validate_remote_and_branch(project_root: Path, base_remote: str, base_branch: str) -> None:
    if not base_remote or base_remote.startswith("-"):
        raise ResumeError("base remote must be a configured non-option name")
    _governed_git(project_root, "remote", "get-url", "--", base_remote)
    if not base_branch or base_branch.startswith("-"):
        raise ResumeError("base branch must be a non-option Git branch name")
    _governed_git(project_root, "check-ref-format", "--branch", base_branch)


def synchronize_contract(
    contract_path: Path,
    *,
    summary_path: Path,
    base_remote: str,
    base_branch: str,
    timestamp: str | None = None,
    project_root: Path = PROJECT_ROOT,
) -> dict[str, Any]:
    """Synchronize one active Work Item and append its source-bound transition."""
    contract_path, summary_path, project_root = (
        contract_path.resolve(),
        summary_path.resolve(),
        project_root.resolve(),
    )
    for path, description in ((contract_path, "Contract"), (summary_path, "Summary")):
        try:
            path.relative_to(project_root)
        except ValueError as exc:
            raise ResumeError(f"{description} must be inside the repository") from exc
    original_contract, original_summary = contract_path.read_bytes(), summary_path.read_bytes()
    contract, summary = _load_json(contract_path, "Contract"), _load_json(summary_path, "Summary")
    work_item_id = contract.get("workItemId")
    if not isinstance(work_item_id, str) or not work_item_id:
        raise ResumeError("Contract workItemId is missing")
    if summary.get("workItemId") != work_item_id:
        raise ResumeError("Summary workItemId does not match Contract")
    receipt = _load_json(receipt_path(work_item_id, project_root=project_root), "Start Receipt")
    receipt_issues = validate_receipt(contract, receipt, project_root=project_root)
    if receipt_issues:
        raise ResumeError("current Work Item evidence is invalid: " + "; ".join(receipt_issues))
    if contract.get("synchronizationHistory") is not None:
        raise ResumeError("Work Item already has a synchronization transition")
    work_branch = _governed_git(project_root, "branch", "--show-current")
    if not work_branch:
        raise ResumeError("synchronization requires a checked-out dedicated Work Item branch")
    if work_branch == base_branch:
        raise ResumeError("synchronization requires a dedicated non-base Work Item branch")
    receipt_branch = receipt.get("baseBranch")
    if isinstance(receipt_branch, str) and receipt_branch:
        if receipt_branch == base_branch:
            if not work_branch_identifies_work_item(work_branch, work_item_id):
                raise ResumeError("current branch does not identify this Work Item")
        elif receipt_branch != work_branch:
            raise ResumeError("current branch does not match immutable Start Receipt")
    _validate_remote_and_branch(project_root, base_remote, base_branch)
    target_ref = f"refs/remotes/{base_remote}/{base_branch}"
    target = _governed_git(project_root, "rev-parse", "--verify", target_ref)
    if _live_remote_head(project_root, base_remote, base_branch) != target:
        raise ResumeError("remote tracking ref is stale; fetch and retry before synchronization")
    from_base = contract.get("baseCommit")
    if not isinstance(from_base, str) or not from_base:
        raise ResumeError("Contract baseCommit is missing")
    if from_base == target:
        raise ResumeError("Work Item is already based on the remote default branch")
    try:
        _governed_git(project_root, "merge-base", "--is-ancestor", from_base, target)
    except ResumeError as exc:
        raise ResumeError(
            "current Contract baseCommit is not an ancestor of synchronization target"
        ) from exc
    checkpoint_head_before, checkpoint_head_after, checkpoint_paths = (
        _commit_synchronization_checkpoint(project_root, contract, work_item_id)
    )
    head_before = _governed_git(project_root, "rev-parse", "HEAD")
    try:
        _governed_git(project_root, "merge-base", "--is-ancestor", from_base, head_before)
    except ResumeError as exc:
        raise ResumeError("Work Item branch is unrelated to Contract baseCommit") from exc
    _rebase_onto(project_root, target)
    head_after = _governed_git(project_root, "rev-parse", "HEAD")
    transition = {
        "synchronizationVersion": SYNCHRONIZATION_SCHEMA_VERSION,
        "fromBaseCommit": from_base,
        "toBaseCommit": target,
        "baseRemote": base_remote,
        "baseBranch": base_branch,
        "workBranch": work_branch,
        "recordedAt": timestamp or datetime.now(UTC).isoformat(),
        "priorContractDigest": hashlib.sha256(original_contract).hexdigest(),
        "priorSummaryDigest": hashlib.sha256(original_summary).hexdigest(),
        "rebaseHeadBefore": head_before,
        "rebaseHeadAfter": head_after,
    }
    if checkpoint_head_before is not None and checkpoint_head_after is not None:
        transition["checkpointHeadBefore"] = checkpoint_head_before
        transition["checkpointHeadAfter"] = checkpoint_head_after
        transition["checkpointPaths"] = checkpoint_paths
    candidate_contract = dict(contract)
    candidate_contract["baseCommit"], candidate_contract["synchronizationHistory"] = (
        target,
        [transition],
    )
    structure_issues = validate_synchronization_history_structure(
        candidate_contract, str(receipt.get("baseCommit", ""))
    )
    if structure_issues:
        raise ResumeError("synchronization transition is invalid: " + "; ".join(structure_issues))
    candidate_summary = _summary_after_synchronization(summary)
    try:
        _atomic_write_json_pair(contract_path, candidate_contract, summary_path, candidate_summary)
    except BaseException as exc:
        raise ResumeError(f"synchronization evidence write failed: {exc}") from exc
    return transition


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _require_conflicting_merge(source_root: Path, source_head: str, target: str) -> None:
    """Prove the retained source still conflicts without touching its worktree."""
    result = subprocess.run(
        # nosec B603: executable is resolved and validated; SHAs were validated from receipts.
        [governed_git_executable(), "merge-tree", "--write-tree", source_head, target],
        cwd=source_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        raise ResumeError("source Work Item no longer has a synchronization conflict")


def transition_conflicted_synchronization_to_successor(
    *,
    source_root: Path,
    source_contract_path: Path,
    successor_root: Path,
    successor_contract_path: Path,
    base_remote: str,
    base_branch: str,
    issue: str,
    authority: str,
    reason: str,
) -> dict[str, Any]:
    """Bind one clean current-main successor to an untouched sync-conflict source."""
    source_root, successor_root = source_root.resolve(), successor_root.resolve()
    if source_root == successor_root:
        raise ResumeError("conflict successor must use a distinct source and successor worktree")
    if not issue.startswith("https://github.com/spirex-ds-dev/ai-cockpit-template/issues/"):
        raise ResumeError("conflict successor requires a repository Issue URL")
    if not authority.strip() or not reason.strip():
        raise ResumeError("conflict successor requires authority and reason")
    if not _clean_worktree(source_root) or not _clean_worktree(successor_root):
        raise ResumeError(
            "conflict successor requires clean committed source and successor worktrees"
        )
    source_contract_path = source_contract_path.resolve()
    successor_contract_path = successor_contract_path.resolve()
    for path, root, description in (
        (source_contract_path, source_root, "source Contract"),
        (successor_contract_path, successor_root, "successor Contract"),
    ):
        try:
            path.relative_to(root)
        except ValueError as exc:
            raise ResumeError(f"{description} must be inside its declared worktree") from exc
    source_contract = _load_json(source_contract_path, "source Contract")
    successor_contract = _load_json(successor_contract_path, "successor Contract")
    source_task = source_contract.get("workItemId")
    successor_task = successor_contract.get("workItemId")
    if not isinstance(source_task, str) or not source_task:
        raise ResumeError("source Work Item ID is missing")
    if not isinstance(successor_task, str) or not successor_task or successor_task == source_task:
        raise ResumeError("successor Work Item ID must be distinct")
    source_summary_path = source_contract_path.with_name(
        source_contract_path.name.replace(".contract.json", ".summary.json")
    )
    source_outcome_path = source_contract_path.with_name(
        source_contract_path.name.replace(".contract.json", ".outcome.json")
    )
    source_summary = _load_json(source_summary_path, "source Summary")
    source_outcome = _load_json(source_outcome_path, "source Outcome")
    if source_summary.get("workItemId") != source_task:
        raise ResumeError("source Summary Work Item ID does not match")
    if source_outcome.get("workItemId") != source_task or source_outcome.get("status") != "blocked":
        raise ResumeError("source Work Item must retain a blocked Outcome")
    if source_outcome.get("failedGate") != "synchronization_conflict":
        raise ResumeError("source blocked Outcome is not a synchronization conflict")
    if source_contract.get("synchronizationHistory") is not None:
        raise ResumeError("source Work Item already has a synchronization transition")
    source_receipt_path = receipt_path(source_task, project_root=source_root)
    source_receipt = _load_json(source_receipt_path, "source Start Receipt")
    source_issues = validate_receipt(source_contract, source_receipt, project_root=source_root)
    if source_issues:
        raise ResumeError("source Work Item evidence is invalid: " + "; ".join(source_issues))
    successor_receipt_path = receipt_path(successor_task, project_root=successor_root)
    successor_receipt = _load_json(successor_receipt_path, "successor Start Receipt")
    successor_issues = validate_receipt(
        successor_contract, successor_receipt, project_root=successor_root
    )
    if successor_issues:
        raise ResumeError("successor Work Item evidence is invalid: " + "; ".join(successor_issues))
    source_branch = _governed_git(source_root, "branch", "--show-current")
    successor_branch = _governed_git(successor_root, "branch", "--show-current")
    if not work_branch_identifies_work_item(source_branch, source_task):
        raise ResumeError("source branch does not identify its Work Item")
    if successor_branch != f"codex/{successor_task}":
        raise ResumeError("successor must be on its dedicated codex Work Item branch")
    _validate_remote_and_branch(source_root, base_remote, base_branch)
    target = _governed_git(
        source_root, "rev-parse", "--verify", f"refs/remotes/{base_remote}/{base_branch}"
    )
    if _live_remote_head(source_root, base_remote, base_branch) != target:
        raise ResumeError(
            "remote tracking ref is stale; fetch before conflict successor transition"
        )
    source_base = source_contract.get("baseCommit")
    if not isinstance(source_base, str) or len(source_base) != 40:
        raise ResumeError("source Contract baseCommit is invalid")
    source_head = _governed_git(source_root, "rev-parse", "HEAD")
    _governed_git(source_root, "merge-base", "--is-ancestor", source_base, source_head)
    _governed_git(source_root, "merge-base", "--is-ancestor", source_base, target)
    _require_conflicting_merge(source_root, source_head, target)
    if successor_contract.get("baseCommit") != target:
        raise ResumeError("successor Contract is not bound to the current remote default branch")
    _governed_git(successor_root, "merge-base", "--is-ancestor", target, "HEAD")
    receipt_directory = successor_root / ".ai/work-items/conflict-successor-receipts"
    receipt_path_value = receipt_directory / f"{source_task}.json"
    if receipt_path_value.exists():
        raise ResumeError("conflict successor receipt already exists")
    receipt = {
        "receiptVersion": 1,
        "kind": "synchronization_conflict_successor",
        "issue": issue,
        "authority": authority,
        "reason": reason,
        "targetBaseCommit": target,
        "source": {
            "workItemId": source_task,
            "branch": source_branch,
            "baseCommit": source_base,
            "checkpointHead": source_head,
            "startReceipt": {
                "path": source_receipt_path.relative_to(source_root).as_posix(),
                "sha256": _sha256(source_receipt_path),
            },
            "contract": {
                "path": source_contract_path.relative_to(source_root).as_posix(),
                "sha256": _sha256(source_contract_path),
            },
            "summary": {
                "path": source_summary_path.relative_to(source_root).as_posix(),
                "sha256": _sha256(source_summary_path),
            },
            "outcome": {
                "path": source_outcome_path.relative_to(source_root).as_posix(),
                "sha256": _sha256(source_outcome_path),
            },
        },
        "successor": {
            "workItemId": successor_task,
            "branch": successor_branch,
            "baseCommit": target,
            "startReceipt": {
                "path": successor_receipt_path.relative_to(successor_root).as_posix(),
                "sha256": _sha256(successor_receipt_path),
            },
            "contract": {
                "path": successor_contract_path.relative_to(successor_root).as_posix(),
                "sha256": _sha256(successor_contract_path),
            },
        },
        "recordedAt": datetime.now(UTC).isoformat(),
    }
    original_successor_contract = successor_contract_path.read_bytes()
    receipt_directory.mkdir(parents=True, exist_ok=True)
    try:
        _atomic_write_json(receipt_path_value, receipt)
        successor_contract["conflictSuccessorReceipt"] = {
            "path": receipt_path_value.relative_to(successor_root).as_posix(),
            "sha256": _sha256(receipt_path_value),
        }
        _atomic_write_json(successor_contract_path, successor_contract)
    except BaseException:
        successor_contract_path.write_bytes(original_successor_contract)
        receipt_path_value.unlink(missing_ok=True)
        raise
    return receipt


def _predecessor_transition_fields(contract: dict[str, Any], target: str) -> dict[str, Any]:
    predecessor = contract.get("predecessorWorkItem")
    if not isinstance(predecessor, dict):
        raise ResumeError("predecessorWorkItem must be an evidence object")
    snapshot = predecessor_closure_snapshot(predecessor)
    failed = [field for field, value in snapshot.items() if value is not True]
    if failed:
        if "statusClosed" in failed:
            raise ResumeError("predecessor status must be closed")
        raise ResumeError(f"predecessor closure is incomplete: {', '.join(failed)}")
    work_item_id = predecessor.get("workItemId")
    if not isinstance(work_item_id, str) or not work_item_id:
        raise ResumeError("predecessor Work Item ID is missing")
    pr = predecessor.get("pr")
    merge_commit = pr.get("mergeCommit") if isinstance(pr, dict) else None
    if merge_commit != target:
        raise ResumeError("predecessor merge commit must equal resume target")
    closure = predecessor.get("closure")
    manifest = closure.get("evidence") if isinstance(closure, dict) else None
    if not isinstance(manifest, str) or not manifest:
        raise ResumeError("predecessor archive manifest path is missing")
    return {
        "predecessorWorkItemId": work_item_id,
        "predecessorMergeCommit": merge_commit,
        "predecessorManifestPath": manifest,
        "predecessorClosure": snapshot,
    }


def resume_contract(
    contract_path: Path,
    *,
    base_remote: str,
    base_branch: str,
    timestamp: str | None = None,
    project_root: Path = PROJECT_ROOT,
) -> dict[str, Any]:
    """Validate live repository facts, append one transition, and atomically write."""
    contract_path = contract_path.resolve()
    project_root = project_root.resolve()
    try:
        contract_path.relative_to(project_root)
    except ValueError as exc:
        raise ResumeError("Contract must be inside the repository") from exc
    original_bytes = contract_path.read_bytes()
    contract = _load_json(contract_path, "Contract")
    work_item_id = contract.get("workItemId")
    if not isinstance(work_item_id, str) or not work_item_id:
        raise ResumeError("Contract workItemId is missing")
    receipt_file = receipt_path(work_item_id, project_root=project_root)
    receipt = _load_json(receipt_file, "Start Receipt")
    current_issues = validate_receipt(
        contract,
        receipt,
        project_root=project_root,
        require_latest_predecessor=False,
    )
    if current_issues:
        raise ResumeError("current Work Item evidence is invalid: " + "; ".join(current_issues))

    work_branch = _git(project_root, "branch", "--show-current")
    if not work_branch:
        raise ResumeError("resume requires a checked-out dedicated Work Item branch")
    if work_branch == base_branch:
        raise ResumeError("resume requires a dedicated non-base Work Item branch")
    receipt_branch = receipt.get("baseBranch")
    history = contract.get("resumeHistory")
    if isinstance(history, list) and history:
        first_transition = history[0]
        first_work_branch = (
            first_transition.get("workBranch") if isinstance(first_transition, dict) else None
        )
        if first_work_branch != work_branch:
            raise ResumeError("current branch does not match the first resume transition")
    if isinstance(receipt_branch, str) and receipt_branch:
        if receipt_branch == base_branch:
            if not work_branch_identifies_work_item(work_branch, work_item_id):
                raise ResumeError("compatibility work branch does not identify this Work Item")
        elif receipt_branch != work_branch:
            raise ResumeError("current branch does not match immutable Start Receipt")
    target_ref = f"refs/remotes/{base_remote}/{base_branch}"
    target = _git(project_root, "rev-parse", "--verify", target_ref)
    head = _git(project_root, "rev-parse", "HEAD")
    from_base = str(contract.get("baseCommit", ""))
    if not from_base:
        raise ResumeError("Contract baseCommit is missing")
    if from_base == target:
        raise ResumeError("Work Item is already based on the remote default branch")
    try:
        _git(project_root, "merge-base", "--is-ancestor", from_base, target)
    except ResumeError as exc:
        raise ResumeError(
            "current Contract baseCommit is not an ancestor of resume target"
        ) from exc
    try:
        _git(project_root, "merge-base", "--is-ancestor", target, head)
    except ResumeError as exc:
        raise ResumeError("Work Item branch is not rebased onto the resume target") from exc

    predecessor_fields = _predecessor_transition_fields(contract, target)
    transition = {
        "resumeVersion": RESUME_SCHEMA_VERSION,
        "fromBaseCommit": from_base,
        "toBaseCommit": target,
        "baseRemote": base_remote,
        "baseBranch": base_branch,
        "workBranch": work_branch,
        "recordedAt": timestamp or datetime.now(UTC).isoformat(),
        "priorContractDigest": hashlib.sha256(original_bytes).hexdigest(),
        **predecessor_fields,
    }
    candidate = dict(contract)
    history = contract.get("resumeHistory", [])
    if not isinstance(history, list):
        raise ResumeError("existing resumeHistory must be an array")
    candidate["resumeHistory"] = [*history, transition]
    candidate["baseCommit"] = target
    candidate_issues = validate_resume_history(candidate, receipt, project_root=project_root)
    if candidate_issues:
        raise ResumeError("resume transition is invalid: " + "; ".join(candidate_issues))

    _atomic_write_json(contract_path, candidate)
    return transition


def main() -> int:
    parser = argparse.ArgumentParser(description="Resume a paused governed Work Item.")
    parser.add_argument("--contract", required=True)
    parser.add_argument("--summary")
    parser.add_argument("--base-remote", required=True)
    parser.add_argument("--base-branch", required=True)
    parser.add_argument("--synchronize", action="store_true")
    parser.add_argument("--transition-conflict-successor", action="store_true")
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--source-contract")
    parser.add_argument("--successor-contract")
    parser.add_argument("--issue")
    parser.add_argument("--authority")
    parser.add_argument("--reason")
    parser.add_argument(
        "--project-root",
        type=Path,
        help="Explicit target repository root for a governed synchronization.",
    )
    args = parser.parse_args()
    try:
        project_root = (args.project_root or PROJECT_ROOT).resolve()
        contract_path = project_root / args.contract
        if args.transition_conflict_successor:
            required = (
                args.source_root,
                args.source_contract,
                args.successor_contract,
                args.issue,
                args.authority,
                args.reason,
            )
            if not all(required):
                raise ResumeError(
                    "conflict successor requires source root/Contract, successor Contract, Issue, authority, and reason"
                )
            transition_conflicted_synchronization_to_successor(
                source_root=args.source_root,
                source_contract_path=args.source_root / args.source_contract,
                successor_root=project_root,
                successor_contract_path=project_root / args.successor_contract,
                base_remote=args.base_remote,
                base_branch=args.base_branch,
                issue=args.issue,
                authority=args.authority,
                reason=args.reason,
            )
            operation = "conflict successor transition"
        elif args.synchronize:
            summary_path = (
                project_root / args.summary
                if args.summary
                else contract_path.with_name(
                    contract_path.name.replace(".contract.json", ".summary.json")
                )
            )
            transition = synchronize_contract(
                contract_path,
                summary_path=summary_path,
                base_remote=args.base_remote,
                base_branch=args.base_branch,
                project_root=project_root,
            )
        else:
            transition = resume_contract(
                contract_path,
                base_remote=args.base_remote,
                base_branch=args.base_branch,
                project_root=project_root,
            )
    except (OSError, ResumeError) as exc:
        print(f"[ERROR] Work Item resume failed: {exc}")
        return 1
    operation = locals().get("operation", "synchronization" if args.synchronize else "resume")
    if args.transition_conflict_successor:
        print("Work Item conflict successor transition recorded")
    else:
        print(
            f"Work Item {operation} recorded: {transition['fromBaseCommit']} -> {transition['toBaseCommit']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

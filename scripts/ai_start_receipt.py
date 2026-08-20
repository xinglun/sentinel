"""Create and validate immutable Work Item Start Receipts."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RECEIPTS_DIR = PROJECT_ROOT / ".ai" / "work-items" / "starts"
RECEIPT_SCHEMA_VERSION = 1
RECEIPT_PREFIX = ".ai/work-items/starts/"
RESUME_SCHEMA_VERSION = 1
SYNCHRONIZATION_SCHEMA_VERSION = 1
RESUME_REQUIRED_FIELDS = (
    "resumeVersion",
    "fromBaseCommit",
    "toBaseCommit",
    "baseRemote",
    "baseBranch",
    "workBranch",
    "recordedAt",
    "priorContractDigest",
    "predecessorWorkItemId",
    "predecessorMergeCommit",
    "predecessorManifestPath",
    "predecessorClosure",
)
SYNCHRONIZATION_REQUIRED_FIELDS = (
    "synchronizationVersion",
    "fromBaseCommit",
    "toBaseCommit",
    "baseRemote",
    "baseBranch",
    "workBranch",
    "recordedAt",
    "priorContractDigest",
    "priorSummaryDigest",
    "rebaseHeadBefore",
    "rebaseHeadAfter",
)


def receipt_path(work_item_id: str, *, project_root: Path = PROJECT_ROOT) -> Path:
    return project_root / ".ai" / "work-items" / "starts" / f"{work_item_id}.json"


def _digest(value: Any) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def scope_digest(scope: list[str]) -> str:
    return _digest(scope)


def skeleton_digest(contract: dict[str, Any]) -> str:
    """Digest fields established by ai-start and stable for later contract edits."""
    stable = {
        key: contract.get(key)
        for key in ("contractVersion", "workItemId", "mode", "title", "baseCommit")
    }
    return _digest(stable)


def current_branch(*, project_root: Path = PROJECT_ROOT) -> str:
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=project_root,
        check=False,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def work_branch_identifies_work_item(branch: str, work_item_id: str) -> bool:
    """Return whether compatibility recovery uses the canonical Work Item branch."""
    if not branch or not work_item_id:
        return False
    return branch == f"codex/{work_item_id}"


def build_receipt(
    contract: dict[str, Any],
    *,
    timestamp: str | None = None,
    project_root: Path = PROJECT_ROOT,
) -> dict[str, Any]:
    work_item_id = contract.get("workItemId")
    scope = contract.get("scope")
    base_commit = contract.get("baseCommit")
    if not isinstance(work_item_id, str) or not work_item_id:
        raise ValueError("Contract workItemId is required for a Start Receipt")
    if not isinstance(scope, list) or not all(isinstance(item, str) for item in scope):
        raise ValueError("Contract scope must be a string list for a Start Receipt")
    if not isinstance(base_commit, str) or not base_commit:
        raise ValueError("Contract baseCommit is required for a Start Receipt")
    receipt = {
        "receiptVersion": RECEIPT_SCHEMA_VERSION,
        "workItemId": work_item_id,
        "receiptPath": f"{RECEIPT_PREFIX}{work_item_id}.json",
        "baseCommit": base_commit,
        "baseBranch": current_branch(project_root=project_root),
        "startTimestamp": timestamp or datetime.now(UTC).isoformat(),
        "initialScopeDigest": scope_digest(scope),
        "contractSkeletonDigest": skeleton_digest(contract),
    }
    if contract.get("concurrencyBoundary") is not None:
        receipt["concurrencyBoundaryDigest"] = _digest(contract["concurrencyBoundary"])
    if contract.get("calibrationCorrective") is not None:
        receipt["calibrationCorrectiveDigest"] = _digest(contract["calibrationCorrective"])
    return receipt


def receipt_binding(receipt: dict[str, Any]) -> dict[str, str]:
    binding = {
        "path": str(receipt["receiptPath"]),
        "baseCommit": str(receipt["baseCommit"]),
        "initialScopeDigest": str(receipt["initialScopeDigest"]),
        "contractSkeletonDigest": str(receipt["contractSkeletonDigest"]),
    }
    if "concurrencyBoundaryDigest" in receipt:
        binding["concurrencyBoundaryDigest"] = str(receipt["concurrencyBoundaryDigest"])
    if "calibrationCorrectiveDigest" in receipt:
        binding["calibrationCorrectiveDigest"] = str(receipt["calibrationCorrectiveDigest"])
    return binding


def _is_digest(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def _closed_snapshot_issues(value: Any, location: str) -> list[str]:
    if not isinstance(value, dict):
        return [f"{location} must be an evidence object"]
    issues: list[str] = []
    for field in (
        "statusClosed",
        "prMerged",
        "closureSucceeded",
        "localBranchDeleted",
        "remoteBranchDeleted",
        "baseSynchronized",
    ):
        if value.get(field) is not True:
            issues.append(f"{location}.{field} must be true")
    return issues


def predecessor_closure_snapshot(predecessor: dict[str, Any]) -> dict[str, bool]:
    closure = predecessor.get("closure")
    pr = predecessor.get("pr")
    return {
        "statusClosed": predecessor.get("status") == "closed",
        "prMerged": isinstance(pr, dict) and pr.get("merged") is True,
        "closureSucceeded": isinstance(closure, dict) and closure.get("succeeded") is True,
        "localBranchDeleted": (
            isinstance(closure, dict) and closure.get("localBranchDeleted") is True
        ),
        "remoteBranchDeleted": (
            isinstance(closure, dict) and closure.get("remoteBranchDeleted") is True
        ),
        "baseSynchronized": (isinstance(closure, dict) and closure.get("baseSynchronized") is True),
    }


def validate_resume_history_structure(contract: dict[str, Any], receipt_base: str) -> list[str]:
    """Validate the append-only transition shape without consulting the repository."""
    contract_base = contract.get("baseCommit")
    history = contract.get("resumeHistory")
    if receipt_base == contract_base and history is None:
        return []
    if not isinstance(history, list) or not history:
        return ["resumeHistory is required when Start Receipt and Contract baseCommit differ"]

    issues: list[str] = []
    expected_from = receipt_base
    for index, transition in enumerate(history):
        location = f"resumeHistory[{index}]"
        if not isinstance(transition, dict):
            issues.append(f"{location} must be an evidence object")
            continue
        for field in RESUME_REQUIRED_FIELDS:
            if field not in transition:
                issues.append(f"{location} missing field: {field}")
        if any(field not in transition for field in RESUME_REQUIRED_FIELDS):
            continue
        if transition.get("resumeVersion") != RESUME_SCHEMA_VERSION:
            issues.append(f"{location}.resumeVersion is unsupported")
        for field in (
            "fromBaseCommit",
            "toBaseCommit",
            "baseRemote",
            "baseBranch",
            "workBranch",
            "predecessorWorkItemId",
            "predecessorMergeCommit",
            "predecessorManifestPath",
        ):
            if not isinstance(transition.get(field), str) or not transition[field].strip():
                issues.append(f"{location}.{field} must be a non-empty string")
        if transition.get("fromBaseCommit") != expected_from:
            origin = (
                "the immutable Start Receipt"
                if index == 0
                else f"resumeHistory[{index - 1}].toBaseCommit"
            )
            issues.append(f"{location}.fromBaseCommit does not continue from {origin}")
        if transition.get("toBaseCommit") == transition.get("fromBaseCommit"):
            issues.append(f"{location}.toBaseCommit must advance the baseline")
        if transition.get("predecessorMergeCommit") != transition.get("toBaseCommit"):
            issues.append(f"{location}.predecessorMergeCommit must equal toBaseCommit")
        if transition.get("workBranch") == transition.get("baseBranch"):
            issues.append(f"{location}.workBranch must be a dedicated non-base branch")
        try:
            datetime.fromisoformat(str(transition.get("recordedAt")))
        except ValueError:
            issues.append(f"{location}.recordedAt is not ISO-8601")
        if not _is_digest(transition.get("priorContractDigest")):
            issues.append(f"{location}.priorContractDigest must be a SHA-256 digest")
        manifest_path = str(transition.get("predecessorManifestPath", ""))
        manifest = Path(manifest_path)
        if (
            manifest.is_absolute()
            or ".." in manifest.parts
            or not manifest_path.startswith(".ai/work-items/archive/")
            or not manifest_path.endswith(".archive-manifest.json")
        ):
            issues.append(f"{location}.predecessorManifestPath is not a canonical archive path")
        issues.extend(
            _closed_snapshot_issues(
                transition.get("predecessorClosure"), f"{location}.predecessorClosure"
            )
        )
        expected_from = str(transition.get("toBaseCommit", ""))

    synchronization = contract.get("synchronizationHistory")
    if history and isinstance(history[-1], dict):
        final_target = history[-1].get("toBaseCommit")
        follows_resume = (
            isinstance(synchronization, list)
            and synchronization
            and isinstance(synchronization[0], dict)
            and synchronization[0].get("fromBaseCommit") == final_target
        )
        if final_target != contract_base and not follows_resume:
            issues.append("resumeHistory final toBaseCommit does not match Contract baseCommit")
    return issues


def validate_synchronization_history_structure(
    contract: dict[str, Any], receipt_base: str
) -> list[str]:
    """Validate local-only baseline transition evidence without repository access."""
    history = contract.get("synchronizationHistory")
    if history is None:
        return []
    if not isinstance(history, list) or not history:
        return ["synchronizationHistory must be a non-empty array when present"]
    resume = contract.get("resumeHistory")
    expected_from = receipt_base
    if isinstance(resume, list) and resume and isinstance(resume[-1], dict):
        expected_from = str(resume[-1].get("toBaseCommit", ""))
    issues: list[str] = []
    for index, transition in enumerate(history):
        location = f"synchronizationHistory[{index}]"
        if not isinstance(transition, dict):
            issues.append(f"{location} must be an evidence object")
            continue
        for field in SYNCHRONIZATION_REQUIRED_FIELDS:
            if field not in transition:
                issues.append(f"{location} missing field: {field}")
        if any(field not in transition for field in SYNCHRONIZATION_REQUIRED_FIELDS):
            continue
        if transition.get("synchronizationVersion") != SYNCHRONIZATION_SCHEMA_VERSION:
            issues.append(f"{location}.synchronizationVersion is unsupported")
        for field in (
            "fromBaseCommit",
            "toBaseCommit",
            "baseRemote",
            "baseBranch",
            "workBranch",
            "rebaseHeadBefore",
            "rebaseHeadAfter",
        ):
            if not isinstance(transition.get(field), str) or not transition[field].strip():
                issues.append(f"{location}.{field} must be a non-empty string")
        if transition.get("fromBaseCommit") != expected_from:
            issues.append(f"{location}.fromBaseCommit does not continue from the prior baseline")
        if transition.get("toBaseCommit") == transition.get("fromBaseCommit"):
            issues.append(f"{location}.toBaseCommit must advance the baseline")
        if transition.get("workBranch") == transition.get("baseBranch"):
            issues.append(f"{location}.workBranch must be a dedicated non-base branch")
        try:
            datetime.fromisoformat(str(transition.get("recordedAt")))
        except ValueError:
            issues.append(f"{location}.recordedAt is not ISO-8601")
        for field in ("priorContractDigest", "priorSummaryDigest"):
            if not _is_digest(transition.get(field)):
                issues.append(f"{location}.{field} must be a SHA-256 digest")
        checkpoint_fields = (
            "checkpointHeadBefore",
            "checkpointHeadAfter",
            "checkpointPaths",
        )
        if any(field in transition for field in checkpoint_fields):
            for field in ("checkpointHeadBefore", "checkpointHeadAfter"):
                if not isinstance(transition.get(field), str) or not transition[field].strip():
                    issues.append(f"{location}.{field} must be a non-empty string")
            checkpoint_paths = transition.get("checkpointPaths")
            if (
                not isinstance(checkpoint_paths, list)
                or not checkpoint_paths
                or any(not isinstance(path, str) or not path for path in checkpoint_paths)
            ):
                issues.append(f"{location}.checkpointPaths must be a non-empty list")
        expected_from = str(transition.get("toBaseCommit", ""))
    if (
        history
        and isinstance(history[-1], dict)
        and history[-1].get("toBaseCommit") != contract.get("baseCommit")
    ):
        issues.append(
            "synchronizationHistory final toBaseCommit does not match Contract baseCommit"
        )
    return issues


def _git_is_ancestor(project_root: Path, ancestor: str, descendant: str) -> bool:
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=project_root,
        check=False,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def _manifest_issues(transition: dict[str, Any], *, project_root: Path, location: str) -> list[str]:
    relative = str(transition["predecessorManifestPath"])
    manifest_path = project_root / relative
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return [f"{location}: predecessor archive manifest is missing: {relative}"]
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        return [f"{location}: predecessor archive manifest is invalid: {exc}"]
    issues: list[str] = []
    if manifest.get("format") != "ai-cockpit-archive-manifest":
        issues.append(f"{location}: predecessor archive manifest format is unsupported")
    if manifest.get("manifestVersion") != 1:
        issues.append(f"{location}: predecessor archive manifest version is unsupported")
    if manifest.get("workItemId") != transition.get("predecessorWorkItemId"):
        issues.append(f"{location}: predecessor archive manifest Work Item does not match")
    for kind in ("contract", "summary"):
        path_key = f"{kind}Path"
        digest_key = f"{kind}Sha256"
        bound_path = manifest.get(path_key)
        if not isinstance(bound_path, str) or not bound_path:
            issues.append(f"{location}: predecessor manifest {path_key} is missing")
            continue
        evidence_path = project_root / bound_path
        try:
            actual_digest = hashlib.sha256(evidence_path.read_bytes()).hexdigest()
        except OSError:
            issues.append(f"{location}: predecessor manifest {path_key} does not exist")
            continue
        if manifest.get(digest_key) != actual_digest:
            issues.append(f"{location}: predecessor manifest {digest_key} does not match")
    return issues


def _latest_predecessor_issues(contract: dict[str, Any], transition: dict[str, Any]) -> list[str]:
    predecessor = contract.get("predecessorWorkItem")
    if not isinstance(predecessor, dict):
        return ["predecessorWorkItem must be an evidence object for the latest resume"]
    issues: list[str] = []
    snapshot = predecessor_closure_snapshot(predecessor)
    issues.extend(_closed_snapshot_issues(snapshot, "predecessorWorkItem"))
    if predecessor.get("workItemId") != transition.get("predecessorWorkItemId"):
        issues.append("predecessor Work Item must equal the latest resume transition")
    pr = predecessor.get("pr")
    merge_commit = pr.get("mergeCommit") if isinstance(pr, dict) else None
    if merge_commit != transition.get("predecessorMergeCommit"):
        issues.append("predecessor merge commit must equal resume target")
    closure = predecessor.get("closure")
    evidence = closure.get("evidence") if isinstance(closure, dict) else None
    if evidence != transition.get("predecessorManifestPath"):
        issues.append("predecessor closure evidence must equal resume manifest path")
    if snapshot != transition.get("predecessorClosure"):
        issues.append("predecessor closure snapshot does not match Contract evidence")
    return issues


def validate_resume_history(
    contract: dict[str, Any],
    receipt: dict[str, Any],
    *,
    project_root: Path = PROJECT_ROOT,
    require_latest_predecessor: bool = True,
) -> list[str]:
    """Validate resume structure, Git ancestry, archive evidence, and latest closure."""
    receipt_base = str(receipt.get("baseCommit", ""))
    issues = validate_resume_history_structure(contract, receipt_base)
    history = contract.get("resumeHistory")
    if issues or not isinstance(history, list) or not history:
        return issues
    expected_work_branch = ""
    for index, transition in enumerate(history):
        if not isinstance(transition, dict):
            continue
        location = f"resumeHistory[{index}]"
        if not _git_is_ancestor(
            project_root,
            str(transition.get("fromBaseCommit", "")),
            str(transition.get("toBaseCommit", "")),
        ):
            issues.append(f"{location}: fromBaseCommit is not an ancestor of toBaseCommit")
        transition_work_branch = transition.get("workBranch")
        if isinstance(transition_work_branch, str):
            if expected_work_branch and transition_work_branch != expected_work_branch:
                issues.append(f"{location}: workBranch does not match the first resume transition")
            elif not expected_work_branch:
                expected_work_branch = transition_work_branch
        receipt_branch = receipt.get("baseBranch")
        transition_base_branch = transition.get("baseBranch")
        if isinstance(receipt_branch, str) and receipt_branch:
            if receipt_branch == transition_base_branch:
                if not work_branch_identifies_work_item(
                    str(transition_work_branch), str(contract.get("workItemId", ""))
                ):
                    issues.append(
                        f"{location}: compatibility workBranch does not identify this Work Item"
                    )
            elif transition_work_branch != receipt_branch:
                issues.append(f"{location}: workBranch does not match immutable Start Receipt")
        issues.extend(_manifest_issues(transition, project_root=project_root, location=location))
    if require_latest_predecessor and isinstance(history[-1], dict):
        issues.extend(_latest_predecessor_issues(contract, history[-1]))
    return issues


def validate_receipt(
    contract: dict[str, Any],
    receipt: dict[str, Any] | None,
    *,
    project_root: Path = PROJECT_ROOT,
    require_tracked: bool = False,
    require_latest_predecessor: bool = True,
) -> list[str]:
    """Return fail-closed issues for a receipt and its Contract binding."""
    issues: list[str] = []
    if receipt is None:
        return ["Start Receipt is missing"]
    required = (
        "receiptVersion",
        "workItemId",
        "receiptPath",
        "baseCommit",
        "startTimestamp",
        "initialScopeDigest",
        "contractSkeletonDigest",
    )
    for key in required:
        if key not in receipt:
            issues.append(f"Start Receipt missing field: {key}")
    if issues:
        return issues
    if receipt.get("receiptVersion") != RECEIPT_SCHEMA_VERSION:
        issues.append("Start Receipt receiptVersion is unsupported")
    work_item_id = contract.get("workItemId")
    if receipt.get("workItemId") != work_item_id:
        issues.append("Start Receipt workItemId does not match Contract")
    expected_path = f"{RECEIPT_PREFIX}{work_item_id}.json"
    if receipt.get("receiptPath") != expected_path:
        issues.append("Start Receipt receiptPath is not the canonical repository-relative path")
    if receipt.get("baseCommit") != contract.get("baseCommit"):
        has_resume = contract.get("resumeHistory") is not None
        has_synchronization = contract.get("synchronizationHistory") is not None
        if not has_resume and not has_synchronization:
            issues.append("Start Receipt baseCommit does not match Contract")
        if has_resume:
            resume_issues = validate_resume_history(
                contract,
                receipt,
                project_root=project_root,
                require_latest_predecessor=require_latest_predecessor,
            )
            if resume_issues:
                issues.append("Start Receipt baseCommit does not match Contract")
                issues.extend(resume_issues)
        if has_synchronization:
            synchronization_issues = validate_synchronization_history_structure(
                contract, str(receipt.get("baseCommit", ""))
            )
            if synchronization_issues:
                issues.append("Start Receipt baseCommit does not match Contract")
                issues.extend(synchronization_issues)
    elif contract.get("resumeHistory") is not None:
        issues.extend(
            validate_resume_history(
                contract,
                receipt,
                project_root=project_root,
                require_latest_predecessor=require_latest_predecessor,
            )
        )
    elif contract.get("synchronizationHistory") is not None:
        issues.extend(
            validate_synchronization_history_structure(contract, str(receipt.get("baseCommit", "")))
        )
    try:
        datetime.fromisoformat(str(receipt["startTimestamp"]))
    except ValueError:
        issues.append("Start Receipt startTimestamp is not ISO-8601")
    if (
        not isinstance(receipt.get("initialScopeDigest"), str)
        or len(receipt["initialScopeDigest"]) != 64
    ):
        issues.append("Start Receipt initialScopeDigest must be a SHA-256 digest")
    if (
        not isinstance(receipt.get("contractSkeletonDigest"), str)
        or len(receipt["contractSkeletonDigest"]) != 64
    ):
        issues.append("Start Receipt contractSkeletonDigest must be a SHA-256 digest")
    boundary = contract.get("concurrencyBoundary")
    if boundary is not None:
        digest = receipt.get("concurrencyBoundaryDigest")
        if not _is_digest(digest):
            issues.append("Start Receipt concurrencyBoundaryDigest must be a SHA-256 digest")
        elif digest != _digest(boundary):
            issues.append("Start Receipt concurrencyBoundaryDigest does not match Contract")
    corrective = contract.get("calibrationCorrective")
    if corrective is not None:
        digest = receipt.get("calibrationCorrectiveDigest")
        if not _is_digest(digest):
            issues.append("Start Receipt calibrationCorrectiveDigest must be a SHA-256 digest")
        elif digest != _digest(corrective):
            issues.append("Start Receipt calibrationCorrectiveDigest does not match Contract")
    binding = contract.get("startReceipt")
    if not isinstance(binding, dict):
        issues.append("Contract startReceipt binding is missing")
    elif binding != receipt_binding(receipt):
        issues.append("Contract startReceipt binding does not match Receipt")
    if require_tracked:
        result = subprocess.run(
            ["git", "ls-files", "--error-unmatch", str(receipt["receiptPath"])],
            cwd=project_root,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            issues.append("Start Receipt is not Git-tracked")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate a Work Item Start Receipt.")
    parser.add_argument("--contract", required=True)
    parser.add_argument("--receipt")
    args = parser.parse_args()
    contract_path = PROJECT_ROOT / args.contract
    try:
        contract = json.loads(contract_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"Start Receipt check failed: {exc}")
        return 1
    path = PROJECT_ROOT / args.receipt if args.receipt else receipt_path(contract["workItemId"])
    try:
        receipt = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError, ValueError):
        receipt = None
    issues = validate_receipt(contract, receipt, require_tracked=True)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}")
        return 1
    print(f"Start Receipt check passed: {path.relative_to(PROJECT_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

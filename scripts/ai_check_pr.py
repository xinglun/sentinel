#!/usr/bin/env python3
"""PR diff の archive Work Item 整合性を検証する。"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from ai_check_summary import validate_summary
from ai_check_work_item import validate_contract
from ai_observability import create_observability, elapsed_ms


PROJECT_ROOT = Path(__file__).resolve().parents[1]
REPORT_PATH = PROJECT_ROOT / "target" / "ai_pr_report.json"
ARCHIVE_PREFIX = ".ai/work-items/archive/"
ACTIVE_PREFIX = ".ai/work-items/active/"
ARCHIVE_SUFFIXES = (".contract.json", ".summary.json", ".review.json")
PAIR_SUFFIXES = (".contract.json", ".summary.json")
POLICY_PATH = PROJECT_ROOT / ".ai" / "guards" / "pr_evidence_policy.yaml"


def run_git(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=PROJECT_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data


def has_known_suffix(path: str) -> bool:
    return path.endswith(ARCHIVE_SUFFIXES)


def stem(path: str) -> str:
    for suffix in ARCHIVE_SUFFIXES:
        if path.endswith(suffix):
            return path[: -len(suffix)]
    raise ValueError(f"archive evidence path ではありません: {path}")


def changed_name_status(base: str) -> list[tuple[str, str]]:
    result = run_git(["diff", "--name-status", "--no-renames", f"{base}...HEAD"])
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    changes: list[tuple[str, str]] = []
    for line in result.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) >= 2:
            changes.append((parts[0], parts[-1]))
    return changes


def archive_changes(changes: list[tuple[str, str]]) -> list[tuple[str, str]]:
    return [
        (status, path)
        for status, path in changes
        if path.startswith(ARCHIVE_PREFIX) and has_known_suffix(path)
    ]


def git_blob(revision: str, path: str) -> bytes | None:
    result = run_git(["show", f"{revision}:{path}"])
    if result.returncode != 0:
        return None
    return result.stdout.encode()


def _git_blob_hash(revision: str, path: str) -> str:
    """指定 revision の path に対応する Git blob hash を返す。"""
    result = run_git(["rev-parse", f"{revision}:{path}"])
    return result.stdout.strip() if result.returncode == 0 else ""


def _worktree_blob_hash(path: str) -> str:
    """現在の worktree にある path の Git blob hash を返す。"""
    result = run_git(["hash-object", "--no-filters", path])
    return result.stdout.strip() if result.returncode == 0 else ""


def _is_no_op_restore(base: str, path: str) -> bool:
    """許可された baseline blob への復元だけを no-op と判定する。"""
    worktree_blob = _worktree_blob_hash(path)
    if not worktree_blob:
        return False
    return any(
        baseline_blob and baseline_blob == worktree_blob
        for revision in (base, f"{base}^")
        for baseline_blob in [_git_blob_hash(revision, path)]
    )


def archive_evidence_changes(base: str) -> dict[str, str]:
    """PR diff から archive evidence の変更だけを status map として返す。"""
    result: dict[str, str] = {}
    for status, path in changed_name_status(base):
        if not (path.startswith(ARCHIVE_PREFIX) and has_known_suffix(path)):
            continue
        if status == "M" and _is_no_op_restore(base, path):
            continue
        result[path] = status
    return result


def archived_contract_paths(base: str) -> list[Path]:
    """PR diff に含まれる archive Contract path を列挙する。"""
    return [
        PROJECT_ROOT / stem(path)
        for path in archive_evidence_changes(base)
        if path.endswith(".contract.json")
    ]


def archive_pair_rank(contract_path: Path, summary_path: Path) -> tuple[int, str, str]:
    """archive pair の安定した並び順を返す。"""
    try:
        contract_rel = contract_path.relative_to(PROJECT_ROOT).as_posix()
        summary_rel = summary_path.relative_to(PROJECT_ROOT).as_posix()
    except ValueError:
        return 0, contract_path.as_posix(), summary_path.as_posix()
    try:
        summary = load_json(summary_path)
    except (OSError, ValueError, json.JSONDecodeError):
        summary = {}
    sequence = summary.get("archiveSequence") if isinstance(summary, dict) else None
    if isinstance(sequence, int) and not isinstance(sequence, bool) and sequence > 0:
        return sequence, contract_rel, summary_rel
    return 0, contract_rel, summary_rel


def is_ancestor(ancestor: str, descendant: str) -> bool:
    return run_git(["merge-base", "--is-ancestor", ancestor, descendant]).returncode == 0


def declared_archive_repairs(changes: list[tuple[str, str]]) -> dict[str, dict[str, Any]]:
    repairs: dict[str, dict[str, Any]] = {}
    for status, path in archive_changes(changes):
        if status != "A" or not path.endswith(".contract.json"):
            continue
        try:
            contract = load_json(PROJECT_ROOT / path)
        except (OSError, json.JSONDecodeError, ValueError):
            continue
        repair = contract.get("archiveRepair")
        if isinstance(repair, dict) and isinstance(repair.get("targetPath"), str):
            repairs[repair["targetPath"]] = repair
    return repairs


def valid_declared_repair(path: str, repair: dict[str, Any], base: str) -> bool:
    required = ("targetPath", "restoreFromCommit", "baseContentSha256", "restoredContentSha256", "reason")
    if any(not isinstance(repair.get(key), str) or not repair[key].strip() for key in required):
        return False
    if repair["targetPath"] != path or not is_ancestor(repair["restoreFromCommit"], base):
        return False
    base_blob = git_blob(base, path)
    restored_blob = git_blob(repair["restoreFromCommit"], path)
    head_blob = git_blob("HEAD", path)
    return bool(base_blob and restored_blob and head_blob) and (
        hashlib.sha256(base_blob).hexdigest() == repair["baseContentSha256"]
        and hashlib.sha256(restored_blob).hexdigest() == repair["restoredContentSha256"]
        and head_blob == restored_blob
        and hashlib.sha256(head_blob).hexdigest() == repair["restoredContentSha256"]
    )


def validate_archive_bundle(changes: list[tuple[str, str]], base: str) -> list[str]:
    issues: list[str] = []
    archive = archive_changes(changes)

    repairs = declared_archive_repairs(changes)
    modified = [(status, path) for status, path in archive if status != "A"]
    for status, path in modified:
        if not (len(modified) == 1 and status == "M" and valid_declared_repair(path, repairs.get(path, {}), base)):
            issues.append(f"archive path は append-only でなければなりません: {status} {path}")

    pair_stems = sorted(
        {
            stem(path)
            for status, path in archive
            if status == "A" and path.endswith(PAIR_SUFFIXES)
        }
    )
    for pair_stem in pair_stems:
        contract_rel = f"{pair_stem}.contract.json"
        summary_rel = f"{pair_stem}.summary.json"
        added_paths = {path for status, path in archive if status == "A"}
        if contract_rel not in added_paths or summary_rel not in added_paths:
            issues.append(f"archive evidence は Contract と Summary を同じ PR で追加してください: {pair_stem}")
            continue
        contract_path = PROJECT_ROOT / contract_rel
        summary_path = PROJECT_ROOT / summary_rel
        if not contract_path.exists():
            issues.append(f"archive Contract が存在しません: {contract_rel}")
            continue
        if not summary_path.exists():
            issues.append(f"archive Summary が存在しません: {summary_rel}")
            continue
        try:
            contract = load_json(contract_path)
            summary = load_json(summary_path)
        except (OSError, json.JSONDecodeError, ValueError) as exc:
            issues.append(f"archive pair を読めません: {contract_rel}: {exc}")
            continue

        if summary.get("contractPath") != contract_path.relative_to(PROJECT_ROOT).as_posix():
            issues.append(f"{summary_rel}: contractPath が archive Contract Path と一致しません。")
        issues.extend(f"{contract_rel}: {issue}" for issue in validate_contract(contract))
        issues.extend(
            f"{summary_rel}: {issue}"
            for issue in validate_summary(summary, contract)
        )

    return issues


def policy_exempt_paths() -> list[str]:
    patterns: list[str] = []
    in_exempt = False
    if not POLICY_PATH.exists():
        return patterns
    for raw in POLICY_PATH.read_text(encoding="utf-8").splitlines():
        stripped = raw.strip()
        if stripped == "exemptPaths:":
            in_exempt = True
        elif stripped.endswith(":"):
            in_exempt = False
        elif in_exempt and stripped.startswith("- "):
            patterns.append(stripped[2:].strip().strip('"').strip("'"))
    return patterns


def is_exempt(path: str) -> bool:
    return any(fnmatch.fnmatch(path, pattern) for pattern in policy_exempt_paths())


def changed_file_paths(summary: dict[str, Any]) -> set[str]:
    values = summary.get("changedFiles", [])
    return {
        item.get("path", "") for item in values
        if isinstance(item, dict) and isinstance(item.get("path"), str)
    }


def contract_owns_path(contract: dict[str, Any], summary: dict[str, Any], path: str) -> bool:
    scope = contract.get("scope", [])
    out_of_scope = contract.get("outOfScope", [])
    in_scope = any(isinstance(pattern, str) and fnmatch.fnmatch(path, pattern) for pattern in scope)
    excluded = any(isinstance(pattern, str) and fnmatch.fnmatch(path, pattern) for pattern in out_of_scope)
    return in_scope and not excluded and path in changed_file_paths(summary)


def validate_evidence_ownership(changes: list[tuple[str, str]]) -> list[str]:
    issues: list[str] = []
    pairs: list[tuple[dict[str, Any], dict[str, Any]]] = []
    for _, path in archive_changes(changes):
        if not path.endswith(".contract.json"):
            continue
        summary_path = PROJECT_ROOT / f"{stem(path)}.summary.json"
        contract_path = PROJECT_ROOT / path
        if contract_path.exists() and summary_path.exists():
            contract, summary = load_json(contract_path), load_json(summary_path)
            if contract.get("contractVersion") != 2:
                issues.append(f"{path}: PR evidence requires contractVersion: 2")
            else:
                pairs.append((contract, summary))
    for _, path in changes:
        if path.startswith(ARCHIVE_PREFIX) or path.startswith(ACTIVE_PREFIX) or is_exempt(path):
            continue
        if not any(contract_owns_path(contract, summary, path) for contract, summary in pairs):
            issues.append(f"PR changed path lacks archive evidence ownership: {path}")
    return issues


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="PR diff の archive Work Item 整合性を検証します。")
    parser.add_argument("--base", default=os.environ.get("AI_BASE_COMMIT", ""))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.base.strip():
        print("❌ --base または AI_BASE_COMMIT が必要です", file=sys.stderr)
        return 2

    start = time.time()
    try:
        changes = changed_name_status(args.base.strip())
    except (OSError, RuntimeError) as exc:
        print(f"❌ PR diff を読めません: {exc}", file=sys.stderr)
        return 1

    issues = validate_archive_bundle(changes, args.base.strip())
    issues.extend(validate_evidence_ownership(changes))
    report = {
        "baseCommit": args.base.strip(),
        "status": "error" if issues else "passed",
        "changedPaths": [path for _, path in changes],
        "archivePaths": [path for _, path in archive_changes(changes)],
        "issues": issues,
    }
    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    REPORT_PATH.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    obs = create_observability(work_item_id="")
    duration = elapsed_ms(start)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        print(f"report: {REPORT_PATH.relative_to(PROJECT_ROOT)}")
        obs.check_failed(check_id="aiPr", duration_ms=duration, detail=f"{len(issues)} issue(s)")
        return 1

    print("✅ PR archive guard passed")
    print(f"report: {REPORT_PATH.relative_to(PROJECT_ROOT)}")
    obs.check_passed(check_id="aiPr", duration_ms=duration)
    return 0


if __name__ == "__main__":
    sys.exit(main())

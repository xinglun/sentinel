#!/usr/bin/env python3
"""Predict critical-file coverage regressions for the current PR diff."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess  # nosec B404: fixed list-form Git and current-interpreter test execution only
import sys
from collections.abc import Callable
from pathlib import Path

from ai_common import included
from check_critical_coverage import CRITICAL_MINIMUMS

PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_POLICY = PROJECT_ROOT / ".ai" / "guards" / "changed_critical_coverage_policy.json"
DEFAULT_REPORT = PROJECT_ROOT / "target" / "changed-critical-coverage.json"


def load_policy(path: Path) -> dict:
    try:
        policy = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot load changed-critical coverage policy: {exc}") from exc
    if not isinstance(policy, dict) or policy.get("version") != 1:
        raise ValueError("changed-critical coverage policy version must be 1")
    if not isinstance(policy.get("criticalFiles"), dict):
        raise TypeError("changed-critical coverage policy must declare criticalFiles")
    return policy


def select_changed_critical(
    changed_files: list[str],
    policy: dict,
    critical_minimums: dict[str, float],
) -> tuple[list[str], list[str]]:
    changed = set(changed_files)
    configured = policy.get("criticalFiles", {})
    selected: list[str] = []
    tests: list[str] = []
    for path, authoritative_floor in critical_minimums.items():
        if path not in changed:
            continue
        entry = configured.get(path) if isinstance(configured, dict) else None
        if not isinstance(entry, dict):
            raise TypeError(f"missing changed-critical test mapping: {path}")
        configured_floor = entry.get("minimum")
        if not isinstance(configured_floor, (int, float)) or float(configured_floor) != float(
            authoritative_floor
        ):
            raise ValueError(
                f"{path}: configured minimum {configured_floor!r} does not match "
                f"authoritative floor {authoritative_floor:g}"
            )
        declared_tests = entry.get("tests")
        if (
            not isinstance(declared_tests, list)
            or not declared_tests
            or not all(isinstance(item, str) and item for item in declared_tests)
        ):
            raise ValueError(f"missing changed-critical test mapping: {path}")
        selected.append(path)
        for test_path in declared_tests:
            if test_path not in tests:
                tests.append(test_path)
    return selected, tests


def focused_coverage_failures(
    report: dict,
    selected: list[str],
    critical_minimums: dict[str, float],
) -> list[str]:
    files = report.get("files", {})
    failures: list[str] = []
    for path in selected:
        data = files.get(path) if isinstance(files, dict) else None
        summary = data.get("summary", {}) if isinstance(data, dict) else {}
        covered = summary.get("percent_covered") if isinstance(summary, dict) else None
        minimum = critical_minimums[path]
        if not isinstance(covered, (int, float)):
            failures.append(f"{path}: missing from focused coverage report")
        elif covered < minimum:
            failures.append(f"{path}: {covered:.2f}% is below {minimum:g}%")
    return failures


def git_changed_files(base: str) -> list[str]:
    commands = [
        ["git", "diff", "--name-only", f"{base}...HEAD"],
        ["git", "diff", "--name-only"],
        ["git", "diff", "--cached", "--name-only"],
        ["git", "ls-files", "--others", "--exclude-standard"],
    ]
    changed: list[str] = []
    for command in commands:
        result = subprocess.run(  # nosec B603: fixed list-form Git executable; base is a Git revision argument
            command,
            cwd=PROJECT_ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            raise ValueError(
                f"cannot resolve changed files from {base}: "
                f"{(result.stderr or result.stdout).strip()}"
            )
        for line in result.stdout.splitlines():
            path = line.strip()
            if path and path not in changed:
                changed.append(path)
    return changed


def _git_text(command: list[str], *, project_root: Path) -> str:
    """Run a fixed Git metadata command or fail closed with its diagnostic."""
    result = subprocess.run(  # nosec B603: callers use fixed Git subcommands and revisions
        command,
        cwd=project_root,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode:
        raise ValueError(
            f"cannot build pre-archive candidate: {(result.stderr or result.stdout).strip()}"
        )
    return result.stdout


def _candidate_paths(base: str, *, project_root: Path) -> list[str]:
    """Return every path whose bytes may be included by a future archive commit."""
    commands = [
        ["git", "diff", "--name-only", f"{base}...HEAD"],
        ["git", "diff", "--name-only"],
        ["git", "diff", "--cached", "--name-only"],
        ["git", "ls-files", "--others", "--exclude-standard"],
    ]
    paths: list[str] = []
    for command in commands:
        for raw in _git_text(command, project_root=project_root).splitlines():
            path = raw.strip()
            if path and path not in paths:
                paths.append(path)
    return paths


def _baseline_paths(value: object) -> list[str]:
    """Normalize v2 baseline records to paths for ownership comparison."""
    if not isinstance(value, list):
        raise TypeError("coverage Contract baselineDirtyPaths must be a list")
    paths: list[str] = []
    for index, item in enumerate(value):
        if isinstance(item, str) and item:
            paths.append(item)
            continue
        if isinstance(item, dict) and isinstance(item.get("path"), str) and item["path"]:
            paths.append(item["path"])
            continue
        raise ValueError(f"coverage Contract baselineDirtyPaths[{index}] must declare a path")
    return paths


def candidate_snapshot(
    *, base: str, project_root: Path, contract_path: Path | None = None
) -> dict[str, object]:
    """Build a content-addressed pre-archive candidate without creating a commit.

    The snapshot records the bytes currently present in the worktree for every
    committed, staged, unstaged, or untracked path that would be captured by
    the canonical post-archive commit.  It deliberately does not use only
    `git status`: two edits to the same path must produce different bindings.
    """
    head = _git_text(["git", "rev-parse", "HEAD"], project_root=project_root).strip()
    if not head:
        raise ValueError("cannot build pre-archive candidate without HEAD")
    paths = _candidate_paths(base, project_root=project_root)
    contract: dict[str, object] | None = None
    if contract_path is not None:
        try:
            loaded = json.loads(contract_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            raise ValueError(f"cannot load coverage Contract: {exc}") from exc
        if not isinstance(loaded, dict):
            raise ValueError("coverage Contract must be an object")
        contract = loaded
        scope = contract.get("scope")
        baseline = contract.get("baselineDirtyPaths", [])
        if not isinstance(scope, list) or not all(isinstance(item, str) for item in scope):
            raise ValueError("coverage Contract scope must be a list of paths")
        scope_paths = list(scope)
        baseline_paths = _baseline_paths(baseline)
    # Summary, Outcome, status, and report are generated after a successful
    # gate records its binding. Including them would create a self-reference:
    # persisting the evidence would necessarily invalidate the evidence. They
    # remain scope-checked above and are separately digested by Outcome and
    # archive-manifest contracts.
    derived_paths: set[str] = {
        ".ai/cockpit/current_status.md",
        ".ai/cockpit/task_report.json",
        ".ai/cockpit/task_report.md",
    }
    if contract is not None and contract_path is not None:
        work_item = contract.get("workItemId")
        if isinstance(work_item, str) and work_item:
            active = f".ai/work-items/active/{work_item}"
            derived_paths.update(
                {
                    f"{active}.summary.json",
                    f"{active}.outcome.json",
                    f"{active}.outcome.md",
                }
            )
            lifecycle_surface_paths = {
                *derived_paths,
                f"{active}.contract.json",
                f"{active}.successor-receipt.json",
                f".ai/work-items/starts/{work_item}.json",
            }
        else:
            lifecycle_surface_paths = set(derived_paths)
        foreign = [
            path
            for path in paths
            if path not in baseline_paths
            and path not in lifecycle_surface_paths
            and not included(path, scope_paths)
        ]
        if foreign:
            raise ValueError(
                "pre-archive candidate contains Contract-unowned path(s): " + ", ".join(foreign)
            )
    candidate_paths = [path for path in paths if path not in derived_paths]

    files: list[dict[str, str]] = []
    for path in sorted(candidate_paths):
        candidate = project_root / path
        if candidate.is_symlink():
            raise ValueError(f"pre-archive candidate path must not be a symbolic link: {path}")
        if candidate.exists():
            if not candidate.is_file():
                raise ValueError(f"pre-archive candidate path must be a regular file: {path}")
            digest = hashlib.sha256(candidate.read_bytes()).hexdigest()
            state = "present"
        else:
            digest = hashlib.sha256(b"deleted\0" + path.encode("utf-8")).hexdigest()
            state = "deleted"
        files.append({"path": path, "state": state, "sha256": digest})

    diff_payload = {
        "baseCommit": base,
        "candidateHead": head,
        "candidateFiles": files,
    }
    candidate_tree_payload: dict[str, object] = {
        "baseCommit": base,
        "candidateHead": head,
        "files": files,
    }
    if contract is not None and contract_path is not None:
        candidate_tree_payload["contractSha256"] = hashlib.sha256(
            contract_path.read_bytes()
        ).hexdigest()
    canonical_tree = json.dumps(
        candidate_tree_payload, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    canonical_diff = json.dumps(diff_payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return {
        "baseCommit": base,
        "candidateHead": head,
        "candidateFiles": files,
        "excludedDerivedPaths": sorted(path for path in paths if path in derived_paths),
        "candidateTreeDigest": hashlib.sha256(canonical_tree).hexdigest(),
        "candidateDiffDigest": hashlib.sha256(canonical_diff).hexdigest(),
        # Retained for consumers of the former report schema; it is now content-addressed.
        "candidateStateDigest": hashlib.sha256(canonical_tree).hexdigest(),
    }


def candidate_binding(*, base: str, project_root: Path) -> dict[str, str]:
    """Bind a report to the PR base and the exact candidate observed by pytest."""
    head = subprocess.run(  # nosec B603 B607 - fixed list-form Git metadata lookup
        ["git", "rev-parse", "HEAD"],
        cwd=project_root,
        text=True,
        capture_output=True,
        check=False,
    )
    status = subprocess.run(  # nosec B603 B607 - fixed list-form Git metadata lookup
        ["git", "status", "--porcelain", "--untracked-files=all"],
        cwd=project_root,
        text=True,
        capture_output=True,
        check=False,
    )
    if head.returncode or status.returncode or not head.stdout.strip():
        raise ValueError("cannot bind changed-critical coverage to the candidate Git state")
    return {
        "baseCommit": base,
        "candidateHead": head.stdout.strip(),
        "candidateStateDigest": hashlib.sha256(status.stdout.encode("utf-8")).hexdigest(),
    }


def adoption_bootstrap_paths(changed_files: list[str], contract_path: Path | None) -> list[str]:
    """Return explicit adoption bootstrap paths exempt from template-only coverage tests."""
    if contract_path is None:
        return []
    try:
        contract = json.loads(contract_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot load coverage Contract: {exc}") from exc
    if not isinstance(contract, dict) or contract.get("workItemId") != "adopt_ai_cockpit":
        return []
    patterns = contract.get("adoptionBootstrapPaths")
    if (
        not isinstance(patterns, list)
        or not patterns
        or not all(isinstance(pattern, str) and pattern for pattern in patterns)
    ):
        raise ValueError("adoption coverage exemption requires declared bootstrap paths")
    return [path for path in changed_files if included(path, patterns)]


def run_predictor(
    *,
    base: str,
    policy_path: Path,
    report_path: Path,
    project_root: Path,
    run_command: Callable[[list[str]], int],
    critical_minimums: dict[str, float],
    contract_path: Path | None = None,
) -> int:
    policy = load_policy(policy_path)
    snapshot = candidate_snapshot(base=base, project_root=project_root, contract_path=contract_path)
    candidate_files = snapshot.get("candidateFiles", [])
    if not isinstance(candidate_files, list):
        raise TypeError("pre-archive candidate snapshot is missing candidateFiles")
    changed_files = [
        item["path"]
        for item in candidate_files
        if isinstance(item, dict) and isinstance(item.get("path"), str)
    ]
    bootstrap_paths = adoption_bootstrap_paths(changed_files, contract_path)
    selected, tests = select_changed_critical(
        [path for path in changed_files if path not in bootstrap_paths],
        policy,
        critical_minimums,
    )
    if not selected:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        if bootstrap_paths:
            report_path.write_text(
                json.dumps(
                    {
                        "applicability": {
                            "status": "not_applicable",
                            "reason": "adoption_bootstrap_runtime",
                            "contract": contract_path.as_posix() if contract_path else None,
                            "excludedPaths": bootstrap_paths,
                        },
                        "binding": snapshot,
                    },
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
                encoding="utf-8",
            )
            print("changed-critical coverage: not applicable; adoption bootstrap runtime")
            return 0
        report_path.write_text(
            json.dumps(
                {
                    "applicability": {
                        "status": "not_applicable",
                        "reason": "no_critical_script_changed",
                        "contract": contract_path.as_posix() if contract_path else None,
                        "excludedPaths": [],
                    },
                    "binding": snapshot,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
        print("changed-critical coverage: not applicable; no critical script changed")
        return 0
    missing_tests = [path for path in tests if not (project_root / path).is_file()]
    if missing_tests:
        raise ValueError(
            "changed-critical test mapping references missing file(s): " + ", ".join(missing_tests)
        )
    report_path.parent.mkdir(parents=True, exist_ok=True)
    command = [
        sys.executable,
        "-m",
        "pytest",
        "-q",
        "--cov=scripts",
        f"--cov-report=json:{report_path}",
        "--cov-report=term-missing:skip-covered",
        *tests,
    ]
    result = run_command(command)
    if result != 0:
        return result
    try:
        report = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot load focused coverage report: {exc}") from exc
    failures = focused_coverage_failures(report, selected, critical_minimums)
    if failures:
        for failure in failures:
            print(f"[ERROR] {failure}", file=sys.stderr)
        return 1
    report["binding"] = snapshot
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    rendered = ", ".join(f"{path}>={critical_minimums[path]:g}%" for path in selected)
    print(f"changed-critical coverage passed: {rendered}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--contract", type=Path)
    args = parser.parse_args()

    def run(command: list[str]) -> int:
        return subprocess.run(  # nosec B603: fixed current interpreter and repository-controlled pytest arguments
            command, cwd=PROJECT_ROOT, check=False
        ).returncode

    try:
        return run_predictor(
            base=args.base,
            policy_path=args.policy,
            report_path=args.report,
            project_root=PROJECT_ROOT,
            run_command=run,
            critical_minimums=CRITICAL_MINIMUMS,
            contract_path=args.contract,
        )
    except (TypeError, ValueError) as exc:
        print(f"changed-critical coverage failed: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

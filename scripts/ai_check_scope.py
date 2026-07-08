#!/usr/bin/env python3
"""Work Item Contract の scope / outOfScope と実 diff の対応を検証する。"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from ai_json import load_json
from ai_observability import create_observability, elapsed_ms


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEPENDENCY_SCOPE_RULES: dict[str, list[str]] = {
    "src/core/presentation.rs": [
        "src/core/presentation_assembler.rs",
        "src/core/report.rs",
        "src/core/presentation_tests.rs",
        "src/core/report_ui_tests.rs",
    ],
    "src/core/presentation_assembler.rs": [
        "src/core/presentation_tests.rs",
        "src/core/report_ui_tests.rs",
    ],
    "src/core/report.rs": ["src/core/report_ui_tests.rs"],
    "src/core/i18n.rs": [
        "src/core/presentation_tests.rs",
        "src/core/report_ui_tests.rs",
    ],
    "src/core/trend_cohesion.rs": [
        "src/core/engine.rs",
        "src/core/transition_log.rs",
        "src/core/presentation_tests.rs",
        "src/core/report_ui_tests.rs",
    ],
}


def run_git(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=PROJECT_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def changed_paths() -> list[str]:
    result = run_git(["diff", "--name-only", "HEAD"])
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    paths = [line.strip() for line in result.stdout.splitlines() if line.strip()]

    untracked = run_git(["ls-files", "--others", "--exclude-standard"])
    if untracked.returncode != 0:
        raise RuntimeError(untracked.stderr.strip())
    paths.extend(line.strip() for line in untracked.stdout.splitlines() if line.strip())
    return sorted(set(paths))


def matches(pattern: str, path: str) -> bool:
    normalized = pattern.rstrip("/")
    if normalized.endswith("/**"):
        prefix = normalized[:-3]
        return path == prefix or path.startswith(f"{prefix}/")
    if any(ch in normalized for ch in "*?["):
        return fnmatch.fnmatch(path, normalized)
    return path == normalized


def included(path: str, patterns: list[str]) -> bool:
    return any(matches(pattern, path) for pattern in patterns)


def dependency_scope_issues(paths: list[str], scope: list[str]) -> list[str]:
    triggers = sorted(set(paths) | set(scope))
    issues: list[str] = []
    for trigger, required_paths in DEPENDENCY_SCOPE_RULES.items():
        if not included(trigger, triggers):
            continue
        missing = [path for path in required_paths if not included(path, scope)]
        if missing:
            issues.append(
                f"dependency scope が不足しています: {trigger} requires scope entries: {', '.join(missing)}"
            )
    return issues


def string_list(data: dict[str, Any], key: str) -> list[str]:
    value = data.get(key, [])
    return [item for item in value if isinstance(item, str)]


def file_fingerprint(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def baseline_dirty_paths(contract: dict[str, Any]) -> dict[str, dict[str, Any]]:
    value = contract.get("baselineDirtyPaths", [])
    baseline: dict[str, dict[str, Any]] = {}
    if not isinstance(value, list):
        return baseline
    for item in value:
        if isinstance(item, str):
            baseline[item] = {"path": item}
        elif isinstance(item, dict):
            path = item.get("path")
            if isinstance(path, str):
                baseline[path] = item
    return baseline


def baseline_matches(path: str, record: dict[str, Any]) -> bool:
    fingerprint = record.get("fingerprint")
    if isinstance(fingerprint, str) and fingerprint:
        file_path = PROJECT_ROOT / path
        if not file_path.exists() or not file_path.is_file():
            return fingerprint == "deleted"
        return file_fingerprint(file_path) == fingerprint
    return True


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Work Item scope と実 diff を検証します。")
    parser.add_argument("contract", nargs="?")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.contract:
        print("ℹ️ Skipping scope check (no active contract provided)")
        return 0
    start = time.time()
    try:
        contract = load_json(Path(args.contract))
        baseline_records = baseline_dirty_paths(contract)
        paths = changed_paths()
    except (OSError, json.JSONDecodeError, ValueError, RuntimeError) as exc:
        print(f"❌ scope guard を実行できません: {exc}", file=sys.stderr)
        return 1

    work_item_id = contract.get("workItemId", "")
    obs = create_observability(work_item_id=work_item_id)

    scope = string_list(contract, "scope")
    out_of_scope = string_list(contract, "outOfScope")
    allow_patterns = []
    destructive = contract.get("destructiveChangePolicy")
    if isinstance(destructive, dict):
        allow_patterns = [item for item in destructive.get("allowPatterns", []) if isinstance(item, str)]

    issues: list[str] = []
    for path in paths:
        if included(path, allow_patterns):
            continue
        if included(path, out_of_scope):
            issues.append(f"outOfScope に該当します: {path}")
        if baseline_matches(path, baseline_records.get(path, {})):
            continue
        if not included(path, scope):
            issues.append(f"scope に含まれていません: {path}")

    issues.extend(dependency_scope_issues(paths, scope))

    duration = elapsed_ms(start)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        print(f"❌ scope guard failed: {len(issues)} issue(s)", file=sys.stderr)
        obs.check_failed(check_id="aiScope", duration_ms=duration, detail=f"{len(issues)} issue(s)")
        return 1
    print(f"✅ scope guard passed: {len(paths)} changed path(s) covered")
    obs.check_passed(
        check_id="aiScope",
        duration_ms=duration,
        fields={"changedPaths": len(paths), "dependencyIssues": 0},
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

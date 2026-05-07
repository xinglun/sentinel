#!/usr/bin/env python3
"""Work Item Contract の scope / outOfScope と実 diff の対応を検証する。"""

from __future__ import annotations

import argparse
import fnmatch
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from ai_observability import create_observability, elapsed_ms


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data


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


def string_list(data: dict[str, Any], key: str) -> list[str]:
    value = data.get(key, [])
    return [item for item in value if isinstance(item, str)]


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
        if not included(path, scope):
            issues.append(f"scope に含まれていません: {path}")

    duration = elapsed_ms(start)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        print(f"❌ scope guard failed: {len(issues)} issue(s)", file=sys.stderr)
        obs.check_failed(check_id="aiScope", duration_ms=duration, detail=f"{len(issues)} issue(s)")
        return 1
    print(f"✅ scope guard passed: {len(paths)} changed path(s) covered")
    obs.check_passed(check_id="aiScope", duration_ms=duration, fields={"changedPaths": len(paths)})
    return 0


if __name__ == "__main__":
    sys.exit(main())

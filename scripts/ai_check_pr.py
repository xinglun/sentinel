#!/usr/bin/env python3
"""PR diff の archive Work Item 整合性を検証する。"""

from __future__ import annotations

import argparse
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
    result = run_git(["diff", "--name-status", f"{base}...HEAD"])
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


def validate_archive_bundle(changes: list[tuple[str, str]]) -> list[str]:
    issues: list[str] = []
    archive = archive_changes(changes)

    for status, path in archive:
        if status != "A":
            issues.append(f"archive path は append-only でなければなりません: {status} {path}")

    pair_stems = sorted(
        {
            stem(path)
            for _, path in archive
            if path.endswith(PAIR_SUFFIXES)
        }
    )
    for pair_stem in pair_stems:
        contract_rel = f"{pair_stem}.contract.json"
        summary_rel = f"{pair_stem}.summary.json"
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

    issues = validate_archive_bundle(changes)
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

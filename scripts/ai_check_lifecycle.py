#!/usr/bin/env python3
"""Work Item ディレクトリ全体のライフサイクル整合性を検証する。"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path
from typing import Any

from ai_observability import AiEventLevel, AiEventType, create_observability, elapsed_ms


PROJECT_ROOT = Path(__file__).resolve().parents[1]
WORK_ITEMS_DIR = PROJECT_ROOT / ".ai" / "work-items"
ACTIVE_DIR = WORK_ITEMS_DIR / "active"
ARCHIVE_DIR = WORK_ITEMS_DIR / "archive"
CURRENT_STATUS = PROJECT_ROOT / ".ai" / "cockpit" / "current_status.md"


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def check_directory(directory: Path) -> list[str]:
    issues: list[str] = []
    if not directory.exists():
        return issues

    contracts: set[str] = set()
    summaries: set[Path] = set()
    reviews: set[Path] = set()

    for path in directory.rglob("*.json"):
        if path.is_file():
            name = path.name
            if name.endswith(".contract.json"):
                contracts.add(name.replace(".contract.json", ""))
            elif name.endswith(".summary.json"):
                summaries.add(path)
            elif name.endswith(".review.json"):
                reviews.add(path)

    for summary_path in summaries:
        work_item_id = summary_path.name.replace(".summary.json", "")
        if work_item_id not in contracts:
            issues.append(f"Orphaned Summary (no Contract found in same dir): {summary_path.relative_to(PROJECT_ROOT)}")
            continue

        try:
            summary = load_json(summary_path)
            contract_path_rel = summary.get("contractPath")
            if not contract_path_rel:
                issues.append(f"Summary missing 'contractPath': {summary_path.relative_to(PROJECT_ROOT)}")
                continue

            expected_contract_path = PROJECT_ROOT / contract_path_rel
            actual_contract_path = summary_path.with_name(f"{work_item_id}.contract.json")
            if expected_contract_path != actual_contract_path:
                issues.append(
                    f"Summary 'contractPath' points to wrong location: {contract_path_rel} "
                    f"(should be {actual_contract_path.relative_to(PROJECT_ROOT)})"
                )
        except Exception as exc:
            issues.append(f"Failed to read summary {summary_path.relative_to(PROJECT_ROOT)}: {exc}")

    for review_path in reviews:
        work_item_id = review_path.name.replace(".review.json", "")
        if work_item_id not in contracts:
            issues.append(f"Orphaned Review (no Contract found in same dir): {review_path.relative_to(PROJECT_ROOT)}")

    return issues


def check_current_status_consistency() -> list[str]:
    issues: list[str] = []
    if not CURRENT_STATUS.exists():
        return issues

    try:
        status_text = CURRENT_STATUS.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"Failed to read current_status.md: {exc}"]

    if "- State: `no_active_work_item`" not in status_text:
        return issues

    active_files = sorted(
        path
        for path in ACTIVE_DIR.glob("*.json")
        if path.name.endswith((".contract.json", ".summary.json", ".review.json"))
    )
    if active_files:
        listed = ", ".join(path.relative_to(PROJECT_ROOT).as_posix() for path in active_files)
        issues.append(f"current_status is no_active_work_item but active Work Item files remain: {listed}")
    return issues


def check_active_archive_duplicates() -> list[str]:
    """同一 Work Item が active と archive に同時存在しないことを確認する。"""
    issues: list[str] = []
    if not ACTIVE_DIR.exists() or not ARCHIVE_DIR.exists():
        return issues

    archive_names = {
        path.name
        for path in ARCHIVE_DIR.rglob("*.json")
        if path.name.endswith((".contract.json", ".summary.json", ".review.json"))
    }
    duplicates = sorted(
        path
        for path in ACTIVE_DIR.glob("*.json")
        if path.name.endswith((".contract.json", ".summary.json", ".review.json"))
        and path.name in archive_names
    )
    if duplicates:
        listed = ", ".join(path.relative_to(PROJECT_ROOT).as_posix() for path in duplicates)
        issues.append(f"Active Work Item files duplicate archived files: {listed}")
    return issues


def main() -> int:
    start = time.time()
    obs = create_observability()

    issues: list[str] = []
    
    # Ignore _templates dir
    for subdir in WORK_ITEMS_DIR.iterdir():
        if subdir.is_dir() and subdir.name != "_templates":
            issues.extend(check_directory(subdir))
    issues.extend(check_current_status_consistency())
    issues.extend(check_active_archive_duplicates())

    duration = elapsed_ms(start)

    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        print(f"❌ lifecycle check failed: {len(issues)} issue(s)", file=sys.stderr)
        obs.check_failed(check_id="aiLifecycle", duration_ms=duration, detail=f"{len(issues)} issue(s)")
        return 1

    print("✅ lifecycle check passed: no orphaned files or path mismatches")
    obs.check_passed(check_id="aiLifecycle", duration_ms=duration)
    return 0


if __name__ == "__main__":
    sys.exit(main())

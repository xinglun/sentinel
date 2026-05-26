#!/usr/bin/env python3
"""Work Item を active/ から archive/YYYY/ へ移動する。"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

from ai_observability import AiEventLevel, AiEventType, create_observability


PROJECT_ROOT = Path(__file__).resolve().parents[1]
ACTIVE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "active"
ARCHIVE_BASE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "archive"


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def save_json(path: Path, data: dict[str, Any]) -> None:
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def remove_active_residue(files_to_move: list[tuple[Path, Path]]) -> bool:
    """archive 済みの source file が active 側へ残った場合は除去する。"""
    ok = True
    for src, target in files_to_move:
        if not src.exists():
            continue
        if not target.exists():
            print(
                f"❌ Active residue cannot be removed because archive target is missing: {src.relative_to(PROJECT_ROOT)}",
                file=sys.stderr,
            )
            ok = False
            continue
        try:
            src.unlink()
            print(f"✅ Removed active residue: {src.relative_to(PROJECT_ROOT)}")
        except OSError as exc:
            print(f"❌ Failed to remove active residue: {src.relative_to(PROJECT_ROOT)}: {exc}", file=sys.stderr)
            ok = False
    return ok


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Archive a Work Item.")
    parser.add_argument("contract", help="Path to the active contract JSON.")
    parser.add_argument("--dry-run", action="store_true", help="Print actions without modifying files.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    contract_path = Path(args.contract).resolve()

    if not contract_path.is_relative_to(ACTIVE_DIR):
        print(f"❌ Contract must be in {ACTIVE_DIR.relative_to(PROJECT_ROOT)}", file=sys.stderr)
        return 1

    if not contract_path.exists():
        print(f"❌ Contract not found: {contract_path.relative_to(PROJECT_ROOT)}", file=sys.stderr)
        return 1

    try:
        contract = load_json(contract_path)
    except Exception as exc:
        print(f"❌ Failed to read contract: {exc}", file=sys.stderr)
        return 1

    work_item_id = contract.get("workItemId")
    if not work_item_id:
        print("❌ Contract missing 'workItemId'", file=sys.stderr)
        return 1

    file_basename = contract_path.name.replace(".contract.json", "")
    mode = contract.get("mode")
    summary_path = ACTIVE_DIR / f"{file_basename}.summary.json"
    review_path = ACTIVE_DIR / f"{file_basename}.review.json"

    if mode == "code" and not summary_path.exists():
        print(f"❌ mode: code requires Summary, but not found: {summary_path.relative_to(PROJECT_ROOT)}", file=sys.stderr)
        return 1

    year = str(datetime.now().year)
    target_dir = ARCHIVE_BASE_DIR / year

    files_to_move: list[tuple[Path, Path]] = [(contract_path, target_dir / contract_path.name)]
    if summary_path.exists():
        files_to_move.append((summary_path, target_dir / summary_path.name))
    if review_path.exists():
        files_to_move.append((review_path, target_dir / review_path.name))

    for _, target in files_to_move:
        if target.exists():
            print(f"❌ Target already exists: {target.relative_to(PROJECT_ROOT)}", file=sys.stderr)
            return 1

    if args.dry_run:
        print("Dry run: The following files would be archived:")
        for src, target in files_to_move:
            print(f"  {src.relative_to(PROJECT_ROOT)} -> {target.relative_to(PROJECT_ROOT)}")
        if summary_path.exists():
            new_contract_rel = (target_dir / contract_path.name).relative_to(PROJECT_ROOT).as_posix()
            print(f"  Summary 'contractPath' would be updated to: {new_contract_rel}")
        return 0

    target_dir.mkdir(parents=True, exist_ok=True)

    # If summary exists, read it and update contractPath before moving
    if summary_path.exists():
        try:
            summary = load_json(summary_path)
            new_contract_rel = (target_dir / contract_path.name).relative_to(PROJECT_ROOT).as_posix()
            summary["contractPath"] = new_contract_rel
            save_json(summary_path, summary)
        except Exception as exc:
            print(f"❌ Failed to update summary: {exc}", file=sys.stderr)
            return 1

    for src, target in files_to_move:
        src.rename(target)
        print(f"✅ Moved: {target.relative_to(PROJECT_ROOT)}")

    if not remove_active_residue(files_to_move):
        return 1

    obs = create_observability(work_item_id=work_item_id)
    from ai_observability import AiEvent

    obs.record(AiEvent(
        event_type=AiEventType.CHECK_PASSED,
        level=AiEventLevel.INFO,
        message=f"Work Item archived to {year}",
        check_id="aiArchive",
        fields={"year": year, "files": len(files_to_move)}
    ))
    status_result = subprocess.run(
        [sys.executable, "scripts/ai_generate_status.py", "--no-active"],
        cwd=PROJECT_ROOT,
        check=False,
    )
    if status_result.returncode != 0:
        print("❌ Failed to generate no-active cockpit status", file=sys.stderr)
        return status_result.returncode
    consistency_result = subprocess.run(
        [sys.executable, "scripts/ai_check_status_consistency.py"],
        cwd=PROJECT_ROOT,
        check=False,
    )
    if consistency_result.returncode != 0:
        print("❌ Cockpit status consistency check failed after archive", file=sys.stderr)
        return consistency_result.returncode
    return 0


if __name__ == "__main__":
    sys.exit(main())

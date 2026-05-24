#!/usr/bin/env python3
"""新しい Work Item Contract / Summary の骨格を作成する。"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

from ai_observability import AiRunContext, create_observability


PROJECT_ROOT = Path(__file__).resolve().parents[1]
ACTIVE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "active"
ARCHIVE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "archive"
CURRENT_STATUS = PROJECT_ROOT / ".ai" / "cockpit" / "current_status.md"


def slug(value: str) -> str:
    normalized = re.sub(r"[^a-zA-Z0-9_-]+", "_", value.strip().lower()).strip("_")
    if not normalized:
        raise ValueError("TASK は空にできません。")
    return normalized


def write_json(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def current_status_is_no_active() -> bool:
    if not CURRENT_STATUS.exists():
        return False
    return "- State: `no_active_work_item`" in CURRENT_STATUS.read_text(encoding="utf-8")


def archived_counterparts(active_path: Path) -> list[Path]:
    if not ARCHIVE_DIR.exists():
        return []
    return sorted(path for path in ARCHIVE_DIR.rglob(active_path.name) if path.is_file())


def equivalent_archived_residue(active_path: Path, archive_path: Path) -> bool:
    if active_path.name.endswith(".summary.json"):
        try:
            active_json = json.loads(active_path.read_text(encoding="utf-8"))
            archive_json = json.loads(archive_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return False
        active_json.pop("contractPath", None)
        archive_json.pop("contractPath", None)
        return active_json == archive_json
    return active_path.read_bytes() == archive_path.read_bytes()


def cleanup_archived_active_residue() -> int:
    if not current_status_is_no_active() or not ACTIVE_DIR.exists():
        return 0

    removed = 0
    for active_path in sorted(ACTIVE_DIR.glob("*.json")):
        if not active_path.name.endswith((".contract.json", ".summary.json", ".review.json")):
            continue
        counterparts = archived_counterparts(active_path)
        if len(counterparts) != 1:
            continue
        if not equivalent_archived_residue(active_path, counterparts[0]):
            continue
        active_path.unlink()
        removed += 1

    if removed:
        print(f"✅ ai-start preflight cleaned archived active residue: {removed} file(s)")
    return removed


def run_preflight_checks() -> int:
    cleanup_archived_active_residue()
    checks = [
        ("lifecycle", PROJECT_ROOT / "scripts" / "ai_check_lifecycle.py"),
        ("status consistency", PROJECT_ROOT / "scripts" / "ai_check_status_consistency.py"),
    ]
    for label, script in checks:
        result = subprocess.run([sys.executable, str(script)], cwd=PROJECT_ROOT, check=False)
        if result.returncode != 0:
            print(f"❌ ai-start preflight failed: {label}", file=sys.stderr)
            return result.returncode
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="AI Work Item の skeleton を作成します。")
    parser.add_argument("--task", required=True, help="task id。例: risk_taxonomy_refine")
    parser.add_argument("--title", help="Work Item title。未指定時は task id を使う。")
    parser.add_argument("--mode", default="investigate", choices=["investigate", "author_todo", "code", "review", "cleanup"])
    parser.add_argument("--force", action="store_true", help="既存 skeleton を上書きする。")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        task = slug(args.task)
    except ValueError as exc:
        print(f"❌ {exc}", file=sys.stderr)
        return 2

    preflight_code = run_preflight_checks()
    if preflight_code != 0:
        return preflight_code

    contract_path = ACTIVE_DIR / f"{task}.contract.json"
    summary_path = ACTIVE_DIR / f"{task}.summary.json"
    if not args.force and (contract_path.exists() or summary_path.exists()):
        print(f"❌ Work Item は既に存在します: {task}", file=sys.stderr)
        return 1

    title = args.title or task.replace("_", " ")
    contract_rel = contract_path.relative_to(PROJECT_ROOT).as_posix()
    summary_rel = summary_path.relative_to(PROJECT_ROOT).as_posix()
    contract = {
        "contractVersion": 1,
        "workItemId": task,
        "mode": args.mode,
        "title": title,
        "scope": [contract_rel, summary_rel],
        "outOfScope": [],
        "sources": [{"path": contract_rel, "reason": "Work Item の初期 skeleton。"}],
        "unknowns": ["scope / sources / acceptance を task に合わせて確定する。"],
        "notCodable": args.mode == "code",
        "acceptance": ["Work Item Contract が task に合わせて更新されている。"],
        "verification": [
            {"command": f"make check-ai-contract CONTRACT={contract_rel}", "required": True},
            {"command": f"make check-ai-scope CONTRACT={contract_rel}", "required": True},
            {"command": "make fmt-check", "required": True},
            {"command": "make check-ai-backtrack", "required": True},
            {"command": f"make check-ai-change-summary SUMMARY={summary_rel} CONTRACT={contract_rel}", "required": True},
            {"command": f"make generate-cockpit-status CONTRACT={contract_rel} SUMMARY={summary_rel}", "required": True},
            {"command": f"make check-ai-status CONTRACT={contract_rel} SUMMARY={summary_rel}", "required": True},
        ],
        "destructiveChangePolicy": {"allowed": False, "requiresHumanApproval": True, "allowPatterns": []},
        "rollbackNote": "この Work Item の diff を revert する。",
    }
    summary = {
        "workItemId": task,
        "contractPath": contract_rel,
        "changedFiles": [
            {"path": contract_rel, "reason": "Work Item Contract skeleton を作成した。"},
            {"path": summary_rel, "reason": "AI Change Summary skeleton を作成した。"},
        ],
        "sourcesUsed": [contract_rel],
        "verification": [{"command": item["command"], "result": "not_run"} for item in contract["verification"]],
        "unknownsRemaining": ["scope / sources / acceptance を task に合わせて確定する。"],
        "risk": {"level": "medium", "detail": "初期 skeleton のため、実装前に Contract を確定する必要がある。"},
        "generatedFiles": [],
        "destructiveChanges": [],
        "observedIssues": [],
    }
    write_json(contract_path, contract)
    write_json(summary_path, summary)
    print(f"✅ Work Item skeleton created: {task}")
    print(f"contract: {contract_rel}")
    print(f"summary: {summary_rel}")

    # -- observability --
    obs = create_observability(work_item_id=task)
    obs.work_item_started(fields={"mode": args.mode, "title": title})

    return 0


if __name__ == "__main__":
    sys.exit(main())

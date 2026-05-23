#!/usr/bin/env python3
"""新しい Work Item Contract / Summary の骨格を作成する。"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

from ai_observability import AiRunContext, create_observability


PROJECT_ROOT = Path(__file__).resolve().parents[1]
ACTIVE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "active"


def slug(value: str) -> str:
    normalized = re.sub(r"[^a-zA-Z0-9_-]+", "_", value.strip().lower()).strip("_")
    if not normalized:
        raise ValueError("TASK は空にできません。")
    return normalized


def write_json(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


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

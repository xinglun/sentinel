#!/usr/bin/env python3
"""Contract と Summary から Cockpit current_status.md を生成する。"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from ai_observability import create_observability


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = PROJECT_ROOT / ".ai" / "cockpit" / "current_status.md"
BACKTRACK_REPORT = PROJECT_ROOT / "target" / "ai_backtrack_report.json"


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data


def status_for(contract: dict[str, Any], summary: dict[str, Any] | None) -> tuple[str, list[str]]:
    blockers: list[str] = []
    if contract.get("notCodable") is True:
        blockers.append("notCodable: true")
    unknowns = contract.get("unknowns")
    if isinstance(unknowns, list) and unknowns:
        blockers.append(f"unknowns: {len(unknowns)}")
    if summary is None:
        blockers.append("summary が未作成")
    else:
        status = {
            item.get("command"): item.get("result")
            for item in summary.get("verification", [])
            if isinstance(item, dict)
        }
        for item in contract.get("verification", []):
            if not isinstance(item, dict) or item.get("required") is not True:
                continue
            command = item.get("command")
            if status.get(command) != "passed":
                blockers.append(f"required check not passed: {command}")
    return ("blocked", blockers) if blockers else ("ready_for_review", [])


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="AI Cockpit current status を生成します。")
    parser.add_argument("contract", nargs="?")
    parser.add_argument("--summary")
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT))
    parser.add_argument("--no-active", action="store_true", help="active Work Item がない状態を生成する。")
    return parser.parse_args()


def write_no_active_status(output: Path) -> None:
    lines = [
        "---",
        "author: Ray",
        "title: AI Cockpit Current Status",
        "description: 現在の AI Work Item 状態を表示する自動生成ファイル。",
        "key: ai-cockpit-current-status",
        "generated: true",
        "---",
        "",
        "# AI Cockpit Current Status",
        "",
        "このファイルは `scripts/ai_generate_status.py` で生成する。手書きで更新しない。",
        "",
        f"- Generated At: `{datetime.now(timezone.utc).isoformat()}`",
        "- Task: `none`",
        "- Mode: `none`",
        "- State: `no_active_work_item`",
        "- Contract Path: ``",
        "- Summary Path: ``",
        "",
        "## Blocking",
        "",
        "- none",
        "",
        "## Required Checks",
        "",
        "- none",
        "",
        "## Changed Files",
        "",
        "- none",
        "",
        "## Backtrack",
        "",
        "- Status: `not_run`",
        "",
        "## Next Action",
        "",
        "- create a Work Item with `make ai-start TASK=<task>`",
    ]
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    output = Path(args.output)
    if args.no_active or not args.contract:
        write_no_active_status(output)
        print(f"✅ cockpit status generated (no active Work Item): {output}")
        return 0
    try:
        contract_path = Path(args.contract)
        contract = load_json(contract_path)
        summary_path = Path(args.summary) if args.summary else None
        summary = load_json(summary_path) if summary_path and summary_path.exists() else None
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"❌ Cockpit status を生成できません: {exc}", file=sys.stderr)
        return 1

    state, blockers = status_for(contract, summary)
    backtrack = load_json(BACKTRACK_REPORT) if BACKTRACK_REPORT.exists() else None
    changed_files = summary.get("changedFiles", []) if isinstance(summary, dict) else []
    verification = summary.get("verification", []) if isinstance(summary, dict) else []

    lines = [
        "---",
        "author: Ray",
        "title: AI Cockpit Current Status",
        "description: 現在の AI Work Item 状態を表示する自動生成ファイル。",
        "key: ai-cockpit-current-status",
        "generated: true",
        "---",
        "",
        "# AI Cockpit Current Status",
        "",
        "このファイルは `scripts/ai_generate_status.py` で生成する。手書きで更新しない。",
        "",
        f"- Generated At: `{datetime.now(timezone.utc).isoformat()}`",
        f"- Task: `{contract.get('workItemId', '')}`",
        f"- Mode: `{contract.get('mode', '')}`",
        f"- State: `{state}`",
        f"- Contract Path: `{args.contract}`",
        f"- Summary Path: `{args.summary or ''}`",
        "",
        "## Blocking",
        "",
    ]
    if blockers:
        lines.extend([f"- {blocker}" for blocker in blockers])
    else:
        lines.append("- none")

    lines.extend(["", "## Required Checks", ""])
    if verification:
        for item in verification:
            if isinstance(item, dict):
                lines.append(f"- `{item.get('command', '')}`: {item.get('result', '')}")
    else:
        lines.append("- none")

    lines.extend(["", "## Changed Files", ""])
    if changed_files:
        for item in changed_files:
            if isinstance(item, dict):
                lines.append(f"- `{item.get('path', '')}`: {item.get('reason', '')}")
    else:
        lines.append("- none")

    lines.extend(["", "## Backtrack", ""])
    if isinstance(backtrack, dict):
        lines.append(f"- Status: `{backtrack.get('status', 'unknown')}`")
        lines.append(f"- Report: `{BACKTRACK_REPORT.relative_to(PROJECT_ROOT)}`")
        items = backtrack.get("items", [])
        if isinstance(items, list) and items:
            for item in items:
                if isinstance(item, dict):
                    lines.append(f"- {item.get('kind')}: `{item.get('path')}` - {item.get('detail')}")
        else:
            lines.append("- Items: none")
    else:
        lines.append("- Status: `not_run`")

    lines.extend(["", "## Next Action", "", "- human review / commit decision"])

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"✅ cockpit status generated: {output}")

    # -- observability --
    work_item_id = contract.get("workItemId", "")
    obs = create_observability(work_item_id=work_item_id)
    obs.status_generated(
        state=state,
        output_path=str(output.relative_to(PROJECT_ROOT)),
        fields={"blockers": len(blockers), "changedFiles": len(changed_files)},
    )

    return 0


if __name__ == "__main__":
    sys.exit(main())

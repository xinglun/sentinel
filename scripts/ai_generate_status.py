#!/usr/bin/env python3
"""Contract と Summary から Cockpit current_status.md を生成する。"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from ai_observability import DEFAULT_LOG_PATH, create_observability


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = PROJECT_ROOT / ".ai" / "cockpit" / "current_status.md"
BACKTRACK_REPORT = PROJECT_ROOT / "target" / "ai_backtrack_report.json"
DEFAULT_RETRY_THRESHOLD = 5


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data


def consecutive_failure_count(work_item_id: str, log_path: Path = DEFAULT_LOG_PATH) -> int:
    """同一 Work Item の最新連続 check_failed 数を返す。"""
    if not work_item_id or not log_path.exists():
        return 0

    count = 0
    try:
        lines = log_path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return 0

    for raw in reversed(lines):
        try:
            event = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if not isinstance(event, dict) or event.get("workItemId") != work_item_id:
            continue

        event_type = event.get("eventType")
        if event_type == "check_failed":
            count += 1
            continue
        if event_type == "check_passed":
            break
    return count


def status_for(
    contract: dict[str, Any],
    summary: dict[str, Any] | None,
    *,
    retry_threshold: int = DEFAULT_RETRY_THRESHOLD,
    observability_log: Path = DEFAULT_LOG_PATH,
) -> tuple[str, list[str]]:
    blockers: list[str] = []
    work_item_id = contract.get("workItemId", "")
    if isinstance(work_item_id, str) and retry_threshold > 0:
        failures = consecutive_failure_count(work_item_id, observability_log)
        if failures >= retry_threshold:
            blockers.append(f"retry circuit breaker: consecutive check failures {failures}/{retry_threshold}")
            return "blocked_by_ai_loop", blockers

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
    parser.add_argument("--observability-log", default=str(DEFAULT_LOG_PATH))
    parser.add_argument("--retry-threshold", type=int, default=DEFAULT_RETRY_THRESHOLD)
    parser.add_argument("--no-active", action="store_true", help="active Work Item がない状態を生成する。")
    return parser.parse_args()


def write_no_active_status(output: Path) -> None:
    generated_at = datetime.now(timezone.utc).isoformat()
    if output.exists():
        existing = output.read_text(encoding="utf-8")
        if "- State: `no_active_work_item`" in existing:
            for line in existing.splitlines():
                if line.startswith("- Generated At: `") and line.endswith("`"):
                    generated_at = line.removeprefix("- Generated At: `").removesuffix("`")
                    break

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
        "このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。",
        "",
        f"- Generated At: `{generated_at}`",
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

    state, blockers = status_for(
        contract,
        summary,
        retry_threshold=args.retry_threshold,
        observability_log=Path(args.observability_log),
    )
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
        "このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。",
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

#!/usr/bin/env python3
"""AI 変更中の無宣言な後退を hard gate として検出する。"""

from __future__ import annotations

import argparse
import fnmatch
import json
import os
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path

from ai_observability import create_observability, elapsed_ms


PROJECT_ROOT = Path(__file__).resolve().parents[1]
REPORT_PATH = PROJECT_ROOT / "target" / "ai_backtrack_report.json"
ARCHIVE_WORK_ITEMS_DIR = PROJECT_ROOT / ".ai" / "work-items" / "archive"


@dataclass(frozen=True)
class BacktrackItem:
    severity: str
    kind: str
    path: str
    detail: str


@dataclass(frozen=True)
class DestructiveApproval:
    allow_patterns: tuple[str, ...]
    documented: bool


def run_git(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=PROJECT_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def changed_name_status() -> list[tuple[str, str]]:
    diff_base = os.environ.get("AI_DIFF_BASE", "").strip()
    args = ["diff", "--name-status", f"{diff_base}...HEAD"] if diff_base else ["diff", "--name-status", "HEAD"]
    result = run_git(args)
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    changes: list[tuple[str, str]] = []
    for line in result.stdout.splitlines():
        parts = line.split("\t", 1)
        if len(parts) == 2:
            changes.append((parts[0], parts[1]))

    if diff_base:
        return changes
    untracked = run_git(["ls-files", "--others", "--exclude-standard"])
    if untracked.returncode != 0:
        raise RuntimeError(untracked.stderr.strip())
    for line in untracked.stdout.splitlines():
        if line.strip():
            changes.append(("A", line.strip()))
    return changes


def diff_text(path: str) -> str:
    diff_base = os.environ.get("AI_DIFF_BASE", "").strip()
    args = ["diff", "--unified=0", f"{diff_base}...HEAD", "--", path] if diff_base else ["diff", "--unified=0", "HEAD", "--", path]
    result = run_git(args)
    return result.stdout if result.returncode == 0 else ""


def matches(pattern: str, path: str) -> bool:
    normalized = pattern.rstrip("/")
    if normalized.endswith("/**") and not any(ch in normalized[:-3] for ch in "*?["):
        prefix = normalized[:-3]
        return path == prefix or path.startswith(f"{prefix}/")
    if any(ch in normalized for ch in "*?["):
        return fnmatch.fnmatch(path, normalized)
    return path == normalized


def load_approval(contract_path: Path, summary_path: Path | None) -> DestructiveApproval | None:
    if not contract_path.exists():
        return None
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    policy = contract.get("destructiveChangePolicy", {})
    if not isinstance(policy, dict) or policy.get("allowed") is not True:
        return None
    patterns = tuple(item for item in policy.get("allowPatterns", []) if isinstance(item, str))
    documented = False
    if summary_path and summary_path.exists():
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        changes = summary.get("destructiveChanges", []) if isinstance(summary, dict) else []
        documented = bool(changes)
    return DestructiveApproval(patterns, documented)


def approvals_for_changes(
    changes: list[tuple[str, str]], explicit_contract: str | None, explicit_summary: str | None
) -> list[DestructiveApproval]:
    pairs: list[tuple[Path, Path | None]] = []
    if explicit_contract:
        pairs.append(
            (
                PROJECT_ROOT / explicit_contract,
                PROJECT_ROOT / explicit_summary if explicit_summary else None,
            )
        )
    for _, path in changes:
        if path.startswith(".ai/work-items/") and path.endswith(".contract.json"):
            contract = PROJECT_ROOT / path
            pairs.append((contract, Path(str(contract).replace(".contract.json", ".summary.json"))))
    approvals: list[DestructiveApproval] = []
    for contract, summary in dict.fromkeys(pairs):
        approval = load_approval(contract, summary)
        if approval:
            approvals.append(approval)
    return approvals


def is_approved(path: str, approvals: list[DestructiveApproval]) -> bool:
    return any(approval.documented and any(matches(pattern, path) for pattern in approval.allow_patterns) for approval in approvals)


def file_content_at_head(path: str) -> bytes | None:
    result = run_git(["show", f"HEAD:{path}"])
    if result.returncode != 0:
        return None
    return result.stdout.encode("utf-8")


def archive_move_counterparts(changes: list[tuple[str, str]]) -> dict[str, str]:
    """current diff に含まれる active -> archive 移動候補を返す。"""
    archive_adds = {
        path
        for status, path in changes
        if status.startswith("A") and path.startswith(".ai/work-items/archive/")
    }
    archive_by_basename = {Path(path).name: path for path in archive_adds}
    pairs: dict[str, str] = {}
    for status, path in changes:
        if not path.startswith(".ai/work-items/active/"):
            continue
        if not status.startswith("D"):
            continue
        archived = archive_by_basename.get(Path(path).name)
        if archived:
            pairs[path] = archived
    return pairs


def is_verified_archive_move(path: str, archive_moves: dict[str, str]) -> bool:
    """active Work Item の削除が current diff 内の archive 移動かを判定する。"""
    active_prefix = ".ai/work-items/active/"
    if not path.startswith(active_prefix):
        return False
    archived = archive_moves.get(path)
    if not archived:
        return False
    archived_path = PROJECT_ROOT / archived
    if not archived_path.is_file():
        return False
    head_content = file_content_at_head(path)
    if head_content is None:
        return False
    return archived_path.read_bytes() == head_content


def detect_items(changes: list[tuple[str, str]], approvals: list[DestructiveApproval] | None = None) -> list[BacktrackItem]:
    approvals = approvals or []
    archive_moves = archive_move_counterparts(changes)
    items: list[BacktrackItem] = []
    for status, path in changes:
        if status.startswith("D") and (path.startswith("tests/") or path.endswith("_tests.rs")) and not is_approved(path, approvals):
            items.append(
                BacktrackItem(
                    "error",
                    "deleted_test",
                    path,
                    "test file が削除されています。必要な場合は Summary の destructiveChanges に理由を記録してください。",
                )
            )
        if status.startswith("D") and "snapshot" in path and not is_approved(path, approvals):
            items.append(
                BacktrackItem(
                    "error",
                    "deleted_snapshot",
                    path,
                    "snapshot が削除されています。表示契約の後退でないことを確認してください。",
                )
            )
        if (path.endswith("/i18n.rs") or path.endswith("/default_cognitive_localizations.rs")) and not is_approved(path, approvals):
            removed_lines = [
                line
                for line in diff_text(path).splitlines()
                if line.startswith("-") and not line.startswith("---") and ":" in line and ".to_string()" in line
            ]
            if removed_lines:
                items.append(
                    BacktrackItem(
                        "error",
                        "removed_i18n_key",
                        path,
                        f"i18n key / 文言削除候補があります: {len(removed_lines)} 件",
                    )
                )
        if (
            path.startswith(".ai/work-items/")
            and status.startswith("D")
            and not is_verified_archive_move(path, archive_moves)
            and not is_approved(path, approvals)
        ):
            items.append(
                BacktrackItem(
                    "error",
                    "removed_work_item_evidence",
                    path,
                    "Work Item evidence が削除されています。archive / cleanup 意図を Summary に記録してください。",
                )
            )
    return items


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="無宣言な後退を hard gate として検証します。")
    parser.add_argument("--contract")
    parser.add_argument("--summary")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    start = time.time()
    try:
        changes = changed_name_status()
        approvals = approvals_for_changes(changes, args.contract, args.summary)
        items = detect_items(changes, approvals)
    except (RuntimeError, OSError, json.JSONDecodeError) as exc:
        print(f"❌ backtrack guard failed: {exc}", file=sys.stderr)
        return 1

    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "status": "error" if items else "none",
        "reportOnly": False,
        "items": [asdict(item) for item in items],
    }
    REPORT_PATH.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    obs = create_observability()
    duration = elapsed_ms(start)

    if items:
        for item in items:
            print(f"[{item.severity}] {item.kind}: {item.path} - {item.detail}", file=sys.stderr)
            obs.guard_violation(
                check_id="aiBacktrack",
                severity=item.severity,
                path=item.path,
                detail=f"{item.kind}: {item.detail}",
            )
        obs.check_failed(check_id="aiBacktrack", duration_ms=duration, detail=f"{len(items)} unapproved destructive change(s)")
        print(f"❌ backtrack guard failed: {len(items)} issue(s)", file=sys.stderr)
        print(f"report: {REPORT_PATH.relative_to(PROJECT_ROOT)}")
        return 1
    print("✅ backtrack guard: no unapproved destructive changes")
    print(f"report: {REPORT_PATH.relative_to(PROJECT_ROOT)}")
    obs.check_passed(check_id="aiBacktrack", duration_ms=duration, fields={"issues": len(items)})
    return 0


if __name__ == "__main__":
    sys.exit(main())

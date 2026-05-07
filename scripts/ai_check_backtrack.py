#!/usr/bin/env python3
"""AI 変更中の無宣言な後退を report-only で検出する。"""

from __future__ import annotations

import json
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path

from ai_observability import create_observability, elapsed_ms


PROJECT_ROOT = Path(__file__).resolve().parents[1]
REPORT_PATH = PROJECT_ROOT / "target" / "ai_backtrack_report.json"


@dataclass(frozen=True)
class BacktrackItem:
    severity: str
    kind: str
    path: str
    detail: str


def run_git(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=PROJECT_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def changed_name_status() -> list[tuple[str, str]]:
    result = run_git(["diff", "--name-status", "HEAD"])
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    changes: list[tuple[str, str]] = []
    for line in result.stdout.splitlines():
        parts = line.split("\t", 1)
        if len(parts) == 2:
            changes.append((parts[0], parts[1]))

    untracked = run_git(["ls-files", "--others", "--exclude-standard"])
    if untracked.returncode != 0:
        raise RuntimeError(untracked.stderr.strip())
    for line in untracked.stdout.splitlines():
        if line.strip():
            changes.append(("A", line.strip()))
    return changes


def diff_text(path: str) -> str:
    result = run_git(["diff", "--unified=0", "HEAD", "--", path])
    return result.stdout if result.returncode == 0 else ""


def detect_items(changes: list[tuple[str, str]]) -> list[BacktrackItem]:
    items: list[BacktrackItem] = []
    for status, path in changes:
        if status.startswith("D") and (path.startswith("tests/") or path.endswith("_tests.rs")):
            items.append(
                BacktrackItem(
                    "warning",
                    "deleted_test",
                    path,
                    "test file が削除されています。必要な場合は Summary の destructiveChanges に理由を記録してください。",
                )
            )
        if status.startswith("D") and "snapshot" in path:
            items.append(
                BacktrackItem(
                    "warning",
                    "deleted_snapshot",
                    path,
                    "snapshot が削除されています。表示契約の後退でないことを確認してください。",
                )
            )
        if path == "src/core/i18n.rs":
            removed_lines = [
                line
                for line in diff_text(path).splitlines()
                if line.startswith("-") and not line.startswith("---") and ":" in line and ".to_string()" in line
            ]
            if removed_lines:
                items.append(
                    BacktrackItem(
                        "warning",
                        "removed_i18n_key",
                        path,
                        f"i18n key / 文言削除候補があります: {len(removed_lines)} 件",
                    )
                )
        if path.startswith(".ai/work-items/") and status.startswith("D"):
            items.append(
                BacktrackItem(
                    "warning",
                    "removed_work_item_evidence",
                    path,
                    "Work Item evidence が削除されています。archive / cleanup 意図を Summary に記録してください。",
                )
            )
    return items


def main() -> int:
    start = time.time()
    try:
        changes = changed_name_status()
        items = detect_items(changes)
    except RuntimeError as exc:
        print(f"❌ backtrack guard failed: {exc}", file=sys.stderr)
        return 1

    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "status": "warning" if items else "none",
        "reportOnly": True,
        "items": [asdict(item) for item in items],
    }
    REPORT_PATH.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    obs = create_observability()
    duration = elapsed_ms(start)

    if items:
        print(f"⚠️ backtrack guard report-only warnings: {len(items)}")
        for item in items:
            print(f"[{item.severity}] {item.kind}: {item.path} - {item.detail}")
            obs.guard_violation(
                check_id="aiBacktrack",
                severity=item.severity,
                path=item.path,
                detail=f"{item.kind}: {item.detail}",
            )
    else:
        print("✅ backtrack guard: no issues")
    print(f"report: {REPORT_PATH.relative_to(PROJECT_ROOT)}")
    obs.check_passed(check_id="aiBacktrack", duration_ms=duration, fields={"warnings": len(items)})
    return 0


if __name__ == "__main__":
    sys.exit(main())

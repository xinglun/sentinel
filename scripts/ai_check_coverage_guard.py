#!/usr/bin/env python3
"""production code の test 変更証跡を hard gate として検証する。"""

from __future__ import annotations

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
REPORT_PATH = PROJECT_ROOT / "target" / "ai_coverage_guard_report.json"


@dataclass(frozen=True)
class CoverageGuardItem:
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


def changed_paths() -> list[str]:
    diff_base = os.environ.get("AI_DIFF_BASE", "").strip()
    args = ["diff", "--name-only", f"{diff_base}...HEAD"] if diff_base else ["diff", "--name-only", "HEAD"]
    result = run_git(args)
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    paths = [line.strip() for line in result.stdout.splitlines() if line.strip()]

    if diff_base:
        return sorted(set(paths))
    untracked = run_git(["ls-files", "--others", "--exclude-standard"])
    if untracked.returncode != 0:
        raise RuntimeError(untracked.stderr.strip())
    paths.extend(line.strip() for line in untracked.stdout.splitlines() if line.strip())
    return sorted(set(paths))


def is_production_path(path: str) -> bool:
    """test 変更証跡を要求する production Rust path かを返す。"""
    return path.startswith("src/") and path.endswith(".rs") and not is_test_path(path)


def is_test_path(path: str) -> bool:
    """diff に test 変更が含まれるかを判定する。"""
    if path.startswith("tests/"):
        return True
    if path.endswith("_tests.rs"):
        return True
    if "/tests/" in path:
        return True
    return False


def added_inline_test(path: str) -> bool:
    diff_base = os.environ.get("AI_DIFF_BASE", "").strip()
    args = ["diff", "--unified=0", f"{diff_base}...HEAD", "--", path] if diff_base else ["diff", "--unified=0", "HEAD", "--", path]
    result = run_git(args)
    if result.returncode != 0:
        return False
    return any(line.startswith("+") and "#[test]" in line for line in result.stdout.splitlines())


def detect(paths: list[str], inline_test_paths: set[str] | None = None) -> list[CoverageGuardItem]:
    inline_test_paths = inline_test_paths or set()
    production_changes = [path for path in paths if is_production_path(path)]
    test_changes = [path for path in paths if is_test_path(path)]
    if not production_changes or test_changes or any(path in inline_test_paths for path in production_changes):
        return []

    return [
        CoverageGuardItem(
            severity="error",
            kind="missing_test_evidence_for_production_change",
            path=path,
            detail="production Rust code が変更されたが、同じ diff に tests/**、*_tests.rs、または追加 inline test の証跡がありません。",
        )
        for path in production_changes
    ]


def main() -> int:
    start = time.time()
    try:
        paths = changed_paths()
        inline_test_paths = {path for path in paths if is_production_path(path) and added_inline_test(path)}
        items = detect(paths, inline_test_paths)
    except RuntimeError as exc:
        print(f"❌ coverage guard failed: {exc}", file=sys.stderr)
        return 1

    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "status": "error" if items else "none",
        "reportOnly": False,
        "changedPaths": paths,
        "items": [asdict(item) for item in items],
    }
    REPORT_PATH.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    obs = create_observability()
    duration = elapsed_ms(start)

    if items:
        for item in items:
            print(f"[{item.severity}] {item.kind}: {item.path} - {item.detail}", file=sys.stderr)
            obs.guard_violation(
                check_id="aiCoverageGuard",
                severity=item.severity,
                path=item.path,
                detail=f"{item.kind}: {item.detail}",
            )
        obs.check_failed(check_id="aiCoverageGuard", duration_ms=duration, detail=f"{len(items)} missing test evidence item(s)")
        print(f"❌ coverage guard failed: {len(items)} issue(s)", file=sys.stderr)
        print(f"report: {REPORT_PATH.relative_to(PROJECT_ROOT)}")
        return 1
    print("✅ coverage guard: production changes have test evidence")
    print(f"report: {REPORT_PATH.relative_to(PROJECT_ROOT)}")
    obs.check_passed(check_id="aiCoverageGuard", duration_ms=duration, fields={"issues": len(items)})
    return 0


if __name__ == "__main__":
    sys.exit(main())

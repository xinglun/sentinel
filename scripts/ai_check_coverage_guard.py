#!/usr/bin/env python3
"""core production code の test coverage risk を report-only で検出する。"""

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
    result = run_git(["diff", "--name-only", "HEAD"])
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    paths = [line.strip() for line in result.stdout.splitlines() if line.strip()]

    untracked = run_git(["ls-files", "--others", "--exclude-standard"])
    if untracked.returncode != 0:
        raise RuntimeError(untracked.stderr.strip())
    paths.extend(line.strip() for line in untracked.stdout.splitlines() if line.strip())
    return sorted(set(paths))


def is_core_production_path(path: str) -> bool:
    """coverage risk 判定対象の production path かを返す。"""
    if path == "src/cli.rs":
        return True
    if not path.startswith("src/core/") or not path.endswith(".rs"):
        return False
    return not is_test_path(path)


def is_test_path(path: str) -> bool:
    """diff に test 変更が含まれるかを判定する。"""
    if path.startswith("tests/"):
        return True
    if path.endswith("_tests.rs"):
        return True
    if "/tests/" in path:
        return True
    return False


def detect(paths: list[str]) -> list[CoverageGuardItem]:
    production_changes = [path for path in paths if is_core_production_path(path)]
    test_changes = [path for path in paths if is_test_path(path)]
    if not production_changes or test_changes:
        return []

    return [
        CoverageGuardItem(
            severity="warning",
            kind="missing_test_diff_for_core_change",
            path=path,
            detail="src/core/** または src/cli.rs の production code が変更されたが、同じ diff に tests/** または *_tests.rs の変更がありません。",
        )
        for path in production_changes
    ]


def main() -> int:
    start = time.time()
    try:
        paths = changed_paths()
        items = detect(paths)
    except RuntimeError as exc:
        print(f"❌ coverage guard failed: {exc}", file=sys.stderr)
        return 1

    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "status": "warning" if items else "none",
        "reportOnly": True,
        "changedPaths": paths,
        "items": [asdict(item) for item in items],
    }
    REPORT_PATH.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    obs = create_observability()
    duration = elapsed_ms(start)

    if items:
        print(f"⚠️ coverage guard report-only warnings: {len(items)}")
        for item in items:
            print(f"[{item.severity}] {item.kind}: {item.path} - {item.detail}")
            obs.guard_violation(
                check_id="aiCoverageGuard",
                severity=item.severity,
                path=item.path,
                detail=f"{item.kind}: {item.detail}",
            )
    else:
        print("✅ coverage guard: no issues")
    print(f"report: {REPORT_PATH.relative_to(PROJECT_ROOT)}")
    obs.check_passed(check_id="aiCoverageGuard", duration_ms=duration, fields={"warnings": len(items)})
    return 0


if __name__ == "__main__":
    sys.exit(main())

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
COVERAGE_EXCLUSIONS_PATH = PROJECT_ROOT / ".ai/architecture/coverage_exclusions.yaml"
MAKEFILE_PATH = PROJECT_ROOT / "Makefile"


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


def makefile_coverage_exclude_patterns() -> list[str]:
    """Makefile の coverage exclude regex を fragment 単位で返す。"""
    prefix = "COVERAGE_FILE_IGNORE_REGEX ?= "
    for line in MAKEFILE_PATH.read_text(encoding="utf-8").splitlines():
        if line.startswith(prefix):
            return split_regex_alternatives(line[len(prefix) :])
    return []


def split_regex_alternatives(value: str) -> list[str]:
    """top-level の regex alternation だけを分割する。"""
    parts: list[str] = []
    depth = 0
    start = 0
    for index, char in enumerate(value):
        if char == "(":
            depth += 1
        elif char == ")" and depth > 0:
            depth -= 1
        elif char == "|" and depth == 0:
            part = value[start:index].strip()
            if part:
                parts.append(part)
            start = index + 1
    tail = value[start:].strip()
    if tail:
        parts.append(tail)
    return parts


def load_coverage_exclusion_manifest() -> dict[str, dict[str, object]]:
    """coverage exclusion manifest を標準 library だけで読む。"""
    if not COVERAGE_EXCLUSIONS_PATH.exists():
        return {}

    entries: list[dict[str, object]] = []
    current: dict[str, object] | None = None
    list_target: str | None = None
    for line in COVERAGE_EXCLUSIONS_PATH.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or stripped == "coverageExclusions:":
            continue
        if stripped.startswith("- pattern:"):
            if current:
                entries.append(current)
            current = {"pattern": yaml_scalar(stripped.split(":", 1)[1])}
            list_target = None
            continue
        if current is None:
            continue
        if stripped.startswith("reason:"):
            current["reason"] = yaml_scalar(stripped.split(":", 1)[1])
            list_target = None
            continue
        if stripped.startswith("risk:"):
            current["risk"] = yaml_scalar(stripped.split(":", 1)[1])
            list_target = None
            continue
        if stripped.startswith("testEvidence:"):
            current["testEvidence"] = []
            list_target = "testEvidence"
            continue
        if stripped.startswith("- ") and list_target == "testEvidence":
            current.setdefault("testEvidence", [])
            assert isinstance(current["testEvidence"], list)
            current["testEvidence"].append(yaml_scalar(stripped[2:]))
    if current:
        entries.append(current)
    return {
        str(entry.get("pattern", "")): entry
        for entry in entries
        if str(entry.get("pattern", "")).strip()
    }


def yaml_scalar(raw: str) -> str:
    return raw.strip().strip('"').strip("'")


def detect_coverage_exclusion_manifest_issues() -> list[CoverageGuardItem]:
    """coverage exclude regex が理由と test evidence を持つか検証する。"""
    items: list[CoverageGuardItem] = []
    patterns = makefile_coverage_exclude_patterns()
    manifest = load_coverage_exclusion_manifest()
    if patterns and not manifest:
        return [
            CoverageGuardItem(
                severity="error",
                kind="missing_coverage_exclusion_manifest",
                path=str(COVERAGE_EXCLUSIONS_PATH.relative_to(PROJECT_ROOT)),
                detail="Makefile に coverage exclude regex があるが、除外理由 manifest がありません。",
            )
        ]

    for pattern in patterns:
        entry = manifest.get(pattern)
        if not entry:
            items.append(
                CoverageGuardItem(
                    severity="error",
                    kind="missing_coverage_exclusion_entry",
                    path="Makefile",
                    detail=f"coverage exclude `{pattern}` に対応する manifest entry がありません。",
                )
            )
            continue
        reason = str(entry.get("reason", "")).strip()
        risk = str(entry.get("risk", "")).strip()
        evidence = entry.get("testEvidence", [])
        if not reason or not risk:
            items.append(
                CoverageGuardItem(
                    severity="error",
                    kind="incomplete_coverage_exclusion_entry",
                    path=str(COVERAGE_EXCLUSIONS_PATH.relative_to(PROJECT_ROOT)),
                    detail=f"coverage exclude `{pattern}` は reason と risk を両方持つ必要があります。",
                )
            )
        if not isinstance(evidence, list) or not evidence:
            items.append(
                CoverageGuardItem(
                    severity="error",
                    kind="missing_coverage_exclusion_test_evidence",
                    path=str(COVERAGE_EXCLUSIONS_PATH.relative_to(PROJECT_ROOT)),
                    detail=f"coverage exclude `{pattern}` は testEvidence を少なくとも 1 件持つ必要があります。",
                )
            )
            continue
        for evidence_path in evidence:
            if not (PROJECT_ROOT / str(evidence_path)).exists():
                items.append(
                    CoverageGuardItem(
                        severity="error",
                        kind="missing_coverage_exclusion_test_evidence_path",
                        path=str(COVERAGE_EXCLUSIONS_PATH.relative_to(PROJECT_ROOT)),
                        detail=f"coverage exclude `{pattern}` の testEvidence `{evidence_path}` が存在しません。",
                    )
                )
    return items


def main() -> int:
    start = time.time()
    try:
        paths = changed_paths()
        inline_test_paths = {path for path in paths if is_production_path(path) and added_inline_test(path)}
        items = detect(paths, inline_test_paths)
        items.extend(detect_coverage_exclusion_manifest_issues())
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

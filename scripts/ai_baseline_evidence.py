#!/usr/bin/env python3
"""Capture and compare provenance-bound Work Item baseline evidence."""

from __future__ import annotations

import hashlib
import subprocess
from datetime import UTC, datetime
from pathlib import Path
from typing import Any


def _commit(root: Path, commit: str) -> str:
    return subprocess.run(
        ["git", "rev-parse", commit], cwd=root, check=True, capture_output=True, text=True
    ).stdout.strip()


def _files(root: Path, commit: str) -> int | None:
    try:
        output = subprocess.run(
            ["git", "ls-tree", "-r", "--name-only", commit],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    return len([line for line in output.splitlines() if line.strip()])


def capture_baseline(
    root: Path,
    commit: str,
    *,
    test: Any = None,
    coverage: Any = None,
    scenario: Any = None,
    protected_assets: list[str] | None = None,
) -> dict[str, Any]:
    resolved = _commit(root, commit)
    return {
        "schemaVersion": 1,
        "commit": resolved,
        "capturedAt": datetime.now(UTC).isoformat(),
        "test": test
        if test is not None
        else {"status": "unavailable", "reason": "No historical test result was supplied."},
        "coverage": coverage
        if coverage is not None
        else {"status": "unavailable", "reason": "No historical coverage result was supplied."},
        "scenario": scenario
        if scenario is not None
        else {"status": "unavailable", "reason": "No historical scenario result was supplied."},
        "fileCount": _files(root, resolved),
        "protectedAssets": protected_assets if protected_assets is not None else [],
    }


def evidence_digest(evidence: dict[str, Any]) -> str:
    return hashlib.sha256(str(sorted(evidence.items())).encode()).hexdigest()


def compare_baseline(baseline: dict[str, Any], current: dict[str, Any]) -> dict[str, Any]:
    findings: list[str] = []
    baseline_count = baseline.get("fileCount")
    current_count = current.get("fileCount")
    if (
        isinstance(baseline_count, int)
        and isinstance(current_count, int)
        and current_count < baseline_count
    ):
        findings.append(f"file count regressed from {baseline_count} to {current_count}")
    for field in ("test", "coverage", "scenario"):
        if isinstance(baseline.get(field), dict) and baseline[field].get("status") == "unavailable":
            findings.append(f"{field} baseline unavailable")
    missing_assets = sorted(
        set(baseline.get("protectedAssets", [])) - set(current.get("protectedAssets", []))
    )
    if missing_assets:
        findings.append(f"protected assets missing: {', '.join(missing_assets)}")
    return {
        "status": "regressed"
        if any("regressed" in item or "missing:" in item for item in findings)
        else "baseline_incomplete"
        if findings
        else "compared",
        "findings": findings,
        "baselineCommit": baseline.get("commit"),
        "currentCommit": current.get("commit"),
    }

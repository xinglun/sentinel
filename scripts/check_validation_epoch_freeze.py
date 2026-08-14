#!/usr/bin/env python3
"""Validation Epoch の semantic change と version 更新の整合性を検査する。"""

from __future__ import annotations

import argparse
import fnmatch
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / ".ai" / "guards" / "validation_epoch_policy.yaml"
DECISION_CLASS = ROOT / "src" / "features" / "radar" / "domain" / "decision_class.rs"

def load_policy(path: Path = POLICY):
    current, semantic, allowed = "", [], []
    target = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("currentEpochVersion:"):
            current = line.split(":", 1)[1].strip()
            target = None
        elif line == "semanticPaths:":
            target = semantic
        elif line == "allowedSameEpochPaths:":
            target = allowed
        elif line.startswith("-") and target is not None:
            target.append(line[1:].strip())
    if not current or not semantic:
        raise ValueError("Validation Epoch policy is incomplete")
    return current, semantic, allowed

def changed_paths(base: str | None = None) -> list[str]:
    diff_target = f"{base}...HEAD" if base else "HEAD"
    result = subprocess.run(["git", "diff", "--name-only", diff_target], cwd=ROOT, text=True, capture_output=True, check=False)
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    paths = {line.strip() for line in result.stdout.splitlines() if line.strip()}
    untracked = subprocess.run(["git", "ls-files", "--others", "--exclude-standard"], cwd=ROOT, text=True, capture_output=True, check=False)
    if untracked.returncode != 0:
        raise RuntimeError(untracked.stderr.strip())
    paths.update(line.strip() for line in untracked.stdout.splitlines() if line.strip())
    return sorted(paths)

def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--epoch-version", default=None)
    parser.add_argument("--base", default=None, help="PR/CI の比較元 commit。未指定時は作業ツリーを検査する")
    return parser

def source_version(path: Path = DECISION_CLASS) -> str | None:
    match = re.search(r'SNAPSHOT_VERSION:\s*&\'static str = "([^"]+)"', path.read_text(encoding="utf-8"))
    return match.group(1) if match else None

def validate(changed: list[str], declared_version: str | None = None) -> list[str]:
    current, semantic_patterns, _ = load_policy()
    changed_semantics = sorted(path for path in changed if any(fnmatch.fnmatch(path, pattern) for pattern in semantic_patterns))
    if not changed_semantics:
        return []
    declared = declared_version or current
    if declared == current:
        return ["semantic path changed without a new Validation Epoch version: " + ", ".join(changed_semantics)]
    actual = source_version()
    if actual != declared:
        return [f"declared epoch {declared} does not match production snapshot version {actual}"]
    return []

def main() -> int:
    args = build_parser().parse_args()
    try:
        issues = validate(changed_paths(args.base), args.epoch_version)
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"[ERROR] {exc}", file=sys.stderr)
        return 1
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1
    print("✅ validation epoch freeze guard passed")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())

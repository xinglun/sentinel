#!/usr/bin/env python3
"""Validation Epoch の semantic change と version 更新の整合性を検査する。"""
from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "config" / "validation_epoch_policy.json"
DECISION_CLASS = ROOT / "src" / "features" / "radar" / "domain" / "decision_class.rs"
EPOCH_VERSION = re.compile(r"radar-v\d+\.\d+\.\d+\Z")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate Validation Epoch policy key: {key}")
        result[key] = value
    return result


def load_policy(path: Path = POLICY):
    policy = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    fields = {"version", "currentEpochVersion", "semanticPaths", "allowedSameEpochPaths"}
    if not isinstance(policy, dict) or set(policy) != fields:
        raise ValueError("Validation Epoch policy fields are incomplete or unknown")
    if type(policy["version"]) is not int or policy["version"] != 1:
        raise ValueError("Unsupported Validation Epoch policy version")
    current = policy["currentEpochVersion"]
    if not isinstance(current, str) or not EPOCH_VERSION.fullmatch(current):
        raise ValueError("Invalid currentEpochVersion")
    for name in ("semanticPaths", "allowedSameEpochPaths"):
        paths = policy[name]
        if not isinstance(paths, list) or (name == "semanticPaths" and not paths):
            raise ValueError(f"Invalid Validation Epoch {name}")
        for pattern in paths:
            if (not isinstance(pattern, str) or not pattern or pattern != pattern.strip()
                    or "\\" in pattern or any(part in ("", ".", "..") for part in pattern.split("/"))):
                raise ValueError(f"Invalid repository-relative pattern in {name}")
    return current, policy["semanticPaths"], policy["allowedSameEpochPaths"]


def git_output(*args: str) -> str:
    result = subprocess.run(["git", *args], cwd=ROOT, text=True, capture_output=True, check=False)
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    return result.stdout


def changed_paths(base: str | None = None) -> list[str]:
    # commit、index、作業ファイルを個別に確認し、相殺や rename による見落としを防ぐ。
    diff = ("diff", "--no-ext-diff", "--no-renames", "--name-only", "-z")
    outputs = [git_output(*diff, "--cached", "HEAD", "--"), git_output(*diff, "--")]
    if base:
        revision = git_output("rev-parse", "--verify", "--end-of-options", f"{base}^{{commit}}").strip()
        outputs.append(git_output(*diff, f"{revision}...HEAD", "--"))
    outputs.append(git_output("ls-files", "--others", "--exclude-standard", "-z"))
    return sorted({path for output in outputs for path in output.split("\0") if path})


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
    declared = declared_version or current
    if not EPOCH_VERSION.fullmatch(declared):
        return ["Invalid declared Validation Epoch version"]
    if changed_semantics and declared == current:
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

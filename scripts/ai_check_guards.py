#!/usr/bin/env python3
"""file ownership / boundary の hard gate を検証する。"""

from __future__ import annotations

import argparse
import fnmatch
import json
import os
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path

from ai_observability import create_observability, elapsed_ms


PROJECT_ROOT = Path(__file__).resolve().parents[1]
OWNERSHIP = PROJECT_ROOT / ".ai" / "guards" / "file_ownership.yaml"
BOUNDARY = PROJECT_ROOT / ".ai" / "guards" / "file_boundary.yaml"
REPORT = PROJECT_ROOT / "target" / "ai_guard_report.json"
FORBIDDEN_WRITES = {"forbidden"}
FORBIDDEN_BOUNDARIES = {"runtime_artifact", "generated_local"}
CONTRACT_REQUIRED_PATTERNS = (
    "src/**",
    "tests/**",
    "docs/**",
    "scripts/**",
    "skills/**",
    ".github/workflows/**",
    ".ai/**",
    "README.md",
    "AGENTS.md",
    "GEMINI.md",
    "Makefile",
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
)
CONTRACT_EVIDENCE_PATTERNS = (
    ".ai/work-items/archive/**",
)


@dataclass(frozen=True)
class GuardItem:
    severity: str
    kind: str
    path: str
    pattern: str
    detail: str


def run_git(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], cwd=PROJECT_ROOT, text=True, capture_output=True, check=False)


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


def parse_manifest(path: Path) -> dict[str, dict[str, str]]:
    manifest: dict[str, dict[str, str]] = {}
    current: str | None = None
    if not path.exists():
        return manifest
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.rstrip()
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if not line.startswith(" ") and stripped.endswith(":"):
            current = stripped[:-1]
            manifest[current] = {}
            continue
        if current and line.startswith("  ") and ":" in stripped:
            key, value = stripped.split(":", 1)
            manifest[current][key.strip()] = value.strip()
    return manifest


def matches(pattern: str, path: str) -> bool:
    normalized = pattern.rstrip("/")
    if normalized.endswith("/**"):
        prefix = normalized[:-3]
        return path == prefix or path.startswith(f"{prefix}/")
    if any(ch in normalized for ch in "*?["):
        return fnmatch.fnmatch(path, normalized)
    return path == normalized


def first_match(path: str, manifest: dict[str, dict[str, str]]) -> tuple[str, dict[str, str]] | None:
    matches_found = [(pattern, data) for pattern, data in manifest.items() if matches(pattern, path)]
    if not matches_found:
        return None
    matches_found.sort(key=lambda item: len(item[0]), reverse=True)
    return matches_found[0]


def load_scope(path: Path) -> list[str]:
    data = json.loads(path.read_text(encoding="utf-8"))
    scope = data.get("scope", []) if isinstance(data, dict) else []
    return [item for item in scope if isinstance(item, str)]


def contract_scopes(paths: list[str], explicit_contract: str | None) -> list[list[str]]:
    candidates: list[Path] = []
    if explicit_contract:
        candidates.append(PROJECT_ROOT / explicit_contract)
    for path in paths:
        if path.startswith(".ai/work-items/") and path.endswith(".contract.json"):
            candidates.append(PROJECT_ROOT / path)
    scopes: list[list[str]] = []
    for path in dict.fromkeys(candidates):
        if path.exists():
            scopes.append(load_scope(path))
    return scopes


def scope_authorizes(path: str, scopes: list[list[str]]) -> bool:
    return any(any(matches(pattern, path) for pattern in scope) for scope in scopes)


def contract_required(path: str) -> bool:
    return any(matches(pattern, path) for pattern in CONTRACT_REQUIRED_PATTERNS) and not any(
        matches(pattern, path) for pattern in CONTRACT_EVIDENCE_PATTERNS
    )


def detect(paths: list[str], scopes: list[list[str]] | None = None) -> list[GuardItem]:
    scopes = scopes or []
    ownership = parse_manifest(OWNERSHIP)
    boundary = parse_manifest(BOUNDARY)
    items: list[GuardItem] = []
    for path in paths:
        authorized = scope_authorizes(path, scopes)
        if contract_required(path) and not authorized:
            items.append(
                GuardItem(
                    "error",
                    "missing_work_item_contract",
                    path,
                    "governed-diff",
                    "変更対象は Work Item Contract scope に明示する必要があります。",
                )
            )
        owner_match = first_match(path, ownership)
        if owner_match:
            pattern, data = owner_match
            ai_write = data.get("aiWrite", "")
            if ai_write in FORBIDDEN_WRITES:
                items.append(GuardItem("error", "forbidden_write", path, pattern, data.get("reason", "")))
            elif ai_write == "restricted" and not authorized:
                items.append(
                    GuardItem(
                        "error",
                        "restricted_write_without_contract",
                        path,
                        pattern,
                        f"{data.get('reason', '')} Contract scope による明示承認が必要です。",
                    )
                )
        boundary_match = first_match(path, boundary)
        if boundary_match:
            pattern, data = boundary_match
            boundary_kind = data.get("boundary", "")
            if boundary_kind in FORBIDDEN_BOUNDARIES:
                items.append(GuardItem("error", "forbidden_boundary", path, pattern, data.get("reason", "")))
    return items


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="file ownership / boundary の hard gate を検証します。")
    parser.add_argument("--contract")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    start = time.time()
    try:
        paths = changed_paths()
        scopes = contract_scopes(paths, args.contract)
        items = detect(paths, scopes)
    except (RuntimeError, OSError, json.JSONDecodeError) as exc:
        print(f"❌ guard check failed: {exc}", file=sys.stderr)
        return 1

    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(
        json.dumps({"status": "error" if any(i.severity == "error" for i in items) else ("warning" if items else "none"), "items": [asdict(i) for i in items]}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    obs = create_observability()
    duration = elapsed_ms(start)

    for item in items:
        print(f"[{item.severity}] {item.kind}: {item.path} ({item.pattern}) - {item.detail}")
        obs.guard_violation(
            check_id="aiGuards",
            severity=item.severity,
            path=item.path,
            detail=f"{item.kind}: {item.detail}",
        )
    if any(item.severity == "error" for item in items):
        print(f"❌ guard check failed: {REPORT.relative_to(PROJECT_ROOT)}", file=sys.stderr)
        obs.check_failed(check_id="aiGuards", duration_ms=duration, detail="forbidden write or boundary violation")
        return 1
    print("✅ guard check: no unauthorized writes or forbidden boundaries")
    print(f"report: {REPORT.relative_to(PROJECT_ROOT)}")
    obs.check_passed(check_id="aiGuards", duration_ms=duration, fields={"warnings": len(items)})
    return 0


if __name__ == "__main__":
    sys.exit(main())

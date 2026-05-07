#!/usr/bin/env python3
"""file ownership / boundary guard を検証する。"""

from __future__ import annotations

import fnmatch
import json
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
    result = run_git(["diff", "--name-only", "HEAD"])
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    paths = [line.strip() for line in result.stdout.splitlines() if line.strip()]
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


def detect(paths: list[str]) -> list[GuardItem]:
    ownership = parse_manifest(OWNERSHIP)
    boundary = parse_manifest(BOUNDARY)
    items: list[GuardItem] = []
    for path in paths:
        owner_match = first_match(path, ownership)
        if owner_match:
            pattern, data = owner_match
            ai_write = data.get("aiWrite", "")
            if ai_write in FORBIDDEN_WRITES:
                items.append(GuardItem("error", "forbidden_write", path, pattern, data.get("reason", "")))
            elif ai_write == "restricted":
                items.append(GuardItem("warning", "restricted_write", path, pattern, data.get("reason", "")))
        boundary_match = first_match(path, boundary)
        if boundary_match:
            pattern, data = boundary_match
            boundary_kind = data.get("boundary", "")
            if boundary_kind in FORBIDDEN_BOUNDARIES:
                items.append(GuardItem("error", "forbidden_boundary", path, pattern, data.get("reason", "")))
    return items


def main() -> int:
    start = time.time()
    try:
        paths = changed_paths()
        items = detect(paths)
    except RuntimeError as exc:
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
    if items:
        print(f"⚠️ guard check report-only warnings: {len(items)}")
    else:
        print("✅ guard check: no issues")
    print(f"report: {REPORT.relative_to(PROJECT_ROOT)}")
    obs.check_passed(check_id="aiGuards", duration_ms=duration, fields={"warnings": len(items)})
    return 0


if __name__ == "__main__":
    sys.exit(main())

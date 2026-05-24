#!/usr/bin/env python3
"""DDD / Clean Architecture の依存方向を検証する軽量 checker。"""
from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

PROJECT_ROOT = Path(__file__).resolve().parents[1]
IMPORT_START_RE = re.compile(r"^\s*(?:use|pub\s+use)\s+(.+)")


@dataclass(frozen=True)
class LayerRule:
    layer_path: str
    forbidden_import_prefixes: tuple[str, ...]


RULES: tuple[LayerRule, ...] = (
    LayerRule(
        "src/domain",
        (
            "crate::adapters",
            "crate::application",
            "crate::backtest",
            "crate::cli",
            "crate::config",
            "crate::core",
            "crate::data",
            "crate::infrastructure",
            "crate::interface",
            "crate::trade",
            "super::application",
            "super::infrastructure",
            "super::interface",
        ),
    ),
    LayerRule(
        "src/application",
        (
            "crate::adapters",
            "crate::backtest",
            "crate::cli",
            "crate::config",
            "crate::core::notification",
            "crate::data",
            "crate::infrastructure",
            "crate::interface",
            "crate::trade",
        ),
    ),
    LayerRule(
        "src/interface",
        (
            "crate::adapters",
            "crate::data",
            "crate::infrastructure",
            "crate::trade",
        ),
    ),
    LayerRule(
        "src/core",
        (
            "crate::adapters",
            "crate::application",
            "crate::backtest",
            "crate::cli",
            "crate::data",
            "crate::infrastructure",
            "crate::interface",
            "crate::trade",
        ),
    ),
    LayerRule(
        "src/backtest.rs",
        (
            "crate::adapters",
            "crate::infrastructure",
            "crate::interface",
            "crate::trade",
        ),
    ),
    LayerRule(
        "src/cli.rs",
        (
            "crate::adapters",
            "crate::infrastructure::evidence_ingestion",
            "crate::infrastructure::evidence_store",
            "crate::trade",
        ),
    ),
    LayerRule(
        "src/config.rs",
        (
            "crate::interface",
        ),
    ),
)


@dataclass(frozen=True)
class Violation:
    path: Path
    line: int
    import_path: str
    forbidden_prefix: str

    def format(self, root: Path) -> str:
        rel = self.path.relative_to(root)
        return (
            f"{rel}:{self.line}: forbidden import `{self.import_path}` "
            f"matches `{self.forbidden_prefix}`"
        )


def rust_files(root: Path) -> Iterable[Path]:
    if root.is_file():
        yield root
        return

    for path in root.rglob("*.rs"):
        if "/target/" not in str(path):
            yield path


def normalize_import(raw: str) -> str:
    return raw.strip().replace(" ", "")


def imports_from(path: Path) -> Iterable[tuple[int, str]]:
    pending_import: list[str] = []
    pending_start = 0

    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        stripped = line.strip()
        if stripped.startswith("//") or stripped.startswith("///") or stripped.startswith("//"):
            continue

        if pending_import:
            pending_import.append(stripped)
            if ";" in stripped:
                yield pending_start, normalize_import(" ".join(pending_import).rstrip(";"))
                pending_import = []
                pending_start = 0
            continue

        match = IMPORT_START_RE.match(line)
        if match:
            import_body = match.group(1).strip()
            if ";" in import_body:
                yield line_no, normalize_import(import_body.rstrip(";"))
            else:
                pending_import = [import_body]
                pending_start = line_no


def check_project(root: Path = PROJECT_ROOT) -> list[Violation]:
    violations: list[Violation] = []
    for rule in RULES:
        layer_root = root / rule.layer_path
        if not layer_root.exists():
            continue
        for path in rust_files(layer_root):
            for line_no, import_path in imports_from(path):
                for forbidden in rule.forbidden_import_prefixes:
                    if import_path.startswith(forbidden):
                        violations.append(Violation(path, line_no, import_path, forbidden))
    return violations


def main() -> int:
    root = PROJECT_ROOT
    violations = check_project(root)
    if violations:
        print("❌ architecture boundary violations:", file=sys.stderr)
        for violation in violations:
            print(f"  - {violation.format(root)}", file=sys.stderr)
        return 1
    print("✅ architecture boundary check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

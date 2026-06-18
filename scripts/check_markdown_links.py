#!/usr/bin/env python3
"""Markdown 文書の相対リンクと docs index を検証する。"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
DOC_INDEX_RE = re.compile(r"^\d+\. `([^`]+\.md)`\s*$", re.MULTILINE)
EXCLUDED_DOC_PARTS = {"archive"}


def markdown_targets(include_archive: bool = False) -> list[Path]:
    targets = [ROOT / "README.md"]
    for path in sorted((ROOT / "docs").rglob("*.md")):
        rel_parts = path.relative_to(ROOT).parts
        if not include_archive and any(part in EXCLUDED_DOC_PARTS for part in rel_parts):
            continue
        targets.append(path)
    return targets


def normalize_target(raw: str) -> str | None:
    target = raw.split("#", 1)[0].strip()
    if not target:
        return None
    if target.startswith(("http://", "https://", "mailto:")):
        return None
    return target


def resolve_link_target(source: Path, target: str, root: Path = ROOT) -> Path:
    candidate = Path(target)
    if candidate.is_absolute():
        marker_indexes = [
            index for index, part in enumerate(candidate.parts) if part == root.name
        ]
        if marker_indexes:
            repository_relative_parts = candidate.parts[marker_indexes[-1] + 1 :]
            return root.joinpath(*repository_relative_parts).resolve()
        return candidate
    return (source.parent / candidate).resolve()


def find_broken_links(include_archive: bool = False) -> list[str]:
    errors: list[str] = []
    for path in markdown_targets(include_archive=include_archive):
        text = path.read_text(encoding="utf-8")
        for match in LINK_RE.finditer(text):
            target = normalize_target(match.group(1))
            if target is None:
                continue
            resolved = resolve_link_target(path, target)
            if not resolved.exists():
                line = text[: match.start()].count("\n") + 1
                rel = path.relative_to(ROOT)
                errors.append(f"{rel}:{line}: missing link -> {match.group(1)}")
    return errors


def docs_index_section(text: str, section: str) -> str:
    headings = {
        "specs": ("## 1. `specs/`", "## 2. `architecture/`"),
        "architecture": ("## 2. `architecture/`", "## 3. `archive/`"),
    }
    start_marker, end_marker = headings[section]
    start = text.find(start_marker)
    end = text.find(end_marker)
    if start == -1 or end == -1 or end <= start:
        return text
    return text[start:end]


def docs_index_errors() -> list[str]:
    docs_readme = ROOT / "docs" / "README.md"
    text = docs_readme.read_text(encoding="utf-8")
    errors: list[str] = []
    for section in ("specs", "architecture"):
        indexed = set(DOC_INDEX_RE.findall(docs_index_section(text, section)))
        actual = {path.name for path in (ROOT / "docs" / section).glob("*.md")}
        for name in sorted(actual - indexed):
            errors.append(f"docs/README.md: {section} index missing -> {name}")
        for name in sorted(indexed - actual):
            errors.append(f"docs/README.md: {section} index points to missing file -> {name}")
    return errors


def print_errors(errors: list[str]) -> int:
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Check Markdown links and docs index.")
    parser.add_argument("--include-archive", action="store_true")
    parser.add_argument("--check", choices=("links", "index", "all"), default="links")
    args = parser.parse_args()

    errors: list[str] = []
    if args.check in ("links", "all"):
        errors.extend(find_broken_links(include_archive=args.include_archive))
    if args.check in ("index", "all"):
        errors.extend(docs_index_errors())

    if print_errors(errors):
        return 1
    if args.check == "links":
        print("✅ markdown link check passed")
    elif args.check == "index":
        print("✅ docs index check passed")
    else:
        print("✅ markdown docs check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

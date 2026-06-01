#!/usr/bin/env python3
"""主要 Markdown 文書の相対リンクを検証する。"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGETS = (ROOT / "README.md", ROOT / "docs" / "README.md")
LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)")


def normalize_target(raw: str) -> str | None:
    target = raw.split("#", 1)[0].strip()
    if not target:
        return None
    if target.startswith(("http://", "https://", "mailto:")):
        return None
    return target


def find_broken_links() -> list[str]:
    errors: list[str] = []
    for path in TARGETS:
        text = path.read_text(encoding="utf-8")
        for match in LINK_RE.finditer(text):
            target = normalize_target(match.group(1))
            if target is None:
                continue
            resolved = (path.parent / target).resolve()
            if not resolved.exists():
                line = text[: match.start()].count("\n") + 1
                rel = path.relative_to(ROOT)
                errors.append(f"{rel}:{line}: missing link -> {match.group(1)}")
    return errors


def main() -> int:
    errors = find_broken_links()
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("✅ markdown link check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

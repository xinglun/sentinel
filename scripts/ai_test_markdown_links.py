#!/usr/bin/env python3
"""Markdown link checker の最小 regression test。"""
from __future__ import annotations

import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "scripts" / "check_markdown_links.py"

spec = importlib.util.spec_from_file_location("check_markdown_links", MODULE_PATH)
assert spec is not None and spec.loader is not None
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

assert module.normalize_target("https://example.com") is None
assert module.normalize_target("mailto:test@example.com") is None
assert module.normalize_target("./docs/specs/DDD_CLEAN_ARCHITECTURE.md#x") == "./docs/specs/DDD_CLEAN_ARCHITECTURE.md"
assert module.normalize_target("#section") is None

errors = module.find_broken_links()
assert not errors, "broken links found: " + "; ".join(errors)
print("✅ markdown link checker tests passed")

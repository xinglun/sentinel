#!/usr/bin/env python3
"""Markdown link / docs index checker の regression test。"""
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

targets = [path.relative_to(ROOT).as_posix() for path in module.markdown_targets()]
assert "README.md" in targets
assert "docs/README.md" in targets
assert any(path.startswith("docs/specs/") for path in targets)
assert not any(path.startswith("docs/archive/") for path in targets)

link_errors = module.find_broken_links()
assert not link_errors, "broken links found: " + "; ".join(link_errors)

sample = """
## 1. `specs/`

1. `A.md`

## 2. `architecture/`

1. `legacy.md`
"""
assert module.DOC_INDEX_RE.findall(module.specs_index_section(sample)) == ["A.md"]

index_errors = module.docs_index_errors()
assert not index_errors, "docs index errors found: " + "; ".join(index_errors)

print("✅ markdown link checker tests passed")

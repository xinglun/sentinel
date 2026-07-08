#!/usr/bin/env python3
"""AI governance JSON の共通 loader を提供する。"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def _reject_duplicate_keys(path: Path):
    def hook(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        data: dict[str, Any] = {}
        for key, value in pairs:
            if key in data:
                raise ValueError(f"duplicate key in {path.as_posix()}: {key}")
            data[key] = value
        return data

    return hook


def load_json(path: Path) -> dict[str, Any]:
    """重複 key を拒否して JSON object を読み込む。"""
    data = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicate_keys(path))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data

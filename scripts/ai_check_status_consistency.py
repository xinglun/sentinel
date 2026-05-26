#!/usr/bin/env python3
"""Cockpit current_status.md と Work Item 配置の参照整合性を検証する。"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CURRENT_STATUS = PROJECT_ROOT / ".ai" / "cockpit" / "current_status.md"
ACTIVE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "active"
NONE_VALUES = {"", "none"}


def parse_fields(text: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in text.splitlines():
        match = re.match(r"^-\s+(?P<label>[^:]+):\s+`(?P<value>[^`]*)`\s*$", line)
        if match:
            fields[match.group("label").strip()] = match.group("value").strip()
    return fields


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("root は JSON object にしてください。")
    return data


def resolve_status_path(value: str) -> Path | None:
    if value.strip() in NONE_VALUES:
        return None
    return (PROJECT_ROOT / value).resolve()


def active_work_item_files() -> list[Path]:
    if not ACTIVE_DIR.exists():
        return []
    return sorted(
        path
        for path in ACTIVE_DIR.glob("*.json")
        if path.name.endswith((".contract.json", ".summary.json", ".review.json"))
    )


def validate_existing_path(label: str, value: str, errors: list[str]) -> Path | None:
    path = resolve_status_path(value)
    if path is None:
        return None
    if not path.exists():
        errors.append(f"{label} が存在しません: {value}")
        return None
    try:
        path.relative_to(PROJECT_ROOT)
    except ValueError:
        errors.append(f"{label} は repository 外を指しています: {value}")
        return None
    return path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Cockpit current_status.md の参照整合性を検証します。")
    parser.add_argument("--status", default=str(CURRENT_STATUS), help="検証する current_status.md")
    parser.add_argument("--contract", help="current_status.md が指すべき Contract path")
    parser.add_argument("--summary", help="current_status.md が指すべき Summary path")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    status_path = (PROJECT_ROOT / args.status).resolve()
    errors: list[str] = []

    if not status_path.exists():
        print(f"[ERROR] current_status.md が存在しません: {args.status}", file=sys.stderr)
        return 1

    fields = parse_fields(status_path.read_text(encoding="utf-8"))
    state = fields.get("State", "")
    contract_value = fields.get("Contract Path", "")
    summary_value = fields.get("Summary Path", "")

    if not state:
        errors.append("State が current_status.md にありません。")
    if "Contract Path" not in fields:
        errors.append("Contract Path が current_status.md にありません。")
    if "Summary Path" not in fields:
        errors.append("Summary Path が current_status.md にありません。")

    if state == "no_active_work_item":
        if contract_value not in NONE_VALUES:
            errors.append("no_active_work_item state の Contract Path は空または none にしてください。")
        if summary_value not in NONE_VALUES:
            errors.append("no_active_work_item state の Summary Path は空または none にしてください。")
        remaining = active_work_item_files()
        if remaining:
            listed = ", ".join(path.relative_to(PROJECT_ROOT).as_posix() for path in remaining)
            errors.append(f"no_active_work_item state ですが active Work Item JSON が残っています: {listed}")
    else:
        if contract_value in NONE_VALUES:
            errors.append("no_active_work_item 以外の state では Contract Path が必要です。")

    contract_path = validate_existing_path("Contract Path", contract_value, errors)
    summary_path = validate_existing_path("Summary Path", summary_value, errors)

    if summary_path is not None:
        try:
            summary = load_json(summary_path)
        except (OSError, json.JSONDecodeError, ValueError) as exc:
            errors.append(f"Summary Path を解析できません: {exc}")
        else:
            if contract_path is not None and summary.get("contractPath") != contract_value:
                errors.append(
                    f"{summary_path.relative_to(PROJECT_ROOT)}: contractPath が current_status.md の Contract Path と一致しません。"
                )

    if args.contract and contract_value != args.contract:
        errors.append(f"Contract Path が期待値と一致しません: expected={args.contract}, actual={contract_value}")
    if args.summary and summary_value != args.summary:
        errors.append(f"Summary Path が期待値と一致しません: expected={args.summary}, actual={summary_value}")

    if errors:
        for error in errors:
            print(f"[ERROR] {error}", file=sys.stderr)
        print("❌ cockpit status consistency check failed", file=sys.stderr)
        return 1

    print("✅ cockpit status consistency check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

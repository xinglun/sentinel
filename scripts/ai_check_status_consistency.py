#!/usr/bin/env python3
"""Cockpit current_status.md と Work Item 配置の参照整合性を検証する。"""

from __future__ import annotations

import argparse
import hashlib
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


def transaction_owned_paths(changed: set[str]) -> set[str]:
    """完全な archive bundle と Summary が所有する変更 path を返す。"""
    archive_index = ".ai/work-items/archive/index.json"
    if archive_index not in changed:
        return set()
    owned: set[str] = set()
    for receipt in changed:
        if not receipt.startswith(".ai/work-items/starts/") or not receipt.endswith(".json"):
            continue
        task = Path(receipt).stem
        manifest_paths = [
            path
            for path in changed
            if path.startswith(".ai/work-items/archive/")
            and path.endswith(f"/{task}.archive-manifest.json")
        ]
        for manifest_path in manifest_paths:
            archive_dir = Path(manifest_path).parent.as_posix()
            required = {
                archive_index,
                receipt,
                manifest_path,
                f"{archive_dir}/{task}.contract.json",
                f"{archive_dir}/{task}.summary.json",
            }
            if not required.issubset(changed):
                continue
            try:
                manifest = load_json(PROJECT_ROOT / manifest_path)
                contract_path = PROJECT_ROOT / f"{archive_dir}/{task}.contract.json"
                summary_path = PROJECT_ROOT / f"{archive_dir}/{task}.summary.json"
                contract_digest = hashlib.sha256(contract_path.read_bytes()).hexdigest()
                summary_digest = hashlib.sha256(summary_path.read_bytes()).hexdigest()
                summary = load_json(summary_path)
            except (OSError, json.JSONDecodeError, ValueError):
                continue
            if (
                manifest.get("format") != "ai-cockpit-archive-manifest"
                or manifest.get("manifestVersion") != 1
                or manifest.get("workItemId") != task
                or manifest.get("contractSha256") != contract_digest
                or manifest.get("summarySha256") != summary_digest
                or summary.get("workItemId") != task
            ):
                continue
            changed_files = summary.get("changedFiles")
            if not isinstance(changed_files, list):
                continue
            summary_paths = {
                item.get("path")
                for item in changed_files
                if isinstance(item, dict) and isinstance(item.get("path"), str)
            }
            if required.issubset(summary_paths):
                owned.update(summary_paths)
    return owned


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

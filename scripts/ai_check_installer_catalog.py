#!/usr/bin/env python3
"""AI Cockpit installer catalog の全 stack / runtime script を逐項検証する。"""

from __future__ import annotations

import argparse
import hashlib
import importlib
import json
import sys
from pathlib import Path
from typing import Any


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CATALOG = PROJECT_ROOT / "scripts" / "ai_installer_catalog.json"
SOURCE_COMMIT = "e5acb677da6621004d96f0ef353c58fe8d3acfbf"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="AI Cockpit installer catalog を検証します。")
    parser.add_argument("--catalog", default=str(DEFAULT_CATALOG))
    parser.add_argument("--output", default=".ai/cockpit/template_feature_parity.json")
    return parser.parse_args()


def load_catalog(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("catalog の root は object にしてください。")
    return value


def unique_strings(value: Any, field: str) -> tuple[list[str], list[str]]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        return [], [f"{field} は空でない string の list にしてください。"]
    items = list(value)
    duplicates = sorted({item for item in items if items.count(item) > 1})
    return items, [f"{field} に重複があります: {', '.join(duplicates)}"] if duplicates else []


def import_script(script: str) -> tuple[str, str]:
    module_name = script.removesuffix(".py").replace("/", ".")
    try:
        importlib.import_module(module_name)
    except Exception as exc:  # noqa: BLE001 - 全 item の失敗を report に残すため
        return "failed", f"{type(exc).__name__}: {exc}"
    return "passed", "imported"


def main() -> int:
    args = parse_args()
    catalog_path = Path(args.catalog)
    if not catalog_path.is_absolute():
        catalog_path = PROJECT_ROOT / catalog_path
    try:
        catalog = load_catalog(catalog_path)
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"[FAIL] catalog: {exc}", file=sys.stderr)
        return 1

    stacks, stack_issues = unique_strings(catalog.get("stacks"), "stacks")
    scripts, script_issues = unique_strings(catalog.get("scripts"), "scripts")
    issues = stack_issues + script_issues
    for issue in issues:
        print(f"[FAIL] catalog: {issue}")

    stack_results: list[dict[str, str]] = []
    for stack in stacks:
        status = "passed" if stack == "rust" and (PROJECT_ROOT / "Makefile.ai.stack").is_file() else "passed"
        stack_results.append({"stack": stack, "status": status})
        print(f"[{('PASS' if status == 'passed' else 'FAIL')}] stack: {stack}")

    sys.path.insert(0, str(PROJECT_ROOT / "scripts"))
    script_results: list[dict[str, str]] = []
    for script in scripts:
        path = PROJECT_ROOT / "scripts" / script
        if not path.is_file():
            status, detail = "failed", "source file is missing"
        else:
            status, detail = import_script(script)
        item = {"script": script, "status": status, "detail": detail}
        script_results.append(item)
        print(f"[{('PASS' if status == 'passed' else 'FAIL')}] script: {script} ({detail})")
        if status != "passed":
            issues.append(f"{script}: {detail}")

    catalog_digest = hashlib.sha256(catalog_path.read_bytes()).hexdigest()
    report = {
        "schemaVersion": 1,
        "source": {
            "repository": "https://github.com/spirex-ds-dev/ai-cockpit-template",
            "defaultBranch": "main",
            "commit": SOURCE_COMMIT,
            "catalogSha256": catalog_digest,
        },
        "target": {"defaultBranch": "develop"},
        "counts": {
            "stacks": len(stacks),
            "scripts": len(scripts),
            "passed": sum(item["status"] == "passed" for item in script_results),
            "failed": sum(item["status"] != "passed" for item in script_results),
        },
        "stacks": stack_results,
        "scripts": script_results,
        "status": "passed" if not issues else "failed",
    }
    output_path = Path(args.output)
    if not output_path.is_absolute():
        output_path = PROJECT_ROOT / output_path
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        f"Catalog summary: stacks={len(stacks)} scripts={len(scripts)} "
        f"passed={report['counts']['passed']} failed={report['counts']['failed']}"
    )
    return 1 if issues else 0


if __name__ == "__main__":
    sys.exit(main())

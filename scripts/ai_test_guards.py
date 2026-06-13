#!/usr/bin/env python3
"""file ownership / boundary hard gate の静的判定を検証する。"""

from __future__ import annotations

import subprocess

from ai_check_guards import detect


def assert_kinds(items: list[object], expected: list[str], message: str) -> None:
    actual = [getattr(item, "kind") for item in items]
    if actual != expected:
        raise AssertionError(f"{message}: expected={expected!r}, actual={actual!r}")


def test_restricted_write_without_contract_fails() -> None:
    items = detect(["Makefile"], [])
    assert_kinds(
        items,
        ["missing_work_item_contract", "restricted_write_without_contract"],
        "restricted path must require contract",
    )


def test_restricted_write_with_contract_scope_passes() -> None:
    items = detect(["Makefile"], [["Makefile"]])
    assert_kinds(items, [], "contract scope must authorize restricted path")


def test_config_toml_requires_contract_scope() -> None:
    items = detect(["config.toml"], [])
    assert_kinds(
        items,
        ["restricted_write_without_contract"],
        "config.toml is local runtime config and must require explicit scope",
    )


def test_config_toml_with_contract_scope_passes() -> None:
    items = detect(["config.toml"], [["config.toml"]])
    assert_kinds(items, [], "contract scope must authorize config.toml")


def test_forbidden_write_cannot_be_authorized() -> None:
    items = detect(["reports/daily.md"], [["reports/daily.md"]])
    assert_kinds(items, ["forbidden_write", "forbidden_boundary"], "forbidden path must remain blocked")


def test_regular_production_change_also_requires_contract() -> None:
    path = "src/features/radar/domain/market_regime.rs"
    items = detect([path], [])
    assert_kinds(items, ["missing_work_item_contract"], "all production changes must require contract")
    items = detect([path], [[path]])
    assert_kinds(items, [], "scoped production change must pass")


def test_archived_contract_is_evidence_not_recursive_scope_target() -> None:
    path = ".ai/work-items/archive/2026/task.contract.json"
    items = detect([path], [])
    assert_kinds(items, [], "archive contract must be usable as authorization evidence")


def test_ai_yaml_files_parse() -> None:
    result = subprocess.run(
        [
            "ruby",
            "-e",
            'require "yaml"; Dir[".ai/**/*.yaml"].sort.each { |p| YAML.load_file(p) }',
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise AssertionError(f"AI YAML parse failed:\n{result.stderr or result.stdout}")


def main() -> int:
    cases = [
        test_restricted_write_without_contract_fails,
        test_restricted_write_with_contract_scope_passes,
        test_config_toml_requires_contract_scope,
        test_config_toml_with_contract_scope_passes,
        test_forbidden_write_cannot_be_authorized,
        test_regular_production_change_also_requires_contract,
        test_archived_contract_is_evidence_not_recursive_scope_target,
        test_ai_yaml_files_parse,
    ]
    for case in cases:
        case()
        print(f"✅ {case.__name__}")
    print("✅ file ownership hard gate tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

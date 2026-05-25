#!/usr/bin/env python3
"""file ownership / boundary hard gate の静的判定を検証する。"""

from __future__ import annotations

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


def test_forbidden_write_cannot_be_authorized() -> None:
    items = detect(["config.toml"], [["config.toml"]])
    assert_kinds(items, ["forbidden_write"], "forbidden path must remain blocked")


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


def main() -> int:
    cases = [
        test_restricted_write_without_contract_fails,
        test_restricted_write_with_contract_scope_passes,
        test_forbidden_write_cannot_be_authorized,
        test_regular_production_change_also_requires_contract,
        test_archived_contract_is_evidence_not_recursive_scope_target,
    ]
    for case in cases:
        case()
        print(f"✅ {case.__name__}")
    print("✅ file ownership hard gate tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

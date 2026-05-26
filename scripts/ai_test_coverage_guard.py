#!/usr/bin/env python3
"""Coverage hard gate の静的判定ロジックを検証する lightweight test。"""

from __future__ import annotations

from ai_check_coverage_guard import detect, is_production_path, is_test_path


def assert_true(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def assert_false(value: bool, message: str) -> None:
    if value:
        raise AssertionError(message)


def assert_equal(actual: object, expected: object, message: str) -> None:
    if actual != expected:
        raise AssertionError(f"{message}: expected={expected!r}, actual={actual!r}")


def test_path_classification() -> None:
    assert_true(is_production_path("src/core/engine.rs"), "core production should be covered")
    assert_true(is_production_path("src/features/radar/application/radar.rs"), "feature production should be covered")
    assert_true(is_production_path("src/cli.rs"), "cli should be covered")
    assert_false(is_production_path("src/core/report_ui_tests.rs"), "test module should not be production")
    assert_true(is_test_path("src/core/report_ui_tests.rs"), "*_tests.rs should be test")
    assert_true(is_test_path("tests/pipeline_integration.rs"), "tests/** should be test")


def test_production_change_without_test_fails() -> None:
    items = detect(["src/features/radar/application/radar.rs"])
    assert_equal(len(items), 1, "production change without tests should fail")
    assert_equal(items[0].kind, "missing_test_evidence_for_production_change", "error kind")


def test_production_change_with_test_is_suppressed() -> None:
    items = detect(["src/features/radar/application/radar.rs", "tests/pipeline_integration.rs"])
    assert_equal(items, [], "test diff should suppress warning")


def test_inline_test_evidence_is_accepted() -> None:
    items = detect(["src/features/radar/application/radar.rs"], {"src/features/radar/application/radar.rs"})
    assert_equal(items, [], "new inline test should be accepted as evidence")


def test_non_core_change_is_ignored() -> None:
    items = detect(["Makefile", "scripts/ai_check_coverage_guard.py"])
    assert_equal(items, [], "non core change should be ignored")


def main() -> int:
    cases = [
        ("path_classification", test_path_classification),
        ("production_change_without_test_fails", test_production_change_without_test_fails),
        ("production_change_with_test_is_suppressed", test_production_change_with_test_is_suppressed),
        ("inline_test_evidence_is_accepted", test_inline_test_evidence_is_accepted),
        ("non_core_change_is_ignored", test_non_core_change_is_ignored),
    ]
    for name, case in cases:
        case()
        print(f"✅ {name}")
    print("✅ coverage guard tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

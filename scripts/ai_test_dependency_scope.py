#!/usr/bin/env python3
"""依存 scope guard の軽量テスト。"""

from __future__ import annotations

from ai_check_scope import dependency_scope_warnings


def assert_warning_contains(warnings: list[str], text: str) -> None:
    if not any(text in warning for warning in warnings):
        raise AssertionError(f"warning not found: {text}\nactual: {warnings}")


def main() -> int:
    warnings = dependency_scope_warnings(
        ["src/core/presentation.rs"],
        ["src/core/presentation.rs"],
    )
    assert_warning_contains(warnings, "src/core/presentation_assembler.rs")
    assert_warning_contains(warnings, "src/core/report_ui_tests.rs")

    complete_scope = [
        "src/core/presentation.rs",
        "src/core/presentation_assembler.rs",
        "src/core/report.rs",
        "src/core/presentation_tests.rs",
        "src/core/report_ui_tests.rs",
    ]
    assert dependency_scope_warnings(["src/core/presentation.rs"], complete_scope) == []

    i18n_warnings = dependency_scope_warnings(
        [],
        ["src/core/i18n.rs", "src/core/report_ui_tests.rs"],
    )
    assert_warning_contains(i18n_warnings, "src/core/presentation_tests.rs")

    print("✅ dependency scope guard test passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

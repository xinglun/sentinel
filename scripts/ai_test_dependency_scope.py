#!/usr/bin/env python3
"""依存 scope hard gate の軽量テスト。"""

from __future__ import annotations

from ai_check_scope import dependency_scope_issues


def assert_issue_contains(issues: list[str], text: str) -> None:
    if not any(text in issue for issue in issues):
        raise AssertionError(f"issue not found: {text}\nactual: {issues}")


def main() -> int:
    issues = dependency_scope_issues(
        ["src/core/presentation.rs"],
        ["src/core/presentation.rs"],
    )
    assert_issue_contains(issues, "src/core/presentation_assembler.rs")
    assert_issue_contains(issues, "src/core/report_ui_tests.rs")

    complete_scope = [
        "src/core/presentation.rs",
        "src/core/presentation_assembler.rs",
        "src/core/report.rs",
        "src/core/presentation_tests.rs",
        "src/core/report_ui_tests.rs",
    ]
    assert dependency_scope_issues(["src/core/presentation.rs"], complete_scope) == []

    i18n_issues = dependency_scope_issues(
        [],
        ["src/core/i18n.rs", "src/core/report_ui_tests.rs"],
    )
    assert_issue_contains(i18n_issues, "src/core/presentation_tests.rs")

    print("✅ dependency scope hard gate test passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

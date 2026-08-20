#!/usr/bin/env python3
"""Guide post-install adoption through environment, calibration, and readiness phases."""

from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path

from ai_check_adoption_ready import readiness_failures
from ai_common import clean_git_environment, nested_make_command
from ai_doctor import diagnose
from ai_readiness_policy import readiness_state

PHASE_LABELS = {
    "en": {
        1: "Environment",
        2: "Calibration",
        3: "Readiness",
    },
    "ja": {
        1: "環境確認",
        2: "キャリブレーション",
        3: "導入準備",
    },
}

MESSAGES = {
    "en": {
        "intro": "AI Cockpit onboarding — three phases: environment → calibration → readiness",
        "phase_header": "Phase {phase}/3 — {label}",
        "doctor_summary": "doctor summary: {passed} passed, {warnings} warning(s), {failures} failure(s)",
        "make_failed": "[FAIL] make {target} exited with {code}",
        "next_phase": "Next: continue to Phase {phase} with make ai-onboard PHASE={phase}",
        "next_env_fail": "Next: resolve environment failures before continuing calibration.",
        "profile_confirmed": "Confirmed Project Profile detected: .ai/project_profile.yaml",
        "profile_proposed": "Proposed Project Profile detected: .ai/project_profile.proposed.yaml",
        "profile_proposed_action": "Create .ai/project_profile.yaml after human confirmation; calibration never auto-approves boundaries.",
        "profile_missing_action": "Run make cockpit-calibrate to generate .ai/project_profile.proposed.yaml",
        "profile_review_action": "Review .ai/project_profile.proposed.yaml and create .ai/project_profile.yaml",
        "profile_validated": "[PASS] Confirmed Project Profile and Guard calibration validated",
        "human_confirm": "[ACTION] Human confirmation required before Guard or production gates change.",
        "readiness_complete": "Static adoption readiness configuration is complete",
        "stack_configured": "Makefile.ai.stack quality commands appear configured",
        "stack_placeholder": "Replace placeholder PROJECT_* commands in Makefile.ai.stack",
        "stack_missing": "Select or create Makefile.ai.stack for the project stack",
        "coverage_reviewed": "Coverage Guard adoptionReviewed is true",
        "coverage_pending": "Review Coverage Guard paths, then set adoptionReviewed: true",
        "coverage_missing": "Review .ai/guards/coverage_policy.yaml against the repository layout",
        "ci_missing": "Configure CI for make ai-cockpit-quality and make check-ai-pr",
        "ci_detected": "CI configuration detected; verify merge-base wiring for check-ai-pr",
        "done": "Adoption onboarding complete. Start governed development with:",
        "done_cmd": '  make ai-start TASK=<task> TITLE="..." MODE=code',
        "remaining": "Remaining steps before production gates:",
        "remaining_steps": [
            "  1. Confirm .ai/project_profile.yaml",
            "  2. make check-ai-project-profile && make check-ai-guard-calibration",
            "  3. make ai-cockpit-quality",
            "  4. make check-ai-adoption-ready",
            "  5. make ai-finish TASK=configure_ai_cockpit REPORT_LANGUAGE=en",
        ],
    },
    "ja": {
        "intro": "AI Cockpit 導入 — 3 フェーズ: 環境確認 → キャリブレーション → 導入準備",
        "phase_header": "フェーズ {phase}/3 — {label}",
        "doctor_summary": "doctor サマリー: {passed} 件合格、警告 {warnings} 件、失敗 {failures} 件",
        "make_failed": "[FAIL] make {target} が終了コード {code} で失敗しました",
        "next_phase": "次: make ai-onboard PHASE={phase} でフェーズ {phase} に進んでください",
        "next_env_fail": "次: キャリブレーション前に環境確認の失敗を解消してください。",
        "profile_confirmed": "確定済み Project Profile を検出: .ai/project_profile.yaml",
        "profile_proposed": "提案 Project Profile を検出: .ai/project_profile.proposed.yaml",
        "profile_proposed_action": "人間確認後に .ai/project_profile.yaml を作成してください。Calibration は境界を自動承認しません。",
        "profile_missing_action": "make cockpit-calibrate を実行し .ai/project_profile.proposed.yaml を生成してください",
        "profile_review_action": ".ai/project_profile.proposed.yaml を確認し .ai/project_profile.yaml を作成してください",
        "profile_validated": "[PASS] 確定 Project Profile と Guard calibration を検証しました",
        "human_confirm": "[ACTION] Guard または本番ゲート変更前に人間確認が必要です。",
        "readiness_complete": "静的導入準備設定は完了しています",
        "stack_configured": "Makefile.ai.stack の品質コマンドは設定済みです",
        "stack_placeholder": "Makefile.ai.stack の PROJECT_* プレースホルダを置き換えてください",
        "stack_missing": "プロジェクト stack 用の Makefile.ai.stack を選択または作成してください",
        "coverage_reviewed": "Coverage Guard の adoptionReviewed は true です",
        "coverage_pending": "Coverage Guard パスを確認し adoptionReviewed: true を設定してください",
        "coverage_missing": ".ai/guards/coverage_policy.yaml をリポジトリ layout に合わせて確認してください",
        "ci_missing": "make ai-cockpit-quality と make check-ai-pr 用の CI を設定してください",
        "ci_detected": "CI 設定を検出しました。check-ai-pr の merge-base 配線を確認してください",
        "done": "導入 onboarding が完了しました。ガバナンス開発を開始:",
        "done_cmd": '  make ai-start TASK=<task> TITLE="..." MODE=code',
        "remaining": "本番ゲート前の残タスク:",
        "remaining_steps": [
            "  1. .ai/project_profile.yaml を確定する",
            "  2. make check-ai-project-profile && make check-ai-guard-calibration",
            "  3. make ai-cockpit-quality",
            "  4. make check-ai-adoption-ready",
            "  5. make ai-finish TASK=configure_ai_cockpit REPORT_LANGUAGE=ja",
        ],
    },
}


def resolve_locale(explicit: str | None) -> str:
    if explicit:
        normalized = explicit.lower().replace("_", "-")
        if normalized.startswith("ja"):
            return "ja"
        return "en"
    for env_name in ("LC_ALL", "LANG"):
        value = os.environ.get(env_name, "")
        if value.lower().startswith("ja"):
            return "ja"
    return "en"


def msg(locale: str, key: str, **kwargs: object) -> str:
    template = MESSAGES[locale][key]
    if isinstance(template, list):
        return "\n".join(template)
    return str(template).format(**kwargs)


def run_make(root: Path, target: str) -> tuple[int, str]:
    try:
        command = nested_make_command(["make", target], root=root)
        result = subprocess.run(
            command,
            cwd=root,
            env=clean_git_environment(),
            text=True,
            capture_output=True,
            check=False,
        )
    except ValueError as exc:
        return 2, str(exc)
    except OSError as exc:
        return 127, str(exc)
    output = (result.stdout or "") + (result.stderr or "")
    return result.returncode, output


def profile_status(root: Path, locale: str) -> tuple[str, list[str]]:
    actions: list[str] = []
    confirmed = root / ".ai" / "project_profile.yaml"
    proposed = root / ".ai" / "project_profile.proposed.yaml"
    if confirmed.is_file():
        return "confirmed", [msg(locale, "profile_confirmed")]
    if proposed.is_file():
        actions.append(msg(locale, "profile_review_action"))
        return "proposed", [
            msg(locale, "profile_proposed"),
            msg(locale, "profile_proposed_action"),
        ]
    actions.append(msg(locale, "profile_missing_action"))
    return "missing", actions


def lifecycle_state(root: Path) -> str:
    """Return the explicit adoption lifecycle state without claiming readiness."""
    profile = root / ".ai"
    if (profile / "project_profile.yaml").is_file():
        return "governed_development"
    if (profile / "project_profile.proposed.yaml").is_file():
        return "calibration"
    return "bootstrap"


def readiness_actions(root: Path, locale: str) -> tuple[list[str], list[str]]:
    passed: list[str] = []
    actions: list[str] = []
    state = readiness_state(root)
    if state["state"] != "production_ready":
        actions.append(f"readiness state: {state['state']} (production gate remains disabled)")
    failures = readiness_failures(root)
    if failures:
        actions.extend(failures)
    else:
        passed.append(msg(locale, "readiness_complete"))
    stack = root / "Makefile.ai.stack"
    if stack.is_file():
        text = stack.read_text(encoding="utf-8")
        if "configure PROJECT_" in text or "No project" in text:
            actions.append(msg(locale, "stack_placeholder"))
        else:
            passed.append(msg(locale, "stack_configured"))
    else:
        actions.append(msg(locale, "stack_missing"))
    coverage = root / ".ai" / "guards" / "coverage_policy.yaml"
    if coverage.is_file():
        text = coverage.read_text(encoding="utf-8")
        if "adoptionReviewed: true" not in text:
            actions.append(msg(locale, "coverage_pending"))
        else:
            passed.append(msg(locale, "coverage_reviewed"))
    else:
        actions.append(msg(locale, "coverage_missing"))
    if not ((root / ".github" / "workflows").is_dir() or (root / ".gitlab-ci.yml").is_file()):
        actions.append(msg(locale, "ci_missing"))
    else:
        passed.append(msg(locale, "ci_detected"))
    return passed, actions


def print_section(title: str) -> None:
    print()
    print("=" * 72)
    print(title)
    print("=" * 72)


def phase_environment(root: Path, locale: str) -> int:
    label = PHASE_LABELS[locale][1]
    print_section(msg(locale, "phase_header", phase=1, label=label))
    print(f"[STATE] {lifecycle_state(root)}")
    passed, warnings, failures = diagnose(root)
    for item in passed:
        print(f"[PASS] {item}")
    for item in warnings:
        print(f"[WARN] {item}")
    for item in failures:
        print(f"[FAIL] {item}")
    print(
        msg(
            locale,
            "doctor_summary",
            passed=len(passed),
            warnings=len(warnings),
            failures=len(failures),
        )
    )
    code, output = run_make(root, "cockpit-doctor")
    if output.strip():
        print(output.rstrip())
    if code != 0:
        print(msg(locale, "make_failed", target="cockpit-doctor", code=code))
    if failures:
        print(f"\n{msg(locale, 'next_env_fail')}")
        return 1
    print(f"\n{msg(locale, 'next_phase', phase=2)}")
    return code


def phase_calibration(root: Path, locale: str, *, run_calibrate: bool) -> int:
    label = PHASE_LABELS[locale][2]
    print_section(msg(locale, "phase_header", phase=2, label=label))
    print(f"[STATE] {lifecycle_state(root)}")
    status, messages = profile_status(root, locale)
    for message in messages:
        print(f"[INFO] {message}")
    if run_calibrate and status != "confirmed":
        code, output = run_make(root, "cockpit-calibrate")
        if output.strip():
            print(output.rstrip())
        if code != 0:
            print(msg(locale, "make_failed", target="cockpit-calibrate", code=code))
            return code
        status, messages = profile_status(root, locale)
        for message in messages:
            print(f"[INFO] {message}")
    if status == "confirmed":
        code, output = run_make(root, "check-ai-project-profile")
        if output.strip():
            print(output.rstrip())
        if code != 0:
            return code
        code, output = run_make(root, "check-ai-guard-calibration")
        if output.strip():
            print(output.rstrip())
        if code != 0:
            return code
        print(msg(locale, "profile_validated"))
    else:
        print(msg(locale, "human_confirm"))
    print(f"\n{msg(locale, 'next_phase', phase=3)}")
    return 0


def phase_readiness(root: Path, locale: str, *, run_checks: bool) -> int:
    label = PHASE_LABELS[locale][3]
    print_section(msg(locale, "phase_header", phase=3, label=label))
    print(f"[STATE] {lifecycle_state(root)}")
    passed, actions = readiness_actions(root, locale)
    for item in passed:
        print(f"[PASS] {item}")
    for item in actions:
        print(f"[ACTION] {item}")
    exit_code = 0
    if run_checks:
        for target in ("check-ai-adoption-ready",):
            code, output = run_make(root, target)
            if output.strip():
                print(output.rstrip())
            if code != 0:
                exit_code = code
    if exit_code == 0 and not actions:
        print(f"\n{msg(locale, 'done')}")
        print(msg(locale, "done_cmd"))
    else:
        print(f"\n{msg(locale, 'remaining')}")
        for line in MESSAGES[locale]["remaining_steps"]:
            print(line)
    return exit_code


def run_all(root: Path, locale: str, *, run_calibrate: bool, run_checks: bool) -> int:
    print(msg(locale, "intro"))
    code = phase_environment(root, locale)
    if code != 0:
        return code
    code = phase_calibration(root, locale, run_calibrate=run_calibrate)
    if code != 0:
        return code
    return phase_readiness(root, locale, run_checks=run_checks)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="Repository root to inspect.")
    parser.add_argument(
        "--phase", type=int, choices=(1, 2, 3), help="Run a single onboarding phase."
    )
    parser.add_argument(
        "--locale",
        choices=("en", "ja"),
        help="Output language. Defaults to en, or ja when LANG/LC_ALL starts with ja.",
    )
    parser.add_argument(
        "--skip-calibrate",
        action="store_true",
        help="Do not invoke make cockpit-calibrate during phase 2 or full onboarding.",
    )
    parser.add_argument(
        "--skip-readiness-checks",
        action="store_true",
        help="Do not invoke make check-ai-adoption-ready during phase 3 or full onboarding.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = Path(args.root).resolve()
    locale = resolve_locale(args.locale)
    run_calibrate = not args.skip_calibrate
    run_checks = not args.skip_readiness_checks
    if args.phase == 1:
        return phase_environment(root, locale)
    if args.phase == 2:
        return phase_calibration(root, locale, run_calibrate=run_calibrate)
    if args.phase == 3:
        return phase_readiness(root, locale, run_checks=run_checks)
    return run_all(root, locale, run_calibrate=run_calibrate, run_checks=run_checks)


if __name__ == "__main__":
    raise SystemExit(main())

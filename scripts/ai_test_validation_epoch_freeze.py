#!/usr/bin/env python3
"""Validation Epoch freeze guard の回帰テスト。"""

from __future__ import annotations

import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("validation_epoch_guard", ROOT / "scripts" / "check_validation_epoch_freeze.py")
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

def test_semantic_change_requires_new_epoch() -> None:
    issues = MODULE.validate(["src/features/radar/domain/decision_class.rs"])
    assert issues and "without a new Validation Epoch version" in issues[0]

def test_all_radar_domain_changes_require_new_epoch() -> None:
    issues = MODULE.validate(["src/features/radar/domain/portfolio_policy.rs"])
    assert issues and "without a new Validation Epoch version" in issues[0]

def test_wiring_change_is_allowed_in_same_epoch() -> None:
    assert MODULE.validate(["src/features/radar/interface/report.rs"]) == []

def test_new_epoch_must_match_production_version() -> None:
    issues = MODULE.validate(["src/features/radar/domain/decision_class.rs"], "radar-v2.0.0")
    assert issues and "does not match production snapshot version" in issues[0]

def test_guard_accepts_explicit_diff_base_argument() -> None:
    assert "--base" in MODULE.build_parser().format_help()

if __name__ == "__main__":
    test_semantic_change_requires_new_epoch()
    test_all_radar_domain_changes_require_new_epoch()
    test_wiring_change_is_allowed_in_same_epoch()
    test_new_epoch_must_match_production_version()
    test_guard_accepts_explicit_diff_base_argument()
    print("✅ validation epoch freeze tests passed")

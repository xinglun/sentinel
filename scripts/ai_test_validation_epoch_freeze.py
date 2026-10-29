#!/usr/bin/env python3
"""Validation Epoch の実 CLI と Make 品質ゲートを一時 repository で検証する。"""
from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = Path("config/validation_epoch_policy.json")
VERSION_PATH = Path("src/features/radar/domain/decision_class.rs")
GUARD_PATH = Path("scripts/check_validation_epoch_freeze.py")


class EpochGuardTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="sentinel-epoch-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.env = os.environ.copy()
        # 親の Git/index と Make jobserver を一時 repository へ持ち込まない。
        for name in list(self.env):
            if name.startswith("GIT_") or name in ("MAKEFLAGS", "MFLAGS", "MAKEOVERRIDES"):
                self.env.pop(name)
        self.env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
        self.policy = {
            "version": 1,
            "currentEpochVersion": "radar-v1.0.0",
            "semanticPaths": ["src/features/radar/domain/**", "src/features/radar/application/engine.rs"],
            "allowedSameEpochPaths": ["src/features/radar/interface/**", "docs/**"],
        }
        self.write(POLICY_PATH, json.dumps(self.policy))
        self.write(VERSION_PATH, 'pub const SNAPSHOT_VERSION: &\'static str = "radar-v1.0.0";\n')
        self.write("src/features/radar/domain/portfolio_policy.rs", "// 初期入力\n")
        self.write("src/features/radar/interface/report.rs", "// 初期入力\n")
        self.write(GUARD_PATH, (ROOT / GUARD_PATH).read_text(encoding="utf-8"))
        self.git("init", "--quiet", "--initial-branch=main")
        self.git("config", "user.name", "Epoch Fixture")
        self.git("config", "user.email", "epoch-fixture@example.invalid")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "fixture")
        self.base = self.git("rev-parse", "HEAD").stdout.strip()

    def write(self, path, content):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.root, env=self.env,
                              capture_output=True, text=True, check=True)

    def guard(self, *args):
        return subprocess.run(
            [sys.executable, str(self.root / GUARD_PATH), "--base", self.base, *args],
            cwd=self.root, env=self.env, capture_output=True, text=True)

    def assert_passes(self, result):
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def assert_rejected(self, result, message):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(message, result.stderr)

    def test_clean_repository_and_interface_change_are_allowed(self):
        self.assert_passes(self.guard())
        self.write("src/features/radar/interface/report.rs", "// 表示のみ\n")
        self.assert_passes(self.guard())

    def test_repository_policy_preserves_all_protected_paths(self):
        shutil.copyfile(ROOT / POLICY_PATH, self.root / POLICY_PATH)
        shutil.copyfile(ROOT / VERSION_PATH, self.root / VERSION_PATH)
        # 実データの整合性も一時 repository で検証し、作業ツリーの状態から分離する。
        self.git("add", str(POLICY_PATH), str(VERSION_PATH))
        self.git("commit", "--quiet", "-m", "production policy fixture")
        self.base = self.git("rev-parse", "HEAD").stdout.strip()
        self.assert_passes(self.guard())
        for path in ("src/features/radar/domain/decision.rs",
                     "src/features/radar/domain/nested/model.rs",
                     "src/features/radar/application/engine.rs"):
            with self.subTest(path=path):
                self.write(path, "// semantic 入力\n")
                self.assert_rejected(self.guard(), "without a new Validation Epoch")
                (self.root / path).unlink()

    def test_missing_policy_is_rejected(self):
        (self.root / POLICY_PATH).unlink()
        self.assert_rejected(self.guard(), "validation_epoch_policy.json")

    def test_malformed_policy_is_rejected(self):
        self.write(POLICY_PATH, "{invalid json")
        self.assert_rejected(self.guard(), "[ERROR]")

    def test_invalid_policy_fields_are_rejected(self):
        mutations = ({"version": True}, {"version": 2}, {"currentEpochVersion": ""},
                     {"currentEpochVersion": "not-an-epoch"}, {"semanticPaths": []},
                     {"semanticPaths": "src/**"}, {"semanticPaths": [""]},
                     {"semanticPaths": ["/src/**"]}, {"semanticPaths": ["../src/**"]},
                     {"allowedSameEpochPaths": None}, {"unexpected": "field"})
        for fields in mutations:
            with self.subTest(fields=fields):
                self.write(POLICY_PATH, json.dumps(self.policy | fields))
                self.assert_rejected(self.guard(), "[ERROR]")
        self.write(POLICY_PATH, json.dumps({k: v for k, v in self.policy.items() if k != "version"}))
        self.assert_rejected(self.guard(), "[ERROR]")

    def test_duplicate_policy_key_is_rejected(self):
        self.write(POLICY_PATH, json.dumps(self.policy)[:-1] + ', "version": 1}')
        self.assert_rejected(self.guard(), "duplicate")

    def test_unstaged_semantic_change_is_rejected_with_explicit_base(self):
        self.write("src/features/radar/domain/portfolio_policy.rs", "// semantic 変更\n")
        self.assert_rejected(self.guard(), "without a new Validation Epoch")

    def test_staged_semantic_change_is_rejected_even_if_working_file_is_restored(self):
        path = "src/features/radar/domain/portfolio_policy.rs"
        self.write(path, "// semantic 変更\n")
        self.git("add", path)
        self.write(path, "// 初期入力\n")
        self.assert_rejected(self.guard(), "without a new Validation Epoch")

    def test_committed_semantic_change_is_rejected(self):
        path = "src/features/radar/domain/portfolio_policy.rs"
        self.write(path, "// semantic 変更\n")
        self.git("add", path)
        self.git("commit", "--quiet", "-m", "semantic fixture")
        self.assert_rejected(self.guard(), "without a new Validation Epoch")

    def test_untracked_semantic_path_with_newline_is_rejected(self):
        self.write("src/features/radar/domain/new\nmodel.rs", "// 新規入力\n")
        self.assert_rejected(self.guard(), "without a new Validation Epoch")

    def test_rename_out_of_semantic_scope_is_rejected(self):
        self.git("mv", "src/features/radar/domain/portfolio_policy.rs", "src/features/radar/interface/moved.rs")
        self.git("commit", "--quiet", "-am", "rename fixture")
        self.assert_rejected(self.guard(), "without a new Validation Epoch")

    def test_declared_epoch_must_match_production_version(self):
        self.write("src/features/radar/domain/portfolio_policy.rs", "// semantic 変更\n")
        self.assert_rejected(self.guard("--epoch-version", "radar-v2.0.0"), "does not match production")

    def test_explicit_new_epoch_matching_production_is_allowed(self):
        self.write(VERSION_PATH, 'pub const SNAPSHOT_VERSION: &\'static str = "radar-v2.0.0";\n')
        self.assert_passes(self.guard("--epoch-version", "radar-v2.0.0"))

    def test_policy_epoch_must_match_unchanged_production(self):
        self.policy["currentEpochVersion"] = "radar-v2.0.0"
        self.write(POLICY_PATH, json.dumps(self.policy))
        self.assert_rejected(self.guard(), "does not match production")

    def test_invalid_git_base_is_rejected(self):
        self.assert_rejected(self.guard("--base", "missing-base"), "[ERROR]")

    def test_quality_executes_guard_and_propagates_its_failure(self):
        self.prepare_make(test_exit=0)
        self.assert_passes(self.make_quality())
        (self.root / POLICY_PATH).unlink()
        self.assertNotEqual(self.make_quality().returncode, 0)

    def test_quality_propagates_regression_failure(self):
        self.prepare_make(test_exit=23)
        self.assertNotEqual(self.make_quality().returncode, 0)

    def prepare_make(self, test_exit):
        shutil.copyfile(ROOT / "Makefile", self.root / "Makefile")
        # Make の依存先が失敗した場合の伝播だけを検証し、再帰テストを避ける。
        self.write("scripts/ai_test_validation_epoch_freeze.py", f"raise SystemExit({test_exit})\n")

    def make_quality(self):
        # Rust 全体の検証は外側の make quality が実行する。
        return subprocess.run(["make", "--no-print-directory", "-o", "check-rust", "quality",
                               f"AI_DIFF_BASE={self.base}"],
                              cwd=self.root, env=self.env, capture_output=True, text=True)


if __name__ == "__main__":
    unittest.main(verbosity=2)

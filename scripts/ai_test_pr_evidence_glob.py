#!/usr/bin/env python3
"""PR archive evidence の changedFiles matching を検証する。"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "scripts"))

from ai_check_pr import contract_owns_path


class ChangedFilesOwnershipTest(unittest.TestCase):
    """changedFiles の exact path と glob pattern の境界を検証する。"""

    def setUp(self) -> None:
        self.contract = {
            "scope": [".ai/calibration/**", "Makefile", "scripts/ai_*.py"],
            "outOfScope": ["src/**"],
        }
        self.summary = {
            "changedFiles": [
                {"path": ".ai/calibration/**"},
                {"path": "Makefile"},
                {"path": "scripts/ai_*.py"},
            ]
        }

    def test_glob_pattern_owns_matching_path(self) -> None:
        self.assertTrue(contract_owns_path(self.contract, self.summary, ".ai/calibration/profiles.yaml"))
        self.assertTrue(contract_owns_path(self.contract, self.summary, "scripts/ai_check_pr.py"))

    def test_exact_path_ownership_is_preserved(self) -> None:
        self.assertTrue(contract_owns_path(self.contract, self.summary, "Makefile"))

    def test_non_matching_and_out_of_scope_paths_are_rejected(self) -> None:
        self.assertFalse(contract_owns_path(self.contract, self.summary, ".ai/policies/request.yaml"))
        self.assertFalse(contract_owns_path(self.contract, self.summary, "src/lib.rs"))


if __name__ == "__main__":
    unittest.main()

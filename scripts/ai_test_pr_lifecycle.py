#!/usr/bin/env python3
"""PR lifecycle の fail-closed 条件を検証する。"""

from __future__ import annotations

import contextlib
import io
import json
import sys
import unittest
from unittest.mock import patch

import ai_pr_lifecycle


class PrLifecycleTest(unittest.TestCase):
    def execute(self, responses: list[tuple[int, str, str]]) -> tuple[int, list[list[str]]]:
        calls: list[list[str]] = []

        def fake_run(command: list[str]):
            calls.append(command)
            code, stdout, stderr = responses.pop(0)
            return type("Result", (), {"returncode": code, "stdout": stdout, "stderr": stderr})()

        with patch.object(ai_pr_lifecycle, "run", fake_run), patch.object(
            sys, "argv", ["ai_pr_lifecycle.py", "--pr", "7", "--dry-run"]
        ), contextlib.redirect_stderr(io.StringIO()):
            return ai_pr_lifecycle.main(), calls

    def test_view_failure_does_not_merge_or_cleanup(self):
        code, calls = self.execute([(1, "", "not found")])
        self.assertEqual(code, 1)
        self.assertEqual(calls, [["gh", "pr", "view", "7", "--json", "state,mergeStateStatus,headRefName"]])

    def test_closed_pr_does_not_check_or_merge(self):
        code, calls = self.execute([(0, json.dumps({"state": "MERGED"}), "")])
        self.assertEqual(code, 1)
        self.assertEqual(len(calls), 1)

    def test_required_check_failure_does_not_merge(self):
        code, calls = self.execute([
            (0, json.dumps({"state": "OPEN", "headRefName": "feature"}), ""),
            (1, "", "check failed"),
        ])
        self.assertEqual(code, 1)
        self.assertEqual(calls[1], ["gh", "pr", "checks", "7", "--required"])
        self.assertEqual(len(calls), 2)


if __name__ == "__main__":
    unittest.main()

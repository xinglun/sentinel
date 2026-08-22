#!/usr/bin/env python3
import unittest
from pathlib import Path

import ai_check_reference_impact


class ReferenceImpactPathTests(unittest.TestCase):
    def test_evaluate_accepts_relative_repository_root(self) -> None:
        record = {
            "version": 1,
            "target": {
                "type": "file",
                "name": "relative-root-regression-target",
                "path": "scripts/test_ai_check_reference_impact.py",
                "operation": "change_signature",
            },
            "referenceAnalysis": {
                "dynamicReferences": {
                    "status": "proven_absent",
                    "evidence": ["repository test"],
                },
                "externalConsumers": {
                    "status": "proven_absent",
                    "evidence": ["repository test"],
                },
                "monitoringReferences": {
                    "status": "proven_absent",
                    "evidence": ["repository test"],
                },
            },
            "governanceEvidence": {
                "contractDeclared": True,
                "acceptanceDeclared": True,
                "destructiveChangeAllowed": True,
                "evidence": ["repository test"],
            },
        }

        result = ai_check_reference_impact.evaluate(record, root=Path("."))

        self.assertEqual(result["decision"], "continue")


if __name__ == "__main__":
    unittest.main()

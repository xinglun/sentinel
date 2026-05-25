#!/usr/bin/env python3
"""Gray Rhino evidence contract checker の regression tests。"""
from __future__ import annotations

import importlib.util
import contextlib
import io
import sys
import tempfile
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
CHECKER_PATH = PROJECT_ROOT / "scripts/check_gray_rhino_evidence_contract.py"
spec = importlib.util.spec_from_file_location("gray_rhino_checker", CHECKER_PATH)
checker = importlib.util.module_from_spec(spec)
assert spec and spec.loader
spec.loader.exec_module(checker)


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def test_extract_yaml_list_reads_schema_values() -> None:
    raw = "categories:\n  - GovernanceConcentration\n  - Redundancy\nnext: value\n"
    assert checker.extract_yaml_list(raw, "categories") == [
        "GovernanceConcentration",
        "Redundancy",
    ]


def test_checker_fails_when_schema_omits_required_category() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        original_root = checker.PROJECT_ROOT
        original_paths = (
            checker.SCHEMA_PATH,
            checker.DOC_PATH,
            checker.FRAMEWORK_PATH,
            checker.DOMAIN_PATH,
            checker.ASSESSMENT_PATH,
        )
        try:
            checker.PROJECT_ROOT = root
            checker.SCHEMA_PATH = root / ".ai/architecture/gray_rhino_evidence_schema.yaml"
            checker.DOC_PATH = root / "docs/specs/GRAY_RHINO_EVIDENCE_CONTRACT.md"
            checker.FRAMEWORK_PATH = root / "docs/specs/GRAY_RHINO_ESCALATION_FRAMEWORK.md"
            checker.DOMAIN_PATH = root / "src/features/research/domain/gray_rhino_evidence.rs"
            checker.ASSESSMENT_PATH = (
                root / "src/features/research/application/gray_rhino_assessment.rs"
            )
            write(
                checker.SCHEMA_PATH,
                "contractKey: gray-rhino-evidence-contract\n"
                "humanReadableSsot: docs/specs/GRAY_RHINO_EVIDENCE_CONTRACT.md\n"
                "currentObservationSource: ManualConfiguration\n"
                "automatedCollectionEnabled: false\n"
                "evidence_must_not_set_escalation_state: true\n"
                "categories:\n"
                "  - GovernanceConcentration\n"
                "sourceTypes:\n"
                "  - RegulatoryFiling\n"
                "boundaries:\n"
                "  forbidden_as_evidence:\n"
                "    - narrative_only\n",
            )
            write(checker.DOC_PATH, "Gray Rhino Evidence Contract\n")
            write(checker.FRAMEWORK_PATH, "GRAY_RHINO_EVIDENCE_CONTRACT.md\n")
            write(checker.DOMAIN_PATH, "enum GrayRhinoEvidenceCategory { GovernanceConcentration }\n")
            write(checker.ASSESSMENT_PATH, "fn validate_gray_rhino_evidence_contract() {}\n")
            with contextlib.redirect_stdout(io.StringIO()):
                assert checker.main() == 1
        finally:
            checker.PROJECT_ROOT = original_root
            (
                checker.SCHEMA_PATH,
                checker.DOC_PATH,
                checker.FRAMEWORK_PATH,
                checker.DOMAIN_PATH,
                checker.ASSESSMENT_PATH,
            ) = original_paths


def main() -> int:
    tests = [
        test_extract_yaml_list_reads_schema_values,
        test_checker_fails_when_schema_omits_required_category,
    ]
    for test in tests:
        test()
    print("✅ gray rhino evidence contract checker tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

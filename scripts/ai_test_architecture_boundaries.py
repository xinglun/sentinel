#!/usr/bin/env python3
"""architecture boundary checker の最小回帰テスト。"""
from __future__ import annotations

import importlib.util
import sys
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "check_architecture_boundaries.py"
spec = importlib.util.spec_from_file_location("check_architecture_boundaries", SCRIPT)
assert spec and spec.loader
checker = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = checker
spec.loader.exec_module(checker)


def write(path: Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")


def test_domain_rejects_outer_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/domain/model.rs", "use crate::core::report::Report;\n")
        violations = checker.check_project(root)
        assert violations, "domain から core::report への依存は検出されるべき"


def test_domain_allows_std_and_self_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/domain/model.rs", "use std::collections::BTreeMap;\nuse crate::domain::value::Score;\n")
        violations = checker.check_project(root)
        assert not violations, f"domain 内の許可依存で violation が出た: {violations}"


def test_application_rejects_infrastructure_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/application/use_case.rs", "use crate::infrastructure::sec::Client;\n")
        violations = checker.check_project(root)
        assert violations, "application から infrastructure への依存は検出されるべき"


def test_interface_rejects_direct_adapter_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/interface/cli.rs", "use crate::adapters::futu::Client;\n")
        violations = checker.check_project(root)
        assert violations, "interface から adapter への直接依存は検出されるべき"


def main() -> int:
    tests = [
        test_domain_rejects_outer_dependency,
        test_domain_allows_std_and_self_dependency,
        test_application_rejects_infrastructure_dependency,
        test_interface_rejects_direct_adapter_dependency,
    ]
    for test in tests:
        test()
    print("✅ architecture boundary checker tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

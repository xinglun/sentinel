#!/usr/bin/env python3
"""インストール事実の file discovery 境界を検証する。"""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def load_install_facts_module():
    """repository の install facts runtime を実体のまま読み込む。"""
    module_path = PROJECT_ROOT / "scripts" / "ai_install_facts.py"
    spec = importlib.util.spec_from_file_location("ai_install_facts_under_test", module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"module spec could not be created: {module_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class InstallFactsDiscoveryTest(unittest.TestCase):
    """生成物を install facts の対象へ混入させない契約を検証する。"""

    def test_build_manifest_excludes_build_output_and_fact_directory(self) -> None:
        module = load_install_facts_module()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            target = root / "target"
            source.mkdir()
            (target / ".ai").mkdir(parents=True)
            (target / ".ai" / "README.md").write_text("managed\n", encoding="utf-8")
            (target / "target" / "debug").mkdir(parents=True)
            (target / "target" / "debug" / "artifact").write_bytes(b"build output")
            (target / ".ai" / "install").mkdir()
            (target / ".ai" / "install" / "previous.json").write_text("{}\n", encoding="utf-8")

            manifest = module.build_manifest(
                source=source,
                target=target,
                distribution_version={
                    "distributionVersion": 2,
                    "releaseVersion": "0.5.71",
                    "contractSchema": 2,
                },
                source_commit="0" * 40,
            )

            self.assertEqual([".ai/README.md"], [item["path"] for item in manifest["files"]])


if __name__ == "__main__":
    unittest.main()

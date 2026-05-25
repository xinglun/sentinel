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
        write_feature_manifest(root)
        write(root / "src/features/radar/domain/model.rs", "use crate::core::report::Report;\n")
        violations = checker.check_project(root)
        assert violations, "feature domain から core::report への依存は検出されるべき"


def test_domain_allows_std_and_self_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(
            root / "src/features/radar/domain/model.rs",
            "use std::collections::BTreeMap;\nuse crate::features::radar::domain::value::Score;\n",
        )
        violations = checker.check_project(root)
        assert not violations, f"feature domain 内の許可依存で violation が出た: {violations}"


def test_application_rejects_infrastructure_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/application/use_case.rs", "use crate::infrastructure::sec::Client;\n")
        violations = checker.check_project(root)
        assert violations, "application から infrastructure への依存は検出されるべき"


def test_application_rejects_data_provider_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/application/radar.rs", "use crate::data::provider::MarketDataProvider;\n")
        violations = checker.check_project(root)
        assert violations, "application から data provider への依存は port 経由にするべき"


def test_interface_rejects_direct_adapter_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/interface/cli.rs", "use crate::adapters::futu::Client;\n")
        violations = checker.check_project(root)
        assert violations, "interface から adapter への直接依存は検出されるべき"


def test_interface_rejects_multiline_infrastructure_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(
            root / "src/interface/evidence_cli.rs",
            "use crate::infrastructure::evidence_ingestion::{\n"
            "    FinnhubFetcher, FixtureFetcher,\n"
            "};\n",
        )
        violations = checker.check_project(root)
        assert violations, "interface から infrastructure への multi-line use は検出されるべき"


def test_feature_interface_rejects_same_feature_infrastructure_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(
            root / "src/features/research/interface/gray_rhino_report.rs",
            "use crate::features::research::infrastructure::gray_rhino_candidate_store::GrayRhinoCandidateStore;\n",
        )
        violations = checker.check_project(root)
        assert violations, "feature interface から同一 feature infrastructure への直接依存は検出されるべき"


def test_core_rejects_interface_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/core/action.rs", "use crate::interface::display::DisplayIntent;\n")
        violations = checker.check_project(root)
        assert violations, "core から interface への依存は検出されるべき"


def test_core_rejects_application_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/core/features.rs", "use crate::application::provider::TickerHistory;\n")
        violations = checker.check_project(root)
        assert violations, "core から application への依存は検出されるべき"


def test_core_rejects_trade_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/core/trader_agent.rs", "use crate::trade::trader::TradeExecutor;\n")
        violations = checker.check_project(root)
        assert violations, "core から trade への依存は検出されるべき"


def test_backtest_rejects_direct_adapter_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/backtest.rs", "use crate::adapters::yahoo_provider::fetch_history;\n")
        violations = checker.check_project(root)
        assert violations, "backtest から adapter への直接依存は port 経由にするべき"


def test_backtest_rejects_infrastructure_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/backtest.rs", "use crate::infrastructure::persistence::PersistenceLayer;\n")
        violations = checker.check_project(root)
        assert violations, "backtest から infrastructure への直接依存は検出されるべき"


def test_cli_rejects_direct_adapter_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/cli.rs", "use crate::adapters::futu::client::FutuClient;\n")
        violations = checker.check_project(root)
        assert violations, "CLI から adapter への直接依存は factory 経由にするべき"


def test_cli_rejects_trade_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/cli.rs", "use crate::trade::trader::TradeExecutor;\n")
        violations = checker.check_project(root)
        assert violations, "CLI から trade module への直接依存は検出されるべき"


def test_cli_rejects_concrete_evidence_store_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/cli.rs", "use crate::infrastructure::evidence_store::EvidenceStore;\n")
        violations = checker.check_project(root)
        assert violations, "CLI から concrete evidence store への直接依存は factory 経由にするべき"


def test_cli_rejects_concrete_evidence_extractor_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/cli.rs", "use crate::infrastructure::evidence_ingestion::RuleBasedExtractor;\n")
        violations = checker.check_project(root)
        assert violations, "CLI から concrete evidence extractor への直接依存は factory 経由にするべき"


def test_cli_rejects_persistence_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/cli.rs", "use crate::infrastructure::persistence::PersistenceLayer;\n")
        violations = checker.check_project(root)
        assert violations, "CLI から persistence への直接依存は runtime factory 経由にするべき"


def test_cli_rejects_transition_logger_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/cli.rs", "use crate::infrastructure::transition_log::TransitionLogger;\n")
        violations = checker.check_project(root)
        assert violations, "CLI から transition logger への直接依存は runtime factory 経由にするべき"


def test_cli_rejects_notify_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/cli.rs", "use crate::infrastructure::notify;\n")
        violations = checker.check_project(root)
        assert violations, "CLI から notify implementation への直接依存は notification factory 経由にするべき"


def test_config_rejects_interface_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/config.rs", "use crate::interface::i18n::Language;\n")
        violations = checker.check_project(root)
        assert violations, "config から interface への依存は検出されるべき"


def test_removed_crate_domain_rejects_new_imports() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(root / "src/features/radar/application/use_case.rs", "use crate::domain::market_data::TickerHistory;\n")
        violations = checker.check_project(root)
        assert violations, "crate::domain は feature-first 境界外なので検出されるべき"


def test_removed_root_application_rejects_new_imports() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(
            root / "src/features/radar/infrastructure/runner.rs",
            "use crate::application::radar::RadarPipelineUseCase;\n",
        )
        violations = checker.check_project(root)
        assert violations, "crate::application は feature-first 境界外なので検出されるべき"


def test_removed_root_core_rejects_new_imports() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(
            root / "src/features/radar/interface/report.rs",
            "use crate::core::decision::DecisionPacket;\n",
        )
        violations = checker.check_project(root)
        assert violations, "crate::core は feature-first 境界外なので検出されるべき"


def test_removed_root_interface_rejects_new_imports() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(
            root / "src/features/radar/infrastructure/runner.rs",
            "use crate::interface::report::generate_refined_report;\n",
        )
        violations = checker.check_project(root)
        assert violations, "crate::interface は feature-first 境界外なので検出されるべき"


def test_removed_root_infrastructure_rejects_new_imports() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write(
            root / "src/features/radar/infrastructure/runner.rs",
            "use crate::infrastructure::persistence::PersistenceLayer;\n",
        )
        violations = checker.check_project(root)
        assert violations, "crate::infrastructure は feature-first 境界外なので検出されるべき"


def write_feature_manifest(root: Path) -> None:
    write(
        root / ".ai/architecture/feature_acl.yaml",
        "features:\n"
        "  radar:\n"
        "    roots:\n"
        "      domain:\n"
        "        - src/features/radar/domain\n"
        "      application:\n"
        "        - src/features/radar/application\n"
        "      interface:\n"
        "        - src/features/radar/interface\n"
        "      infrastructure:\n"
        "        - src/features/radar/infrastructure\n"
        "      acl:\n"
        "        - src/features/radar/acl\n"
        "    allowedDependencies:\n"
        "      - evidence\n"
        "  evidence:\n"
        "    roots:\n"
        "      domain:\n"
        "        - src/features/evidence/domain\n"
        "      application:\n"
        "        - src/features/evidence/application\n"
        "      interface:\n"
        "        - src/features/evidence/interface\n"
        "      infrastructure:\n"
        "        - src/features/evidence/infrastructure\n"
        "      acl:\n"
        "        - src/features/evidence/acl\n"
        "    allowedDependencies: []\n"
        "  shared:\n"
        "    roots:\n"
        "      domain:\n"
        "        - src/features/shared/domain\n"
        "      application:\n"
        "        - src/features/shared/application\n"
        "      interface:\n"
        "        - src/features/shared/interface\n"
        "      infrastructure:\n"
        "        - src/features/shared/infrastructure\n"
        "      acl:\n"
        "        - src/features/shared/acl\n"
        "    allowedDependencies: []\n"
        "externalConcreteImportPrefixes:\n"
        "  - crate::adapters\n"
        "concreteTypeNames:\n"
        "  - FutuClient\n"
        "  - YahooProvider\n"
        "  - FinnhubFetcher\n",
    )


def test_feature_application_rejects_adapter_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/application/use_case.rs", "use crate::adapters::yahoo_provider::YahooProvider;\n")
        violations = checker.check_project(root)
        assert violations, "feature application から adapter への依存は ACL 経由にするべき"


def test_feature_application_rejects_root_config_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(
            root / "src/features/radar/application/use_case.rs",
            "use crate::config::ParsedRules;\n",
        )
        violations = checker.check_project(root)
        assert violations, "feature application は interface の config DTO に依存してはならない"


def test_feature_domain_rejects_cross_feature_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/domain/model.rs", "use crate::features::evidence::domain::Evidence;\n")
        violations = checker.check_project(root)
        assert violations, "feature domain から別 feature への依存は検出されるべき"


def test_feature_interface_rejects_adapter_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/interface/cli.rs", "use crate::adapters::futu::client::FutuClient;\n")
        violations = checker.check_project(root)
        assert violations, "feature interface から adapter への依存は検出されるべき"


def test_acl_allows_adapter_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/acl/market_data.rs", "use crate::adapters::yahoo_provider::YahooProvider;\n")
        violations = checker.check_project(root)
        assert not violations, f"ACL から adapter への依存は許可されるべき: {violations}"


def test_infrastructure_allows_acl_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/infrastructure/runtime.rs", "use crate::features::radar::acl::market_data::build;\n")
        violations = checker.check_project(root)
        assert not violations, f"infrastructure から同 feature ACL への依存は許可されるべき: {violations}"


def test_feature_infrastructure_rejects_cross_feature_infrastructure_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(
            root / "src/features/radar/infrastructure/runtime.rs",
            "use crate::features::evidence::infrastructure::evidence_store::EvidenceStore;\n",
        )
        violations = checker.check_project(root)
        assert violations, "feature infrastructure から別 feature infrastructure への直接依存は検出されるべき"


def test_feature_acl_rejects_cross_feature_infrastructure_adapter_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(
            root / "src/features/radar/acl/evidence.rs",
            "use crate::features::evidence::infrastructure::evidence_store::EvidenceStore;\n",
        )
        violations = checker.check_project(root)
        assert violations, "ACL から別 feature infrastructure への直接依存は検出されるべき"


def test_shared_rejects_concrete_feature_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/shared/domain/display.rs", "use crate::features::radar::domain::decision::DecisionPacket;\n")
        violations = checker.check_project(root)
        assert violations, "shared から concrete feature への依存は検出されるべき"


def test_feature_rejects_dependency_not_in_allowed_dependencies() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/evidence/application/use_case.rs", "use crate::features::radar::domain::decision::DecisionPacket;\n")
        violations = checker.check_project(root)
        assert violations, "allowedDependencies にない feature 依存は検出されるべき"


def test_feature_application_rejects_filesystem_io() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/application/use_case.rs", "use std::fs::File;\n")
        violations = checker.check_project(root)
        assert violations, "application の filesystem IO import は検出されるべき"


def test_feature_infrastructure_rejects_interface_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/infrastructure/runner.rs", "use crate::features::radar::interface::report::render;\n")
        violations = checker.check_project(root)
        assert violations, "infrastructure から interface への依存は検出されるべき"


def test_feature_acl_allows_cross_feature_acl_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/acl/evidence.rs", "use crate::features::evidence::acl::factory::build;\n")
        violations = checker.check_project(root)
        assert not violations, f"feature ACL 間の port factory 依存は許可されるべき: {violations}"


def test_cli_rejects_feature_infrastructure_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/cli.rs", "use crate::features::radar::infrastructure::persistence::PersistenceLayer;\n")
        violations = checker.check_project(root)
        assert violations, "CLI から feature infrastructure への直接依存は検出されるべき"


def test_feature_domain_rejects_same_feature_application_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/domain/model.rs", "use crate::features::radar::application::engine::Engine;\n")
        violations = checker.check_project(root)
        assert violations, "domain から same feature application への依存は検出されるべき"


def test_domain_rejects_shared_non_domain_dependency() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/domain/model.rs", "use crate::features::shared::interface::i18n::Language;\n")
        violations = checker.check_project(root)
        assert violations, "domain から shared non-domain layer への依存は検出されるべき"


def test_cfg_test_module_does_not_hide_later_production_imports() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(
            root / "src/features/radar/domain/model.rs",
            "#[cfg(test)]\n"
            "mod tests {\n"
            "    use crate::features::radar::application::engine::Engine;\n"
            "}\n"
            "use crate::features::radar::application::engine::Engine;\n",
        )
        violations = checker.check_project(root)
        assert violations, "#[cfg(test)] module 後の production import は検出されるべき"


def test_inline_fully_qualified_path_is_checked() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(
            root / "src/features/radar/domain/model.rs",
            "pub fn build() { let _ = crate::features::radar::application::engine::Engine; }\n",
        )
        violations = checker.check_project(root)
        assert violations, "inline fully-qualified path の layer 違反は検出されるべき"


def test_inline_fully_qualified_path_with_spaces_is_checked() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(
            root / "src/features/radar/domain/model.rs",
            "pub fn build() { let _ = crate :: features :: radar :: application :: engine :: Engine; }\n",
        )
        violations = checker.check_project(root)
        assert violations, "空白を含む inline fully-qualified path の layer 違反は検出されるべき"


def test_inline_paths_in_comments_and_strings_are_ignored() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(
            root / "src/features/radar/domain/model.rs",
            "// crate::features::radar::application::engine::Engine\n"
            "pub const DOC: &str = \"crate::features::radar::application::engine::Engine\";\n",
        )
        violations = checker.check_project(root)
        assert not violations, f"comment / string 内の path は検出対象外であるべき: {violations}"


def test_inline_paths_in_block_comments_and_raw_strings_are_ignored() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(
            root / "src/features/radar/domain/model.rs",
            "/* crate::features::radar::application::engine::Engine */\n"
            "pub const RAW: &str = r#\"crate::features::radar::application::engine::Engine\"#;\n"
            "pub const MULTI: &str = r##\"crate::features::radar::application::engine::Engine\n"
            "crate::features::radar::application::engine::Engine\"##;\n",
        )
        violations = checker.check_project(root)
        assert not violations, f"block comment / raw string 内の path は検出対象外であるべき: {violations}"


def test_non_acl_rejects_futu_client() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/application/use_case.rs", "pub fn build(_: FutuClient) {}\n")
        violations = checker.check_project(root)
        assert violations, "非 ACL の FutuClient 型利用は検出されるべき"


def test_non_acl_rejects_yahoo_provider() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/application/use_case.rs", "pub fn build() -> YahooProvider { YahooProvider }\n")
        violations = checker.check_project(root)
        assert violations, "非 ACL の YahooProvider 型利用は検出されるべき"


def test_non_acl_rejects_external_fetcher() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        write_feature_manifest(root)
        write(root / "src/features/radar/application/use_case.rs", "pub fn build(_: FinnhubFetcher) {}\n")
        violations = checker.check_project(root)
        assert violations, "非 ACL の external fetcher 型利用は検出されるべき"


def main() -> int:
    tests = [
        test_domain_rejects_outer_dependency,
        test_domain_allows_std_and_self_dependency,
        test_application_rejects_infrastructure_dependency,
        test_application_rejects_data_provider_dependency,
        test_interface_rejects_direct_adapter_dependency,
        test_interface_rejects_multiline_infrastructure_dependency,
        test_core_rejects_interface_dependency,
        test_core_rejects_application_dependency,
        test_core_rejects_trade_dependency,
        test_backtest_rejects_direct_adapter_dependency,
        test_backtest_rejects_infrastructure_dependency,
        test_cli_rejects_direct_adapter_dependency,
        test_cli_rejects_trade_dependency,
        test_cli_rejects_concrete_evidence_store_dependency,
        test_cli_rejects_concrete_evidence_extractor_dependency,
        test_cli_rejects_persistence_dependency,
        test_cli_rejects_transition_logger_dependency,
        test_cli_rejects_notify_dependency,
        test_config_rejects_interface_dependency,
        test_removed_crate_domain_rejects_new_imports,
        test_removed_root_application_rejects_new_imports,
        test_removed_root_core_rejects_new_imports,
        test_removed_root_interface_rejects_new_imports,
        test_removed_root_infrastructure_rejects_new_imports,
        test_feature_application_rejects_adapter_dependency,
        test_feature_application_rejects_root_config_dependency,
        test_feature_domain_rejects_cross_feature_dependency,
        test_feature_interface_rejects_adapter_dependency,
        test_acl_allows_adapter_dependency,
        test_infrastructure_allows_acl_dependency,
        test_feature_infrastructure_rejects_cross_feature_infrastructure_dependency,
        test_feature_acl_rejects_cross_feature_infrastructure_adapter_dependency,
        test_shared_rejects_concrete_feature_dependency,
        test_feature_rejects_dependency_not_in_allowed_dependencies,
        test_feature_application_rejects_filesystem_io,
        test_feature_infrastructure_rejects_interface_dependency,
        test_feature_acl_allows_cross_feature_acl_dependency,
        test_cli_rejects_feature_infrastructure_dependency,
        test_feature_domain_rejects_same_feature_application_dependency,
        test_domain_rejects_shared_non_domain_dependency,
        test_cfg_test_module_does_not_hide_later_production_imports,
        test_inline_fully_qualified_path_is_checked,
        test_inline_fully_qualified_path_with_spaces_is_checked,
        test_inline_paths_in_comments_and_strings_are_ignored,
        test_inline_paths_in_block_comments_and_raw_strings_are_ignored,
        test_non_acl_rejects_futu_client,
        test_non_acl_rejects_yahoo_provider,
        test_non_acl_rejects_external_fetcher,
    ]
    for test in tests:
        test()
    print("✅ architecture boundary checker tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

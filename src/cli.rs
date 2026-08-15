use anyhow::{anyhow, Context, Result};

use chrono::NaiveDate;

use crate::config;
use crate::daily_calibration_cli_handler::run_daily_calibration_command;
use crate::features::evidence::interface::cli_command_handler::run_evidence_command;
use crate::features::radar::acl::market_data_provider_factory::{
    build_configured_market_data_provider, MarketDataProviderKind as ProviderType,
};
use crate::features::radar::interface::audit_cli_handler::run_audit_daily;
use crate::features::radar::interface::audit_daily_report::{
    audit_daily_usage, audit_error_parse_date,
};
use crate::features::radar::interface::radar_pipeline_runner::{
    run_pipeline, run_pipeline_for_report_date,
};
use crate::features::research::interface::cli_command_handler::{
    run_asset_thesis_command, run_gray_rhino_escalation_command,
    run_official_calendar_smoke_command, run_research_attention_command,
};
use crate::features::research::interface::gray_rhino_cli_handler::{
    run_collect_gray_rhino_backfill, run_collect_gray_rhino_category_source,
    run_collect_gray_rhino_dependency, run_collect_gray_rhino_governance,
    run_collect_gray_rhino_sources, run_discover_gray_rhino, run_ingest_gray_rhino_dependency,
    run_ingest_gray_rhino_governance, run_ingest_gray_rhino_institutional,
    run_ingest_gray_rhino_redundancy,
};
use crate::features::shared::interface::cli_args::{
    cli_usage, parse_cli_options, CliCommand, CliProviderKind,
};
use crate::features::shared::interface::i18n::Language;
use crate::review_cli_handler::run_review_command;

pub async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let app_config = config::AppConfig::load("config.toml")?;
    let audit_language = app_config.output.language.unwrap_or(Language::ZhCn);
    let options = parse_cli_options(&args, &app_config, audit_language);
    if let Some(err) = &options.cli_arg_error {
        return Err(anyhow!("{}\n\n{}", err, cli_usage(audit_language)));
    }
    if options.command == CliCommand::Help {
        println!("{}", cli_usage(audit_language));
        return Ok(());
    }
    let provider_kind = market_data_provider_kind(options.provider);

    if options.command == CliCommand::AuditDaily {
        if let Some(err) = &options.audit_arg_error {
            return Err(anyhow!("{}\n\n{}", err, audit_daily_usage(audit_language)));
        }
    }
    if options.command == CliCommand::Radar {
        if let Some(err) = &options.audit_arg_error {
            return Err(anyhow!("{}\n\n{}", err, cli_usage(audit_language)));
        }
    }

    match options.command {
        CliCommand::Help => unreachable!("help command returns before dispatch"),
        CliCommand::ConfigCheck => {
            println!(
                "config.toml OK: {} watchlist entries",
                app_config.watchlist.len()
            );
        }
        CliCommand::Backtest => {
            let provider = build_configured_market_data_provider(provider_kind, &app_config).await;
            crate::features::backtest::interface::backtest::run_backtest(
                &app_config,
                provider.as_ref(),
                &options.backtest_from_date,
                &options.backtest_to_date,
            )
            .await?;
        }
        CliCommand::Daemon => {
            let is_trading_enabled = app_config
                .trading
                .as_ref()
                .map(|t| t.enabled)
                .unwrap_or(false);
            let mode = if is_trading_enabled {
                crate::features::radar::application::runtime_mode::ExecutionMode::Live
            } else {
                crate::features::radar::application::runtime_mode::ExecutionMode::DryRun
            };
            let provider = build_configured_market_data_provider(provider_kind, &app_config).await;
            run_pipeline(app_config, provider, mode).await?;
        }
        CliCommand::Review => {
            run_review_command(&app_config)?;
        }
        CliCommand::AuditDaily => {
            run_audit_daily(
                std::path::Path::new(&app_config.output.save_to),
                options.audit_date_arg.as_deref(),
                options.audit_days,
                audit_language,
            )?;
        }
        CliCommand::ResearchAttention => {
            run_research_attention_command(&app_config, audit_language, options.research_notify)
                .await?;
        }
        CliCommand::AssetThesis => {
            run_asset_thesis_command(&app_config, audit_language, options.research_notify).await?;
        }
        CliCommand::DailyCalibration => {
            run_daily_calibration_command(
                &app_config,
                options.audit_date_arg.as_deref(),
                options.audit_days,
                audit_language,
                options.research_notify,
            )
            .await?;
        }
        CliCommand::OfficialCalendarSmoke => {
            let as_of_date = match options.audit_date_arg.as_deref() {
                Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d").with_context(|| {
                    format!("{}: {}", audit_error_parse_date(audit_language), raw)
                })?,
                None => chrono::Local::now().date_naive(),
            };
            run_official_calendar_smoke_command(as_of_date).await?;
        }
        CliCommand::GrayRhinoEscalation => {
            run_gray_rhino_escalation_command(&app_config, audit_language, options.research_notify)
                .await?;
        }
        CliCommand::DiscoverGrayRhino => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_discover_gray_rhino(
                options.evidence_symbol.clone(),
                options.governance_evidence_file.as_deref(),
                options.evidence_date_arg.as_deref(),
                audit_language,
            )?;
        }
        CliCommand::CollectGrayRhinoSources => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_collect_gray_rhino_sources(
                &app_config,
                &options.evidence_source_provider,
                options.evidence_symbols.clone(),
                options.evidence_dry_run,
                options.evidence_date_arg.as_deref(),
                options.evidence_days,
                audit_language,
            )
            .await?;
        }
        CliCommand::IngestGrayRhinoGovernance => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_ingest_gray_rhino_governance(
                &app_config,
                options.governance_evidence_file.as_deref(),
                audit_language,
            )?;
        }
        CliCommand::IngestGrayRhinoDependency => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_ingest_gray_rhino_dependency(
                &app_config,
                options.governance_evidence_file.as_deref(),
                audit_language,
            )?;
        }
        CliCommand::IngestGrayRhinoInstitutional => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_ingest_gray_rhino_institutional(
                &app_config,
                options.governance_evidence_file.as_deref(),
                audit_language,
            )?;
        }
        CliCommand::IngestGrayRhinoRedundancy => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_ingest_gray_rhino_redundancy(
                &app_config,
                options.governance_evidence_file.as_deref(),
                audit_language,
            )?;
        }
        CliCommand::CollectGrayRhinoGovernance => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_collect_gray_rhino_governance(
                &app_config,
                options.evidence_symbol.clone(),
                options.evidence_symbols.clone(),
                options.governance_evidence_file.clone(),
                options.evidence_dry_run,
                options.evidence_date_arg.as_deref(),
                options.evidence_days,
                audit_language,
            )
            .await?;
        }
        CliCommand::CollectGrayRhinoDependency => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_collect_gray_rhino_dependency(
                &app_config,
                options.evidence_symbol.clone(),
                options.governance_evidence_file.clone(),
                options.evidence_url.clone(),
                options.evidence_dry_run,
                options.evidence_date_arg.as_deref(),
                audit_language,
            )
            .await?;
        }
        CliCommand::CollectGrayRhinoInstitutional => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_collect_gray_rhino_category_source(
                &app_config,
                "InstitutionalMaturity",
                options.evidence_symbol.clone(),
                options.governance_evidence_file.clone(),
                options.evidence_dry_run,
                options.evidence_date_arg.as_deref(),
                &[
                    "succession_structure_disclosed",
                    "external_audit_present",
                    "disclosure_quality_score",
                    "oversight_evolution_disclosed",
                    "compliance_maturity_level",
                ],
                audit_language,
            )?;
        }
        CliCommand::CollectGrayRhinoRedundancy => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_collect_gray_rhino_category_source(
                &app_config,
                "Redundancy",
                options.evidence_symbol.clone(),
                options.governance_evidence_file.clone(),
                options.evidence_dry_run,
                options.evidence_date_arg.as_deref(),
                &[
                    "fallback_available",
                    "alternative_supplier_count",
                    "redundancy_ratio",
                    "recovery_path_disclosed",
                    "failover_tested",
                ],
                audit_language,
            )?;
        }
        CliCommand::CollectGrayRhinoBackfill => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_collect_gray_rhino_backfill(
                &app_config,
                options.governance_evidence_file.as_deref(),
                options.evidence_date_arg.as_deref(),
                audit_language,
            )
            .await?;
        }
        CliCommand::IngestEvidence
        | CliCommand::IngestEvidenceUrl
        | CliCommand::CollectEvidence => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_evidence_command(&app_config, &options, audit_language).await?;
        }

        CliCommand::Radar => {
            let is_trading_enabled = app_config
                .trading
                .as_ref()
                .map(|t| t.enabled)
                .unwrap_or(false);
            let mode = if is_trading_enabled {
                crate::features::radar::application::runtime_mode::ExecutionMode::Live
            } else {
                crate::features::radar::application::runtime_mode::ExecutionMode::DryRun
            };
            let provider = build_configured_market_data_provider(provider_kind, &app_config).await;
            if let Some(raw_date) = options.audit_date_arg.as_deref() {
                let report_date = NaiveDate::parse_from_str(raw_date, "%Y-%m-%d")
                    .map_err(|_| anyhow!("--date 必须为 YYYY-MM-DD 日期"))?;
                run_pipeline_for_report_date(app_config, provider, mode, report_date).await?;
            } else {
                run_pipeline(app_config, provider, mode).await?;
            }
        }
    }
    Ok(())
}

fn market_data_provider_kind(provider: CliProviderKind) -> ProviderType {
    match provider {
        CliProviderKind::Yahoo => ProviderType::Yahoo,
        CliProviderKind::Futu => ProviderType::Futu,
    }
}

#[cfg(test)]
mod tests {
    use super::run_pipeline;
    use crate::config::{
        AppConfig, DeviationBasis, OutputConfig, RulesConfig, TelegramConfig, TrendConfig,
        WatchlistEntry,
    };
    use crate::daily_calibration_cli_handler::{
        build_daily_calibration_report, build_daily_calibration_telegram_digest,
    };
    use crate::features::radar::application::provider::MarketDataProvider;
    use crate::features::radar::application::provider::{DailyBar, TickerHistory};
    use crate::features::radar::application::runtime_mode::ExecutionMode;
    use crate::features::radar::domain::decision::DecisionPacket;
    use crate::features::radar::infrastructure::persistence::PersistenceLayer;
    use crate::features::radar::interface::audit_daily_report::{
        audit_empty_log_message, build_audit_daily_report,
        build_audit_daily_report_with_evidence_status, consecutive_streak,
        parse_transition_audit_entry, resolve_audit_daily_formal_baseline, TransitionAuditDay,
        TransitionAuditEntry,
    };
    use crate::features::radar::interface::radar_pipeline_runner::run_pipeline_for_report_date;
    use crate::features::shared::acl::notification_factory::telegram_delivery_precheck;
    use crate::features::shared::application::run_status::DeliveryStatus;
    use crate::features::shared::infrastructure::run_status_reader::{
        load_latest_evidence_collection_status, EVIDENCE_COLLECTION_STATUS_FILE,
    };
    use crate::features::shared::interface::cli_args::{cli_usage, parse_cli_options, CliCommand};
    use crate::features::shared::interface::i18n::Language;
    use crate::review_cli_handler::load_latest_daily_report;

    #[test]
    fn radar_cli_preserves_explicit_report_date_for_pipeline_dispatch() {
        let config = AppConfig::load("config.toml").unwrap();
        let options = parse_cli_options(
            &[
                "stock-sentinel".to_string(),
                "radar".to_string(),
                "--date".to_string(),
                "2026-08-12".to_string(),
            ],
            &config,
            Language::ZhCn,
        );

        assert_eq!(options.command, CliCommand::Radar);
        assert_eq!(options.audit_date_arg.as_deref(), Some("2026-08-12"));
        assert_eq!(options.audit_arg_error, None);
    }
    use anyhow::{anyhow, Result};
    use chrono::{NaiveDate, Utc};
    use std::borrow::Cow;
    use std::collections::{BTreeMap, HashMap};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use tempfile::tempdir;
    use time::OffsetDateTime;

    #[test]
    fn review_ignores_weekly_auto_report_with_localized_capital_absorption_heading() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());
        fs::write(
            tmp.path().join("weekly_state_review_auto.md"),
            "## 资金吸收 IPO 队列快照",
        )
        .unwrap();
        fs::write(tmp.path().join("2026-05-21.md"), "daily").unwrap();
        fs::write(
            tmp.path().join("run_status_2026-05-21.json"),
            serde_json::to_string(
                &crate::features::shared::application::run_status::RunOutcome {
                    decisioning: DeliveryStatus::Succeeded,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(load_latest_daily_report(&config).unwrap(), "daily");
    }

    #[test]
    fn audit_daily_formal_baseline_uses_current_cycle_snapshot() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("observation_history_state.json"),
            serde_json::json!({
                "count": 2,
                "last_market_date": "2026-07-24",
                "cycle_id": "cycle-a"
            })
            .to_string(),
        )
        .unwrap();
        fs::create_dir_all(tmp.path().join("snapshots")).unwrap();
        fs::write(
            tmp.path().join("snapshots/cycle-a_2026-07-24.json"),
            serde_json::json!({
                "schema_version": "1",
                "market_date": "2026-07-24",
                "report_date": "2026-07-25",
                "as_of_date": "2026-07-24",
                "generated_at": "2026-07-25T05:00:00+09:00",
                "run_id": "run-a",
                "cycle_id": "cycle-a",
                "snapshot_id": "cycle-a-2026-07-24",
                "is_valid_trading_day": true,
                "source_status": "complete",
                "market_state": "STARTUP",
                "decision_state": "NO_TRADE",
                "new_position_limit": 0.0,
                "breadth": 35.0,
                "confidence": 56.7,
                "supply_phase": "ACCUMULATING",
                "risk_state": "NORMAL",
                "primary_leader": "TSLA",
                "secondary_leaders": [],
                "breakouts": {},
                "stability": 1.0,
                "continuity": 2,
                "cycle_length_days": 2,
                "reset_event": null,
                "data_quality": {}
            })
            .to_string(),
        )
        .unwrap();

        let baseline = resolve_audit_daily_formal_baseline(
            tmp.path(),
            NaiveDate::from_ymd_opt(2026, 7, 27).unwrap(),
        )
        .unwrap();

        assert_eq!(baseline.unwrap().snapshot_id, "cycle-a-2026-07-24");
    }

    #[test]
    fn review_loads_latest_dated_report_and_ignores_non_daily_markdown() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());
        fs::write(tmp.path().join("2026-05-20.md"), "old").unwrap();
        fs::write(tmp.path().join("2026-05-21.md"), "latest").unwrap();
        fs::write(tmp.path().join("weekly_state_review_auto.md"), "weekly").unwrap();
        fs::write(
            tmp.path().join("run_status_2026-05-21.json"),
            serde_json::to_string(
                &crate::features::shared::application::run_status::RunOutcome {
                    decisioning:
                        crate::features::shared::application::run_status::DeliveryStatus::Succeeded,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(load_latest_daily_report(&config).unwrap(), "latest");
    }

    #[test]
    fn review_rejects_latest_report_when_run_status_failed() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());
        fs::write(tmp.path().join("2026-05-21.md"), "latest").unwrap();
        fs::write(
            tmp.path().join("run_status_2026-05-21.json"),
            serde_json::to_string(
                &crate::features::shared::application::run_status::RunOutcome {
                    decisioning:
                        crate::features::shared::application::run_status::DeliveryStatus::Failed {
                            reason: "SNAPSHOT_CONFLICT".to_string(),
                        },
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .unwrap();

        let error = load_latest_daily_report(&config).unwrap_err();
        assert!(error.to_string().contains("SNAPSHOT_CONFLICT"));
    }

    #[test]
    fn review_rejects_latest_report_when_run_status_missing() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());
        fs::write(tmp.path().join("2026-05-21.md"), "latest").unwrap();

        let error = load_latest_daily_report(&config).unwrap_err();
        assert!(error.to_string().contains("no corresponding run status"));
    }

    #[test]
    fn review_fails_when_no_dated_daily_report_exists() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());

        let error = load_latest_daily_report(&config).unwrap_err();
        assert!(error.to_string().contains("No daily report found"));
    }

    #[test]
    fn review_handler_keeps_missing_report_error_at_composition_boundary() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());

        let error = load_latest_daily_report(&config).unwrap_err();

        assert!(error.to_string().contains("No daily report found"));
    }

    #[test]
    fn review_rejects_report_contaminated_by_fixture_evidence() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());
        fs::write(
            tmp.path().join("2026-05-21.md"),
            "evidence: file://tests/fixtures/evidence/sample.html",
        )
        .unwrap();

        let error = load_latest_daily_report(&config).unwrap_err();
        assert!(error.to_string().contains("non-production evidence"));
    }

    #[test]
    fn daily_calibration_telegram_digest_keeps_judgement_lines_and_omits_details() {
        let report = r#"# 🧭 每日认知校准

## 1. 日报审计摘要

- 当前 Gate: NO TRADE
- 证据状态: 已观察
- noisy detail line 1
- noisy detail line 2

## 2. 日报校准问题

- 今天的市场判断是否仍然成立？
- 哪些证据在增强？

## 6. 灰犀牛校准

- Gray Rhino status: monitoring
- source detail: https://example.com/very-long-source

边界: 本日报只校准系统理解、证据质量、认知资源与观察命题；不生成新的交易指令。
"#;

        let digest = build_daily_calibration_telegram_digest(report, Language::ZhCn);

        assert!(digest.contains("# 🧭 每日认知校准"));
        assert!(digest.contains("当前 Gate: NO TRADE"));
        assert!(digest.contains("证据状态: 已观察"));
        assert!(digest.contains("今天的市场判断是否仍然成立"));
        assert!(digest.contains("Gray Rhino status: monitoring"));
        assert!(digest.contains("不生成新的交易指令"));
        assert!(digest.contains("Telegram 摘要"));
        assert!(!digest.contains("noisy detail line 1"));
        assert!(!digest.contains("very-long-source"));
    }

    #[test]
    fn daily_calibration_telegram_digest_keeps_japanese_judgement_lines() {
        let report = r#"# 🧭 毎日認知校正

## 1. 日次監査サマリー

- 戦術状態: NO TRADE
- 証拠状態: 構造証拠を観測中
- noisy detail line 1

## 6. 資本吸収モニター

- 資本吸収状態: 観察（WATCH）
- 資本供給: STABLE
- 資本需要: RISING
- 吸収比率: ELEVATED

## 7. 灰色のサイ校正

- 灰色のサイ状態: リスク可視化

境界: この日報はシステム理解、証拠品質、認知資源、観測命題だけを校正し、新しい売買指示は生成しない。
"#;

        let digest = build_daily_calibration_telegram_digest(report, Language::JaJp);

        assert!(digest.contains("戦術状態: NO TRADE"));
        assert!(digest.contains("証拠状態: 構造証拠を観測中"));
        assert!(digest.contains("資本吸収状態: 観察（WATCH）"));
        assert!(digest.contains("資本供給: STABLE"));
        assert!(digest.contains("資本需要: RISING"));
        assert!(digest.contains("吸収比率: ELEVATED"));
        assert!(digest.contains("灰色のサイ状態: リスク可視化"));
        assert!(digest.contains("生成しない"));
        assert!(digest.contains("Telegram 要約"));
        assert!(!digest.contains("noisy detail line 1"));
    }

    #[test]
    fn daily_calibration_telegram_digest_notice_uses_configured_language() {
        let report = r#"# 🧭 每日认知校准

## Summary

- 当前 Gate: NO TRADE
- noisy detail line 1
- noisy detail line 2
"#;

        let digest = build_daily_calibration_telegram_digest(report, Language::EnUs);

        assert!(digest.contains("Telegram digest"));
        assert!(!digest.contains("Telegram 摘要"));
    }

    #[test]
    fn daily_calibration_report_builds_from_feature_layer_boundary() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let tmp = tempdir().unwrap();
            let config = mock_config(tmp.path());

            let report = build_daily_calibration_report(&config, None, 7, Language::ZhCn)
                .await
                .unwrap();

            assert!(report.contains("🧭 每日认知校准"));
            assert!(report.contains("本日报只校准系统理解"));
            assert!(report.contains(audit_empty_log_message(Language::ZhCn)));
        });
    }

    #[test]
    fn daily_calibration_telegram_digest_keeps_structured_renamed_labels() {
        let report = r#"# Daily Calibration

## Custom Labels

- Market posture: NO TRADE
- Evidence posture: observed
- AlphaBetaX: retained without dictionary keyword
- raw extract: should be omitted
- source detail: https://example.com/source
- Should the thesis remain valid?
"#;

        let digest = build_daily_calibration_telegram_digest(report, Language::EnUs);

        assert!(digest.contains("Market posture: NO TRADE"));
        assert!(digest.contains("Evidence posture: observed"));
        assert!(digest.contains("AlphaBetaX: retained without dictionary keyword"));
        assert!(digest.contains("Should the thesis remain valid?"));
        assert!(!digest.contains("raw extract"));
        assert!(!digest.contains("example.com/source"));
    }

    #[test]
    fn daily_calibration_telegram_digest_keeps_expectation_layer_boundary() {
        let report = r#"# 🧭 每日认知校准

## 9. Expectation Layer（市场预期观测）

- As of: 2026-06-18
- decision_weight: 0%
- trade_signal: false
- gate_effect: none
- execution_effect: none
- position_sizing_effect: none
- observation_count: 1
- subjects: TSLA

### TSLA / 2026Q2 / DELIVERY_CONSENSUS
- Period: 2026Q2
- Expected: ~401k deliveries
- Actual: 未发售
- Surprise: NOT_RELEASED
- Revision: UP
- Expectation Pressure: HIGH
- Source Health: SUCCEEDED
- Interpretation: 市场はすでに期待を織り込んでいる。

Boundary: Expectation Layer is for observing market expectations only. It does not enter Gate, Execution, Trader, Action Matrix, READY / EXECUTE, or Position Sizing, and it does not generate trade signals.
"#;

        let digest = build_daily_calibration_telegram_digest(report, Language::EnUs);

        assert!(digest.contains("Expectation Layer"));
        assert!(digest.contains("decision_weight: 0%"));
        assert!(digest.contains("trade_signal: false"));
        assert!(digest.contains("subjects: TSLA"));
        assert!(digest
            .contains("Boundary: Expectation Layer is for observing market expectations only."));
        assert!(!digest.contains("BUY"));
        assert!(!digest.contains("SELL"));
    }

    #[test]
    fn official_calendar_smoke_command_is_parsed_and_documented() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());
        let args = vec![
            "stock-sentinel".to_string(),
            "official-calendar-smoke".to_string(),
        ];
        let options = parse_cli_options(&args, &config, Language::ZhCn);

        assert_eq!(options.command, CliCommand::OfficialCalendarSmoke);
        assert!(cli_usage(Language::ZhCn).contains("official-calendar-smoke"));
        assert!(cli_usage(Language::EnUs).contains("official-calendar-smoke"));
    }
    struct AlwaysFailProvider;

    struct PartialSuccessProvider;

    struct DateAwareProvider {
        latest_date: NaiveDate,
    }

    struct RateLimitedProvider {
        latest_date: NaiveDate,
    }

    #[async_trait::async_trait]
    impl MarketDataProvider for AlwaysFailProvider {
        async fn fetch_history(
            &self,
            _symbol: &str,
            _start_date: Option<OffsetDateTime>,
            _end_date: Option<OffsetDateTime>,
        ) -> Result<crate::features::radar::application::provider::TickerHistory<'static>> {
            Err(anyhow!("synthetic fetch failure"))
        }
    }

    #[async_trait::async_trait]
    impl MarketDataProvider for PartialSuccessProvider {
        async fn fetch_history(
            &self,
            symbol: &str,
            _start_date: Option<OffsetDateTime>,
            _end_date: Option<OffsetDateTime>,
        ) -> Result<crate::features::radar::application::provider::TickerHistory<'static>> {
            match symbol {
                "AAA" => Ok(create_mock_history(symbol, 100.0, 60, 0.002)),
                _ => Err(anyhow!("synthetic partial fetch failure")),
            }
        }
    }

    #[async_trait::async_trait]
    impl MarketDataProvider for DateAwareProvider {
        async fn fetch_history(
            &self,
            symbol: &str,
            _start_date: Option<OffsetDateTime>,
            _end_date: Option<OffsetDateTime>,
        ) -> Result<crate::features::radar::application::provider::TickerHistory<'static>> {
            match symbol {
                "AAA" => Ok(create_mock_history_ending(
                    symbol,
                    100.0,
                    60,
                    0.002,
                    self.latest_date,
                )),
                _ => Err(anyhow!("synthetic partial fetch failure")),
            }
        }
    }

    #[async_trait::async_trait]
    impl MarketDataProvider for RateLimitedProvider {
        async fn fetch_history(
            &self,
            symbol: &str,
            _start_date: Option<OffsetDateTime>,
            _end_date: Option<OffsetDateTime>,
        ) -> Result<crate::features::radar::application::provider::TickerHistory<'static>> {
            match symbol {
                "AAA" => Ok(create_mock_history_ending(
                    symbol,
                    100.0,
                    60,
                    0.002,
                    self.latest_date,
                )),
                "BBB" => Err(anyhow!("provider returned HTTP 429")),
                _ => Err(anyhow!("synthetic fetch failure")),
            }
        }
    }

    fn create_mock_history(
        symbol: &str,
        start_price: f64,
        count: usize,
        daily_change: f64,
    ) -> TickerHistory<'static> {
        let start_date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let mut bars = Vec::with_capacity(count);
        let mut current_price = start_price;

        for i in 0..count {
            bars.push(DailyBar {
                date: start_date + chrono::Duration::days(i as i64),
                open: None,
                high: None,
                low: None,
                close: current_price,
                volume: Some(1000.0),
            });
            current_price *= 1.0 + daily_change;
        }

        TickerHistory {
            symbol: symbol.to_string(),
            bars: Cow::Owned(bars),
            total_trading_days: count,
            latest_quote_timestamp: Some(Utc::now().timestamp()),
        }
    }

    fn create_mock_history_ending(
        symbol: &str,
        start_price: f64,
        count: usize,
        daily_change: f64,
        latest_date: NaiveDate,
    ) -> TickerHistory<'static> {
        let start_date = latest_date - chrono::Duration::days((count - 1) as i64);
        let mut bars = Vec::with_capacity(count);
        let mut current_price = start_price;

        for i in 0..count {
            bars.push(DailyBar {
                date: start_date + chrono::Duration::days(i as i64),
                open: None,
                high: None,
                low: None,
                close: current_price,
                volume: Some(1000.0),
            });
            current_price *= 1.0 + daily_change;
        }

        TickerHistory {
            symbol: symbol.to_string(),
            bars: Cow::Owned(bars),
            total_trading_days: count,
            latest_quote_timestamp: Some(Utc::now().timestamp()),
        }
    }

    #[test]
    fn date_aware_mock_history_ends_on_requested_date() {
        let latest_date = NaiveDate::from_ymd_opt(2026, 7, 30).unwrap();
        let history = create_mock_history_ending("AAA", 100.0, 3, 0.002, latest_date);

        assert_eq!(history.bars.last().map(|bar| bar.date), Some(latest_date));
    }

    fn mock_config(save_to: &Path) -> AppConfig {
        AppConfig {
            version: 1,
            output: OutputConfig {
                timezone: "UTC".to_string(),
                format: "markdown".to_string(),
                save_to: save_to.display().to_string(),
                weight_kind: Some("equal".to_string()),
                language: Some(Language::ZhCn),
                compact_transition_evidence_in_no_trade: true,
            },
            telegram: None,
            futu: None,
            finnhub: None,
            fred: None,
            trading: None,
            provider: Some("yahoo".to_string()),
            rules: RulesConfig {
                trend: TrendConfig::default(),
                deviation_bands: BTreeMap::new(),
                actions: HashMap::new(),
                sizing_multipliers: None,
                core_assets: None,
                min_state_duration: None,
                inertia: None,
                trend_cohesion: None,
                breakout: None,
                market_state_engine: None,
                market_benchmarks: None,
            },
            watchlist: ["AAA", "BBB", "CCC", "DDD", "EEE", "FFF"]
                .into_iter()
                .map(|symbol| WatchlistEntry {
                    symbol: symbol.to_string(),
                    weight: None,
                    market: "US".to_string(),
                    owner_ma_days: 20,
                    leash_ma_days: 5,
                    deviation_basis: DeviationBasis::Owner,
                    enable: true,
                    trade_enabled: Some(false),
                    trade_amount: None,
                    event_tags: None,
                })
                .collect(),
            sec: None,
            research_attention: None,
            asset_thesis: None,
            macro_gravity: None,
            capital_absorption: None,
            capital_dynamics: None,
            gray_rhino_escalation: None,
            gray_rhino_provider_registry: None,
        }
    }

    fn assert_audit_snapshot(file_name: &str, actual: &str) {
        let snapshot_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("snapshots")
            .join(file_name);
        let expected = fs::read_to_string(&snapshot_path).unwrap_or_else(|_| {
            panic!(
                "Audit snapshot not found. expected path: {}",
                snapshot_path.display()
            )
        });
        assert_eq!(
            expected.trim_end(),
            actual.trim_end(),
            "audit snapshot mismatch: {}",
            snapshot_path.display()
        );
    }

    fn sample_audit_days() -> Vec<TransitionAuditDay> {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-21T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [],
                "opportunity_mode": {"from":"NoTradeCold","to":"NoTradeCold","changed": false},
                "scout_days_without_expansion": 0,
                "scout_abort_days": 3,
                "scout_reset_triggered": false,
                "breakout_active_count": 0
            }))
            .unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": true,
                    "added": ["DirectionalCohesion"],
                    "removed": [],
                    "persisting": ["StabilityThreshold","ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [
                    {"symbol":"GOOG","from_status":"NoBreakout","to_status":"EmergingBreakout","status_changed":true,"risk_changed":false}
                ],
                "opportunity_mode": {"from":"NoTradeCold","to":"NoTradeScout","changed": true},
                "scout_days_without_expansion": 1,
                "scout_abort_days": 3,
                "scout_reset_triggered": false,
                "breakout_active_count": 1,
                "trend_recognition": {
                    "state": "EarlyLeader",
                    "diffusion_score": 0.45,
                    "lag_state": true,
                    "single_asset_decay_day": 1,
                    "single_asset_decay_max": 2,
                    "conviction_score": 0.45,
                    "substantive": {
                        "capex_payoff_signal": false,
                        "earnings_validation": false,
                        "order_visibility": false,
                        "event_days_since": 0,
                        "records": [
                            {
                                "source": "OfficialIR",
                                "evidence_type": "EarningsValidation",
                                "confidence": 0.95,
                                "description": "Earnings beat expectations by 15%",
                                "event_date": "2026-04-22",
                                "symbol": "GOOG",
                                "source_url": "https://example.com/ir/goog"
                            },
                            {
                                "source": "NewsMedia",
                                "evidence_type": "CapexPayoff",
                                "confidence": 0.80,
                                "description": "Cloud division shows strong ROI",
                                "event_date": "2026-04-21",
                                "symbol": "GOOG",
                                "source_url": "https://news.example.com/goog-cloud"
                            },
                            {
                                "source": "OfficialIR",
                                "evidence_type": "EarningsValidation",
                                "confidence": 0.90,
                                "description": "Earnings beat expectations by 15%",
                                "event_date": "2026-04-21",
                                "symbol": "GOOG",
                                "source_url": "https://example.com/ir/goog-followup"
                            }
                        ]
                    }
                }
            }))
            .unwrap(),
        };
        vec![
            TransitionAuditDay {
                date: entry_day1.date,
                events: vec![entry_day1],
            },
            TransitionAuditDay {
                date: entry_day2.date,
                events: vec![entry_day2],
            },
        ]
    }

    #[test]
    fn evidence_collection_status_sidecar_is_loaded_for_run_status() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join(EVIDENCE_COLLECTION_STATUS_FILE),
            r#"{"status":"failed","reason":"network timeout"}"#,
        )
        .unwrap();

        let status = load_latest_evidence_collection_status(tmp.path());
        assert_eq!(
            status,
            DeliveryStatus::Failed {
                reason: "network timeout".to_string()
            }
        );
    }

    #[test]
    fn audit_daily_renders_evidence_collection_status() {
        let days = sample_audit_days();
        let report_zh = build_audit_daily_report_with_evidence_status(
            &days,
            1,
            14,
            Language::ZhCn,
            Some(&DeliveryStatus::Succeeded),
        );
        assert!(report_zh.contains("- 今日证据采集状态: 成功"));
        assert!(report_zh.contains("- 历史证据存量:"));

        let report_en = build_audit_daily_report_with_evidence_status(
            &days,
            1,
            14,
            Language::EnUs,
            Some(&DeliveryStatus::Failed {
                reason: "missing key".to_string(),
            }),
        );
        assert!(report_en.contains("- Today's evidence collection status: failed (missing key)"));
        assert!(report_en.contains("- Historical evidence stock:"));

        let report_ja = build_audit_daily_report_with_evidence_status(
            &days,
            1,
            14,
            Language::JaJp,
            Some(&DeliveryStatus::Skipped),
        );
        assert!(report_ja.contains("- 本日の証拠収集状態: スキップ"));
        assert!(report_ja.contains("- 履歴証拠ストック:"));
    }

    #[test]
    fn persists_normal_runs_and_skips_diagnostic_only_runs() {
        assert!(crate::features::radar::application::radar::should_persist_decision_history(3, 0));
        assert!(crate::features::radar::application::radar::should_persist_decision_history(3, 2));
        assert!(crate::features::radar::application::radar::should_persist_decision_history(1, 99));

        assert!(!crate::features::radar::application::radar::should_persist_decision_history(0, 5));
    }

    #[test]
    fn empty_fetch_set_does_not_trigger_diagnostic_skip() {
        assert!(crate::features::radar::application::radar::should_persist_decision_history(0, 0));
    }

    #[test]
    fn telegram_precheck_skips_when_disabled() {
        let cfg = TelegramConfig {
            enabled: false,
            bot_token: "token".to_string(),
            chat_id: "chat".to_string(),
        };
        assert!(matches!(
            telegram_delivery_precheck(Some(&cfg)),
            Err(DeliveryStatus::Skipped)
        ));
    }

    #[test]
    fn telegram_precheck_fails_when_credentials_missing() {
        let cfg = TelegramConfig {
            enabled: true,
            bot_token: "".to_string(),
            chat_id: "".to_string(),
        };
        assert!(matches!(
            telegram_delivery_precheck(Some(&cfg)),
            Err(DeliveryStatus::Failed { .. })
        ));
    }

    #[tokio::test]
    async fn full_fetch_failure_generates_report_without_persisting_history() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());
        let provider: Arc<dyn MarketDataProvider> = Arc::new(AlwaysFailProvider);

        run_pipeline(config, provider, ExecutionMode::Disabled)
            .await
            .unwrap();

        let report_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.extension().and_then(|extension| extension.to_str()) == Some("md")
                    && path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .and_then(|stem| NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok())
                        .is_some()
            })
            .expect("diagnostic markdown report should exist");
        let report_date = report_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("dated report file should have a UTF-8 stem");
        let run_status_path = tmp.path().join(format!("run_status_{}.json", report_date));
        let history_path = tmp.path().join("decision_history.jsonl");
        let daily_packet_path = tmp
            .path()
            .join(format!("decision_packet_{}.json", report_date));
        let execution_gate_log_path = tmp.path().join("execution_gate_log.jsonl");
        let portfolio_snapshot_path = tmp
            .path()
            .join(format!("portfolio_snapshot_{}.json", report_date));
        let account_snapshot_path = tmp
            .path()
            .join(format!("account_snapshot_{}.json", report_date));
        let data_quality_log_path = tmp.path().join("data_quality_log.jsonl");
        let weekly_metrics_path = tmp.path().join("weekly_state_metrics.json");
        let weekly_review_path = tmp.path().join("weekly_state_review_auto.md");

        assert!(
            run_status_path.exists(),
            "run status should still be persisted"
        );
        assert!(
            !history_path.exists(),
            "diagnostic-only run must not create decision history"
        );
        assert!(
            !daily_packet_path.exists(),
            "diagnostic-only run must not create a daily decision packet"
        );
        assert!(execution_gate_log_path.exists());
        assert!(portfolio_snapshot_path.exists());
        assert!(account_snapshot_path.exists());
        assert!(data_quality_log_path.exists());
        assert!(weekly_metrics_path.exists());
        assert!(weekly_review_path.exists());

        let report = std::fs::read_to_string(report_path).unwrap();
        assert!(report.contains("数据不可用"));
        assert!(report.contains("严重"));

        let gate_log = std::fs::read_to_string(execution_gate_log_path).unwrap();
        assert!(gate_log.contains("execution_gate_noop"));

        let quality_log = std::fs::read_to_string(data_quality_log_path).unwrap();
        assert!(quality_log.contains("CRITICAL"));
        let weekly_metrics = std::fs::read_to_string(weekly_metrics_path).unwrap();
        assert!(weekly_metrics.contains("DATA_UNAVAILABLE"));
        let weekly_metrics_json: serde_json::Value = serde_json::from_str(&weekly_metrics).unwrap();
        assert_eq!(
            weekly_metrics_json["weekly_totals"]["days"],
            serde_json::Value::from(1)
        );
        assert_eq!(
            weekly_metrics_json["daily_summaries"][0]["to_state"],
            serde_json::Value::String("DATA_UNAVAILABLE".to_string())
        );

        let run_status: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(run_status_path).unwrap()).unwrap();
        assert_eq!(
            run_status["state_machine"]["to_state"],
            serde_json::Value::String("DATA_UNAVAILABLE".to_string())
        );
    }

    #[tokio::test]
    async fn partial_fetch_failure_preserves_history_and_real_market_state() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join(EVIDENCE_COLLECTION_STATUS_FILE),
            r#"{"status":"succeeded","reason":null}"#,
        )
        .unwrap();
        let config = mock_config(tmp.path());
        let provider: Arc<dyn MarketDataProvider> = Arc::new(PartialSuccessProvider);

        run_pipeline(config, provider, ExecutionMode::Disabled)
            .await
            .unwrap();

        let history_path = tmp.path().join("decision_history.jsonl");
        let report_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.extension().and_then(|ext| ext.to_str()) == Some("md")
                    && path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .and_then(|stem| chrono::NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok())
                        .is_some()
            })
            .expect("daily report should exist for partial-failure runs");
        let run_status_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("run_status_") && name.ends_with(".json"))
                    .unwrap_or(false)
            })
            .expect("run status should exist for partial-failure runs");
        let daily_packet_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("decision_packet_") && name.ends_with(".json"))
                    .unwrap_or(false)
            })
            .expect("daily decision packet must still be produced for partial-failure runs");
        let execution_gate_log_path = tmp.path().join("execution_gate_log.jsonl");
        let portfolio_snapshot_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("portfolio_snapshot_") && name.ends_with(".json"))
                    .unwrap_or(false)
            })
            .expect("portfolio snapshot should exist");
        let account_snapshot_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("account_snapshot_") && name.ends_with(".json"))
                    .unwrap_or(false)
            })
            .expect("account snapshot should exist");
        let data_quality_log_path = tmp.path().join("data_quality_log.jsonl");
        let weekly_metrics_path = tmp.path().join("weekly_state_metrics.json");
        let weekly_review_path = tmp.path().join("weekly_state_review_auto.md");

        assert!(
            history_path.exists(),
            "real decisions must still persist when at least one symbol succeeded"
        );
        assert!(execution_gate_log_path.exists());
        assert!(portfolio_snapshot_path.exists());
        assert!(account_snapshot_path.exists());
        assert!(data_quality_log_path.exists());
        assert!(weekly_metrics_path.exists());
        assert!(weekly_review_path.exists());

        let report = std::fs::read_to_string(report_path).unwrap();
        assert!(report.contains("⚠️"));
        assert!(report.contains("警告"));
        assert!(report.contains("获取失败"));

        let daily_packet: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(daily_packet_path).unwrap()).unwrap();
        assert_eq!(daily_packet["assets"].as_array().map(Vec::len), Some(1));

        let run_status: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(run_status_path).unwrap()).unwrap();
        assert_eq!(
            run_status["evidence_collection"],
            serde_json::Value::String("succeeded".to_string())
        );
        assert_ne!(
            run_status["state_machine"]["to_state"],
            serde_json::Value::String("DATA_UNAVAILABLE".to_string())
        );

        let quality_log = std::fs::read_to_string(data_quality_log_path).unwrap();
        assert!(quality_log.contains("WARNING"));
        let weekly_metrics = std::fs::read_to_string(weekly_metrics_path).unwrap();
        assert!(weekly_metrics.contains("\"include_current_packet\": true"));
        assert!(weekly_metrics.contains("\"latest_context\""));
        assert!(weekly_metrics.contains("\"trend_breadth_mode\""));
        assert!(weekly_metrics.contains("\"market_cycle_position\""));
        assert!(weekly_metrics.contains("\"holding_efficiency\""));
        assert!(weekly_metrics.contains("\"macro_gravity\""));
        assert!(weekly_metrics.contains("\"capital_dynamics\""));
        assert!(weekly_metrics.contains("\"supply_layer\""));
        assert!(weekly_metrics.contains("\"flow_layer\""));
        assert!(weekly_metrics.contains("\"expectation_layer\""));
        assert!(weekly_metrics.contains("\"strategic_context\""));
        assert!(weekly_metrics.contains("\"weekly_totals\""));
        assert!(weekly_metrics.contains("\"daily_summaries\""));
        let weekly_metrics_json: serde_json::Value = serde_json::from_str(&weekly_metrics).unwrap();
        assert_eq!(
            weekly_metrics_json["weekly_totals"]["days"],
            serde_json::Value::from(1)
        );
        assert!(
            weekly_metrics_json["latest_context"]["capital_dynamics"]["supply_layer"].is_object()
        );
        assert!(weekly_metrics_json["latest_context"]["expectation_layer"].is_object());
        assert!(weekly_metrics_json["daily_summaries"][0]["to_state"] != serde_json::Value::Null);
        let weekly_review = std::fs::read_to_string(weekly_review_path).unwrap();
        assert!(weekly_review.contains("## 状态机周度汇总"));
        assert!(weekly_review.contains("## Capital Dynamics（供需观察）"));
        assert!(weekly_review.contains("### 6.1 Supply Layer（Capital Absorption）"));
        assert!(weekly_review.contains("### 6.2 Demand Layer（Flow Layer）"));
        assert!(weekly_review.contains("## Expectation Layer（市场预期观测）"));
        assert!(weekly_review.contains("## 日度状态机时间线"));
        assert!(weekly_review.contains("## 战略上下文快照"));
        assert!(weekly_review.contains("## 宏观引力快照"));
        assert!(weekly_review.contains("## 认知校准快照"));
        assert!(weekly_review.contains("边界: 仅为快照"));
        assert!(weekly_review.contains("不生成交易信号"));
    }

    #[tokio::test]
    async fn injected_pipeline_dates_preserve_cycle_and_append_migrated_history() {
        let tmp = tempdir().unwrap();
        let legacy_packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            ..Default::default()
        };
        fs::write(
            tmp.path().join("decision_packet_2026-07-28.json"),
            serde_json::to_string_pretty(&legacy_packet).unwrap(),
        )
        .unwrap();
        fs::write(
            tmp.path().join(EVIDENCE_COLLECTION_STATUS_FILE),
            r#"{"status":"succeeded","reason":null}"#,
        )
        .unwrap();

        let mut first_config = mock_config(tmp.path());
        first_config.watchlist.truncate(1);
        run_pipeline_for_report_date(
            first_config,
            Arc::new(DateAwareProvider {
                latest_date: NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
            }),
            ExecutionMode::Disabled,
            NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
        )
        .await
        .unwrap();
        let persistence = PersistenceLayer::new(tmp.path());
        let first_state = persistence
            .load_observation_history_state()
            .unwrap()
            .expect("first injected run should persist history state");

        drop(persistence);

        let mut second_config = mock_config(tmp.path());
        second_config.watchlist.truncate(1);
        run_pipeline_for_report_date(
            second_config,
            Arc::new(DateAwareProvider {
                latest_date: NaiveDate::from_ymd_opt(2026, 7, 30).unwrap(),
            }),
            ExecutionMode::Disabled,
            NaiveDate::from_ymd_opt(2026, 7, 30).unwrap(),
        )
        .await
        .unwrap();
        let persistence = PersistenceLayer::new(tmp.path());
        let second_state = persistence
            .load_observation_history_state()
            .unwrap()
            .expect("second injected run should preserve history state");
        let snapshots = persistence.load_trading_day_snapshots().unwrap();

        assert_eq!(second_state.cycle_id, first_state.cycle_id);
        assert!(
            second_state.count > first_state.count,
            "first={first_state:?} second={second_state:?} snapshots={snapshots:?} files={:?}",
            fs::read_dir(tmp.path())
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.file_name())
                .collect::<Vec<_>>()
        );
        assert!(second_state.last_market_date > first_state.last_market_date);
        assert!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.cycle_id == second_state.cycle_id)
                .count()
                >= second_state.count
        );
        assert!(snapshots
            .iter()
            .filter(|snapshot| {
                snapshot.cycle_id == second_state.cycle_id
                    && snapshot.market_date > NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()
            })
            .all(|snapshot| snapshot.breadth_classification.is_some()));
    }

    #[tokio::test]
    async fn rate_limited_symbol_is_reported_and_persisted_as_unavailable_price_volume() {
        let tmp = tempdir().unwrap();
        let mut config = mock_config(tmp.path());
        config.watchlist.truncate(2);
        let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();

        run_pipeline_for_report_date(
            config,
            Arc::new(RateLimitedProvider { latest_date: date }),
            ExecutionMode::Disabled,
            date,
        )
        .await
        .unwrap();

        let report = fs::read_to_string(tmp.path().join("2026-07-29.md")).unwrap();
        assert!(report.contains("### BBB"));
        assert!(report.contains("Structure: UNAVAILABLE"));
        assert!(report.contains("Volume Data Quality: DEGRADED"));
        assert!(report.contains("Decision Weight: 0%"));
        assert!(!report.contains("Buy"));
        assert!(!report.contains("Sell immediately"));

        let observations = PersistenceLayer::new(tmp.path())
            .load_price_volume_observations()
            .unwrap();
        let limited = observations
            .iter()
            .find(|record| record.symbol == "BBB")
            .expect("429 symbol must be persisted as an observation");
        assert_eq!(
            limited.assessment.structure,
            crate::features::radar::domain::price_volume_structure::PriceVolumeStructure::Unavailable
        );
        assert_eq!(
            limited.assessment.quality,
            crate::features::radar::domain::price_volume_structure::VolumeDataQuality::Degraded
        );
        assert_eq!(limited.assessment.boundary.decision_weight_percent, 0);
        assert!(!limited.assessment.boundary.trade_signal);
    }

    #[tokio::test]
    async fn jsonl_only_legacy_history_is_migrated_during_startup() {
        let tmp = tempdir().unwrap();
        let legacy_packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 7, 28).unwrap(),
            ..Default::default()
        };
        fs::write(
            tmp.path().join("decision_history.jsonl"),
            format!("{}\n", serde_json::to_string(&legacy_packet).unwrap()),
        )
        .unwrap();
        fs::write(
            tmp.path().join(EVIDENCE_COLLECTION_STATUS_FILE),
            r#"{"status":"succeeded","reason":null}"#,
        )
        .unwrap();

        let mut config = mock_config(tmp.path());
        config.watchlist.truncate(1);
        run_pipeline_for_report_date(
            config,
            Arc::new(DateAwareProvider {
                latest_date: NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
            }),
            ExecutionMode::Disabled,
            NaiveDate::from_ymd_opt(2026, 7, 29).unwrap(),
        )
        .await
        .unwrap();

        let persistence = PersistenceLayer::new(tmp.path());
        let state = persistence
            .load_observation_history_state()
            .unwrap()
            .expect("startup migration should persist state from JSONL-only history");
        let snapshots = persistence.load_trading_day_snapshots().unwrap();

        assert!(state.count >= 2);
        assert!(snapshots
            .iter()
            .any(|snapshot| snapshot.market_date == NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()));
        assert!(snapshots
            .iter()
            .any(|snapshot| snapshot.market_date == NaiveDate::from_ymd_opt(2026, 7, 29).unwrap()));
    }

    #[tokio::test]
    async fn pipeline_propagates_corrupt_observation_history_state() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("observation_history_state.json"),
            "{\"count\":",
        )
        .unwrap();

        let error = run_pipeline_for_report_date(
            mock_config(tmp.path()),
            Arc::new(AlwaysFailProvider),
            ExecutionMode::Disabled,
            NaiveDate::from_ymd_opt(2026, 7, 30).unwrap(),
        )
        .await
        .expect_err("corrupt observation history state must fail closed");

        assert!(error
            .to_string()
            .contains("deserialize observation history state"));
    }

    #[test]
    fn persistence_loader_rejects_corrupt_observation_history_state() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("observation_history_state.json"),
            "{\"count\":",
        )
        .unwrap();

        let error = PersistenceLayer::new(tmp.path())
            .load_observation_history_state()
            .expect_err("corrupt state must be rejected by the persistence loader");

        assert!(error
            .to_string()
            .contains("deserialize observation history state"));
    }

    #[test]
    fn parse_transition_audit_entry_supports_legacy_and_v2_lines() {
        let legacy = serde_json::json!({
            "timestamp": "2026-04-22T09:00:00+09:00",
            "log": {
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }
        });
        let v2 = serde_json::json!({
            "schema_version": 2,
            "event_type": "state_transition",
            "timestamp": "2026-04-23T09:00:00+09:00",
            "date": "2026-04-23",
            "transition": {
                "no_trade_persists": false,
                "market_state": {"from":"IGNITION","to":"EARLY_CONFIRMATION","changed": true},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": true,
                    "unmet_conditions_changed": true,
                    "added": [],
                    "removed": ["StabilityThreshold","ContinuityThreshold"],
                    "persisting": []
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Forming","changed": true},
                "trend_cohesion_topology": {"from":"NoLeader","to":"SingleLeader","changed": true},
                "breakout_changes": []
            }
        });

        let legacy_entry = parse_transition_audit_entry(&legacy.to_string(), Language::ZhCn)
            .unwrap()
            .expect("legacy entry");
        let v2_entry = parse_transition_audit_entry(&v2.to_string(), Language::ZhCn)
            .unwrap()
            .expect("v2 entry");

        assert_eq!(
            legacy_entry.date,
            NaiveDate::from_ymd_opt(2026, 4, 22).unwrap()
        );
        assert!(!legacy_entry.log.trend_cohesion_gate.to);
        assert_eq!(v2_entry.date, NaiveDate::from_ymd_opt(2026, 4, 23).unwrap());
        assert!(v2_entry.log.trend_cohesion_gate.to);
    }

    #[test]
    fn build_audit_daily_report_emits_five_fixed_sections() {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-21T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": true,
                    "added": ["DirectionalCohesion"],
                    "removed": [],
                    "persisting": ["StabilityThreshold","ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [
                    {"symbol":"GOOG","from_status":"NoBreakout","to_status":"EmergingBreakout","status_changed":true,"risk_changed":false}
                ]
            }))
            .unwrap(),
        };
        let days = vec![
            TransitionAuditDay {
                date: entry_day1.date,
                events: vec![entry_day1],
            },
            TransitionAuditDay {
                date: entry_day2.date,
                events: vec![entry_day2],
            },
        ];
        let report = build_audit_daily_report(&days, 1, 14, Language::ZhCn);
        assert!(report.contains("1. 门槛摘要"));
        assert!(report.contains("2. 状态变化摘要"));
        assert!(report.contains("3. 突破摘要"));
        assert!(report.contains("4. 实体证据摘要"));
        assert!(report.contains("5. 连续段统计"));
        assert!(report.contains("6. 审计一句话"));
        assert!(report.contains("口径: 连续段按日志连续计算（周末自动衔接）"));
        assert!(report.contains("当前状态：NO TRADE；主因："));
        assert!(report.contains("；今日突破：GOOG（新增）；主线状态：未形成。"));
    }

    #[test]
    fn build_audit_daily_report_localizes_to_en_us() {
        let entry = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": []
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [
                    {"symbol":"GOOG","from_status":"NoBreakout","to_status":"EmergingBreakout","status_changed":true,"risk_changed":false}
                ]
            }))
            .unwrap(),
        };
        let days = vec![TransitionAuditDay {
            date: entry.date,
            events: vec![entry],
        }];
        let report = build_audit_daily_report(&days, 0, 14, Language::EnUs);
        assert!(report.contains("1. Gate Summary"));
        assert!(report.contains("2. Transition Summary"));
        assert!(report.contains("3. Breakout Summary"));
        assert!(report.contains("4. Substantive Evidence"));
        assert!(report.contains("5. Streak Metrics"));
        assert!(report.contains("6. Audit One-liner"));
        assert!(report.contains("Methodology: streaks are calculated by log continuity"));
        assert!(report.contains("Current state: NO TRADE; primary blockers:"));
        assert!(report.contains("today's breakout: GOOG (new); mainline status: Not formed."));
    }

    #[test]
    fn build_audit_daily_report_localizes_to_ja_jp() {
        let entry = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": []
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [
                    {"symbol":"GOOG","from_status":"NoBreakout","to_status":"EmergingBreakout","status_changed":true,"risk_changed":false}
                ]
            }))
            .unwrap(),
        };
        let days = vec![TransitionAuditDay {
            date: entry.date,
            events: vec![entry],
        }];
        let report = build_audit_daily_report(&days, 0, 14, Language::JaJp);
        assert!(report.contains("1. ゲートサマリー"));
        assert!(report.contains("2. 状態遷移サマリー"));
        assert!(report.contains("3. ブレイクアウトサマリー"));
        assert!(report.contains("4. 実体的な証拠サマリー"));
        assert!(report.contains("5. 連続区間統計"));
        assert!(report.contains("6. 監査ワンライン要約"));
        assert!(report.contains("口径: 連続区間はログ連続で計算（週末は自動連結）"));
        assert!(report.contains("現在の状態：NO TRADE；主因："));
        assert!(report.contains("本日のブレイクアウト：GOOG（新規）；主線状態：未形成。"));
    }

    #[test]
    fn audit_daily_snapshot_zh_cn() {
        let report = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::ZhCn);
        assert_audit_snapshot("audit_daily_zh_cn.txt", &report);
    }

    #[test]
    fn audit_daily_snapshot_en_us() {
        let report = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::EnUs);
        assert_audit_snapshot("audit_daily_en_us.txt", &report);
    }

    #[test]
    fn audit_daily_snapshot_ja_jp() {
        let report = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::JaJp);
        assert_audit_snapshot("audit_daily_ja_jp.txt", &report);
    }

    #[test]
    fn audit_daily_contract_contains_one_liner_and_methodology_lines() {
        let report_zh = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::ZhCn);
        assert!(report_zh.contains("4. 实体证据摘要"));
        assert!(report_zh.contains("[GOOG] [2026-04-22] [EarningsValidation] (conf:0.95) [OfficialIR] 原始证据说明未提供中文版本 (https://example.com/ir/goog)"));
        assert!(report_zh.contains("[GOOG] [2026-04-21] [CapexPayoff] (conf:0.80) [NewsMedia] 原始证据说明未提供中文版本 (https://news.example.com/goog-cloud)"));
        assert!(report_zh.contains("[GOOG] [2026-04-21] [EarningsValidation] (conf:0.90) [OfficialIR] 原始证据说明未提供中文版本 (https://example.com/ir/goog-followup)"));
        assert!(report_zh.contains("6. 审计一句话"));
        assert!(report_zh.contains("当前状态：NO TRADE；主因："));
        assert!(report_zh.contains("口径: 连续段按日志连续计算（周末自动衔接）"));

        let report_en = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::EnUs);
        assert!(report_en.contains("4. Substantive Evidence"));
        assert!(report_en.contains("[GOOG] [2026-04-22] [EarningsValidation] (conf:0.95) [OfficialIR] Earnings beat expectations by 15% (https://example.com/ir/goog)"));
        assert!(report_en.contains("[GOOG] [2026-04-21] [CapexPayoff] (conf:0.80) [NewsMedia] Cloud division shows strong ROI (https://news.example.com/goog-cloud)"));
        assert!(report_en.contains("[GOOG] [2026-04-21] [EarningsValidation] (conf:0.90) [OfficialIR] Earnings beat expectations by 15% (https://example.com/ir/goog-followup)"));
        assert!(report_en.contains("6. Audit One-liner"));
        assert!(report_en.contains("Current state: NO TRADE; primary blockers:"));
        assert!(report_en.contains(
            "Methodology: streaks are calculated by log continuity (weekends auto-bridged)"
        ));

        let report_ja = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::JaJp);
        assert!(report_ja.contains("4. 実体的な証拠サマリー"));
        assert!(report_ja.contains("[GOOG] [2026-04-22] [EarningsValidation] (conf:0.95) [OfficialIR] 元の証拠説明は日本語で未提供 (https://example.com/ir/goog)"));
        assert!(report_ja.contains("[GOOG] [2026-04-21] [CapexPayoff] (conf:0.80) [NewsMedia] 元の証拠説明は日本語で未提供 (https://news.example.com/goog-cloud)"));
        assert!(report_ja.contains("[GOOG] [2026-04-21] [EarningsValidation] (conf:0.90) [OfficialIR] 元の証拠説明は日本語で未提供 (https://example.com/ir/goog-followup)"));
        assert!(report_ja.contains("6. 監査ワンライン要約"));
        assert!(report_ja.contains("現在の状態：NO TRADE；主因："));
        assert!(report_ja.contains("口径: 連続区間はログ連続で計算（週末は自動連結）"));
    }

    #[test]
    fn audit_daily_excludes_fixture_evidence_and_marks_historical_snapshot_risk() {
        let mut days = sample_audit_days();
        let records = &mut days[1].events[0]
            .log
            .trend_recognition
            .as_mut()
            .unwrap()
            .substantive
            .as_mut()
            .unwrap()
            .records;
        records.push(
            crate::features::shared::domain::evidence::AutomatedEvidenceRecord::new(
                crate::features::shared::domain::evidence::EvidenceSourceType::OfficialIR,
                crate::features::shared::domain::evidence::EvidenceType::CapexPayoff,
                0.8,
                "Detected CAPEX keywords in tests/fixtures/evidence/goog.html".to_string(),
                "2026-04-22".to_string(),
                Some("GOOG".to_string()),
                Some("file://tests/fixtures/evidence/goog.html".to_string()),
                "FIXTURE".to_string(),
            ),
        );

        let report = build_audit_daily_report(&days, 1, 14, Language::ZhCn);
        assert!(!report.contains("tests/fixtures"));
        assert!(!report.contains("file://"));
        assert!(report.contains("已排除非生产来源证据: 1"));
        assert!(report.contains("历史确信度快照可能包含该来源"));
    }

    #[test]
    fn consecutive_streak_treats_trading_day_sequence_as_continuous() {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 17).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-17T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": [],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-20T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": [],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let days = vec![
            TransitionAuditDay {
                date: entry_day1.date,
                events: vec![entry_day1],
            },
            TransitionAuditDay {
                date: entry_day2.date,
                events: vec![entry_day2],
            },
        ];
        let streak = consecutive_streak(&days, 1, |log| !log.trend_cohesion_gate.to);
        assert_eq!(streak, 2);
    }

    #[test]
    fn ready_audit_sentence_uses_no_primary_blocker() {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-21T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": false,
                "market_state": {"from":"IGNITION","to":"EARLY_CONFIRMATION","changed": true},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": true,
                    "unmet_conditions_changed": true,
                    "added": [],
                    "removed": ["StabilityThreshold", "ContinuityThreshold"],
                    "persisting": []
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Forming","changed": true},
                "trend_cohesion_topology": {"from":"NoLeader","to":"SingleLeader","changed": true},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let days = vec![
            TransitionAuditDay {
                date: entry_day1.date,
                events: vec![entry_day1],
            },
            TransitionAuditDay {
                date: entry_day2.date,
                events: vec![entry_day2],
            },
        ];
        let report = build_audit_daily_report(&days, 1, 14, Language::ZhCn);
        assert!(report.contains("当前状态：READY；主因：无；"));
    }

    #[test]
    fn breakout_summary_keeps_intraday_event_history() {
        let morning = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {"from": false, "to": false, "unmet_conditions_changed": false, "added": [], "removed": [], "persisting": []},
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [
                    {"symbol":"GOOG","from_status":"NoBreakout","to_status":"EmergingBreakout","status_changed":true,"risk_changed":false}
                ]
            })).unwrap(),
        };
        let close = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T16:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {"from": false, "to": false, "unmet_conditions_changed": false, "added": [], "removed": [], "persisting": []},
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            })).unwrap(),
        };

        let day = TransitionAuditDay {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            events: vec![morning, close],
        };
        let report = build_audit_daily_report(&[day], 0, 14, Language::ZhCn);
        assert!(report.contains("新增突破: GOOG"));
        assert!(report.contains("今日突破：GOOG（新增）"));
    }

    #[test]
    fn consecutive_streak_breaks_on_missing_weekday_gap() {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-20T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {"from": false, "to": false, "unmet_conditions_changed": false, "added": [], "removed": [], "persisting": []},
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            })).unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {"from": false, "to": false, "unmet_conditions_changed": false, "added": [], "removed": [], "persisting": []},
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            })).unwrap(),
        };
        let days = vec![
            TransitionAuditDay {
                date: entry_day1.date,
                events: vec![entry_day1],
            },
            TransitionAuditDay {
                date: entry_day2.date,
                events: vec![entry_day2],
            },
        ];
        let streak = consecutive_streak(&days, 1, |log| !log.trend_cohesion_gate.to);
        assert_eq!(streak, 1);
    }
}

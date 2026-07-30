use anyhow::{anyhow, Context, Result};

use chrono::NaiveDate;

use crate::config;
use crate::features::evidence::acl::evidence_store_factory::{
    build_batch_evidence_fetcher_adapter, build_evidence_extractor_adapter,
    build_evidence_store_adapter, build_url_evidence_fetcher_adapter,
};
use crate::features::evidence::application::evidence::{
    ingest_manual_evidence, ManualEvidenceIngestionRequest,
};
use crate::features::evidence::application::evidence_ingestion::{
    collect_evidence_batch, collect_evidence_from_source, BatchCollectEvidenceRequest,
    BatchEvidenceTarget, CollectEvidenceRequest,
};
use crate::features::radar::acl::market_data_provider_factory::{
    build_configured_market_data_provider, MarketDataProviderKind as ProviderType,
};
use crate::features::radar::domain::trend_cohesion::EvidenceSourceType;
use crate::features::radar::interface::audit_daily_report::{
    audit_daily_usage, audit_empty_log_message, audit_error_parse_date,
    build_audit_daily_report_with_formal_baseline, build_daily_calibration_context,
    load_transition_audit_days, resolve_audit_daily_formal_baseline, resolve_target_index,
};
use crate::features::radar::interface::radar_pipeline_runner::run_pipeline;
use crate::features::research::interface::cli_command_handler::{
    run_asset_thesis_command, run_gray_rhino_escalation_command,
    run_official_calendar_smoke_command, run_research_attention_command,
};
use crate::features::research::interface::cognitive_reports::{
    build_daily_calibration_report_from_context, enabled_asset_thesis_count,
    enabled_research_attention_count,
};
use crate::features::research::interface::gray_rhino_cli_handler::{
    run_collect_gray_rhino_backfill, run_collect_gray_rhino_category_source,
    run_collect_gray_rhino_dependency, run_collect_gray_rhino_governance,
    run_collect_gray_rhino_sources, run_discover_gray_rhino, run_ingest_gray_rhino_dependency,
    run_ingest_gray_rhino_governance, run_ingest_gray_rhino_institutional,
    run_ingest_gray_rhino_redundancy,
};
use crate::features::shared::acl::notification_factory::{
    load_run_evidence_collection_status, send_required_telegram_notification,
};
use crate::features::shared::interface::cli_args::{
    cli_usage, parse_cli_options, CliCommand, CliProviderKind,
};
use crate::features::shared::interface::i18n::Language;

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
                &app_config,
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
            let report = build_daily_calibration_report(
                &app_config,
                options.audit_date_arg.as_deref(),
                options.audit_days,
                audit_language,
            )
            .await?;
            println!("{}", report);
            if options.research_notify {
                let telegram_report =
                    build_daily_calibration_telegram_digest(&report, audit_language);
                send_required_telegram_notification(
                    app_config.telegram.as_ref(),
                    &telegram_report,
                    "daily-calibration",
                )
                .await?;
            }
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
        CliCommand::IngestEvidence => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }

            let retention_days = app_config
                .get_parsed_rules()
                .market_state_engine
                .evidence_retention_days as i64;
            let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
            let store = build_evidence_store_adapter(&save_dir);
            let outcome = ingest_manual_evidence(
                &store,
                ManualEvidenceIngestionRequest {
                    evidence_type: options.evidence_type_str.clone(),
                    confidence: options.evidence_confidence,
                    description: options.evidence_description.clone(),
                    event_date: options.evidence_date_arg.clone(),
                    symbol: options.evidence_symbol.clone(),
                    source_url: options.evidence_url.clone(),
                    fallback_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
                    retention_days: Some(retention_days),
                },
            )?;
            if outcome.saved_count > 0 {
                println!(
                    "{}",
                    evidence_collection_success_message(audit_language, outcome.saved_count)
                );
            } else {
                println!("{}", evidence_collection_duplicate_message(audit_language));
            }
        }
        CliCommand::IngestEvidenceUrl => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            let symbol = options
                .evidence_symbol
                .clone()
                .ok_or_else(|| anyhow!("--symbol is required"))?;
            let url = options
                .evidence_url
                .clone()
                .ok_or_else(|| anyhow!("--url is required"))?;

            let st = match options.evidence_source_type_str.as_str() {
                "official" => EvidenceSourceType::OfficialIR,
                "news" => EvidenceSourceType::NewsMedia,
                _ => {
                    return Err(anyhow!(
                        "Invalid source type: {}. Use 'official' or 'news'",
                        options.evidence_source_type_str
                    ))
                }
            };

            let fetcher = build_url_evidence_fetcher_adapter(&app_config, &url)?;

            let extractor = build_evidence_extractor_adapter();
            let retention_days = app_config
                .get_parsed_rules()
                .market_state_engine
                .evidence_retention_days as i64;
            let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
            let store = build_evidence_store_adapter(&save_dir);
            let repository = if options.evidence_dry_run {
                None
            } else {
                Some(&store as &dyn crate::features::evidence::application::evidence::EvidenceRepository)
            };
            let outcome = collect_evidence_from_source(
                fetcher.as_ref(),
                extractor.as_ref(),
                repository,
                CollectEvidenceRequest {
                    url: url.clone(),
                    symbol: symbol.clone(),
                    source_type: st,
                    days: options.evidence_days,
                    persist: !options.evidence_dry_run,
                    retention_days: Some(retention_days),
                },
            )
            .await
            .context("Failed to collect evidence from source")?;

            if options.evidence_dry_run {
                println!("{}", evidence_dry_run_title(audit_language));
                println!("{}: {}", evidence_dry_run_source_label(audit_language), url);
                println!(
                    "{}: {}",
                    evidence_dry_run_symbol_label(audit_language),
                    symbol
                );
                if outcome.records.is_empty() {
                    println!("{}", evidence_dry_run_empty_notice(audit_language));
                }
                for (i, r) in outcome.records.iter().enumerate() {
                    println!(
                        "[{}] {}: {:?}, {}: {:.2}, {}: {}",
                        i + 1,
                        evidence_dry_run_type_label(audit_language),
                        r.evidence_type,
                        evidence_dry_run_confidence_label(audit_language),
                        r.confidence,
                        evidence_dry_run_date_label(audit_language),
                        r.event_date
                    );
                    println!(
                        "    {}: {}",
                        evidence_dry_run_desc_label(audit_language),
                        r.description
                    );
                    if let Some(ref url) = r.source_url {
                        println!("    URL:  {}", url);
                    }
                }
                return Ok(());
            }

            if outcome.saved_count > 0 {
                println!(
                    "{}",
                    evidence_collection_success_message(audit_language, outcome.saved_count)
                );
            } else {
                println!("{}", evidence_collection_duplicate_message(audit_language));
            }
        }
        CliCommand::CollectEvidence => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            if options.evidence_symbols.is_empty() {
                return Err(anyhow!("--symbols is required (comma separated)"));
            }

            println!("{}", evidence_batch_title(audit_language));
            println!(
                "{}: {:?}",
                evidence_batch_symbols_title(audit_language),
                options.evidence_symbols
            );
            println!(
                "{}:  {} days",
                evidence_batch_window_title(audit_language),
                options.evidence_days
            );

            let fetcher = build_batch_evidence_fetcher_adapter(
                &app_config,
                &options.evidence_source_provider,
                options.evidence_dry_run,
            )?;

            let extractor = build_evidence_extractor_adapter();
            let targets = options
                .evidence_symbols
                .iter()
                .map(|symbol| {
                    println!(
                        "{} {}...",
                        evidence_batch_fetching_label(audit_language),
                        symbol
                    );
                    // Dry-run で Key がない場合は symbol 自身をファイル名として FixtureFetcher に探させる
                    let url = if app_config.finnhub.is_none() && options.evidence_dry_run {
                        symbol.clone()
                    } else {
                        "finnhub".to_string()
                    };
                    BatchEvidenceTarget {
                        symbol: symbol.clone(),
                        url,
                    }
                })
                .collect::<Vec<_>>();
            let retention_days = app_config
                .get_parsed_rules()
                .market_state_engine
                .evidence_retention_days as i64;
            let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
            let store = build_evidence_store_adapter(&save_dir);
            let repository = if options.evidence_dry_run {
                None
            } else {
                Some(&store as &dyn crate::features::evidence::application::evidence::EvidenceRepository)
            };
            let batch_outcome = collect_evidence_batch(
                fetcher.as_ref(),
                extractor.as_ref(),
                repository,
                BatchCollectEvidenceRequest {
                    targets,
                    source_type: if options.evidence_source_provider == "sec" {
                        EvidenceSourceType::OfficialIR
                    } else {
                        EvidenceSourceType::NewsMedia
                    },
                    days: options.evidence_days,
                    persist: !options.evidence_dry_run,
                    retention_days: Some(retention_days),
                },
            )
            .await?;

            for symbol in &options.evidence_symbols {
                let record_count = batch_outcome
                    .records
                    .iter()
                    .filter(|r| r.symbol.as_deref() == Some(symbol.as_str()))
                    .count();
                if batch_outcome
                    .failures
                    .iter()
                    .any(|failure| failure.symbol == *symbol)
                {
                    continue;
                }
                println!(
                    "  -> {} {}",
                    evidence_batch_extracted_label(audit_language),
                    record_count
                );
            }
            for failure in &batch_outcome.failures {
                eprintln!(
                    "{} for {}: {}",
                    evidence_batch_fetch_error_prefix(audit_language),
                    failure.symbol,
                    failure.error
                );
            }

            println!("{}", evidence_batch_summary_title(audit_language));
            println!(
                "{}: {} {}",
                evidence_batch_processed_label(audit_language),
                options.evidence_symbols.len(),
                evidence_batch_symbols_label(audit_language)
            );
            println!(
                "{}:   {} {}",
                evidence_batch_success_label(audit_language),
                batch_outcome.success_count,
                evidence_batch_symbols_label(audit_language)
            );
            println!(
                "{}:   {} {}",
                evidence_batch_failure_label(audit_language),
                batch_outcome.failure_count,
                evidence_batch_symbols_label(audit_language)
            );

            if options.evidence_dry_run {
                println!("{}", evidence_batch_dry_run_summary_title(audit_language));
                if batch_outcome.records.is_empty() {
                    println!("{}", evidence_batch_no_evidence_notice(audit_language));
                }
                for (i, r) in batch_outcome.records.iter().enumerate() {
                    let date_str = r.event_date.as_str();
                    println!(
                        "[{}] {}: {:?} ({:.2}) | {}: {}",
                        i + 1,
                        r.symbol.as_deref().unwrap_or("GLOBAL"),
                        r.evidence_type,
                        r.confidence,
                        evidence_batch_date_label(audit_language),
                        date_str
                    );
                    println!(
                        "    {}: {}",
                        evidence_batch_desc_label(audit_language),
                        r.description
                    );
                    if let Some(ref url) = r.source_url {
                        println!("    URL:  {}", url);
                    }
                }
                return Ok(());
            }

            println!(
                "{}",
                evidence_batch_success_message(audit_language, batch_outcome.saved_count)
            );
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
            run_pipeline(app_config, provider, mode).await?;
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

fn evidence_dry_run_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 干运行：提取到的证据 ---",
        Language::EnUs => "--- Dry Run: Extracted Evidence ---",
        Language::JaJp => "--- ドライラン: 抽出された証拠 ---",
    }
}

fn evidence_dry_run_source_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源",
        Language::EnUs => "Source",
        Language::JaJp => "ソース",
    }
}

fn evidence_dry_run_symbol_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "标的",
        Language::EnUs => "Symbol",
        Language::JaJp => "シンボル",
    }
}

fn evidence_dry_run_empty_notice(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未发现证据。",
        Language::EnUs => "No evidence found.",
        Language::JaJp => "証拠は見つかりませんでした。",
    }
}

fn evidence_dry_run_type_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "类型",
        Language::EnUs => "Type",
        Language::JaJp => "種別",
    }
}

fn evidence_dry_run_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "置信度",
        Language::EnUs => "Confidence",
        Language::JaJp => "確信度",
    }
}

fn evidence_dry_run_date_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "日期",
        Language::EnUs => "Date",
        Language::JaJp => "日付",
    }
}

fn evidence_dry_run_desc_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "说明",
        Language::EnUs => "Desc",
        Language::JaJp => "説明",
    }
}

fn evidence_collection_success_message(language: Language, count: usize) -> String {
    match language {
        Language::ZhCn => format!("成功摄取 {} 条自动证据记录。", count),
        Language::EnUs => format!(
            "Successfully ingested {} automated evidence records.",
            count
        ),
        Language::JaJp => format!("自動証拠レコードを {} 件取り込みました。", count),
    }
}

fn evidence_collection_duplicate_message(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "证据记录已存在（已去重）。",
        Language::EnUs => "Evidence record already exists (deduplicated).",
        Language::JaJp => "証拠レコードは既に存在します（重複除去済み）。",
    }
}

fn evidence_batch_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 批量证据采集 ---",
        Language::EnUs => "--- Batch Evidence Collection ---",
        Language::JaJp => "--- バッチ証拠収集 ---",
    }
}

fn evidence_batch_fetching_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "正在获取",
        Language::EnUs => "Fetching for",
        Language::JaJp => "取得中",
    }
}

fn evidence_batch_symbols_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "标的",
        Language::EnUs => "Symbols",
        Language::JaJp => "銘柄",
    }
}

fn evidence_batch_window_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "窗口",
        Language::EnUs => "Window",
        Language::JaJp => "期間",
    }
}

fn evidence_batch_extracted_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已提取",
        Language::EnUs => "Extracted",
        Language::JaJp => "抽出済み",
    }
}

fn evidence_batch_fetch_error_prefix(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "  [错误] 获取失败",
        Language::EnUs => "  [ERROR] Failed to fetch",
        Language::JaJp => "  [エラー] 取得失敗",
    }
}

fn evidence_batch_processed_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已处理",
        Language::EnUs => "Processed",
        Language::JaJp => "処理済み",
    }
}

fn evidence_batch_success_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "成功",
        Language::EnUs => "Success",
        Language::JaJp => "成功",
    }
}

fn evidence_batch_failure_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "失败",
        Language::EnUs => "Failure",
        Language::JaJp => "失敗",
    }
}

fn evidence_batch_symbols_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "个标的",
        Language::EnUs => "symbols",
        Language::JaJp => "銘柄",
    }
}

fn evidence_batch_no_evidence_notice(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "批次中未发现证据。",
        Language::EnUs => "No evidence found in batch.",
        Language::JaJp => "バッチ内で証拠は見つかりませんでした。",
    }
}

fn evidence_batch_summary_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 批量采集摘要 ---",
        Language::EnUs => "--- Batch Collection Summary ---",
        Language::JaJp => "--- バッチ収集サマリー ---",
    }
}

fn evidence_batch_dry_run_summary_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 干运行：提取到的证据摘要 ---",
        Language::EnUs => "--- Dry Run: Extracted Evidence Summary ---",
        Language::JaJp => "--- ドライラン: 抽出された証拠サマリー ---",
    }
}

fn evidence_batch_date_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "日期",
        Language::EnUs => "Date",
        Language::JaJp => "日付",
    }
}

fn evidence_batch_desc_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "说明",
        Language::EnUs => "Desc",
        Language::JaJp => "説明",
    }
}

fn evidence_batch_success_message(language: Language, count: usize) -> String {
    match language {
        Language::ZhCn => format!("成功摄取 {} 条批量证据记录。", count),
        Language::EnUs => format!(
            "Successfully ingested {} batch evidence records to store.",
            count
        ),
        Language::JaJp => format!("バッチ証拠レコードを {} 件ストアへ取り込みました。", count),
    }
}

fn run_audit_daily(
    app_config: &config::AppConfig,
    target_date_arg: Option<&str>,
    window_days: usize,
    language: Language,
) -> Result<()> {
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let path = save_dir.join("state_transitions.jsonl");
    let days = load_transition_audit_days(&path, language)?;
    if days.is_empty() {
        println!("{}", audit_empty_log_message(language));
        return Ok(());
    }

    let target_date = match target_date_arg {
        Some(raw) => Some(
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .with_context(|| format!("{}: {}", audit_error_parse_date(language), raw))?,
        ),
        None => None,
    };

    let target_idx = resolve_target_index(&days, target_date, language)?;
    let evidence_collection_status =
        load_run_evidence_collection_status(&save_dir, days[target_idx].date)
            .unwrap_or(crate::features::shared::application::run_status::DeliveryStatus::Skipped);
    let formal_baseline =
        resolve_audit_daily_formal_baseline(&save_dir, days[target_idx].date).unwrap_or(None);
    let report = build_audit_daily_report_with_formal_baseline(
        &days,
        target_idx,
        window_days.max(1),
        language,
        Some(&evidence_collection_status),
        Some(formal_baseline.as_ref()),
    );
    println!("{}", report);
    Ok(())
}

fn run_review_command(config: &config::AppConfig) -> Result<()> {
    println!("{}", load_latest_daily_report(config)?);
    Ok(())
}

fn load_latest_daily_report(config: &config::AppConfig) -> Result<String> {
    let save_dir = std::path::Path::new(&config.output.save_to);
    let latest_path = std::fs::read_dir(save_dir)
        .with_context(|| format!("Failed to read report directory: {}", save_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            let stem = path.file_stem().and_then(|stem| stem.to_str());
            path.extension().and_then(|extension| extension.to_str()) == Some("md")
                && stem
                    .and_then(|stem| NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok())
                    .is_some()
        })
        .max();
    let latest_path =
        latest_path.ok_or_else(|| anyhow!("No daily report found in {}", save_dir.display()))?;

    let report = std::fs::read_to_string(&latest_path).with_context(|| {
        format!(
            "Failed to read latest daily report: {}",
            latest_path.display()
        )
    })?;
    if report.contains("tests/fixtures/") || report.contains("file://") {
        return Err(anyhow!(
            "Latest daily report contains non-production evidence and cannot be reviewed as a valid report: {}",
            latest_path.display()
        ));
    }
    Ok(report)
}

async fn build_daily_calibration_report(
    app_config: &config::AppConfig,
    target_date_arg: Option<&str>,
    window_days: usize,
    language: Language,
) -> Result<String> {
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let context = build_daily_calibration_context(
        save_dir.as_path(),
        target_date_arg,
        window_days,
        enabled_research_attention_count(app_config),
        enabled_asset_thesis_count(app_config),
        language,
    )
    .await?;
    build_daily_calibration_report_from_context(
        app_config,
        &context.audit_section,
        &context.questions_section,
        context.calibration_date,
        window_days,
        language,
    )
    .await
}

fn build_daily_calibration_telegram_digest(report: &str, language: Language) -> String {
    const MAX_LINES: usize = 42;
    const MAX_CHARS: usize = 3200;

    let mut out = String::new();
    let mut retained = 0usize;
    let mut omitted = 0usize;
    let mut keep_next_content_line = false;

    for line in report.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            continue;
        }
        let keep = should_keep_daily_calibration_digest_line(line) || keep_next_content_line;
        keep_next_content_line = line.starts_with('#') || line.starts_with("## ");
        if keep && retained < MAX_LINES && out.len() + line.len() < MAX_CHARS {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
            retained += 1;
        } else {
            omitted += 1;
        }
    }

    if omitted > 0 {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&daily_calibration_digest_omission_notice(language, omitted));
    }
    out
}

fn daily_calibration_digest_omission_notice(language: Language, omitted: usize) -> String {
    match language {
        Language::ZhCn => format!(
            "- Telegram 摘要: 已省略 {} 行明细；CLI 输出保留完整 daily calibration report。",
            omitted
        ),
        Language::JaJp => format!(
            "- Telegram 要約: {} 行の詳細を省略。CLI 出力には daily calibration report の全文を保持。",
            omitted
        ),
        Language::EnUs => format!(
            "- Telegram digest: {} detail line(s) omitted; CLI output keeps the full daily calibration report.",
            omitted
        ),
    }
}

fn should_keep_daily_calibration_digest_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    is_digest_heading(trimmed)
        || is_structured_digest_line(trimmed)
        || is_digest_question_line(trimmed)
        || contains_decision_status_token(trimmed)
}

fn is_digest_heading(trimmed: &str) -> bool {
    trimmed.starts_with('#')
}

fn is_structured_digest_line(trimmed: &str) -> bool {
    if is_noisy_digest_detail(trimmed) {
        return false;
    }
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    body.contains(':') || body.contains('：')
}

fn is_digest_question_line(trimmed: &str) -> bool {
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    body.ends_with('?') || body.ends_with('？')
}

fn contains_decision_status_token(trimmed: &str) -> bool {
    trimmed.contains("NO TRADE") || trimmed.contains("READY") || trimmed.contains("WATCH")
}

fn is_noisy_digest_detail(trimmed: &str) -> bool {
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    let lower = body.to_ascii_lowercase();
    lower.contains("http://")
        || lower.contains("https://")
        || lower.starts_with("source detail")
        || lower.starts_with("raw ")
        || lower.starts_with("raw extract")
        || lower.starts_with("source:")
        || lower.starts_with("sources:")
}

#[cfg(test)]
mod tests {
    use super::{
        build_daily_calibration_report, build_daily_calibration_telegram_digest,
        resolve_audit_daily_formal_baseline, run_pipeline,
    };
    use crate::config::{
        AppConfig, DeviationBasis, OutputConfig, RulesConfig, TelegramConfig, TrendConfig,
        WatchlistEntry,
    };
    use crate::features::radar::application::provider::MarketDataProvider;
    use crate::features::radar::application::provider::{DailyBar, TickerHistory};
    use crate::features::radar::application::runtime_mode::ExecutionMode;
    use crate::features::radar::domain::decision::DecisionPacket;
    use crate::features::radar::infrastructure::persistence::PersistenceLayer;
    use crate::features::radar::interface::audit_daily_report::{
        build_audit_daily_report, build_audit_daily_report_with_evidence_status,
        consecutive_streak, parse_transition_audit_entry, TransitionAuditDay, TransitionAuditEntry,
    };
    use crate::features::radar::interface::radar_pipeline_runner::run_pipeline_for_report_date;
    use crate::features::shared::acl::notification_factory::telegram_delivery_precheck;
    use crate::features::shared::application::run_status::DeliveryStatus;
    use crate::features::shared::infrastructure::run_status_reader::{
        load_latest_evidence_collection_status, EVIDENCE_COLLECTION_STATUS_FILE,
    };
    use crate::features::shared::interface::cli_args::{cli_usage, parse_cli_options, CliCommand};
    use crate::features::shared::interface::i18n::Language;
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

        assert_eq!(super::load_latest_daily_report(&config).unwrap(), "daily");
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

        assert_eq!(super::load_latest_daily_report(&config).unwrap(), "latest");
    }

    #[test]
    fn review_fails_when_no_dated_daily_report_exists() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());

        let error = super::load_latest_daily_report(&config).unwrap_err();
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

        let error = super::load_latest_daily_report(&config).unwrap_err();
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
            assert!(report.contains(super::audit_empty_log_message(Language::ZhCn)));
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

        let today = chrono::Local::now().date_naive().to_string();
        let report_path = tmp.path().join(format!("{}.md", today));
        let run_status_path = tmp.path().join(format!("run_status_{}.json", today));
        let history_path = tmp.path().join("decision_history.jsonl");
        let daily_packet_path = tmp.path().join(format!("decision_packet_{}.json", today));
        let execution_gate_log_path = tmp.path().join("execution_gate_log.jsonl");
        let portfolio_snapshot_path = tmp
            .path()
            .join(format!("portfolio_snapshot_{}.json", today));
        let account_snapshot_path = tmp.path().join(format!("account_snapshot_{}.json", today));
        let data_quality_log_path = tmp.path().join("data_quality_log.jsonl");
        let weekly_metrics_path = tmp.path().join("weekly_state_metrics.json");
        let weekly_review_path = tmp.path().join("weekly_state_review_auto.md");

        assert!(
            report_path.exists(),
            "diagnostic markdown report should exist"
        );
        let report = std::fs::read_to_string(&report_path).unwrap();
        assert!(report.contains("Gravity Layer（估值重力层）"));
        assert!(report.contains("Gravity 与 Trend 独立"));
        assert!(!report.contains("Gravity: Unknown"));
        assert!(tmp.path().join("valuation_gravity_latest.json").exists());
        assert!(tmp
            .path()
            .join(format!("valuation_gravity_{}.json", today))
            .exists());
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
        assert!(report.contains("NO TRADE 连续第 2 天；主因："));
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
        assert!(report.contains("NO TRADE day 1 in a row; primary blockers:"));
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
        assert!(report.contains("NO TRADE 連続 1 日目；主因："));
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
        assert!(report_zh.contains("NO TRADE 连续第 2 天；主因："));
        assert!(report_zh.contains("口径: 连续段按日志连续计算（周末自动衔接）"));

        let report_en = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::EnUs);
        assert!(report_en.contains("4. Substantive Evidence"));
        assert!(report_en.contains("[GOOG] [2026-04-22] [EarningsValidation] (conf:0.95) [OfficialIR] Earnings beat expectations by 15% (https://example.com/ir/goog)"));
        assert!(report_en.contains("[GOOG] [2026-04-21] [CapexPayoff] (conf:0.80) [NewsMedia] Cloud division shows strong ROI (https://news.example.com/goog-cloud)"));
        assert!(report_en.contains("[GOOG] [2026-04-21] [EarningsValidation] (conf:0.90) [OfficialIR] Earnings beat expectations by 15% (https://example.com/ir/goog-followup)"));
        assert!(report_en.contains("6. Audit One-liner"));
        assert!(report_en.contains("NO TRADE day 2 in a row; primary blockers:"));
        assert!(report_en.contains(
            "Methodology: streaks are calculated by log continuity (weekends auto-bridged)"
        ));

        let report_ja = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::JaJp);
        assert!(report_ja.contains("4. 実体的な証拠サマリー"));
        assert!(report_ja.contains("[GOOG] [2026-04-22] [EarningsValidation] (conf:0.95) [OfficialIR] 元の証拠説明は日本語で未提供 (https://example.com/ir/goog)"));
        assert!(report_ja.contains("[GOOG] [2026-04-21] [CapexPayoff] (conf:0.80) [NewsMedia] 元の証拠説明は日本語で未提供 (https://news.example.com/goog-cloud)"));
        assert!(report_ja.contains("[GOOG] [2026-04-21] [EarningsValidation] (conf:0.90) [OfficialIR] 元の証拠説明は日本語で未提供 (https://example.com/ir/goog-followup)"));
        assert!(report_ja.contains("6. 監査ワンライン要約"));
        assert!(report_ja.contains("NO TRADE 連続 2 日目；主因："));
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
        assert!(report.contains("READY 连续第 1 天；主因：无；"));
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

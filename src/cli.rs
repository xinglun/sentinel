use anyhow::{anyhow, Context, Result};

use async_trait::async_trait;
use chrono::NaiveDate;
use sha2::{Digest, Sha256};

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
    audit_daily_usage, audit_empty_log_message, audit_error_parse_date, audit_error_parse_line,
    audit_error_read_file, build_audit_daily_report_with_evidence_status, group_audit_days,
    parse_transition_audit_entry, resolve_target_index, TransitionAuditDay, TransitionAuditEntry,
};
use crate::features::radar::interface::radar_pipeline_runner::run_pipeline;
use crate::features::research::acl::governance_evidence_store_factory::build_governance_evidence_store_adapter;
use crate::features::research::acl::governance_source_adapter_factory::build_governance_source_adapter;
use crate::features::research::acl::gray_rhino_source_adapter_factory::{
    collect_gray_rhino_sources, GrayRhinoSourceCollectionRequest, GrayRhinoSourceProvider,
};
use crate::features::research::application::dependency_evidence::ingest_dependency_concentration_evidence;
use crate::features::research::application::dependency_source_pipeline::{
    collect_dependency_concentration_sources, DependencyFieldCoverage, DependencySourceAdapter,
    DependencySourceCollectionRequest,
};
use crate::features::research::application::governance_evidence::ingest_governance_concentration_evidence;
use crate::features::research::application::governance_source_pipeline::{
    collect_governance_concentration_sources, GovernanceFieldCoverage,
    GovernanceSourceCollectionRequest,
};
use crate::features::research::application::gray_rhino_discovery::{
    discover_gray_rhino_candidates, GrayRhinoDiscoveryInput,
};
use crate::features::research::application::institutional_evidence::ingest_institutional_maturity_evidence;
use crate::features::research::application::redundancy_evidence::ingest_redundancy_evidence;
use crate::features::research::domain::dependency_source::{
    DependencySourceDocument, DependencySourceKind,
};
use crate::features::research::domain::gray_rhino_evidence::{
    DependencyConcentrationEvidence, GovernanceConcentrationEvidence,
    InstitutionalMaturityEvidence, RedundancyEvidence,
};
use crate::features::research::interface::cognitive_reports::{
    build_asset_thesis_report, build_macro_gravity_report, build_research_attention_report,
    daily_calibration_attention_label, daily_calibration_audit_label, daily_calibration_boundary,
    daily_calibration_evidence_none, daily_calibration_evidence_observed,
    daily_calibration_evidence_strong, daily_calibration_gray_rhino_label,
    daily_calibration_macro_gravity_label, daily_calibration_question_attention,
    daily_calibration_question_boundary, daily_calibration_question_evidence,
    daily_calibration_question_gate, daily_calibration_question_market,
    daily_calibration_question_thesis, daily_calibration_questions_label,
    daily_calibration_thesis_label, daily_calibration_title, enabled_asset_thesis_count,
    enabled_research_attention_count,
};
use crate::features::research::interface::gray_rhino_report::{
    build_gray_rhino_daily_report_read_only, build_gray_rhino_escalation_report,
    render_gray_rhino_inline_reference,
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
            run_review(&app_config).await?;
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
            let report = build_research_attention_report(&app_config, audit_language);
            println!("{}", report);
            if options.research_notify {
                send_required_telegram_notification(
                    app_config.telegram.as_ref(),
                    &report,
                    "research-attention",
                )
                .await?;
            }
        }
        CliCommand::AssetThesis => {
            let report = build_asset_thesis_report(&app_config, audit_language);
            println!("{}", report);
            if options.research_notify {
                send_required_telegram_notification(
                    app_config.telegram.as_ref(),
                    &report,
                    "asset-thesis",
                )
                .await?;
            }
        }
        CliCommand::DailyCalibration => {
            let report = build_daily_calibration_report(
                &app_config,
                options.audit_date_arg.as_deref(),
                options.audit_days,
                audit_language,
            )?;
            println!("{}", report);
            if options.research_notify {
                send_required_telegram_notification(
                    app_config.telegram.as_ref(),
                    &report,
                    "daily-calibration",
                )
                .await?;
            }
        }
        CliCommand::GrayRhinoEscalation => {
            let report = build_gray_rhino_escalation_report(&app_config, audit_language);
            println!("{}", report);
            if options.research_notify {
                send_required_telegram_notification(
                    app_config.telegram.as_ref(),
                    &report,
                    "gray-rhino-escalation",
                )
                .await?;
            }
        }
        CliCommand::DiscoverGrayRhino => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_discover_gray_rhino(
                options.evidence_symbol.clone(),
                options.governance_evidence_file.as_deref(),
                options.evidence_date_arg.as_deref(),
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
            )?;
        }
        CliCommand::IngestGrayRhinoDependency => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_ingest_gray_rhino_dependency(
                &app_config,
                options.governance_evidence_file.as_deref(),
            )?;
        }
        CliCommand::IngestGrayRhinoInstitutional => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_ingest_gray_rhino_institutional(
                &app_config,
                options.governance_evidence_file.as_deref(),
            )?;
        }
        CliCommand::IngestGrayRhinoRedundancy => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            run_ingest_gray_rhino_redundancy(
                &app_config,
                options.governance_evidence_file.as_deref(),
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
                    "Successfully ingested {} evidence record.",
                    outcome.saved_count
                );
            } else {
                println!("Evidence record already exists (deduplicated).");
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
                &extractor,
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
                println!("--- Dry Run: Extracted Evidence ---");
                println!("Source: {}", url);
                println!("Symbol: {}", symbol);
                if outcome.records.is_empty() {
                    println!("No evidence found.");
                }
                for (i, r) in outcome.records.iter().enumerate() {
                    println!(
                        "[{}] Type: {:?}, Confidence: {:.2}, Date: {}",
                        i + 1,
                        r.evidence_type,
                        r.confidence,
                        r.event_date
                    );
                    println!("    Desc: {}", r.description);
                    if let Some(ref url) = r.source_url {
                        println!("    URL:  {}", url);
                    }
                }
                return Ok(());
            }

            if outcome.saved_count > 0 {
                println!(
                    "Successfully ingested {} automated evidence records.",
                    outcome.saved_count
                );
            } else {
                println!("Evidence record already exists (deduplicated).");
            }
        }
        CliCommand::CollectEvidence => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            if options.evidence_symbols.is_empty() {
                return Err(anyhow!("--symbols is required (comma separated)"));
            }

            println!("--- Batch Evidence Collection ---");
            println!("Symbols: {:?}", options.evidence_symbols);
            println!("Window:  {} days", options.evidence_days);

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
                    println!("Fetching for {}...", symbol);
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
                &extractor,
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
                println!("  -> Extracted {} records", record_count);
            }
            for failure in &batch_outcome.failures {
                eprintln!(
                    "  [ERROR] Failed to fetch for {}: {}",
                    failure.symbol, failure.error
                );
            }

            println!(
                "
--- Batch Collection Summary ---"
            );
            println!("Processed: {} symbols", options.evidence_symbols.len());
            println!("Success:   {} symbols", batch_outcome.success_count);
            println!("Failure:   {} symbols", batch_outcome.failure_count);

            if options.evidence_dry_run {
                println!(
                    "
--- Dry Run: Extracted Evidence Summary ---"
                );
                if batch_outcome.records.is_empty() {
                    println!("No evidence found in batch.");
                }
                for (i, r) in batch_outcome.records.iter().enumerate() {
                    let date_str = r.event_date.as_str();
                    println!(
                        "[{}] {}: {:?} ({:.2}) | Date: {}",
                        i + 1,
                        r.symbol.as_deref().unwrap_or("GLOBAL"),
                        r.evidence_type,
                        r.confidence,
                        date_str
                    );
                    println!("    Desc: {}", r.description);
                    if let Some(ref url) = r.source_url {
                        println!("    URL:  {}", url);
                    }
                }
                return Ok(());
            }

            println!(
                "
Successfully ingested {} batch evidence records to store.",
                batch_outcome.saved_count
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

fn run_ingest_gray_rhino_governance(
    app_config: &config::AppConfig,
    file_arg: Option<&str>,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let raw = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read governance evidence file: {}", file))?;
    let evidence: GovernanceConcentrationEvidence = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse governance evidence JSON: {}", file))?;
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let outcome = ingest_governance_concentration_evidence(&store, evidence)?;
    if outcome.saved {
        println!("Successfully ingested GovernanceConcentration evidence.");
    } else {
        println!("GovernanceConcentration evidence already exists (deduplicated).");
    }
    println!("Category: GovernanceConcentration");
    println!("Source: {}", outcome.record.source.source_title);
    println!("Observed at: {}", outcome.record.source.observed_at);
    println!("Boundary: evidence only; no escalation, gate, execution, or trading state updated.");
    Ok(())
}

fn run_discover_gray_rhino(
    symbol: Option<String>,
    file_arg: Option<&str>,
    observed_date_arg: Option<&str>,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let subject = symbol.unwrap_or_else(|| "UNKNOWN".to_string());
    let observed_at = match observed_date_arg {
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("Invalid Gray Rhino discovery date: {}", raw))?,
        None => chrono::Local::now().date_naive(),
    };
    let text = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read Gray Rhino discovery source: {}", file))?;
    let candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
        subject,
        source_title: file.to_string(),
        observed_at,
        text,
    });
    println!("--- Gray Rhino Auto Discovery ---");
    println!("{}", render_gray_rhino_inline_reference(&candidates));
    Ok(())
}

async fn run_collect_gray_rhino_sources(
    app_config: &config::AppConfig,
    provider_arg: &str,
    symbols: Vec<String>,
    dry_run: bool,
    observed_date_arg: Option<&str>,
    lookback_days: usize,
) -> Result<()> {
    let provider = GrayRhinoSourceProvider::parse(provider_arg)
        .ok_or_else(|| anyhow!("Unsupported Gray Rhino source provider: {}", provider_arg))?;
    let as_of_date = match observed_date_arg {
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("Invalid Gray Rhino source collection date: {}", raw))?,
        None => chrono::Local::now().date_naive(),
    };
    let outcomes = collect_gray_rhino_sources(
        app_config,
        GrayRhinoSourceCollectionRequest {
            provider,
            symbols,
            save_dir: std::path::PathBuf::from(&app_config.output.save_to),
            as_of_date,
            lookback_days: if provider == GrayRhinoSourceProvider::Sec {
                lookback_days.max(365)
            } else {
                lookback_days
            },
            dry_run,
        },
    )
    .await?;
    println!("--- Gray Rhino Source Collection ---");
    println!("provider: {:?}", provider);
    println!("dry_run: {}", dry_run);
    println!("source_count: {}", outcomes.len());
    let accepted = outcomes.iter().filter(|outcome| outcome.accepted).count();
    let rejected = outcomes.len().saturating_sub(accepted);
    let candidate_count: usize = outcomes.iter().map(|outcome| outcome.candidate_count).sum();
    println!("accepted: {}", accepted);
    println!("rejected: {}", rejected);
    println!("candidate_count: {}", candidate_count);
    let provider_status = if dry_run {
        "skipped"
    } else if accepted == outcomes.len() && accepted > 0 {
        "succeeded"
    } else if accepted > 0 && rejected > 0 {
        "partial_failure"
    } else if accepted == 0 && rejected > 0 {
        "failed"
    } else {
        "skipped"
    };
    println!("provider_status: {}", provider_status);
    for outcome in &outcomes {
        println!(
            "- {} accepted={} planned={} candidates={} path={} taxonomy={} message={}",
            outcome.subject,
            outcome.accepted,
            outcome.planned,
            outcome.candidate_count,
            outcome.repository_path.as_deref().unwrap_or("none"),
            outcome.failure_taxonomy.as_deref().unwrap_or("none"),
            outcome.message
        );
    }
    println!(
        "Boundary: source collection only; no trading recommendation, no Gate override, no trend cohesion mutation, no execution action."
    );
    if provider_status == "failed" {
        return Err(anyhow!(
            "Gray Rhino source collection failed for provider {:?}: no accepted source",
            provider
        ));
    }
    Ok(())
}

fn run_ingest_gray_rhino_dependency(
    app_config: &config::AppConfig,
    file_arg: Option<&str>,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let raw = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read dependency evidence file: {}", file))?;
    let evidence: DependencyConcentrationEvidence = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse dependency evidence JSON: {}", file))?;
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let outcome = ingest_dependency_concentration_evidence(&store, evidence)?;
    if outcome.saved {
        println!("Successfully ingested DependencyConcentration evidence.");
    } else {
        println!("DependencyConcentration evidence already exists (deduplicated).");
    }
    println!("Category: DependencyConcentration");
    println!("Source: {}", outcome.record.source.source_title);
    println!("Observed at: {}", outcome.record.source.observed_at);
    println!("Boundary: evidence only; no escalation, gate, execution, or trading state updated.");
    Ok(())
}

fn run_ingest_gray_rhino_institutional(
    app_config: &config::AppConfig,
    file_arg: Option<&str>,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let raw = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read institutional evidence file: {}", file))?;
    let evidence: InstitutionalMaturityEvidence = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse institutional evidence JSON: {}", file))?;
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let outcome = ingest_institutional_maturity_evidence(&store, evidence)?;
    if outcome.saved {
        println!("Successfully ingested InstitutionalMaturity evidence.");
    } else {
        println!("InstitutionalMaturity evidence already exists (deduplicated).");
    }
    println!("Category: InstitutionalMaturity");
    println!("Source: {}", outcome.record.source.source_title);
    println!("Observed at: {}", outcome.record.source.observed_at);
    println!("Boundary: evidence only; no escalation, gate, execution, or trading state updated.");
    Ok(())
}

fn run_ingest_gray_rhino_redundancy(
    app_config: &config::AppConfig,
    file_arg: Option<&str>,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let raw = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read redundancy evidence file: {}", file))?;
    let evidence: RedundancyEvidence = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse redundancy evidence JSON: {}", file))?;
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let outcome = ingest_redundancy_evidence(&store, evidence)?;
    if outcome.saved {
        println!("Successfully ingested Redundancy evidence.");
    } else {
        println!("Redundancy evidence already exists (deduplicated).");
    }
    println!("Category: Redundancy");
    println!("Source: {}", outcome.record.source.source_title);
    println!("Observed at: {}", outcome.record.source.observed_at);
    println!("Boundary: evidence only; no escalation, gate, execution, or trading state updated.");
    Ok(())
}

async fn run_collect_gray_rhino_governance(
    app_config: &config::AppConfig,
    symbol: Option<String>,
    symbols: Vec<String>,
    source_file: Option<String>,
    dry_run_requested: bool,
    observed_date_arg: Option<&str>,
    lookback_days: usize,
) -> Result<()> {
    let targets = resolve_governance_collection_targets(app_config, symbol, symbols, &source_file)?;
    let observed_at = match observed_date_arg {
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("Invalid governance evidence date: {}", raw))?,
        None => chrono::Local::now().date_naive(),
    };
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let adapter = build_governance_source_adapter(app_config, &save_dir);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let retrieved_at = chrono::Local::now().date_naive();
    let is_live_sec_path = source_file.is_none();
    let persist_evidence = !dry_run_requested && !is_live_sec_path;
    let mut total_sources = 0;
    let mut total_accepted = 0;
    let mut total_saved = 0;
    let mut total_manifest = 0;
    let mut total_audit = 0;
    let mut rejected = Vec::new();
    let mut latest_observed_at = None;
    let mut metric_coverage = Vec::new();
    for target in targets {
        let summary = collect_governance_concentration_sources(
            &adapter,
            &store,
            GovernanceSourceCollectionRequest {
                symbol: Some(target),
                local_file: source_file.clone(),
                observed_at,
                retrieved_at,
                lookback_days: lookback_days.max(1),
                persist_evidence,
            },
        )
        .await?;
        total_sources += summary.source_count;
        total_accepted += summary.accepted_count;
        total_saved += summary.saved_count;
        total_manifest += summary.manifest_count;
        total_audit += summary.audit_count;
        latest_observed_at = latest_observed_at
            .map(|latest: NaiveDate| {
                summary
                    .latest_observed_at
                    .map(|observed| latest.max(observed))
                    .unwrap_or(latest)
            })
            .or(summary.latest_observed_at);
        metric_coverage.extend(summary.metric_coverage);
        rejected.extend(summary.rejected);
    }
    let coverage_ratio = if total_sources == 0 {
        0.0
    } else {
        total_accepted as f64 / total_sources as f64
    };

    println!("--- Gray Rhino Governance Evidence Collection ---");
    println!("Sources:  {}", total_sources);
    println!("Accepted: {}", total_accepted);
    println!("Saved:    {}", total_saved);
    println!("Manifest: {}", total_manifest);
    println!("Audit:    {}", total_audit);
    println!("Dry run:  {}", !persist_evidence);
    println!("Formal evidence persisted: {}", persist_evidence);
    println!("Coverage: {:.1}%", coverage_ratio * 100.0);
    render_governance_field_coverage(&metric_coverage);
    println!("Rejected: {}", rejected.len());
    if let Some(latest) = latest_observed_at {
        println!("Latest observed date: {}", latest);
    }
    for rejection in &rejected {
        println!(
            "  [REJECTED] {}: {}",
            rejection.source_title, rejection.reason
        );
    }
    println!("Boundary: evidence only; no escalation, gate, execution, or trading state updated.");
    Ok(())
}

async fn run_collect_gray_rhino_dependency(
    app_config: &config::AppConfig,
    symbol: Option<String>,
    source_file: Option<String>,
    source_url: Option<String>,
    dry_run_requested: bool,
    observed_date_arg: Option<&str>,
) -> Result<()> {
    let target = symbol.ok_or_else(|| anyhow!("--symbol is required"))?;
    let observed_at = match observed_date_arg {
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("Invalid dependency evidence date: {}", raw))?,
        None => chrono::Local::now().date_naive(),
    };
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let adapter = CliLocalDependencySourceAdapter;
    let summary = collect_dependency_concentration_sources(
        &adapter,
        &store,
        DependencySourceCollectionRequest {
            symbol: Some(target),
            local_file: source_file,
            source_url,
            source_cache_dir: Some(
                save_dir
                    .join("gray_rhino_sources/dependency")
                    .display()
                    .to_string(),
            ),
            observed_at,
            retrieved_at: chrono::Local::now().date_naive(),
            persist_evidence: !dry_run_requested,
        },
    )
    .await?;
    let coverage_ratio = if summary.source_count == 0 {
        0.0
    } else {
        summary.accepted_count as f64 / summary.source_count as f64
    };

    println!("--- Gray Rhino Dependency Evidence Collection ---");
    println!("Sources:  {}", summary.source_count);
    println!("Accepted: {}", summary.accepted_count);
    println!("Saved:    {}", summary.saved_count);
    println!("Manifest: {}", summary.manifest_count);
    println!("Audit:    {}", summary.audit_count);
    println!("Dry run:  {}", dry_run_requested);
    println!("Formal evidence persisted: {}", !dry_run_requested);
    println!("Coverage: {:.1}%", coverage_ratio * 100.0);
    render_dependency_field_coverage(&summary.metric_coverage);
    println!("Rejected: {}", summary.rejected.len());
    if let Some(latest) = summary.latest_observed_at {
        println!("Latest observed date: {}", latest);
    }
    for rejection in &summary.rejected {
        println!(
            "  [REJECTED:{:?}] {}: {}",
            rejection.taxonomy, rejection.source_title, rejection.reason
        );
    }
    println!("Boundary: evidence only; no escalation, gate, execution, or trading state updated.");
    Ok(())
}

async fn run_collect_gray_rhino_backfill(
    app_config: &config::AppConfig,
    file_arg: Option<&str>,
    observed_date_arg: Option<&str>,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let raw = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read Gray Rhino backfill manifest: {}", file))?;
    let entries: Vec<serde_json::Value> = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse Gray Rhino backfill manifest: {}", file))?;
    let started_at = chrono::Local::now().to_rfc3339();
    let run_id = format!(
        "gray-rhino-backfill-{:x}",
        Sha256::digest(format!("{file}:{started_at}").as_bytes())
    );
    let mut processed = 0usize;
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut stale_sources = 0usize;
    let mut drift_sources = 0usize;
    let mut failures = Vec::new();
    let mut categories = Vec::new();
    println!("--- Gray Rhino Multi-Category Backfill Dry Run ---");
    for entry in entries {
        let category = entry
            .get("category")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow!("backfill entry missing category"))?;
        let symbol = entry
            .get("symbol")
            .and_then(|value| value.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();
        let Some(source_file) = entry
            .get("file")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
        else {
            failures.push(serde_json::json!({
                "category": category,
                "symbol": symbol,
                "failure_taxonomy": "unsupported_format",
                "reason": "missing file"
            }));
            rejected += 1;
            processed += 1;
            continue;
        };
        categories.push(category.to_string());
        let source_content = match std::fs::read_to_string(&source_file) {
            Ok(content) => content,
            Err(err) => {
                failures.push(serde_json::json!({
                    "category": category,
                    "symbol": symbol,
                    "source": source_file,
                    "failure_taxonomy": "fetch_failure",
                    "reason": err.to_string()
                }));
                rejected += 1;
                processed += 1;
                continue;
            }
        };
        let source_hash = format!("{:x}", Sha256::digest(source_content.as_bytes()));
        if let Some(expected) = entry
            .get("expected_sha256")
            .and_then(|value| value.as_str())
        {
            if expected != source_hash {
                drift_sources += 1;
            }
        }
        if let (Some(observed), Some(freshness_days)) = (
            entry.get("observed_at").and_then(|value| value.as_str()),
            entry.get("freshness_days").and_then(|value| value.as_i64()),
        ) {
            if let Ok(observed) = NaiveDate::parse_from_str(observed, "%Y-%m-%d") {
                let as_of = observed_date_arg
                    .and_then(|raw| NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok())
                    .unwrap_or_else(|| chrono::Local::now().date_naive());
                if as_of.signed_duration_since(observed).num_days() > freshness_days {
                    stale_sources += 1;
                }
            }
        }
        match category {
            "DependencyConcentration" => {
                run_collect_gray_rhino_dependency(
                    app_config,
                    Some(symbol),
                    Some(source_file),
                    None,
                    true,
                    observed_date_arg,
                )
                .await?;
            }
            "InstitutionalMaturity" => run_collect_gray_rhino_category_source(
                app_config,
                "InstitutionalMaturity",
                Some(symbol),
                Some(source_file),
                true,
                observed_date_arg,
                &[
                    "succession_structure_disclosed",
                    "external_audit_present",
                    "disclosure_quality_score",
                    "oversight_evolution_disclosed",
                    "compliance_maturity_level",
                ],
            )?,
            "Redundancy" => run_collect_gray_rhino_category_source(
                app_config,
                "Redundancy",
                Some(symbol),
                Some(source_file),
                true,
                observed_date_arg,
                &[
                    "fallback_available",
                    "alternative_supplier_count",
                    "redundancy_ratio",
                    "recovery_path_disclosed",
                    "failover_tested",
                ],
            )?,
            other => {
                failures.push(serde_json::json!({
                    "category": other,
                    "symbol": symbol,
                    "source": source_file,
                    "failure_taxonomy": "unsupported_format",
                    "reason": "unsupported category"
                }));
                rejected += 1;
                processed += 1;
                continue;
            }
        }
        accepted += 1;
        processed += 1;
    }
    let finished_at = chrono::Local::now().to_rfc3339();
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    std::fs::create_dir_all(&save_dir)
        .with_context(|| format!("Failed to create output directory: {}", save_dir.display()))?;
    append_cli_jsonl(
        &save_dir.join("gray_rhino_backfill_runs.jsonl"),
        &serde_json::json!({
            "run_id": run_id,
            "mode": "dry_run",
            "manifest": file,
            "categories": categories,
            "source_count": processed,
            "accepted": accepted,
            "rejected": rejected,
            "coverage": if processed == 0 { 0.0 } else { accepted as f64 / processed as f64 },
            "stale_sources": stale_sources,
            "drift_sources": drift_sources,
            "failures": failures,
            "started_at": started_at,
            "finished_at": finished_at,
            "boundary": "evidence only; no escalation, gate, execution, or trading state updated"
        }),
    )?;
    println!("Backfill entries processed: {}", processed);
    println!("Backfill run summary: gray_rhino_backfill_runs.jsonl");
    println!("Boundary: dry-run only; no escalation, gate, execution, or trading state updated.");
    Ok(())
}

fn run_collect_gray_rhino_category_source(
    app_config: &config::AppConfig,
    category: &str,
    symbol: Option<String>,
    source_file: Option<String>,
    dry_run_requested: bool,
    observed_date_arg: Option<&str>,
    metrics: &[&str],
) -> Result<()> {
    let target = symbol.ok_or_else(|| anyhow!("--symbol is required"))?;
    let file = source_file.ok_or_else(|| anyhow!("--file is required"))?;
    let observed_at = match observed_date_arg {
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("Invalid Gray Rhino source date: {}", raw))?,
        None => chrono::Local::now().date_naive(),
    };
    let retrieved_at = chrono::Local::now().date_naive();
    let content = std::fs::read_to_string(&file)
        .with_context(|| format!("Failed to read Gray Rhino source file: {}", file))?;
    let normalized = content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let extracted: Vec<&str> = metrics
        .iter()
        .copied()
        .filter(|metric| metric_found_for_category(category, metric, &normalized))
        .collect();
    let missing_count = metrics.len().saturating_sub(extracted.len());
    let accepted = !extracted.is_empty();
    let taxonomy = if accepted {
        "Accepted"
    } else {
        "MetriclessSource"
    };
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    std::fs::create_dir_all(&save_dir)
        .with_context(|| format!("Failed to create output directory: {}", save_dir.display()))?;
    let manifest_path = save_dir.join(format!(
        "gray_rhino_{}_source_manifest.jsonl",
        category.to_lowercase()
    ));
    let audit_path = save_dir.join(format!(
        "gray_rhino_{}_extraction_audit.jsonl",
        category.to_lowercase()
    ));
    let manifest = serde_json::json!({
        "subject": target,
        "category": category,
        "source_title": file,
        "repository_path": file,
        "observed_at": observed_at,
        "retrieved_at": retrieved_at,
        "content_sha256": format!("{:x}", Sha256::digest(content.as_bytes()))
    });
    let audit = serde_json::json!({
        "subject": manifest["subject"],
        "category": category,
        "source_title": manifest["source_title"],
        "observed_at": observed_at,
        "retrieved_at": retrieved_at,
        "accepted": accepted,
        "rejection_taxonomy": taxonomy,
        "extracted_metrics": extracted,
        "missing_count": missing_count
    });
    append_cli_jsonl(&manifest_path, &manifest)?;
    append_cli_jsonl(&audit_path, &audit)?;

    println!("--- Gray Rhino {category} Evidence Collection ---");
    println!("Sources:  1");
    println!("Accepted: {}", usize::from(accepted));
    println!("Saved:    0");
    println!("Manifest: 1");
    println!("Audit:    1");
    println!("Dry run:  {}", dry_run_requested);
    println!("Formal evidence persisted: false");
    println!(
        "Coverage: {:.1}%",
        (extracted.len() as f64 / metrics.len() as f64) * 100.0
    );
    println!("Field coverage:");
    for metric in metrics {
        let count = usize::from(extracted.contains(metric));
        println!(
            "  {}: {:.1}% ({}/1 extracted, {} missing)",
            metric,
            count as f64 * 100.0,
            count,
            1usize.saturating_sub(count)
        );
    }
    println!("Rejected: {}", usize::from(!accepted));
    if !accepted {
        println!("  [REJECTED:{taxonomy}] {}", manifest["source_title"]);
    }
    println!("Latest observed date: {}", observed_at);
    println!("Boundary: evidence only; no escalation, gate, execution, or trading state updated.");
    Ok(())
}

fn metric_found_for_category(category: &str, metric: &str, normalized: &str) -> bool {
    if normalized.contains(metric) || normalized.contains(&metric.replace('_', " ")) {
        return true;
    }
    metric_aliases(category, metric)
        .iter()
        .any(|alias| normalized.contains(alias))
}

fn metric_aliases(category: &str, metric: &str) -> &'static [&'static str] {
    match (category, metric) {
        ("InstitutionalMaturity", "succession_structure_disclosed") => &[
            "succession plan",
            "succession planning",
            "leadership transition plan",
        ],
        ("InstitutionalMaturity", "external_audit_present") => {
            &["external audit", "independent auditor", "audited by"]
        }
        ("InstitutionalMaturity", "disclosure_quality_score") => &[
            "disclosure quality",
            "comprehensive disclosure",
            "detailed disclosure",
        ],
        ("InstitutionalMaturity", "oversight_evolution_disclosed") => &[
            "oversight evolution",
            "oversight framework evolved",
            "board oversight expanded",
        ],
        ("InstitutionalMaturity", "compliance_maturity_level") => &[
            "compliance maturity",
            "mature compliance",
            "developing compliance",
        ],
        ("Redundancy", "fallback_available") => &[
            "fallback available",
            "fallback provider",
            "backup provider",
            "alternative supplier",
        ],
        ("Redundancy", "alternative_supplier_count") => &[
            "two alternative suppliers",
            "multiple alternative suppliers",
            "alternative suppliers",
        ],
        ("Redundancy", "redundancy_ratio") => &[
            "redundancy ratio",
            "redundant capacity",
            "capacity redundancy",
        ],
        ("Redundancy", "recovery_path_disclosed") => {
            &["recovery path", "recovery plan", "failover path"]
        }
        ("Redundancy", "failover_tested") => &[
            "failover tested",
            "failover test",
            "tested failover",
            "drill completed",
        ],
        _ => &[],
    }
}

fn append_cli_jsonl(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    writeln!(file, "{}", serde_json::to_string(value)?)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

struct CliLocalDependencySourceAdapter;

#[async_trait]
impl DependencySourceAdapter for CliLocalDependencySourceAdapter {
    async fn fetch_dependency_sources(
        &self,
        request: &DependencySourceCollectionRequest,
    ) -> Result<Vec<DependencySourceDocument>> {
        let subject = request
            .symbol
            .clone()
            .unwrap_or_else(|| "UNKNOWN".to_string());
        if let Some(url) = request.source_url.as_ref() {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .context("Failed to build dependency source HTTP client")?;
            let mut last_error = None;
            let mut content = None;
            for _attempt in 0..3 {
                match client.get(url).send().await {
                    Ok(response) => match response.error_for_status() {
                        Ok(response) => match response.text().await {
                            Ok(body) => {
                                content = Some(body);
                                break;
                            }
                            Err(err) => last_error = Some(err.into()),
                        },
                        Err(err) => last_error = Some(err.into()),
                    },
                    Err(err) => last_error = Some(err.into()),
                }
            }
            let content = content.ok_or_else(|| {
                last_error.unwrap_or_else(|| anyhow!("Failed to fetch dependency source URL"))
            })?;
            if let Some(cache_dir) = request.source_cache_dir.as_ref() {
                let cache_dir = std::path::PathBuf::from(cache_dir);
                tokio::fs::create_dir_all(&cache_dir)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to create dependency cache dir: {}",
                            cache_dir.display()
                        )
                    })?;
                let cache_name = format!("{:x}.txt", Sha256::digest(url.as_bytes()));
                tokio::fs::write(cache_dir.join(cache_name), &content)
                    .await
                    .with_context(|| "Failed to cache dependency source body")?;
            }
            let parsed_url = reqwest::Url::parse(url).ok();
            let publisher = parsed_url
                .as_ref()
                .and_then(|parsed| parsed.host_str())
                .unwrap_or("unknown dependency publisher")
                .to_string();
            return Ok(vec![DependencySourceDocument {
                subject: subject.clone(),
                source_kind: DependencySourceKind::LiveDependencyDisclosure,
                source_title: format!("Dependency disclosure: {publisher}"),
                publisher,
                source_url: Some(url.to_string()),
                repository_path: None,
                observed_at: request.observed_at,
                retrieved_at: request.retrieved_at,
                content,
            }]);
        }
        let file = request.local_file.as_ref().ok_or_else(|| {
            anyhow!("--file or --url is required for dependency source collection")
        })?;
        let path = std::path::PathBuf::from(file);
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read dependency source file: {}", file))?;
        Ok(vec![DependencySourceDocument {
            subject: subject.clone(),
            source_kind: DependencySourceKind::LocalDependencyDocument,
            source_title: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("dependency_source")
                .to_string(),
            publisher: subject,
            source_url: None,
            repository_path: Some(file.to_string()),
            observed_at: request.observed_at,
            retrieved_at: request.retrieved_at,
            content,
        }])
    }
}

fn render_dependency_field_coverage(metric_coverage: &[DependencyFieldCoverage]) {
    if metric_coverage.is_empty() {
        return;
    }
    println!("Field coverage:");
    for metric in metric_coverage {
        let total = metric.extracted_count + metric.missing_count;
        println!(
            "  {}: {:.1}% ({}/{} extracted, {} missing)",
            metric.metric,
            metric.coverage_ratio * 100.0,
            metric.extracted_count,
            total,
            metric.missing_count
        );
    }
}

fn render_governance_field_coverage(metric_coverage: &[GovernanceFieldCoverage]) {
    if metric_coverage.is_empty() {
        return;
    }
    println!("Field coverage:");
    for metric in aggregate_governance_field_coverage(metric_coverage) {
        let total = metric.extracted_count + metric.missing_count + metric.invalid_count;
        let coverage_ratio = if total == 0 {
            0.0
        } else {
            metric.extracted_count as f64 / total as f64
        };
        println!(
            "  {}: {:.1}% ({}/{} extracted, {} missing, {} invalid)",
            metric.metric,
            coverage_ratio * 100.0,
            metric.extracted_count,
            total,
            metric.missing_count,
            metric.invalid_count
        );
    }
}

fn aggregate_governance_field_coverage(
    metric_coverage: &[GovernanceFieldCoverage],
) -> Vec<GovernanceFieldCoverage> {
    let mut totals = std::collections::BTreeMap::new();
    for metric in metric_coverage {
        let entry = totals.entry(metric.metric.clone()).or_insert((0, 0, 0));
        entry.0 += metric.extracted_count;
        entry.1 += metric.missing_count;
        entry.2 += metric.invalid_count;
    }
    totals
        .into_iter()
        .map(
            |(metric, (extracted_count, missing_count, invalid_count))| {
                let total = extracted_count + missing_count + invalid_count;
                GovernanceFieldCoverage {
                    metric,
                    extracted_count,
                    missing_count,
                    invalid_count,
                    coverage_ratio: if total == 0 {
                        0.0
                    } else {
                        extracted_count as f64 / total as f64
                    },
                }
            },
        )
        .collect()
}

fn resolve_governance_collection_targets(
    app_config: &config::AppConfig,
    symbol: Option<String>,
    symbols: Vec<String>,
    source_file: &Option<String>,
) -> Result<Vec<String>> {
    if source_file.is_some() {
        return symbol
            .map(|symbol| vec![symbol])
            .or_else(|| symbols.first().cloned().map(|symbol| vec![symbol]))
            .ok_or_else(|| anyhow!("--symbol is required when --file is used"));
    }
    let mut targets = Vec::new();
    if let Some(symbol) = symbol {
        targets.push(symbol);
    }
    targets.extend(
        symbols
            .into_iter()
            .filter(|symbol| !symbol.trim().is_empty()),
    );
    if targets.is_empty() {
        targets = app_config
            .watchlist
            .iter()
            .filter(|entry| entry.enable)
            .map(|entry| entry.symbol.clone())
            .collect();
    }
    if targets.is_empty() {
        return Err(anyhow!("No governance collection targets are configured"));
    }
    targets.sort();
    targets.dedup();
    Ok(targets)
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
    let report = build_audit_daily_report_with_evidence_status(
        &days,
        target_idx,
        window_days.max(1),
        language,
        Some(&evidence_collection_status),
    );
    println!("{}", report);
    Ok(())
}

fn build_daily_calibration_report(
    app_config: &config::AppConfig,
    target_date_arg: Option<&str>,
    window_days: usize,
    language: Language,
) -> Result<String> {
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let path = save_dir.join("state_transitions.jsonl");
    let days = load_transition_audit_days(&path, language)?;

    let mut selected_entry: Option<&TransitionAuditEntry> = None;
    let target_date = match target_date_arg {
        Some(raw) => Some(
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .with_context(|| format!("{}: {}", audit_error_parse_date(language), raw))?,
        ),
        None => None,
    };
    let mut calibration_date = target_date.unwrap_or_else(|| chrono::Local::now().date_naive());
    let audit_section = if days.is_empty() {
        audit_empty_log_message(language).to_string()
    } else {
        let target_idx = resolve_target_index(&days, target_date, language)?;
        calibration_date = days[target_idx].date;
        let evidence_collection_status =
            load_run_evidence_collection_status(&save_dir, days[target_idx].date).unwrap_or(
                crate::features::shared::application::run_status::DeliveryStatus::Skipped,
            );
        selected_entry = Some(days[target_idx].latest());
        build_audit_daily_report_with_evidence_status(
            &days,
            target_idx,
            window_days.max(1),
            language,
            Some(&evidence_collection_status),
        )
    };

    let mut out = String::new();
    out.push_str(daily_calibration_title(language));
    out.push_str("\n\n");
    out.push_str(daily_calibration_audit_label(language));
    out.push_str("\n\n");
    out.push_str(&audit_section);
    out.push_str("\n\n");
    out.push_str(daily_calibration_questions_label(language));
    out.push_str("\n\n");
    out.push_str(&build_daily_calibration_questions(
        app_config,
        selected_entry,
        language,
    ));
    out.push_str("\n\n");
    out.push_str(daily_calibration_attention_label(language));
    out.push_str("\n\n");
    out.push_str(&build_research_attention_report(app_config, language));
    out.push_str("\n\n");
    out.push_str(daily_calibration_thesis_label(language));
    out.push_str("\n\n");
    out.push_str(&build_asset_thesis_report(app_config, language));
    out.push_str("\n\n");
    out.push_str(daily_calibration_macro_gravity_label(language));
    out.push_str("\n\n");
    out.push_str(&build_macro_gravity_report(app_config, language));
    out.push_str("\n\n");
    out.push_str(daily_calibration_gray_rhino_label(language));
    out.push_str("\n\n");
    out.push_str(&build_gray_rhino_daily_report_read_only(
        app_config,
        &save_dir,
        calibration_date,
        language,
    )?);
    out.push_str("\n\n");
    out.push_str(daily_calibration_boundary(language));
    Ok(out)
}

fn build_daily_calibration_questions(
    app_config: &config::AppConfig,
    selected_entry: Option<&TransitionAuditEntry>,
    language: Language,
) -> String {
    let attention_count = enabled_research_attention_count(app_config);
    let thesis_count = enabled_asset_thesis_count(app_config);
    let gate_state = selected_entry
        .map(|entry| {
            if entry.log.trend_cohesion_gate.to {
                "READY"
            } else {
                "NO TRADE"
            }
        })
        .unwrap_or("NO AUDIT");
    let evidence_state = selected_entry
        .and_then(|entry| entry.log.trend_recognition.as_ref())
        .map(|tr| {
            if tr.conviction_score >= 3.0 {
                daily_calibration_evidence_strong(language)
            } else if tr.conviction_score > 0.0 {
                daily_calibration_evidence_observed(language)
            } else {
                daily_calibration_evidence_none(language)
            }
        })
        .unwrap_or(daily_calibration_evidence_none(language));

    format!(
        "{}\n{} {}\n{} {}\n{} {}\n{} {}\n{}",
        daily_calibration_question_market(language),
        daily_calibration_question_gate(language),
        gate_state,
        daily_calibration_question_evidence(language),
        evidence_state,
        daily_calibration_question_attention(language),
        attention_count,
        daily_calibration_question_thesis(language),
        thesis_count,
        daily_calibration_question_boundary(language),
    )
}

fn load_transition_audit_days(
    path: &std::path::Path,
    language: Language,
) -> Result<Vec<TransitionAuditDay>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("{}: {}", audit_error_read_file(language), path.display()))?;

    let mut raw_entries = Vec::<TransitionAuditEntry>::new();
    for (idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(entry) = parse_transition_audit_entry(line, language)
            .with_context(|| format!("{} {}", audit_error_parse_line(language), idx + 1))?
        {
            raw_entries.push(entry);
        }
    }

    raw_entries.sort_by_key(|a| a.timestamp);
    Ok(group_audit_days(raw_entries))
}

fn load_latest_daily_report(config: &crate::config::AppConfig) -> Result<String> {
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

async fn run_review(config: &crate::config::AppConfig) -> Result<()> {
    println!("{}", load_latest_daily_report(config)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load_latest_daily_report, run_pipeline};
    use crate::config::{
        AppConfig, DeviationBasis, OutputConfig, RulesConfig, TelegramConfig, TrendConfig,
        WatchlistEntry,
    };
    use crate::features::radar::application::provider::MarketDataProvider;
    use crate::features::radar::application::provider::{DailyBar, TickerHistory};
    use crate::features::radar::application::runtime_mode::ExecutionMode;
    use crate::features::radar::interface::audit_daily_report::{
        build_audit_daily_report, build_audit_daily_report_with_evidence_status,
        consecutive_streak, parse_transition_audit_entry, TransitionAuditDay, TransitionAuditEntry,
    };
    use crate::features::shared::acl::notification_factory::telegram_delivery_precheck;
    use crate::features::shared::application::run_status::DeliveryStatus;
    use crate::features::shared::infrastructure::run_status_reader::{
        load_latest_evidence_collection_status, EVIDENCE_COLLECTION_STATUS_FILE,
    };
    use crate::features::shared::interface::i18n::Language;
    use anyhow::{anyhow, Result};
    use chrono::{NaiveDate, Utc};
    use std::borrow::Cow;
    use std::collections::{BTreeMap, HashMap};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn review_loads_latest_dated_report_and_ignores_non_daily_markdown() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());
        fs::write(tmp.path().join("2026-05-20.md"), "old").unwrap();
        fs::write(tmp.path().join("2026-05-21.md"), "latest").unwrap();
        fs::write(tmp.path().join("weekly_state_review_auto.md"), "weekly").unwrap();

        assert_eq!(load_latest_daily_report(&config).unwrap(), "latest");
    }

    #[test]
    fn review_fails_when_no_dated_daily_report_exists() {
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
    use time::OffsetDateTime;

    struct AlwaysFailProvider;

    struct PartialSuccessProvider;

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
        assert!(weekly_metrics.contains("\"strategic_context\""));
        let weekly_review = std::fs::read_to_string(weekly_review_path).unwrap();
        assert!(weekly_review.contains("## Strategic Context Snapshot"));
        assert!(weekly_review.contains("## Macro Gravity Snapshot"));
        assert!(weekly_review.contains("## Cognitive Calibration Snapshot"));
        assert!(weekly_review.contains("Boundary: snapshot only"));
        assert!(weekly_review.contains("does not generate trade signals"));
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

use anyhow::{Context, Result};
use std::sync::Arc;

use crate::config;
use crate::features::radar::application::delivery_plan::{
    RadarDeliveryInput, RadarDeliveryPlanner,
};
use crate::features::radar::application::execution_gate::TradingLimits;
use crate::features::radar::application::provider::MarketDataProvider;
use crate::features::radar::domain::rules::{
    ParsedRules as DomainParsedRules, WatchlistEntry as DomainWatchlistEntry,
};
use crate::features::radar::infrastructure::radar_runtime_factory::build_radar_runtime_services;
use crate::features::radar::interface::presentation_assembler::PresentationAssembler;
use crate::features::radar::interface::report::{self, ReportRenderContext};
use crate::features::radar::interface::weekly_state_report::{
    persist_weekly_state_outputs, WeeklyMacroGravityContext, WeeklyReportContext,
};
use crate::features::research::interface::cognitive_reports::{
    credit_stress_label, enabled_asset_thesis_count, enabled_research_attention_count,
    growth_valuation_impact_label, liquidity_condition_label, macro_pressure_label,
    yield_curve_label,
};
use crate::features::research::interface::gray_rhino_report::build_gray_rhino_daily_report;
use crate::features::shared::acl::ledger_factory::build_ledger_adapter;
use crate::features::shared::acl::notification_factory::{
    load_latest_evidence_collection_status, send_telegram_with_status,
};
use crate::features::shared::application::run_status::{
    DeliveryStatus, GrayRhinoCollectionStatus, GrayRhinoProviderStatus,
};
use crate::features::shared::interface::threshold_format::format_threshold_value;

pub(crate) async fn run_pipeline(
    app_config: config::AppConfig,
    provider: Arc<dyn MarketDataProvider>,
    _mode: crate::features::radar::application::runtime_mode::ExecutionMode,
) -> Result<()> {
    let parsed_rules = app_config.get_parsed_rules();
    let domain_rules = DomainParsedRules::from(&parsed_rules);
    let config_arc = Arc::new(app_config);
    let domain_rules_arc = Arc::new(domain_rules);
    let radar_context = crate::features::radar::application::radar::RadarRunContext::new(
        &config_arc.output.save_to,
        chrono::Local::now(),
    );
    let save_dir = radar_context.save_dir();
    if !save_dir.exists() {
        std::fs::create_dir_all(save_dir).context("Failed to create output directory")?;
    }

    let runtime_services = build_radar_runtime_services(save_dir);

    let history = runtime_services
        .persistence
        .load_recent_packets(20)
        .unwrap_or_default();
    let all_evidence = runtime_services
        .evidence_store
        .load_all()
        .unwrap_or_default()
        .into_iter()
        .filter(|record| record.is_production_eligible())
        .collect::<Vec<_>>();
    let prev_packet = history.last();

    let watchlist = config_arc
        .watchlist
        .iter()
        .map(DomainWatchlistEntry::from)
        .collect::<Vec<_>>();
    let prepared_data = crate::features::radar::application::radar::RadarPipelineUseCase::new()
        .acquire_market_data(provider, &watchlist)
        .await;
    let data_acquisition_summary = prepared_data.summary;
    let pipeline_plan = prepared_data.plan;
    let should_persist_history = pipeline_plan.should_persist_history;
    let fetched_ticker_histories = prepared_data.successful_items;
    let ticker_histories = fetched_ticker_histories
        .iter()
        .map(|(history, entry)| (history.clone(), entry))
        .collect::<Vec<_>>();
    let failed_symbols = prepared_data.failed_symbols;

    let mut outcome =
        radar_context.initial_run_outcome(load_latest_evidence_collection_status(save_dir));
    outcome.gray_rhino_collection = load_gray_rhino_collection_status(save_dir, radar_context.date);

    let ledger = Arc::new(build_ledger_adapter(radar_context.save_dir.clone()));
    let (realized_pl, positions) = ledger.get_portfolio_stats();

    if pipeline_plan.should_enter_pipeline_body {
        let packet =
            match crate::features::radar::application::radar::RadarPipelineUseCase::decide_daily(
                &ticker_histories,
                failed_symbols.len(),
                &domain_rules_arc,
                &history,
                &all_evidence,
                &positions,
                radar_context.date,
            ) {
                Ok(decision_outcome) => {
                    outcome.decisioning = decision_outcome.decisioning;
                    decision_outcome.packet
                }
                Err(e) => {
                    outcome.decisioning =
                    crate::features::radar::application::radar::build_decisioning_failure_status(
                        e.to_string(),
                    );
                    runtime_services.persistence.save_run_status(&outcome)?;
                    return Err(e);
                }
            };

        let lang = config_arc
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres_packet = PresentationAssembler::assemble(
            &packet,
            &domain_rules_arc,
            &positions,
            failed_symbols.clone(),
            lang,
        );

        let default_trading_config = config::TradingConfig {
            enabled: false,
            global_budget: 0.0,
            max_daily_budget: None,
        };
        let trading_config = config_arc
            .trading
            .as_ref()
            .unwrap_or(&default_trading_config);
        let trading_limits = TradingLimits {
            enabled: trading_config.enabled,
            global_budget: trading_config.global_budget,
            max_daily_budget: trading_config.max_daily_budget,
        };
        let delivery_plan = RadarDeliveryPlanner::plan(RadarDeliveryInput {
            packet: &packet,
            trading_limits,
            daily_traded: ledger.get_daily_traded_amount(),
            realized_pl,
            positions: &positions,
            failed_symbols: &failed_symbols,
            data_acquisition: data_acquisition_summary,
            previous_market_state: prev_packet.map(|previous| previous.market_regime.market_state),
            should_persist_history,
            timestamp: &radar_context.timestamp,
        });
        let production_records = delivery_plan
            .substantive_records
            .iter()
            .filter(|record| record.is_production_eligible())
            .cloned()
            .collect::<Vec<_>>();
        let _ = runtime_services
            .evidence_store
            .save_records(&production_records);
        runtime_services
            .persistence
            .save_execution_gate_result(&packet, &delivery_plan.execution_result)?;

        let date_str = packet.date.to_string();
        runtime_services
            .persistence
            .save_portfolio_snapshot(&delivery_plan.portfolio_snapshot, &date_str)?;
        runtime_services
            .persistence
            .save_account_snapshot(&delivery_plan.account_snapshot, &date_str)?;
        runtime_services
            .persistence
            .save_data_quality_log(&delivery_plan.data_quality_log)?;

        outcome.state_machine = Some(delivery_plan.state_machine.clone());
        outcome.date = packet.date.to_string();

        if should_persist_history {
            runtime_services.persistence.save_packet(&packet)?;
            runtime_services.persistence.save_daily_packet(&packet)?;
            if let Some(log) = &packet.transition_log {
                let _ = runtime_services
                    .transition_logger
                    .log_transition(packet.date, log);
            }
        }

        let mut report_result = report::generate_refined_report(
            &build_report_render_context(config_arc.as_ref()),
            &pres_packet,
            realized_pl,
            &positions,
            &delivery_plan.prices,
        )?;
        outcome.gray_rhino_rendering = append_gray_rhino_reference_appendix(
            &mut report_result,
            config_arc.as_ref(),
            save_dir,
            packet.date,
            pres_packet.language,
        );

        runtime_services
            .persistence
            .save_markdown_report(&report_result.archival_markdown, &pres_packet.date_str)?;
        persist_weekly_state_outputs(
            save_dir,
            &history,
            &packet,
            should_persist_history,
            &pres_packet,
            &build_weekly_report_context(config_arc.as_ref()),
        )?;

        outcome.notification = send_telegram_with_status(
            config_arc.telegram.as_ref(),
            &report_result.telegram_html_body,
        )
        .await;
        runtime_services.persistence.save_run_status(&outcome)?;
    }
    Ok(())
}

fn build_report_render_context(app_config: &config::AppConfig) -> ReportRenderContext {
    let rules = app_config.get_parsed_rules();
    ReportRenderContext {
        compact_transition_in_no_trade: app_config.output.compact_transition_evidence_in_no_trade,
        compact_stability_threshold: format_threshold_value(
            rules.trend_cohesion.gate_stability_threshold,
        ),
        compact_continuity_threshold: rules.trend_cohesion.gate_continuity_threshold.to_string(),
    }
}

fn build_weekly_report_context(app_config: &config::AppConfig) -> WeeklyReportContext {
    WeeklyReportContext {
        macro_gravity: app_config
            .macro_gravity
            .as_ref()
            .filter(|macro_gravity| macro_gravity.enable.unwrap_or(true))
            .map(|macro_gravity| WeeklyMacroGravityContext {
                rate_pressure: macro_pressure_label(macro_gravity.rate_pressure).to_string(),
                real_yield_pressure: macro_pressure_label(macro_gravity.real_yield_pressure)
                    .to_string(),
                yield_curve: yield_curve_label(macro_gravity.yield_curve).to_string(),
                credit_stress: credit_stress_label(macro_gravity.credit_stress).to_string(),
                liquidity: liquidity_condition_label(macro_gravity.liquidity).to_string(),
                growth_valuation_impact: growth_valuation_impact_label(
                    macro_gravity.growth_valuation_impact,
                )
                .to_string(),
            }),
        research_attention_entries: enabled_research_attention_count(app_config),
        asset_thesis_entries: enabled_asset_thesis_count(app_config),
    }
}

fn append_gray_rhino_reference_appendix(
    report_result: &mut report::ReportResult,
    app_config: &config::AppConfig,
    save_dir: &std::path::Path,
    as_of_date: chrono::NaiveDate,
    language: crate::features::shared::interface::i18n::Language,
) -> DeliveryStatus {
    let appendix = match build_gray_rhino_daily_report(app_config, save_dir, as_of_date, language) {
        Ok(appendix) => {
            if appendix.trim().is_empty() {
                return DeliveryStatus::Skipped;
            }
            appendix
        }
        Err(err) => {
            let reason = err.to_string();
            let appendix = gray_rhino_failure_appendix(language, &reason);
            append_gray_rhino_appendix(report_result, &appendix);
            return DeliveryStatus::Failed { reason };
        }
    };
    append_gray_rhino_appendix(report_result, &appendix);
    DeliveryStatus::Succeeded
}

fn append_gray_rhino_appendix(report_result: &mut report::ReportResult, appendix: &str) {
    let markdown_appendix = format!("\n\n---\n\n{appendix}");
    report_result.markdown_body.push_str(&markdown_appendix);
    report_result.archival_markdown.push_str(&markdown_appendix);
    report_result
        .telegram_html_body
        .push_str(&format!("\n\n{}", appendix));
}

fn load_gray_rhino_collection_status(
    save_dir: &std::path::Path,
    as_of_date: chrono::NaiveDate,
) -> GrayRhinoCollectionStatus {
    let value = std::fs::read_to_string(save_dir.join("gray_rhino_refresh_status_latest.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    let Some(value) = value else {
        return GrayRhinoCollectionStatus {
            status: "skipped".to_string(),
            ..Default::default()
        };
    };
    let date = value
        .get("date")
        .and_then(|value| value.as_str())
        .and_then(|raw| chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok());
    if date != Some(as_of_date) {
        return GrayRhinoCollectionStatus {
            status: "skipped".to_string(),
            ..Default::default()
        };
    }
    let status = value
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    GrayRhinoCollectionStatus {
        status: status.to_string(),
        date: value
            .get("date")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        sec: provider_status(&value, "sec"),
        finnhub: provider_status(&value, "finnhub"),
        fred: provider_status(&value, "fred"),
        failed_providers: value
            .get("failed_providers")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_string)
            .collect(),
    }
}

fn provider_status(value: &serde_json::Value, provider: &str) -> GrayRhinoProviderStatus {
    GrayRhinoProviderStatus {
        status: value
            .get(provider)
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string(),
        accepted: value
            .get(format!("{provider}_accepted"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
        rejected: value
            .get(format!("{provider}_rejected"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
    }
}

fn gray_rhino_failure_appendix(
    language: crate::features::shared::interface::i18n::Language,
    error: &str,
) -> String {
    match language {
        crate::features::shared::interface::i18n::Language::ZhCn => format!(
            "灰犀牛: 失败 / 未知\n- 错误: {error}\n边界声明: 灰犀牛失败只作为审计上下文展示；不改变交易、闸门、趋势或市场状态。"
        ),
        crate::features::shared::interface::i18n::Language::EnUs => format!(
            "Gray Rhino: FAILED / UNKNOWN\n- error: {error}\nBoundary: Gray Rhino failure is reported as audit context only; it does not change trading, Gate, trend, or market state."
        ),
        crate::features::shared::interface::i18n::Language::JaJp => format!(
            "灰色のサイ: 失敗 / 不明\n- エラー: {error}\n境界声明: 灰色のサイの失敗は監査コンテキストとしてのみ表示し、取引、ゲート、トレンド、市場状態を変更しない。"
        ),
    }
}

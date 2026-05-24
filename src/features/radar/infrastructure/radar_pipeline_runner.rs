use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use std::sync::Arc;

use crate::config;
use crate::features::radar::application::engine::Engine;
use crate::features::radar::application::policy::execution_gate::ExecutionGate;
use crate::features::radar::application::policy::ledger::Ledger;
use crate::features::radar::application::provider::MarketDataProvider;
use crate::features::radar::infrastructure::radar_runtime_factory::build_radar_runtime_services;
use crate::features::radar::interface::presentation_assembler::PresentationAssembler;
use crate::features::radar::interface::report;
use crate::features::radar::interface::weekly_state_report::persist_weekly_state_outputs;
use crate::features::shared::acl::notification_factory::send_telegram_with_status;
use crate::features::shared::infrastructure::run_status_reader::load_latest_evidence_collection_status;

pub(crate) async fn run_pipeline(
    app_config: config::AppConfig,
    provider: Arc<dyn MarketDataProvider>,
    _mode: crate::features::radar::application::policy::runtime_mode::ExecutionMode,
) -> Result<()> {
    let parsed_rules = app_config.get_parsed_rules();
    let config_arc = Arc::new(app_config);
    let rules_arc = Arc::new(parsed_rules);
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
        .unwrap_or_default();
    let prev_packet = history.last();

    let fetches = stream::iter(config_arc.watchlist.iter().filter(|w| w.enable))
        .map(|entry| {
            let provider_ref = Arc::clone(&provider);
            async move {
                (
                    provider_ref.fetch_history(&entry.symbol, None, None).await,
                    entry,
                )
            }
        })
        .buffer_unordered(10);

    let fetch_results = fetches
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|(result, entry)| {
            let symbol = entry.symbol.clone();
            (result.map(|history| (history, entry)), symbol)
        });
    let prepared_data = crate::features::radar::application::radar::RadarPipelineUseCase::new()
        .prepare_from_fetch_results(fetch_results);
    let data_acquisition_summary = prepared_data.summary;
    let pipeline_plan = prepared_data.plan;
    let should_persist_history = pipeline_plan.should_persist_history;
    let ticker_histories = prepared_data.successful_items;
    let failed_symbols = prepared_data.failed_symbols;

    let mut outcome =
        radar_context.initial_run_outcome(load_latest_evidence_collection_status(save_dir));

    let ledger = Arc::new(Ledger::new(radar_context.save_dir.clone()));
    let (realized_pl, positions) = ledger.get_portfolio_stats();

    if pipeline_plan.should_enter_pipeline_body {
        let packet = if !ticker_histories.is_empty() {
            match Engine::run_daily_pipeline(
                &ticker_histories,
                &rules_arc,
                &history,
                &all_evidence,
                &positions,
            ) {
                Ok(packet) => {
                    let decision_outcome =
                        crate::features::radar::application::radar::build_successful_decision_outcome(packet);
                    outcome.decisioning = decision_outcome.decisioning;
                    decision_outcome.packet
                }
                Err(e) => {
                    outcome.decisioning =
                        crate::features::radar::application::radar::build_decisioning_failure_status(e.to_string());
                    runtime_services.persistence.save_run_status(&outcome)?;
                    return Err(e);
                }
            }
        } else {
            outcome.decisioning =
                crate::features::radar::application::radar::build_full_fetch_failure_status(
                    failed_symbols.len(),
                );
            crate::features::radar::application::radar::build_diagnostic_packet(radar_context.date)
        };

        if let Some(ref recognition) = packet.trend_recognition {
            if let Some(ref substantive) = recognition.substantive {
                let _ = runtime_services
                    .evidence_store
                    .save_records(&substantive.records);
            }
        }

        let lang = config_arc
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let pres_packet =
            PresentationAssembler::assemble(&packet, &rules_arc, &positions, failed_symbols, lang);

        let default_trading_config = crate::config::TradingConfig {
            enabled: false,
            global_budget: 0.0,
            max_daily_budget: None,
        };
        let trading_config = config_arc
            .trading
            .as_ref()
            .unwrap_or(&default_trading_config);
        let daily_traded = ledger.get_daily_traded_amount();
        let current_exposure: f64 = positions
            .values()
            .map(|(qty, avg_price)| qty * avg_price)
            .sum();
        let buying_power = (trading_config.global_budget - current_exposure).max(0.0);
        let execution_result = ExecutionGate::gate_packet(
            &packet,
            trading_config,
            daily_traded,
            buying_power,
            current_exposure,
        );
        runtime_services
            .persistence
            .save_execution_gate_result(&packet, &execution_result)?;

        let date_str = packet.date.to_string();
        let portfolio_snapshot =
            crate::features::radar::application::radar::build_portfolio_snapshot(
                &date_str,
                realized_pl,
                current_exposure,
                &positions,
            );
        runtime_services
            .persistence
            .save_portfolio_snapshot(&portfolio_snapshot, &date_str)?;

        let failed_fetch_count = pres_packet
            .data_alert
            .as_ref()
            .map(|alert| alert.symbols.len())
            .unwrap_or(0);
        let account_snapshot = crate::features::radar::application::radar::build_account_snapshot(
            crate::features::radar::application::radar::AccountSnapshotInput {
                date: &date_str,
                global_budget: trading_config.global_budget,
                max_daily_budget: trading_config.max_daily_budget,
                daily_traded,
                buying_power,
                current_exposure,
                realized_pl,
                failed_fetch_count,
            },
        );
        runtime_services
            .persistence
            .save_account_snapshot(&account_snapshot, &date_str)?;

        let failed_symbols_for_log = pres_packet
            .data_alert
            .as_ref()
            .map(|alert| alert.symbols.clone())
            .unwrap_or_default();
        let data_quality_log = crate::features::radar::application::radar::build_data_quality_log(
            &radar_context.timestamp,
            &date_str,
            data_acquisition_summary,
            &failed_symbols_for_log,
        );
        runtime_services
            .persistence
            .save_data_quality_log(&data_quality_log)?;

        outcome.state_machine = Some(
            crate::features::radar::application::radar::build_state_machine_summary(
                prev_packet.map(|p| p.market_regime.market_state),
                packet.market_regime.market_state,
                packet.market_regime.transition_audit.as_ref(),
                should_persist_history,
            ),
        );
        outcome.date = packet.date.to_string();

        if should_persist_history {
            runtime_services.persistence.save_packet(&packet)?;
            runtime_services.persistence.save_daily_packet(&packet)?;
            if let Some(log) = &packet.transition_log {
                let _ = runtime_services.transition_logger.log_transition(log);
            }
        }

        let prices: std::collections::HashMap<String, f64> = packet
            .assets
            .iter()
            .map(|a| (a.symbol.clone(), a.price))
            .collect();
        let report_result = report::generate_refined_report(
            &config_arc,
            &pres_packet,
            realized_pl,
            &positions,
            &prices,
        )?;

        runtime_services
            .persistence
            .save_markdown_report(&report_result.archival_markdown, &pres_packet.date_str)?;
        persist_weekly_state_outputs(
            save_dir,
            &history,
            &packet,
            should_persist_history,
            &pres_packet,
            config_arc.as_ref(),
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

use anyhow::{Context, Result};
use chrono::Datelike;
use std::sync::Arc;
use uuid::Uuid;

use crate::config;
use crate::features::radar::acl::evidence_store_factory::build_radar_evidence_store;
use crate::features::radar::application::delivery_plan::{
    RadarDeliveryInput, RadarDeliveryPlanner,
};
use crate::features::radar::application::execution_gate::TradingLimits;
use crate::features::radar::application::provider::MarketDataProvider;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::market_change_driver::{
    build_market_change_driver, MarketChangeSnapshot,
};
use crate::features::radar::domain::rules::{
    ParsedRules as DomainParsedRules, WatchlistEntry as DomainWatchlistEntry,
};
use crate::features::radar::infrastructure::radar_runtime_factory::build_radar_runtime_services;
use crate::features::radar::interface::interpretation_read_model::{
    build_interpretation_layer_view_model, collect_subjects, derive_expectation_quality,
    derive_gravity_data_quality, derive_gravity_data_quality_reason, derive_trend_state,
    has_supply_pressure, InterpretationLayerReadModelInput, InterpretationNarrativeSignal,
};
use crate::features::radar::interface::market_interpretation_read_model::{
    build_leader_observation, build_leader_persistence_view_model,
    build_leadership_snapshot_view_model, LeaderPersistenceReadModelInput,
};
use crate::features::radar::interface::presentation::PresentationPacket;
use crate::features::radar::interface::presentation_assembler::PresentationAssembler;
use crate::features::radar::interface::report::{self, ReportRenderContext};
use crate::features::radar::interface::signal_context_event_read_model::{
    build_signal_context_event_read_model, SignalContextEventReadModelInput,
};
use crate::features::radar::interface::weekly_state_report::{
    persist_weekly_state_outputs, WeeklyMacroGravityContext, WeeklyReportContext,
};
use crate::features::research::application::gray_rhino_daily_report::{
    GrayRhinoDailyReportViewModel, GrayRhinoSnapshotPersistence,
};
use crate::features::research::application::gray_rhino_monitoring_state::{
    GrayRhinoMonitoringDirection, GrayRhinoMonitoringStatus,
};
use crate::features::research::domain::gray_rhino::{GrayRhinoAssessment, RhinoEscalationState};
use crate::features::research::domain::gray_rhino_candidate::GrayRhinoCandidateState;
use crate::features::research::interface::capital_absorption_report_builder::{
    build_capital_absorption_auto_snapshot_with_config,
    build_capital_absorption_ipo_queue_weekly_summary, build_capital_absorption_report_with_auto,
};
use crate::features::research::interface::capital_absorption_supply_phase_read_model::build_supply_phase_view_model_from_snapshot;
use crate::features::research::interface::cognitive_reports::{
    build_expectation_layer_report_with_config, build_expectation_layer_weekly_summary_with_config,
    build_flow_layer_weekly_summary, credit_stress_label, enabled_asset_thesis_count,
    enabled_research_attention_count, growth_valuation_impact_label, liquidity_condition_label,
    macro_pressure_label, yield_curve_label,
};
use crate::features::research::interface::expectation_report_builder::build_expectation_layer_snapshot_from_config;
use crate::features::research::interface::gray_rhino_report::{
    build_gray_rhino_daily_report, build_gray_rhino_daily_report_view_model,
    render_gray_rhino_daily_report,
};
use crate::features::research::interface::valuation_gravity_report_builder::{
    build_valuation_gravity_observation_with_auto, build_valuation_gravity_report_with_auto,
};
use crate::features::shared::acl::ledger_factory::build_ledger_adapter;
use crate::features::shared::acl::notification_factory::{
    load_latest_evidence_collection_status, send_telegram_with_status,
};
use crate::features::shared::application::run_status::{
    DeliveryStatus, GrayRhinoCollectionStatus, GrayRhinoProviderStatus,
};
use crate::features::shared::interface::i18n::get_dictionary;
use crate::features::shared::interface::threshold_format::format_threshold_value;

pub(crate) async fn run_pipeline(
    app_config: config::AppConfig,
    provider: Arc<dyn MarketDataProvider>,
    mode: crate::features::radar::application::runtime_mode::ExecutionMode,
) -> Result<()> {
    run_pipeline_for_report_date(
        app_config,
        provider,
        mode,
        chrono::Local::now().date_naive(),
    )
    .await
}

fn apply_history_baseline_downgrade(
    pres_packet: &mut crate::features::radar::interface::presentation::PresentationPacket,
) {
    if let Some(persistence) = pres_packet.leader_persistence.as_mut() {
        persistence.persistence_days = 0;
        persistence.persistence_value = "BASELINE_UNAVAILABLE".to_string();
        persistence.observed_days_value = "BASELINE_UNAVAILABLE".to_string();
        persistence.breakout_continuity_value = "BASELINE_UNAVAILABLE".to_string();
        persistence.change_from_yesterday_value = "BASELINE_UNAVAILABLE".to_string();
        persistence.leadership_score_value = "BASELINE_UNAVAILABLE".to_string();
        persistence.leadership_score = 0.0;
    }
}

/// テスト時だけ基準日を注入し、通常の CLI 実行では現在日付を使う。
pub(crate) async fn run_pipeline_for_report_date(
    app_config: config::AppConfig,
    provider: Arc<dyn MarketDataProvider>,
    _mode: crate::features::radar::application::runtime_mode::ExecutionMode,
    report_date: chrono::NaiveDate,
) -> Result<()> {
    let parsed_rules = app_config.get_parsed_rules();
    let domain_rules = DomainParsedRules::from(&parsed_rules);
    let config_arc = Arc::new(app_config);
    let domain_rules_arc = Arc::new(domain_rules);
    let save_dir = std::path::PathBuf::from(&config_arc.output.save_to);
    let now = chrono::Local::now();
    let market_date = recent_trading_dates(report_date)
        .into_iter()
        .filter(|date| *date <= report_date)
        .max()
        .unwrap_or(report_date);
    let radar_context = crate::features::radar::application::radar::RadarRunContext {
        date: market_date,
        timestamp: now.to_rfc3339(),
    };
    let save_dir = save_dir.as_path();
    if !save_dir.exists() {
        std::fs::create_dir_all(save_dir).context("Failed to create output directory")?;
    }

    let runtime_services = build_radar_runtime_services(save_dir);
    let evidence_store = build_radar_evidence_store(save_dir);
    if runtime_services
        .persistence
        .legacy_history_migration_needed()?
    {
        runtime_services.persistence.migrate_legacy_history()?;
    }
    let history_state_for_resolution = runtime_services
        .persistence
        .load_observation_history_state()?;
    let state_cycle_id = history_state_for_resolution
        .as_ref()
        .filter(|state| !state.cycle_id.is_empty())
        .map(|state| state.cycle_id.clone());
    let cycle_id_for_resolution = match state_cycle_id {
        Some(cycle_id) => Some(cycle_id),
        None => runtime_services
            .persistence
            .latest_cycle_id_before(radar_context.date)?,
    };

    let previous_snapshot_resolution = runtime_services
        .persistence
        .resolve_previous_snapshot_from_history(
            radar_context.date,
            cycle_id_for_resolution.as_deref(),
        )?;
    let resolved_baseline_packet = previous_snapshot_resolution.snapshot.clone();
    let baseline_packet = previous_snapshot_resolution
        .formal_snapshot
        .as_ref()
        .filter(|snapshot| snapshot.source_status == "complete")
        .and(resolved_baseline_packet);
    let previous_trading_date = previous_snapshot_resolution.previous_market_date;
    let history = runtime_services
        .persistence
        .load_recent_packets_before(radar_context.date, 20)?;
    let pipeline_history = baseline_packet.iter().cloned().collect::<Vec<_>>();
    let leader_observations = runtime_services.persistence.load_leader_observations()?;
    let all_evidence = evidence_store
        .load_all()
        .unwrap_or_default()
        .into_iter()
        .filter(|record| record.is_production_eligible())
        .collect::<Vec<_>>();
    let prev_packet = baseline_packet.as_ref();

    let watchlist = config_arc
        .watchlist
        .iter()
        .map(DomainWatchlistEntry::from)
        .collect::<Vec<_>>();
    let watch_symbols = watchlist
        .iter()
        .map(|entry| entry.symbol.clone())
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

    let ledger = Arc::new(build_ledger_adapter(save_dir.to_path_buf()));
    let (realized_pl, positions) = ledger.get_portfolio_stats();

    if pipeline_plan.should_enter_pipeline_body {
        let mut packet =
            match crate::features::radar::application::radar::RadarPipelineUseCase::decide_daily(
                &ticker_histories,
                failed_symbols.len(),
                &domain_rules_arc,
                &pipeline_history,
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

        if baseline_packet.is_none() {
            packet.transition_log = Some(
                crate::features::radar::domain::transition_log::StateTransitionLog::baseline_unavailable(
                    &packet,
                ),
            );
        }

        let lang = config_arc
            .output
            .language
            .unwrap_or(crate::features::shared::interface::i18n::Language::ZhCn);
        let mut pres_packet = PresentationAssembler::assemble(
            &packet,
            &domain_rules_arc,
            &positions,
            failed_symbols.clone(),
            lang,
        );
        let dict = get_dictionary(lang);
        let expectation_snapshot =
            build_expectation_layer_snapshot_from_config(config_arc.as_ref());
        let gravity_observation =
            build_valuation_gravity_observation_with_auto(config_arc.as_ref(), packet.date).await?;
        let capital_absorption_snapshot: Option<
            crate::features::research::domain::capital_absorption::CapitalAbsorptionAutoSnapshot,
        > = build_capital_absorption_auto_snapshot_with_config(
            config_arc.as_ref(),
            packet.date,
            14,
        )
        .await;
        let current_supply_phase =
            build_supply_phase_view_model_from_snapshot(capital_absorption_snapshot.as_ref(), lang);
        let current_supply_snapshot =
            crate::features::research::interface::capital_absorption_supply_phase_read_model::build_supply_snapshot(
                capital_absorption_snapshot.as_ref(),
            );
        let mut previous_presentation = prev_packet.map(|previous| {
            PresentationAssembler::assemble(
                previous,
                &domain_rules_arc,
                &positions,
                failed_symbols.clone(),
                lang,
            )
        });
        let previous_capital_absorption_snapshot = match prev_packet {
            Some(previous) => {
                build_capital_absorption_auto_snapshot_with_config(
                    config_arc.as_ref(),
                    previous.date,
                    14,
                )
                .await
            }
            None => None,
        };
        let previous_supply_phase = build_supply_phase_view_model_from_snapshot(
            previous_capital_absorption_snapshot.as_ref(),
            lang,
        );
        let previous_supply_snapshot = previous_capital_absorption_snapshot
            .as_ref()
            .map(|snapshot| {
                crate::features::research::interface::capital_absorption_supply_phase_read_model::build_supply_snapshot(Some(snapshot))
            })
            .unwrap_or_else(|| current_supply_snapshot.clone());
        if let (Some(previous_packet), Some(previous_pres)) =
            (prev_packet, previous_presentation.as_mut())
        {
            let previous_packet_date = previous_packet.date;
            let previous_future_calendar = std::thread::spawn(move || {
                crate::features::research::interface::macro_event_calendar_adapter::load_macro_event_calendar_from_env(previous_packet_date)
            })
            .join()
            .unwrap_or_else(|_| {
                crate::features::research::interface::macro_event_calendar_adapter::MacroEventCalendarReadModel::unavailable(
                    previous_packet_date,
                    "macro-event-calendar-connector".to_string(),
                )
            });
            let previous_gravity_observation = build_valuation_gravity_observation_with_auto(
                config_arc.as_ref(),
                previous_packet_date,
            )
            .await?;
            let previous_gray_rhino_daily_report = build_gray_rhino_daily_report_view_model(
                config_arc.as_ref(),
                save_dir,
                previous_packet_date,
                GrayRhinoSnapshotPersistence::ReadOnly,
            )
            .ok();
            previous_pres.interpretation_layer = Some(build_packet_interpretation_layer(
                previous_packet,
                previous_pres,
                &expectation_snapshot,
                &previous_gravity_observation,
                previous_capital_absorption_snapshot.as_ref(),
                previous_gray_rhino_daily_report.as_ref(),
                &previous_future_calendar,
                lang,
                &dict,
            ));
        }
        let previous_leadership_snapshot = previous_presentation
            .as_ref()
            .map(|previous| build_leadership_snapshot_view_model(previous, lang));
        let future_calendar = std::thread::spawn(move || {
            crate::features::research::interface::macro_event_calendar_adapter::load_macro_event_calendar_from_env(packet.date)
        })
            .join()
            .unwrap_or_else(|_| {
                crate::features::research::interface::macro_event_calendar_adapter::MacroEventCalendarReadModel::unavailable(
                    packet.date,
                    "macro-event-calendar-connector".to_string(),
                )
            });
        let future_context =
            build_signal_context_event_read_model(SignalContextEventReadModelInput {
                as_of_date: packet.date,
                expectation_snapshot: Some(&expectation_snapshot),
                future_calendar: Some(&future_calendar),
            });
        let gray_rhino_daily_report = build_gray_rhino_daily_report_view_model(
            config_arc.as_ref(),
            save_dir,
            packet.date,
            GrayRhinoSnapshotPersistence::ReadOnly,
        )
        .ok();
        let (expectation_quality, expectation_quality_reason) =
            derive_expectation_quality(&expectation_snapshot);
        let interpretation_signal = InterpretationNarrativeSignal {
            trend_state: derive_trend_state(pres_packet.transition_evidence.as_ref()),
            trend_available: pres_packet.transition_evidence.is_some(),
            expectation_quality,
            expectation_quality_reason,
            gravity_data_quality: derive_gravity_data_quality(&gravity_observation),
            gravity_data_quality_reason: derive_gravity_data_quality_reason(&gravity_observation),
            gravity_status: gravity_observation
                .snapshot
                .assets
                .iter()
                .filter_map(|asset| asset.gravity)
                .find(|gravity| {
                    matches!(
                        *gravity,
                        crate::features::research::domain::valuation_gravity::GravityStatus::Fair
                            | crate::features::research::domain::valuation_gravity::GravityStatus::Undervalued
                            | crate::features::research::domain::valuation_gravity::GravityStatus::DeepUndervalued
                    )
                })
                .or_else(|| {
                    gravity_observation
                        .snapshot
                        .assets
                        .iter()
                        .find_map(|asset| asset.gravity)
                }),
            supply_pressure: capital_absorption_snapshot
                .as_ref()
                .is_some_and(has_supply_pressure),
            supply_available: capital_absorption_snapshot.is_some(),
            flow_acceleration: packet.market_features.flow_acceleration,
            gray_rhino_escalated: gray_rhino_daily_report.as_ref().is_some_and(|view_model| {
                derive_gray_rhino_escalated_from_daily_report(
                    view_model.assessment.as_ref(),
                    &view_model.monitoring_statuses,
                )
            }),
        };
        let subjects = collect_subjects(&expectation_snapshot, &gravity_observation);
        pres_packet.interpretation_layer = Some(build_interpretation_layer_view_model(
            InterpretationLayerReadModelInput {
                as_of_date: packet.date,
                subjects: &subjects,
                signal: interpretation_signal,
                future_context,
                decision_summary: Some(&pres_packet.decision_summary),
                language: lang,
                dict: &dict,
            },
        ));
        let mut current_leadership_snapshot =
            build_leadership_snapshot_view_model(&pres_packet, lang);
        if baseline_packet.is_none() {
            current_leadership_snapshot.leadership_confidence_value = "LOW".to_string();
        }
        pres_packet.leadership_snapshot = Some(current_leadership_snapshot.clone());
        pres_packet.signal_summary.supply_phase_label = current_supply_phase.phase_label.clone();
        pres_packet.signal_summary.supply_phase_value = current_supply_phase.phase_value.clone();
        let previous_market_interpretation = match (
            prev_packet,
            previous_presentation.as_ref(),
            previous_leadership_snapshot.as_ref(),
        ) {
            (Some(previous_packet), Some(previous_presentation), Some(previous_snapshot)) => {
                crate::features::radar::interface::market_interpretation_read_model::build_market_interpretation_view_model(
                    previous_packet,
                    previous_presentation,
                    previous_snapshot,
                    lang,
                )
            }
            _ => None,
        };
        pres_packet.market_interpretation =
            crate::features::radar::interface::market_interpretation_read_model::build_market_interpretation_view_model_with_baseline(
                &packet,
                &pres_packet,
                &current_leadership_snapshot,
                previous_market_interpretation.as_ref(),
                previous_snapshot_resolution.formal_snapshot.as_ref(),
                lang,
            );
        if baseline_packet.is_none() {
            if let Some(interpretation) = pres_packet.market_interpretation.as_mut() {
                interpretation.rotation_type_value = "BASELINE_UNAVAILABLE".to_string();
                interpretation.rotation_from_values.clear();
                interpretation.rotation_to_values.clear();
                interpretation.rotation_interpretation_value =
                    "Rotation cannot be determined without a valid baseline.".to_string();
                interpretation.narrative_values = vec![
                    "Cross-day narrative cannot be determined without a valid baseline."
                        .to_string(),
                ];
            }
        }
        pres_packet.leader_persistence =
            build_leader_persistence_view_model(LeaderPersistenceReadModelInput {
                persisted_observations: &leader_observations,
                current_packet: &packet,
                current_presentation: &pres_packet,
                language: lang,
                baseline_date: previous_trading_date,
                baseline_status: match previous_snapshot_resolution.status {
                    crate::features::radar::infrastructure::persistence::PreviousSnapshotStatus::Available => "AVAILABLE",
                    crate::features::radar::infrastructure::persistence::PreviousSnapshotStatus::BaselineUnavailable => "BASELINE_UNAVAILABLE",
                },
                formal_baseline: previous_snapshot_resolution.formal_snapshot.as_ref(),
            });
        if baseline_packet.is_none() {
            apply_history_baseline_downgrade(&mut pres_packet);
        }
        pres_packet.market_change_log = Some(build_market_change_log_view_model(
            prev_packet,
            previous_presentation.as_ref(),
            &packet,
            &pres_packet,
            &current_leadership_snapshot,
            previous_leadership_snapshot.as_ref(),
            &current_supply_phase,
            previous_capital_absorption_snapshot
                .as_ref()
                .map(|_| &previous_supply_phase),
            &current_supply_snapshot,
            &previous_supply_snapshot,
            pres_packet.market_interpretation.as_ref(),
            previous_market_interpretation.as_ref(),
            lang,
            previous_snapshot_resolution.formal_snapshot.as_ref(),
        ));

        let history_before = runtime_services
            .persistence
            .load_observation_history_entries()?;
        let history_state_before = history_state_for_resolution.clone();
        let cycle_id = cycle_id_for_resolution
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        if should_persist_history
            && history_state_would_regress(
                history_state_before.as_ref(),
                history_before.len(),
                packet.date,
            )
        {
            outcome.date = packet.date.to_string();
            outcome.decisioning = DeliveryStatus::Failed {
                reason: "HISTORY_REGRESSION".to_string(),
            };
            outcome.data_quality = "HISTORY_REGRESSION".to_string();
            runtime_services.persistence.save_run_status(&outcome)?;
            return Ok(());
        }
        let same_market_date = history_before.iter().any(|entry| entry.date == packet.date);
        let expected_history_count = history_before.len() + usize::from(!same_market_date);
        if should_persist_history {
            let preflight_progression =
                crate::features::radar::domain::observation_timeline::validate_history_progression(
                    history_before.len(),
                    expected_history_count,
                    same_market_date,
                );
            if matches!(
                preflight_progression,
                crate::features::radar::domain::observation_timeline::HistoryProgression::HistoryRegression
                    | crate::features::radar::domain::observation_timeline::HistoryProgression::InvalidGap
            ) {
                outcome.date = packet.date.to_string();
                outcome.decisioning = DeliveryStatus::Failed {
                    reason: "HISTORY_REGRESSION".to_string(),
                };
                outcome.data_quality = "HISTORY_REGRESSION".to_string();
                runtime_services.persistence.save_run_status(&outcome)?;
                return Ok(());
            }
        }
        let mut history_write_transaction = should_persist_history
            .then(|| {
                runtime_services
                    .persistence
                    .begin_history_write_transaction(packet.date, &cycle_id)
            })
            .transpose()?;
        let build_snapshot_probe = |continuity: usize, degraded: bool| {
            crate::features::radar::infrastructure::persistence::TradingDaySnapshot {
                schema_version: "1".to_string(),
                market_date: packet.date,
                report_date,
                as_of_date: packet.date,
                generated_at: radar_context.timestamp.clone(),
                run_id: Uuid::new_v4().to_string(),
                cycle_id: cycle_id.clone(),
                snapshot_id: format!("{}-{}", cycle_id, packet.date),
                is_valid_trading_day: true,
                source_status: if degraded { "degraded" } else { "complete" }.to_string(),
                market_state: format!("{:?}", packet.market_regime.market_state),
                decision_state: if degraded { "NO_TRADE" } else { "OBSERVE" }.to_string(),
                new_position_limit: if degraded {
                    0.0
                } else {
                    config_arc
                        .trading
                        .as_ref()
                        .map(|config| {
                            config
                                .max_daily_budget
                                .unwrap_or(config.global_budget)
                                .max(0.0)
                        })
                        .unwrap_or(0.0)
                },
                breadth: if packet.market_features.total_count == 0 {
                    0.0
                } else {
                    packet.market_features.up_count as f64
                        / packet.market_features.total_count as f64
                        * 100.0
                },
                breadth_classification: Some(
                    pres_packet.signal_summary.breadth_semantic_value.clone(),
                ),
                confidence: packet.market_features.system_confidence,
                supply_phase: pres_packet.signal_summary.supply_phase_value.clone(),
                risk_state: format!("{:?}", packet.market_regime.risk_overlay),
                primary_leader: (!current_leadership_snapshot.primary_leader_value.is_empty())
                    .then_some(current_leadership_snapshot.primary_leader_value.clone()),
                secondary_leaders: current_leadership_snapshot.secondary_leaders_values.clone(),
                breakouts: build_breakout_snapshot_projection(&pres_packet.breakout_summary.items),
                stability: packet.market_features.stability_score,
                continuity,
                cycle_length_days: continuity,
                reset_event: None,
                data_quality: serde_json::json!({
                    "trend": if degraded { "DEGRADED" } else { "HEALTHY" },
                    "history": if degraded { "UNAVAILABLE" } else { "HEALTHY" }
                }),
            }
        };
        if should_persist_history {
            let snapshot_probe =
                build_snapshot_probe(expected_history_count, baseline_packet.is_none());
            if let Err(error) = runtime_services
                .persistence
                .validate_trading_day_snapshot_conflict(&snapshot_probe)
            {
                let reason = error.to_string();
                outcome.date = packet.date.to_string();
                outcome.decisioning = DeliveryStatus::Failed {
                    reason: reason.clone(),
                };
                outcome.data_quality = reason;
                runtime_services.persistence.save_run_status(&outcome)?;
                return Ok(());
            }
        }
        if should_persist_history {
            let expected_history_count = history_before.len() + usize::from(!same_market_date);
            let consistency_error = crate::features::radar::domain::observation_timeline::daily_observation_consistency_gate(
                history_state_before
                    .as_ref()
                    .map(|state| state.count)
                    .unwrap_or(history_before.len()),
                expected_history_count,
                !same_market_date,
                baseline_packet.is_some(),
                pres_packet
                    .leadership_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.leadership_confidence_value.as_str())
                    .unwrap_or("LOW"),
            )
            .err()
            .or_else(|| {
                let mut allowed_leaders = current_leadership_snapshot.secondary_leaders_values.clone();
                if !current_leadership_snapshot.primary_leader_value.is_empty() {
                    allowed_leaders.push(current_leadership_snapshot.primary_leader_value.clone());
                }
                pres_packet
                    .market_interpretation
                    .as_ref()
                    .into_iter()
                    .flat_map(|interpretation| {
                        interpretation
                            .primary_values
                            .iter()
                            .chain(interpretation.supporting_values.iter())
                    })
                    .for_each(|leader| {
                        if !allowed_leaders.contains(leader) {
                            allowed_leaders.push(leader.clone());
                        }
                    });
                crate::features::radar::domain::observation_timeline::cross_layer_consistency_gate(
                    packet.date,
                    baseline_packet.as_ref().map(|previous| previous.date),
                    baseline_packet.is_some(),
                    &current_supply_phase.phase_value,
                    pres_packet
                        .market_change_log
                        .as_ref()
                        .map(|change_log| change_log.supply_phase_value.as_str())
                        .unwrap_or("UNAVAILABLE"),
                    pres_packet
                        .market_interpretation
                        .as_ref()
                        .map(|interpretation| interpretation.narrative_values.as_slice())
                        .unwrap_or(&[]),
                    &allowed_leaders,
                )
                .err()
            });
            if let Some(reason) = consistency_error {
                outcome.date = packet.date.to_string();
                outcome.decisioning = DeliveryStatus::Failed {
                    reason: reason.to_string(),
                };
                outcome.data_quality = reason.to_string();
                runtime_services.persistence.save_run_status(&outcome)?;
                return Ok(());
            }
        }
        if should_persist_history {
            runtime_services.persistence.save_packet(&packet)?;
            if let Some(observation) = build_leader_observation(&packet, &pres_packet) {
                runtime_services
                    .persistence
                    .save_leader_observation(&observation)?;
            }
            if let Some(timeline_entry) =
                build_observation_timeline_entry(&packet, &pres_packet, &current_supply_phase)
            {
                let expected_dates = recent_trading_dates(packet.date);
                runtime_services
                    .persistence
                    .save_observation_timeline_entry(timeline_entry, &expected_dates)?;
            }
        }
        let history_after = runtime_services
            .persistence
            .load_observation_history_entries()?;
        let consistency_gate_failed = if should_persist_history {
            let previous_count = history_state_before
                .as_ref()
                .map(|state| state.count)
                .unwrap_or(history_before.len());
            crate::features::radar::domain::observation_timeline::daily_observation_consistency_gate(
                previous_count,
                history_after.len(),
                !same_market_date,
                baseline_packet.is_some(),
                pres_packet
                    .leadership_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.leadership_confidence_value.as_str())
                    .unwrap_or("LOW"),
            )
            .err()
        } else {
            None
        };
        let history_progression =
            crate::features::radar::domain::observation_timeline::validate_history_progression(
                history_before.len(),
                history_after.len(),
                same_market_date,
            );
        let mut history_blocked = should_persist_history
            && matches!(
                history_progression,
                crate::features::radar::domain::observation_timeline::HistoryProgression::HistoryRegression
                | crate::features::radar::domain::observation_timeline::HistoryProgression::InvalidGap
            );
        history_blocked |= consistency_gate_failed.is_some();
        if should_persist_history {
            if let Some(previous_state) = history_state_before.as_ref() {
                let state_regressed = if packet.date > previous_state.last_market_date {
                    history_after.len() != previous_state.count + 1
                } else if packet.date == previous_state.last_market_date {
                    history_after.len() != previous_state.count
                } else {
                    true
                };
                history_blocked |= state_regressed;
            }
        }
        let safe_downgrade = history_blocked || baseline_packet.is_none();
        if history_blocked {
            outcome.date = packet.date.to_string();
            outcome.decisioning = DeliveryStatus::Failed {
                reason: "HISTORY_REGRESSION".to_string(),
            };
            outcome.data_quality = "HISTORY_REGRESSION".to_string();
            runtime_services.persistence.save_run_status(&outcome)?;
            return Ok(());
        }
        if safe_downgrade {
            if let Some(snapshot) = pres_packet.leadership_snapshot.as_mut() {
                snapshot.leadership_confidence_value = "LOW".to_string();
            }
            if let Some(interpretation) = pres_packet.market_interpretation.as_mut() {
                interpretation.leadership_classification_value = "LOW".to_string();
                interpretation.overall_confidence_value = "LOW".to_string();
                interpretation.trend_confidence_value = "LOW".to_string();
                interpretation.rotation_type_value = "UNAVAILABLE".to_string();
                interpretation.rotation_from_values.clear();
                interpretation.rotation_to_values.clear();
                interpretation.narrative_values = vec![
                    "Cross-day narrative cannot be determined from the current history status."
                        .to_string(),
                ];
            }
            apply_history_baseline_downgrade(&mut pres_packet);
        }
        if history_blocked {
            if let Some(change_log) = pres_packet.market_change_log.as_mut() {
                change_log.baseline_status = "HISTORY_REGRESSION".to_string();
                change_log.change_status = "HISTORY_REGRESSION".to_string();
                change_log.leader_value = current_leadership_snapshot.primary_leader_value.clone();
                change_log.risk_value = "HISTORY_REGRESSION".to_string();
                change_log.confidence_value = "UNAVAILABLE".to_string();
                change_log.structural_change_value = "UNAVAILABLE".to_string();
                change_log.change_level = "UNAVAILABLE".to_string();
                change_log.change_drivers.clear();
                change_log.unchanged_dimensions.clear();
                change_log.summary = "HISTORY_REGRESSION".to_string();
                change_log.summary_values = vec![
                    "History regression detected; cross-day conclusions are unavailable."
                        .to_string(),
                ];
            }
        } else if should_persist_history {
            runtime_services
                .persistence
                .save_observation_history_state(
                    &crate::features::radar::infrastructure::persistence::ObservationHistoryState {
                        count: history_after.len(),
                        last_market_date: packet.date,
                        cycle_id: cycle_id.clone(),
                    },
                )?;
        }

        if should_persist_history {
            let trading_day_snapshot =
                crate::features::radar::infrastructure::persistence::TradingDaySnapshot {
                    schema_version: "1".to_string(),
                    market_date: packet.date,
                    report_date,
                    as_of_date: packet.date,
                    generated_at: radar_context.timestamp.clone(),
                    run_id: Uuid::new_v4().to_string(),
                    cycle_id: cycle_id.clone(),
                    snapshot_id: format!("{}-{}", cycle_id, packet.date),
                    is_valid_trading_day: true,
                    source_status: if safe_downgrade {
                        "degraded"
                    } else {
                        "complete"
                    }
                    .to_string(),
                    market_state: format!("{:?}", packet.market_regime.market_state),
                    decision_state: if safe_downgrade {
                        "NO_TRADE"
                    } else {
                        "OBSERVE"
                    }
                    .to_string(),
                    new_position_limit: if safe_downgrade {
                        0.0
                    } else {
                        config_arc
                            .trading
                            .as_ref()
                            .map(|config| {
                                config
                                    .max_daily_budget
                                    .unwrap_or(config.global_budget)
                                    .max(0.0)
                            })
                            .unwrap_or(0.0)
                    },
                    breadth: if packet.market_features.total_count == 0 {
                        0.0
                    } else {
                        packet.market_features.up_count as f64
                            / packet.market_features.total_count as f64
                            * 100.0
                    },
                    breadth_classification: Some(
                        pres_packet.signal_summary.breadth_semantic_value.clone(),
                    ),
                    confidence: packet.market_features.system_confidence,
                    supply_phase: pres_packet.signal_summary.supply_phase_value.clone(),
                    risk_state: format!("{:?}", packet.market_regime.risk_overlay),
                    primary_leader: (!current_leadership_snapshot.primary_leader_value.is_empty())
                        .then_some(current_leadership_snapshot.primary_leader_value.clone()),
                    secondary_leaders: current_leadership_snapshot.secondary_leaders_values.clone(),
                    breakouts: build_breakout_snapshot_projection(
                        &pres_packet.breakout_summary.items,
                    ),
                    stability: packet.market_features.stability_score,
                    continuity: history_after.len(),
                    cycle_length_days: history_after.len(),
                    reset_event: None,
                    data_quality: serde_json::json!({
                        "trend": if safe_downgrade { "DEGRADED" } else { "HEALTHY" },
                        "history": if safe_downgrade { "UNAVAILABLE" } else { "HEALTHY" }
                    }),
                };
            if let Err(error) = runtime_services
                .persistence
                .save_trading_day_snapshot(&trading_day_snapshot)
            {
                let reason = error.to_string();
                outcome.date = packet.date.to_string();
                outcome.decisioning = DeliveryStatus::Failed {
                    reason: reason.clone(),
                };
                outcome.data_quality = reason;
                runtime_services.persistence.save_run_status(&outcome)?;
                return Ok(());
            }
        }

        if should_persist_history {
            if let Some(log) = &packet.transition_log {
                runtime_services
                    .transition_logger
                    .log_transition(packet.date, log)?;
            }
        }

        if let Some(transaction) = history_write_transaction.take() {
            transaction.commit();
        }

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
            enabled: trading_config.enabled && !safe_downgrade,
            global_budget: if safe_downgrade {
                0.0
            } else {
                trading_config.global_budget
            },
            max_daily_budget: if safe_downgrade {
                Some(0.0)
            } else {
                trading_config.max_daily_budget
            },
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
        let _ = evidence_store.save_records(&production_records);
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
            runtime_services.persistence.save_daily_packet(&packet)?;
        }

        let mut report_context = build_report_render_context(config_arc.as_ref());
        report_context.observation_timeline = runtime_services
            .persistence
            .load_latest_observation_timeline()
            .unwrap_or_default();
        let mut report_result = report::generate_refined_report(
            &report_context,
            &pres_packet,
            realized_pl,
            &positions,
            &delivery_plan.prices,
        )?;
        append_valuation_gravity_reference_appendix(
            &mut report_result,
            config_arc.as_ref(),
            packet.date,
            pres_packet.language,
        )
        .await;
        append_capital_absorption_reference_appendix(
            &mut report_result,
            config_arc.as_ref(),
            packet.date,
            pres_packet.language,
        )
        .await;
        outcome.gray_rhino_rendering = append_gray_rhino_reference_appendix(
            &mut report_result,
            config_arc.as_ref(),
            save_dir,
            packet.date,
            &watch_symbols,
            pres_packet.language,
            gray_rhino_daily_report.as_ref(),
        );
        append_expectation_reference_appendix(
            &mut report_result,
            config_arc.as_ref(),
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
            &build_weekly_report_context(config_arc.as_ref(), save_dir, packet.date),
            outcome.state_machine.as_ref(),
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

fn build_observation_timeline_entry(
    packet: &DecisionPacket,
    presentation: &PresentationPacket,
    supply_phase: &crate::features::research::interface::capital_absorption_supply_phase_read_model::SupplyPhaseViewModel,
) -> Option<crate::features::radar::domain::observation_timeline::ObservationTimelineEntry> {
    let leadership = presentation.leadership_snapshot.as_ref()?;
    let interpretation = presentation.market_interpretation.as_ref()?;
    Some(
        crate::features::radar::domain::observation_timeline::ObservationTimelineEntry {
            date: packet.date,
            primary_leader: leadership.primary_leader_value.clone(),
            secondary_leaders: leadership.secondary_leaders_values.clone(),
            breadth_score: interpretation
                .breadth_score_value
                .parse()
                .unwrap_or_default(),
            concentration_score: interpretation
                .concentration_score_value
                .parse()
                .unwrap_or_default(),
            rotation_score: interpretation
                .rotation_score_value
                .parse()
                .unwrap_or_default(),
            confidence_index: packet.market_features.system_confidence,
            market_state: format!("{:?}", packet.market_regime.market_state),
            supply_phase: supply_phase.phase_value.clone(),
            risk_state: format!("{:?}", packet.market_regime.risk_overlay),
            day_type: interpretation.day_type_value.clone(),
        },
    )
}

fn recent_trading_dates(current: chrono::NaiveDate) -> Vec<chrono::NaiveDate> {
    let mut dates = Vec::with_capacity(7);
    let mut date = current;
    while dates.len()
        < crate::features::radar::domain::observation_timeline::OBSERVATION_TIMELINE_DAYS
    {
        let is_weekend = matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun);
        let is_nyse_holiday = crate::features::research::interface::macro_event_official_calendar_adapter::nyse_market_holidays(date.year()).contains(&date);
        if !is_weekend && !is_nyse_holiday {
            dates.push(date);
        }
        date -= chrono::Duration::days(1);
    }
    dates.sort_unstable();
    dates
}

#[allow(clippy::too_many_arguments)]
fn build_packet_interpretation_layer(
    packet: &DecisionPacket,
    pres_packet: &PresentationPacket,
    expectation_snapshot: &crate::features::research::interface::expectation_report_builder::ExpectationLayerSnapshot,
    gravity_observation: &crate::features::research::application::valuation_gravity::ValuationGravityObservation,
    capital_absorption_snapshot: Option<
        &crate::features::research::domain::capital_absorption::CapitalAbsorptionAutoSnapshot,
    >,
    gray_rhino_daily_report: Option<&GrayRhinoDailyReportViewModel>,
    future_calendar: &crate::features::research::interface::macro_event_calendar_adapter::MacroEventCalendarReadModel,
    language: crate::features::shared::interface::i18n::Language,
    dict: &crate::features::shared::interface::i18n::DisplayDictionary,
) -> crate::features::radar::interface::presentation::InterpretationLayerViewModel {
    let future_context = build_signal_context_event_read_model(SignalContextEventReadModelInput {
        as_of_date: packet.date,
        expectation_snapshot: Some(expectation_snapshot),
        future_calendar: Some(future_calendar),
    });
    let (expectation_quality, expectation_quality_reason) =
        derive_expectation_quality(expectation_snapshot);
    let interpretation_signal = InterpretationNarrativeSignal {
        trend_state: derive_trend_state(pres_packet.transition_evidence.as_ref()),
        trend_available: pres_packet.transition_evidence.is_some(),
        expectation_quality,
        expectation_quality_reason,
        gravity_data_quality: derive_gravity_data_quality(gravity_observation),
        gravity_data_quality_reason: derive_gravity_data_quality_reason(gravity_observation),
        gravity_status: gravity_observation
            .snapshot
            .assets
            .iter()
            .filter_map(|asset| asset.gravity)
            .find(|gravity| {
                matches!(
                    *gravity,
                    crate::features::research::domain::valuation_gravity::GravityStatus::Fair
                        | crate::features::research::domain::valuation_gravity::GravityStatus::Undervalued
                        | crate::features::research::domain::valuation_gravity::GravityStatus::DeepUndervalued
                )
            })
            .or_else(|| {
                gravity_observation
                    .snapshot
                    .assets
                    .iter()
                    .find_map(|asset| asset.gravity)
            }),
        supply_pressure: capital_absorption_snapshot
            .as_ref()
            .is_some_and(|snapshot| has_supply_pressure(snapshot)),
        supply_available: capital_absorption_snapshot.is_some(),
        flow_acceleration: packet.market_features.flow_acceleration,
        gray_rhino_escalated: gray_rhino_daily_report.as_ref().is_some_and(|view_model| {
            derive_gray_rhino_escalated_from_daily_report(
                view_model.assessment.as_ref(),
                &view_model.monitoring_statuses,
            )
        }),
    };
    let subjects = collect_subjects(expectation_snapshot, gravity_observation);
    build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
        as_of_date: packet.date,
        subjects: &subjects,
        signal: interpretation_signal,
        future_context,
        decision_summary: Some(&pres_packet.decision_summary),
        language,
        dict,
    })
}

fn build_report_render_context(app_config: &config::AppConfig) -> ReportRenderContext {
    let rules = app_config.get_parsed_rules();
    ReportRenderContext {
        compact_transition_in_no_trade: app_config.output.compact_transition_evidence_in_no_trade,
        compact_stability_threshold: format_threshold_value(
            rules.trend_cohesion.gate_stability_threshold,
        ),
        compact_continuity_threshold: rules.trend_cohesion.gate_continuity_threshold.to_string(),
        observation_timeline: None,
    }
}

fn build_weekly_report_context(
    app_config: &config::AppConfig,
    save_dir: &std::path::Path,
    as_of_date: chrono::NaiveDate,
) -> WeeklyReportContext {
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
        capital_absorption_ipo_queue: build_capital_absorption_ipo_queue_weekly_summary(
            save_dir, as_of_date,
        ),
        capital_dynamics_flow_layer: build_flow_layer_weekly_summary(
            app_config.capital_dynamics.as_ref(),
        ),
        expectation_layer: build_expectation_layer_weekly_summary_with_config(app_config),
    }
}

async fn append_valuation_gravity_reference_appendix(
    report_result: &mut report::ReportResult,
    app_config: &config::AppConfig,
    as_of_date: chrono::NaiveDate,
    language: crate::features::shared::interface::i18n::Language,
) {
    if let Ok(appendix) =
        build_valuation_gravity_report_with_auto(app_config, as_of_date, language).await
    {
        append_reference_appendix(report_result, &appendix, language);
    }
}

async fn append_capital_absorption_reference_appendix(
    report_result: &mut report::ReportResult,
    app_config: &config::AppConfig,
    as_of_date: chrono::NaiveDate,
    language: crate::features::shared::interface::i18n::Language,
) {
    let appendix =
        build_capital_absorption_report_with_auto(app_config, as_of_date, 14, language).await;
    append_reference_appendix(report_result, &appendix, language);
}

fn append_gray_rhino_reference_appendix(
    report_result: &mut report::ReportResult,
    app_config: &config::AppConfig,
    save_dir: &std::path::Path,
    as_of_date: chrono::NaiveDate,
    watch_symbols: &[String],
    language: crate::features::shared::interface::i18n::Language,
    gray_rhino_daily_report: Option<&GrayRhinoDailyReportViewModel>,
) -> DeliveryStatus {
    let appendix = match gray_rhino_daily_report {
        Some(view_model) => {
            let appendix = render_gray_rhino_daily_report(view_model, watch_symbols, language);
            if appendix.trim().is_empty() {
                return DeliveryStatus::Skipped;
            }
            appendix
        }
        None => match build_gray_rhino_daily_report(app_config, save_dir, as_of_date, language) {
            Ok(appendix) => {
                if appendix.trim().is_empty() {
                    return DeliveryStatus::Skipped;
                }
                appendix
            }
            Err(err) => {
                let reason = err.to_string();
                let appendix = gray_rhino_failure_appendix(language, &reason);
                append_gray_rhino_appendix(report_result, &appendix, language);
                return DeliveryStatus::Failed { reason };
            }
        },
    };
    append_gray_rhino_appendix(report_result, &appendix, language);
    DeliveryStatus::Succeeded
}

fn append_gray_rhino_appendix(
    report_result: &mut report::ReportResult,
    appendix: &str,
    language: crate::features::shared::interface::i18n::Language,
) {
    append_reference_appendix(report_result, appendix, language);
}

fn append_expectation_reference_appendix(
    report_result: &mut report::ReportResult,
    app_config: &config::AppConfig,
    language: crate::features::shared::interface::i18n::Language,
) {
    let appendix = build_expectation_layer_report_with_config(app_config, language);
    append_reference_appendix(report_result, &appendix, language);
}

fn append_reference_appendix(
    report_result: &mut report::ReportResult,
    appendix: &str,
    language: crate::features::shared::interface::i18n::Language,
) {
    let markdown_appendix = format!("\n\n---\n\n{appendix}");
    report_result.markdown_body.push_str(&markdown_appendix);
    report_result.archival_markdown.push_str(&markdown_appendix);
    report_result.telegram_html_body.push_str(&format!(
        "\n\n{}",
        compact_reference_appendix_for_telegram(appendix, language)
    ));
}

fn compact_reference_appendix_for_telegram(
    appendix: &str,
    language: crate::features::shared::interface::i18n::Language,
) -> String {
    const MAX_LINES: usize = 18;
    const MAX_CHARS: usize = 1400;

    let boundary_lines = appendix
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty() && is_reference_boundary_line(line))
        .collect::<Vec<_>>();
    let reserved_chars = boundary_lines
        .iter()
        .map(|line| line.len() + 1)
        .sum::<usize>();
    let detail_line_limit = MAX_LINES.saturating_sub(boundary_lines.len());
    let detail_char_limit = MAX_CHARS.saturating_sub(reserved_chars);
    let mut out = String::new();
    let mut retained = 0usize;
    let mut omitted = 0usize;
    for line in appendix.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            continue;
        }
        if is_reference_boundary_line(line) {
            continue;
        }
        if should_keep_reference_line(line)
            && retained < detail_line_limit
            && out.len() + line.len() < detail_char_limit
        {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
            retained += 1;
        } else {
            omitted += 1;
        }
    }

    for line in boundary_lines {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }

    if omitted > 0 {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&reference_appendix_digest_omission_notice(
            language, omitted,
        ));
    }
    out
}

fn reference_appendix_digest_omission_notice(
    language: crate::features::shared::interface::i18n::Language,
    omitted: usize,
) -> String {
    match language {
        crate::features::shared::interface::i18n::Language::ZhCn => format!(
            "- Telegram 摘要: 已省略 {} 行明细；归档 Markdown 保留完整 appendix。",
            omitted
        ),
        crate::features::shared::interface::i18n::Language::JaJp => format!(
            "- Telegram 要約: {} 行の詳細を省略。アーカイブ Markdown には appendix 全文を保持。",
            omitted
        ),
        crate::features::shared::interface::i18n::Language::EnUs => format!(
            "- Telegram digest: {} detail line(s) omitted; archival Markdown keeps the full appendix.",
            omitted
        ),
    }
}

fn should_keep_reference_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    is_reference_heading(trimmed)
        || is_reference_structured_line(trimmed)
        || contains_reference_status_token(trimmed)
}

fn is_reference_boundary_line(line: &str) -> bool {
    let trimmed = line.trim_start_matches([' ', '-', '*']);
    trimmed.starts_with("Boundary:")
        || trimmed.starts_with("Boundary：")
        || trimmed.starts_with("边界：")
        || trimmed.starts_with("边界声明：")
        || trimmed.starts_with("境界：")
        || trimmed.starts_with("境界声明：")
}

fn is_reference_heading(trimmed: &str) -> bool {
    trimmed.starts_with('#')
        || (!trimmed.starts_with('-') && !trimmed.contains(':') && trimmed.len() <= 80)
}

fn is_reference_structured_line(trimmed: &str) -> bool {
    if is_noisy_reference_detail(trimmed) {
        return false;
    }
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    body.contains(':') || body.contains('：')
}

fn contains_reference_status_token(trimmed: &str) -> bool {
    trimmed.contains("NO TRADE")
        || trimmed.contains("READY")
        || trimmed.contains("WATCH")
        || trimmed.contains("FAILED")
}

fn build_breakout_snapshot_projection(
    items: &[crate::features::radar::interface::presentation::BreakoutItemViewModel],
) -> serde_json::Value {
    items
        .iter()
        .map(|item| {
            let state = match item.status {
                crate::features::radar::interface::presentation::BreakoutDisplayStatus::NoBreakout => "NO_BREAKOUT",
                crate::features::radar::interface::presentation::BreakoutDisplayStatus::EmergingBreakout => "BREAKOUT_EMERGING",
                crate::features::radar::interface::presentation::BreakoutDisplayStatus::ConfirmedBreakout => "BREAKOUT_CONFIRMED",
            };
            (
                item.symbol.clone(),
                serde_json::json!({
                    "state": state,
                    "strength": item.strength_value.parse::<f64>().unwrap_or(0.0),
                    "quality": item.quality_value.parse::<f64>().unwrap_or(0.0),
                    "consecutive_days": item.consecutive_days
                }),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>()
        .into()
}

fn history_state_would_regress(
    previous_state: Option<
        &crate::features::radar::infrastructure::persistence::ObservationHistoryState,
    >,
    history_count: usize,
    current_market_date: chrono::NaiveDate,
) -> bool {
    let Some(previous_state) = previous_state else {
        return false;
    };
    current_market_date < previous_state.last_market_date || history_count != previous_state.count
}

#[allow(clippy::too_many_arguments)]
fn build_market_change_log_view_model(
    prev_packet: Option<&DecisionPacket>,
    previous_presentation: Option<
        &crate::features::radar::interface::presentation::PresentationPacket,
    >,
    packet: &DecisionPacket,
    pres_packet: &crate::features::radar::interface::presentation::PresentationPacket,
    current_leadership_snapshot: &crate::features::radar::interface::presentation::LeadershipSnapshotViewModel,
    previous_leadership_snapshot: Option<
        &crate::features::radar::interface::presentation::LeadershipSnapshotViewModel,
    >,
    current_supply_phase: &crate::features::research::interface::capital_absorption_supply_phase_read_model::SupplyPhaseViewModel,
    previous_supply_phase: Option<
        &crate::features::research::interface::capital_absorption_supply_phase_read_model::SupplyPhaseViewModel,
    >,
    current_supply_snapshot: &crate::features::research::interface::capital_absorption_supply_phase_read_model::SupplySnapshot,
    previous_supply_snapshot: &crate::features::research::interface::capital_absorption_supply_phase_read_model::SupplySnapshot,
    current_market_interpretation: Option<
        &crate::features::radar::interface::presentation::MarketInterpretationViewModel,
    >,
    previous_market_interpretation: Option<
        &crate::features::radar::interface::presentation::MarketInterpretationViewModel,
    >,
    language: crate::features::shared::interface::i18n::Language,
    formal_baseline: Option<
        &crate::features::radar::infrastructure::persistence::TradingDaySnapshot,
    >,
) -> crate::features::radar::interface::presentation::MarketChangeLogViewModel {
    fn breakout_state_signature(
        presentation: &crate::features::radar::interface::presentation::PresentationPacket,
    ) -> String {
        presentation
            .breakout_summary
            .items
            .iter()
            .map(|item| {
                let status = match item.status {
                    crate::features::radar::interface::presentation::BreakoutDisplayStatus::NoBreakout => "NO_BREAKOUT",
                    crate::features::radar::interface::presentation::BreakoutDisplayStatus::EmergingBreakout => "EMERGING",
                    crate::features::radar::interface::presentation::BreakoutDisplayStatus::ConfirmedBreakout => "CONFIRMED",
                };
                format!("{}:{status}", item.symbol)
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    let current_leader = current_leadership_snapshot.primary_leader_value.clone();
    let baseline_available = formal_baseline.is_some();
    if !baseline_available {
        let unavailable_interpretation = match language {
            crate::features::shared::interface::i18n::Language::ZhCn => {
                "无法确定上一交易日基线；本段仅展示当前观测。".to_string()
            }
            crate::features::shared::interface::i18n::Language::EnUs => {
                "Previous trading-day baseline is unavailable; current observation only."
                    .to_string()
            }
            crate::features::shared::interface::i18n::Language::JaJp => {
                "前回取引日の基線を判定できないため、現在の観測のみを表示します。".to_string()
            }
        };
        return crate::features::radar::interface::presentation::MarketChangeLogViewModel {
            baseline_status: "BASELINE_UNAVAILABLE".to_string(),
            change_status: "BASELINE_UNAVAILABLE".to_string(),
            title: "Market Change Log".to_string(),
            leader_label: "Leader".to_string(),
            leader_value: current_leader,
            breadth_label: "Breadth".to_string(),
            breadth_value: pres_packet.signal_summary.breadth_semantic_value.clone(),
            risk_label: "Risk".to_string(),
            risk_value: "BASELINE_UNAVAILABLE".to_string(),
            supply_phase_label: "Supply Phase".to_string(),
            supply_phase_value: current_supply_phase.phase_value.clone(),
            confidence_label: "Confidence".to_string(),
            confidence_value: "BASELINE_UNAVAILABLE".to_string(),
            interpretation_label: "Market Interpretation".to_string(),
            interpretation_value: unavailable_interpretation.clone(),
            structural_change_label: "Structural change".to_string(),
            structural_change_value: "BASELINE_UNAVAILABLE".to_string(),
            change_level: "UNAVAILABLE".to_string(),
            change_drivers: Vec::new(),
            unchanged_dimensions: Vec::new(),
            summary: "BASELINE_UNAVAILABLE".to_string(),
            summary_label: "Summary".to_string(),
            summary_values: vec![unavailable_interpretation],
            boundary: "Boundary: observation only; this log does not change trading, Gate, Execution, Trader, or Position Sizing.".to_string(),
        };
    }
    let previous_leader = formal_baseline
        .and_then(|snapshot| snapshot.primary_leader.clone())
        .or_else(|| {
            previous_leadership_snapshot.map(|snapshot| snapshot.primary_leader_value.clone())
        })
        .or_else(|| prev_packet.and_then(|prev| prev.top_tier_symbols.first().cloned()))
        .unwrap_or_else(|| current_leader.clone());

    let breadth_value = pres_packet.signal_summary.breadth_semantic_value.clone();
    let current_breadth_classification = Some(breadth_value.clone());
    let previous_breadth_classification =
        formal_baseline.and_then(|snapshot| snapshot.breadth_classification.clone());
    let previous_risk_state = formal_baseline
        .map(|snapshot| snapshot.risk_state.as_str())
        .or_else(|| {
            prev_packet.map(|prev| match prev.market_regime.risk_overlay {
                crate::features::radar::domain::market_regime::RiskOverlay::DEFENSIVE => {
                    "DEFENSIVE"
                }
                crate::features::radar::domain::market_regime::RiskOverlay::BROKEN => "BROKEN",
                _ => "NORMAL",
            })
        });
    let risk_value = match previous_risk_state {
        Some(previous) if previous == format!("{:?}", packet.market_regime.risk_overlay) => {
            "unchanged".to_string()
        }
        Some(previous)
            if (previous == "NORMAL" || previous == "DEFENSIVE")
                && matches!(
                    packet.market_regime.risk_overlay,
                    crate::features::radar::domain::market_regime::RiskOverlay::DEFENSIVE
                        | crate::features::radar::domain::market_regime::RiskOverlay::BROKEN
                ) =>
        {
            "upgraded".to_string()
        }
        Some(_) => "downgraded".to_string(),
        None => "BASELINE_UNAVAILABLE".to_string(),
    };
    let supply_phase_value = current_supply_phase.phase_value.clone();
    let previous_supply_phase_value = formal_baseline
        .map(|snapshot| snapshot.supply_phase.clone())
        .or_else(|| previous_supply_phase.map(|phase| phase.phase_value.clone()))
        .unwrap_or_else(|| supply_phase_value.clone());
    let current_confidence = packet.market_features.system_confidence;
    let previous_confidence = formal_baseline
        .map(|snapshot| snapshot.confidence)
        .or_else(|| prev_packet.map(|prev| prev.market_features.system_confidence))
        .unwrap_or(current_confidence);
    let confidence_value = if current_confidence > previous_confidence + 0.5 {
        format!("increased to {:.1}", current_confidence)
    } else if current_confidence + 0.5 < previous_confidence {
        format!("decreased to {:.1}", current_confidence)
    } else {
        format!("unchanged at {:.1}", current_confidence)
    };
    let current_day_type = current_market_interpretation
        .map(|interpretation| interpretation.day_type_value.clone())
        .unwrap_or_else(|| "NORMAL".to_string());
    let previous_day_type = previous_market_interpretation
        .map(|interpretation| interpretation.day_type_value.clone())
        .unwrap_or_else(|| current_day_type.clone());
    let current_ranked_leaders = std::iter::once(current_leader.clone())
        .chain(
            current_leadership_snapshot
                .secondary_leaders_values
                .iter()
                .cloned(),
        )
        .collect::<Vec<_>>();
    let previous_ranked_leaders = formal_baseline
        .map(|snapshot| {
            snapshot
                .primary_leader
                .iter()
                .chain(snapshot.secondary_leaders.iter())
                .cloned()
                .collect::<Vec<_>>()
        })
        .or_else(|| {
            previous_leadership_snapshot.map(|snapshot| {
                std::iter::once(previous_leader.clone())
                    .chain(snapshot.secondary_leaders_values.iter().cloned())
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_else(|| vec![previous_leader.clone()]);
    let current_breakout_state = breakout_state_signature(pres_packet);
    let previous_breakout_state = formal_baseline
        .map(|snapshot| snapshot.breakouts.to_string())
        .or_else(|| previous_presentation.map(breakout_state_signature))
        .unwrap_or_else(|| current_breakout_state.clone());
    let previous_score = if formal_baseline.is_some() {
        0.0
    } else {
        previous_presentation
            .and_then(|presentation| presentation.leader_persistence.as_ref())
            .map(|persistence| persistence.leadership_score)
            .unwrap_or(0.0)
    };
    let current_score = pres_packet
        .leader_persistence
        .as_ref()
        .map(|persistence| persistence.leadership_score)
        .unwrap_or(0.0);
    let change = build_market_change_driver(
        &MarketChangeSnapshot {
            primary_leader: previous_leader.clone(),
            breadth_classification: previous_breadth_classification.clone(),
            supply_phase: previous_supply_phase_value.clone(),
            supply_pressure: if formal_baseline.is_some() {
                "UNAVAILABLE".to_string()
            } else {
                previous_supply_snapshot.pressure.clone()
            },
            market_state: formal_baseline
                .map(|snapshot| snapshot.market_state.clone())
                .or_else(|| {
                    prev_packet.map(|prev| format!("{:?}", prev.market_regime.market_state))
                })
                .unwrap_or_else(|| format!("{:?}", packet.market_regime.market_state)),
            risk_state: formal_baseline
                .map(|snapshot| snapshot.risk_state.clone())
                .or_else(|| {
                    prev_packet.map(|prev| format!("{:?}", prev.market_regime.risk_overlay))
                })
                .unwrap_or_else(|| format!("{:?}", packet.market_regime.risk_overlay)),
            day_type: previous_day_type,
            confidence: previous_confidence,
            score: previous_score,
            ranked_leaders: previous_ranked_leaders,
            breakout_state: previous_breakout_state,
        },
        &MarketChangeSnapshot {
            primary_leader: current_leader.clone(),
            breadth_classification: current_breadth_classification.clone(),
            supply_phase: supply_phase_value.clone(),
            supply_pressure: current_supply_snapshot.pressure.clone(),
            market_state: format!("{:?}", packet.market_regime.market_state),
            risk_state: format!("{:?}", packet.market_regime.risk_overlay),
            day_type: current_day_type,
            confidence: current_confidence,
            score: current_score,
            ranked_leaders: current_ranked_leaders,
            breakout_state: current_breakout_state,
        },
    );
    let change_level = format!("{:?}", change.change_level).to_ascii_uppercase();
    let interpretation_value = match language {
        crate::features::shared::interface::i18n::Language::ZhCn => {
            format!(
                "变化等级: {change_level}（依据：{}）",
                change.change_drivers.join("、")
            )
        }
        crate::features::shared::interface::i18n::Language::EnUs => {
            format!(
                "Change level: {change_level} (drivers: {}).",
                change.change_drivers.join(", ")
            )
        }
        crate::features::shared::interface::i18n::Language::JaJp => {
            format!(
                "変化レベル: {change_level}（根拠: {}）。",
                change.change_drivers.join("、")
            )
        }
    };
    let mut summary_values = Vec::new();
    if current_leader != previous_leader {
        summary_values.push(format!(
            "Leader shifted from {previous_leader} to {current_leader}."
        ));
    } else {
        summary_values.push(format!("Leader remains {current_leader}."));
    }
    summary_values.push(breadth_comparison_summary(
        previous_breadth_classification.as_deref(),
        &breadth_value,
    ));
    summary_values.push(match risk_value.as_str() {
        "upgraded" => "Risk upgraded.".to_string(),
        "downgraded" => "Risk downgraded.".to_string(),
        _ => "Risk remains broadly unchanged.".to_string(),
    });
    summary_values.push(if supply_phase_value != previous_supply_phase_value {
        format!("Supply phase shifted from {previous_supply_phase_value} to {supply_phase_value}.")
    } else {
        format!("Supply phase remains {supply_phase_value}.")
    });
    summary_values.push(format!("Confidence: {confidence_value}."));
    if !change.change_drivers.is_empty() {
        summary_values.push(format!(
            "Change basis: {}.",
            change.change_drivers.join(", ")
        ));
    }
    summary_values.push(interpretation_value.clone());

    crate::features::radar::interface::presentation::MarketChangeLogViewModel {
        baseline_status: "AVAILABLE".to_string(),
        change_status: "DETERMINED".to_string(),
        title: "Market Change Log".to_string(),
        leader_label: "Leader".to_string(),
        leader_value: format!("{previous_leader} -> {current_leader}"),
        breadth_label: "Breadth".to_string(),
        breadth_value: breadth_value.to_string(),
        risk_label: "Risk".to_string(),
        risk_value,
        supply_phase_label: "Supply Phase".to_string(),
        supply_phase_value: supply_phase_value.to_string(),
        confidence_label: "Confidence".to_string(),
        confidence_value,
        interpretation_label: "Market Interpretation".to_string(),
        interpretation_value,
        structural_change_label: "Structural change".to_string(),
        structural_change_value: change_level.clone(),
        change_level,
        change_drivers: change.change_drivers,
        unchanged_dimensions: change.unchanged_dimensions,
        summary: change.summary,
        summary_label: "Summary".to_string(),
        summary_values,
        boundary: "Boundary: observation only; this log does not change trading, Gate, Execution, Trader, or Position Sizing.".to_string(),
    }
}

fn breadth_comparison_summary(previous: Option<&str>, current: &str) -> String {
    match previous {
        Some(previous) if previous == current => format!("Breadth remains {current}."),
        Some(previous) => format!("Breadth shifted from {previous} to {current}."),
        None => {
            "Breadth classification comparison unavailable for the previous snapshot.".to_string()
        }
    }
}

fn is_noisy_reference_detail(trimmed: &str) -> bool {
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

fn derive_gray_rhino_escalated_from_daily_report(
    assessment: Option<&GrayRhinoAssessment>,
    monitoring_statuses: &[GrayRhinoMonitoringStatus],
) -> bool {
    let assessment_escalated = assessment.is_some_and(|assessment| {
        matches!(
            assessment.current.escalation.escalation_state,
            RhinoEscalationState::Expanding | RhinoEscalationState::Critical
        )
    });
    let monitoring_escalated = monitoring_statuses.iter().any(|status| {
        matches!(
            status.current_state,
            GrayRhinoCandidateState::Expanding | GrayRhinoCandidateState::Critical
        ) || status.direction == GrayRhinoMonitoringDirection::Intensifying
    });
    assessment_escalated || monitoring_escalated
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

#[cfg(test)]
mod tests {
    use super::compact_reference_appendix_for_telegram;
    use super::derive_gray_rhino_escalated_from_daily_report;
    use crate::features::research::application::gray_rhino_monitoring_state::{
        GrayRhinoMonitoringDirection, GrayRhinoMonitoringStatus,
    };
    use crate::features::research::domain::gray_rhino::{
        GrayRhinoAssessment, GrayRhinoAssessmentSnapshot, GrayRhinoEscalation,
        GrayRhinoObservationSource, RhinoEscalationState, RiskLevel,
    };
    use crate::features::research::domain::gray_rhino_candidate::{
        GrayRhinoCandidateKind, GrayRhinoCandidateScope, GrayRhinoCandidateState,
    };
    use crate::features::shared::interface::i18n::Language;

    #[test]
    fn telegram_reference_appendix_digest_keeps_judgement_and_omits_detail_lines() {
        let appendix = r#"# Gray Rhino Reference

- Status: monitoring
- risk: governance concentration elevated
- evidence coverage: accepted 3 / rejected 1
- source detail: https://example.com/noisy-source
- raw extract: long filing excerpt
Boundary: context only; no Gate input or trade instruction.
"#;

        let digest = compact_reference_appendix_for_telegram(appendix, Language::EnUs);

        assert!(digest.contains("# Gray Rhino Reference"));
        assert!(digest.contains("Status: monitoring"));
        assert!(digest.contains("risk: governance concentration elevated"));
        assert!(digest.contains("evidence coverage"));
        assert!(digest.contains("Boundary: context only"));
        assert!(digest.contains("Telegram digest"));
        assert!(!digest.contains("noisy-source"));
        assert!(!digest.contains("long filing excerpt"));
    }

    #[test]
    fn telegram_reference_appendix_digest_keeps_capital_absorption_judgement_lines() {
        let appendix = r#"📊 Capital Absorption Early Warning Sensor

Capital absorption status: WATCH
Actual Capital Supply: 12.0B
Capital Demand: RISING
Capital Supply: STABLE
Absorption ratio: ELEVATED
Structural Impact: observation only
- raw source detail: https://example.com/capital-source
Boundary: context only; no Gate input or trade instruction.
"#;

        let digest = compact_reference_appendix_for_telegram(appendix, Language::EnUs);

        assert!(digest.contains("Capital absorption status: WATCH"));
        assert!(digest.contains("Actual Capital Supply: 12.0B"));
        assert!(digest.contains("Capital Demand: RISING"));
        assert!(digest.contains("Capital Supply: STABLE"));
        assert!(digest.contains("Absorption ratio: ELEVATED"));
        assert!(digest.contains("Structural Impact: observation only"));
        assert!(digest.contains("Boundary: context only"));
        assert!(digest.contains("Telegram digest"));
        assert!(!digest.contains("capital-source"));
    }

    #[test]
    fn telegram_reference_appendix_digest_keeps_japanese_capital_absorption_lines() {
        let appendix = r#"📊 資本吸収早期警戒センサー

資本吸収状態: 観察（WATCH）
資本供給: STABLE
資本需要: RISING
吸収比率: ELEVATED
構造影響: 観測のみ
- raw source detail: https://example.com/jp-capital-source
境界: コンテキストのみ。ゲート入力や取引指示ではない。
"#;

        let digest = compact_reference_appendix_for_telegram(appendix, Language::JaJp);

        assert!(digest.contains("資本吸収状態: 観察（WATCH）"));
        assert!(digest.contains("資本供給: STABLE"));
        assert!(digest.contains("資本需要: RISING"));
        assert!(digest.contains("吸収比率: ELEVATED"));
        assert!(digest.contains("構造影響: 観測のみ"));
        assert!(digest.contains("境界: コンテキストのみ"));
        assert!(digest.contains("Telegram 要約"));
        assert!(!digest.contains("jp-capital-source"));
    }

    #[test]
    fn telegram_reference_appendix_digest_notice_uses_configured_language() {
        let appendix = r#"# Gray Rhino Reference

- Status: monitoring
- noisy source detail: https://example.com/noisy-source
Boundary: context only; no Gate input or trade instruction.
"#;

        let digest = compact_reference_appendix_for_telegram(appendix, Language::ZhCn);

        assert!(digest.contains("Telegram 摘要"));
        assert!(!digest.contains("Telegram digest"));
    }

    #[test]
    fn telegram_reference_appendix_digest_keeps_structured_renamed_labels() {
        let appendix = r#"Custom Reference

Posture marker: WATCH
AlphaBetaX: retained without dictionary keyword
- raw extract: should be omitted
- source detail: https://example.com/source
Boundary: context only; no Gate input or trade instruction.
"#;

        let digest = compact_reference_appendix_for_telegram(appendix, Language::EnUs);

        assert!(digest.contains("Custom Reference"));
        assert!(digest.contains("Posture marker: WATCH"));
        assert!(digest.contains("AlphaBetaX: retained without dictionary keyword"));
        assert!(digest.contains("Boundary: context only"));
        assert!(!digest.contains("raw extract"));
        assert!(!digest.contains("example.com/source"));
    }

    #[test]
    fn telegram_valuation_digest_reserves_boundary_after_multiple_assets() {
        let mut appendix = String::from("## Gravity Layer (Valuation Gravity)\n");
        for symbol in ["MSFT", "NVDA", "TSLA"] {
            appendix.push_str(&format!(
                "\n### {symbol}\n- Gravity: Fair\n- Confidence: Low\n- Source: Market Multiple\n- Provider: Finnhub\n- As of: 2026-06-18\n- Source Health: Partial\n- Evidence Count: 5\n- Data Quality Reason: historical fallback\n"
            ));
        }
        appendix.push_str("\nBoundary: Gravity is independent from Trend; it does not affect READY / EXECUTE / Gate / Position Sizing / Trader and produces no trading signal.");

        let digest = compact_reference_appendix_for_telegram(&appendix, Language::EnUs);

        assert!(digest.contains("Boundary: Gravity is independent from Trend"));
        assert!(digest.contains("produces no trading signal"));
        assert!(digest.contains("Telegram digest"));
    }

    #[test]
    fn gray_rhino_escalation_helper_returns_false_without_escalated_daily_report() {
        assert!(!derive_gray_rhino_escalated_from_daily_report(None, &[]));
        assert!(!derive_gray_rhino_escalated_from_daily_report(
            Some(&assessment_with_state(RhinoEscalationState::Background)),
            &[monitoring_status(
                GrayRhinoCandidateState::Visible,
                GrayRhinoMonitoringDirection::Stable,
            )],
        ));
    }

    #[test]
    fn gray_rhino_escalation_helper_returns_true_for_escalated_daily_report() {
        assert!(derive_gray_rhino_escalated_from_daily_report(
            Some(&assessment_with_state(RhinoEscalationState::Expanding)),
            &[monitoring_status(
                GrayRhinoCandidateState::Visible,
                GrayRhinoMonitoringDirection::Stable,
            )],
        ));
        assert!(derive_gray_rhino_escalated_from_daily_report(
            Some(&assessment_with_state(RhinoEscalationState::Background)),
            &[monitoring_status(
                GrayRhinoCandidateState::Visible,
                GrayRhinoMonitoringDirection::Intensifying,
            )],
        ));
    }

    fn assessment_with_state(state: RhinoEscalationState) -> GrayRhinoAssessment {
        GrayRhinoAssessment {
            current: GrayRhinoAssessmentSnapshot {
                schema_version: 1,
                as_of_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
                source: GrayRhinoObservationSource::EvidenceStore,
                escalation: GrayRhinoEscalation {
                    escalation_state: state,
                    risk_expansion_rate: RiskLevel::Low,
                    constraint_growth_rate: RiskLevel::Low,
                    dependency_centralization: RiskLevel::Low,
                    awareness_decay: RiskLevel::Low,
                    narrative_overconfidence: RiskLevel::Low,
                    single_point_fragility: RiskLevel::Low,
                    fallback_survivability_risk: RiskLevel::Low,
                    notes: Vec::new(),
                    suppressed_note_count: 0,
                },
            },
            previous: None,
        }
    }

    fn monitoring_status(
        current_state: GrayRhinoCandidateState,
        direction: GrayRhinoMonitoringDirection,
    ) -> GrayRhinoMonitoringStatus {
        GrayRhinoMonitoringStatus {
            scope: GrayRhinoCandidateScope::Company,
            kind: GrayRhinoCandidateKind::GovernanceConcentration,
            subject: "TSLA".to_string(),
            current_state,
            previous_state: None,
            direction,
            observation_count: 1,
            latest_observed_at: chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            stale_days: 0,
        }
    }

    #[test]
    fn breakout_snapshot_projection_preserves_existing_read_model_facts() {
        let projection = super::build_breakout_snapshot_projection(&[
            crate::features::radar::interface::presentation::BreakoutItemViewModel {
                symbol: "SPY".to_string(),
                status: crate::features::radar::interface::presentation::BreakoutDisplayStatus::EmergingBreakout,
                status_label: "BREAKOUT_EMERGING".to_string(),
                strength_value: "81".to_string(),
                quality_value: "100".to_string(),
                consecutive_days: 4,
                ..Default::default()
            },
        ]);
        assert_eq!(projection["SPY"]["state"], "BREAKOUT_EMERGING");
        assert_eq!(projection["SPY"]["strength"], 81.0);
        assert_eq!(projection["SPY"]["quality"], 100.0);
        assert_eq!(projection["SPY"]["consecutive_days"], 4);
    }

    #[test]
    fn history_regression_blocks_publication_before_persistence() {
        let state = crate::features::radar::infrastructure::persistence::ObservationHistoryState {
            count: 3,
            last_market_date: chrono::NaiveDate::from_ymd_opt(2026, 7, 27).unwrap(),
            cycle_id: "cycle-test".to_string(),
        };
        assert!(super::history_state_would_regress(
            Some(&state),
            2,
            chrono::NaiveDate::from_ymd_opt(2026, 7, 28).unwrap()
        ));
    }

    #[test]
    fn history_baseline_downgrade_preserves_current_breakout_status() {
        let mut presentation =
            crate::features::radar::interface::presentation::PresentationPacket::default();
        presentation.breakout_summary.items.push(
            crate::features::radar::interface::presentation::BreakoutItemViewModel {
                symbol: "U".to_string(),
                status_label: "Emerging Breakout (Day 1)".to_string(),
                ..Default::default()
            },
        );

        super::apply_history_baseline_downgrade(&mut presentation);

        assert_eq!(
            presentation.breakout_summary.items[0].status_label,
            "Emerging Breakout (Day 1)"
        );
        assert_eq!(
            presentation
                .leader_persistence
                .as_ref()
                .map(|value| value.persistence_value.as_str()),
            None
        );
    }

    #[test]
    fn equal_persisted_breadth_classification_does_not_render_raw_value_as_label() {
        let summary = super::breadth_comparison_summary(Some("Very Narrow"), "Very Narrow");

        assert_eq!(summary, "Breadth remains Very Narrow.");
        assert!(!summary.contains("35.0"));
    }

    #[test]
    fn missing_persisted_breadth_classification_is_explicitly_unavailable() {
        let summary = super::breadth_comparison_summary(None, "Very Narrow");

        assert_eq!(
            summary,
            "Breadth classification comparison unavailable for the previous snapshot."
        );
    }
}

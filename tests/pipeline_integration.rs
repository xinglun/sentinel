use chrono::{NaiveDate, Utc};
use std::collections::HashMap;
use stock_sentinel::config::{DeviationBasis, ParsedRules, TrendConfig, WatchlistEntry};
use stock_sentinel::core::action_matrix::AssetAction;
use stock_sentinel::core::decision::DecisionPacket;
use stock_sentinel::core::engine::Engine;
use stock_sentinel::core::features::MarketFeatures;
use stock_sentinel::core::market_regime::{
    LifecycleState, MarketRegimeSnapshot, MarketState, MarketTransitionAudit, RiskOverlay,
};
use stock_sentinel::core::portfolio_policy::PortfolioPolicy;
use stock_sentinel::data::yahoo_provider::{DailyBar, TickerHistory};

use std::borrow::Cow;

fn create_mock_history(
    symbol: &str,
    start_price: f64,
    count: usize,
    daily_change: f64,
) -> TickerHistory<'static> {
    let start_date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
    let mut bars = Vec::new();
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

fn create_mock_rules() -> ParsedRules {
    let mut sorted_bands = vec![
        ("OPTIMAL".to_string(), 0.10),
        ("CRUISE".to_string(), 0.00),
        ("CAUTION".to_string(), -0.05),
        ("DEFEND".to_string(), -0.10),
    ];
    sorted_bands.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut actions = HashMap::new();
    actions.insert("optimal".to_string(), "ACCUMULATE".to_string());
    actions.insert("cruise".to_string(), "HOLD".to_string());
    actions.insert("caution".to_string(), "REDUCE".to_string());
    actions.insert("defend".to_string(), "AVOID".to_string());

    ParsedRules {
        trend: TrendConfig {
            lookback_days: 10,
            flat_threshold_pct: 0.1,
        },
        sorted_bands,
        actions,
        sizing_multipliers: None,
        core_assets: Vec::new(),
        inertia: stock_sentinel::config::ParsedInertia {
            min_state_duration: 3,
            trend_dominant_min_confidence: 55.0,
            core_breakdown_k: 2,
            core_breakdown_avg_deviation: -5.0,
            core_breakdown_breadth_floor: 0.0,
        },
        trend_cohesion: stock_sentinel::config::ParsedTrendCohesionRules::default(),
        breakout: stock_sentinel::config::ParsedBreakoutRules::default(),
        market_state_engine: Default::default(),
        sec: None,
        macro_gravity: None,
    }
}

#[tokio::test]
async fn test_pipeline_bullish_path() {
    // 60 days of steady 0.2% daily gain
    let history = create_mock_history("AAPL", 100.0, 60, 0.002);
    let entry = WatchlistEntry {
        symbol: "AAPL".to_string(),
        weight: None,
        market: "US".to_string(),
        owner_ma_days: 20,
        leash_ma_days: 10,
        deviation_basis: DeviationBasis::Owner,
        enable: true,
        trade_enabled: Some(true),
        trade_amount: Some(1000.0),
        event_tags: None,
    };

    let history2 = create_mock_history("MSFT", 150.0, 60, 0.002);
    let entry2 = WatchlistEntry {
        symbol: "MSFT".to_string(),
        weight: None,
        market: "US".to_string(),
        owner_ma_days: 20,
        leash_ma_days: 10,
        deviation_basis: DeviationBasis::Owner,
        enable: true,
        trade_enabled: Some(true),
        trade_amount: Some(1000.0),
        event_tags: None,
    };

    let rules = create_mock_rules();
    let histories = vec![(history, &entry), (history2, &entry2)];

    // Test transition from IGNITION to NEWBORN (needs confidence 60, age 5)
    let prev_market = MarketRegimeSnapshot {
        market_state: MarketState::IGNITION,
        lifecycle_state: LifecycleState::IGNITION,
        risk_overlay: RiskOverlay::NORMAL,
        reasons: vec![],
        low_stability_streak: 0,
        duration_in_state: 10,
        transition_audit: None,
    };
    let prev_features = MarketFeatures {
        date: NaiveDate::from_ymd_opt(2022, 12, 31).unwrap(),
        up_count: 1,
        total_count: 1,
        system_confidence: 100.0,
        regime_age: 20,
        stability_structural: 100.0,
        ..Default::default()
    };
    let top_tier = vec!["MSFT".to_string(), "AAPL".to_string()];
    let prev_packet1 = DecisionPacket::new(
        NaiveDate::from_ymd_opt(2022, 12, 30).unwrap(),
        MarketFeatures::default(),
        prev_market.clone(),
        None,
        PortfolioPolicy::from_market_regime(&prev_market),
        vec![],
        top_tier.clone(),
        false,
        stock_sentinel::core::trend_cohesion::TrendCohesionSnapshot {
            gate_passed: true,
            continuity_streak: 1,
            ..Default::default()
        },
        None,
        None,
    );
    let prev_packet2 = DecisionPacket::new(
        prev_features.date,
        prev_features,
        prev_market.clone(),
        None,
        PortfolioPolicy::from_market_regime(&prev_market),
        vec![],
        top_tier.clone(),
        false,
        stock_sentinel::core::trend_cohesion::TrendCohesionSnapshot {
            gate_passed: true,
            continuity_streak: 2,
            ..Default::default()
        },
        None,
        None,
    );

    let packet = Engine::run_daily_pipeline(
        &histories,
        &rules,
        &[prev_packet1, prev_packet2],
        &[],
        &HashMap::new(),
    )
    .expect("Pipeline failed");

    assert_eq!(packet.market_regime.market_state, MarketState::NEWBORN);
    println!("Curr: {:?}", packet.top_tier_symbols);
    println!("{:#?}", packet.trend_cohesion);
    assert!(
        packet.assets[0].action == AssetAction::ACCUMULATE
            || packet.assets[0].action == AssetAction::HOLD
            || packet.assets[1].action == AssetAction::ACCUMULATE
    );
}

#[tokio::test]
async fn test_pipeline_bearish_path() {
    // Start at 100, then sudden drop
    let mut bars = Vec::new();
    let start_date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
    for i in 0..40 {
        bars.push(DailyBar {
            date: start_date + chrono::Duration::days(i as i64),
            close: 100.0, // Flat
            volume: Some(1000.0),
        });
    }
    // Sudden sharp crash
    for i in 40..55 {
        let last_close = bars.last().unwrap().close;
        bars.push(DailyBar {
            date: start_date + chrono::Duration::days(i as i64),
            close: last_close * 0.90, // 10% daily drop
            volume: Some(1000.0),
        });
    }

    let history = TickerHistory {
        symbol: "TSLA".to_string(),
        bars: Cow::Owned(bars),
        total_trading_days: 55,
        latest_quote_timestamp: Some(Utc::now().timestamp()),
    };

    let entry = WatchlistEntry {
        symbol: "TSLA".to_string(),
        weight: None,
        market: "US".to_string(),
        owner_ma_days: 20,
        leash_ma_days: 10,
        deviation_basis: DeviationBasis::Owner,
        enable: true,
        trade_enabled: Some(true),
        trade_amount: Some(1000.0),
        event_tags: None,
    };

    let rules = create_mock_rules();
    let histories = vec![(history, &entry)];

    let packet = Engine::run_daily_pipeline(&histories, &rules, &[], &[], &HashMap::new())
        .expect("Pipeline should run");

    assert_eq!(packet.market_regime.market_state, MarketState::DEFENSIVE);

    assert_eq!(packet.assets[0].action, AssetAction::AVOID);
}

#[tokio::test]
async fn test_pipeline_age_continuity() {
    let rules = create_mock_rules();
    let history = create_mock_history("AAPL", 100.0, 60, 0.002);
    let entry = WatchlistEntry {
        symbol: "AAPL".to_string(),
        owner_ma_days: 20,
        leash_ma_days: 10,
        event_tags: None,
        ..Default::default()
    };
    let histories = vec![(history, &entry)];

    // Day 1: Start from scratch (no history)
    let p1 = Engine::run_daily_pipeline(&histories, &rules, &[], &[], &HashMap::new())
        .expect("P1 failed");
    let age1 = p1.market_features.regime_age;

    // Day 2: Pass p1 as history
    let p2 = Engine::run_daily_pipeline(
        &histories,
        &rules,
        std::slice::from_ref(&p1),
        &[],
        &HashMap::new(),
    )
    .expect("P2 failed");
    let age2 = p2.market_features.regime_age;

    // Day 3: Pass p1, p2 as history
    let p3 = Engine::run_daily_pipeline(&histories, &rules, &[p1, p2], &[], &HashMap::new())
        .expect("P3 failed");
    let age3 = p3.market_features.regime_age;

    // Expect exactly 1 day increment per call
    assert_eq!(age2, age1 + 1, "Age should increment by 1 on Day 2");
    assert_eq!(age3, age2 + 1, "Age should increment by 1 on Day 3");
}

#[test]
fn radar_application_policy_keeps_partial_fetch_history_persistent() {
    let summary = stock_sentinel::application::radar::DataAcquisitionSummary::new(1, 8);
    assert!(summary.should_persist_decision_history());
    assert!(!summary.is_full_failure());
}

#[test]
fn radar_application_policy_blocks_full_fetch_failure_history() {
    let summary = stock_sentinel::application::radar::DataAcquisitionSummary::new(0, 9);
    assert!(!summary.should_persist_decision_history());
    assert!(summary.is_full_failure());
}

#[test]
fn radar_application_result_uses_same_persistence_policy() {
    let result = stock_sentinel::application::radar::DataAcquisitionResult::new(
        vec!["NVDA"],
        vec!["MSFT".to_string()],
    );
    let summary = result.summary();

    assert_eq!(summary.successful_fetches, 1);
    assert_eq!(summary.failed_fetches, 1);
    assert!(result.should_persist_decision_history());
    assert!(!result.is_full_failure());
    assert_eq!(result.data_quality_status().as_str(), "WARNING");
}

#[test]
fn radar_application_data_quality_status_keeps_cli_log_contract() {
    use stock_sentinel::application::radar::DataAcquisitionSummary;

    assert_eq!(
        DataAcquisitionSummary::new(2, 0)
            .data_quality_status()
            .as_str(),
        "OK"
    );
    assert_eq!(
        DataAcquisitionSummary::new(2, 1)
            .data_quality_status()
            .as_str(),
        "WARNING"
    );
    assert_eq!(
        DataAcquisitionSummary::new(0, 1)
            .data_quality_status()
            .as_str(),
        "CRITICAL"
    );
}

#[test]
fn radar_application_payload_builders_keep_persistence_schema() {
    use std::collections::HashMap;
    use stock_sentinel::application::radar::{
        build_account_snapshot, build_data_quality_log, build_portfolio_snapshot,
        AccountSnapshotInput, DataAcquisitionSummary,
    };

    let mut positions = HashMap::new();
    positions.insert("NVDA".to_string(), (2.0, 100.0));

    let portfolio = build_portfolio_snapshot("2026-05-24", 7.5, 200.0, &positions);
    let account = build_account_snapshot(AccountSnapshotInput {
        date: "2026-05-24",
        global_budget: 1000.0,
        max_daily_budget: Some(300.0),
        daily_traded: 50.0,
        buying_power: 800.0,
        current_exposure: 200.0,
        realized_pl: 7.5,
        failed_fetch_count: 1,
    });
    let failed_symbols = vec!["MSFT".to_string()];
    let quality = build_data_quality_log(
        "2026-05-24T00:00:00+09:00",
        "2026-05-24",
        DataAcquisitionSummary::new(1, 1),
        &failed_symbols,
    );

    assert_eq!(portfolio["date"], "2026-05-24");
    assert_eq!(portfolio["positions"][0]["symbol"], "NVDA");
    assert_eq!(account["max_daily_budget"], 300.0);
    assert_eq!(account["failed_fetch_count"], 1);
    assert_eq!(quality["successful_fetches"], 1);
    assert_eq!(quality["failed_symbols"][0], "MSFT");
    assert_eq!(quality["status"], "WARNING");
}

#[test]
fn radar_application_state_machine_summary_keeps_run_status_contract() {
    let audit = MarketTransitionAudit {
        from: LifecycleState::IGNITION,
        to: LifecycleState::NEWBORN,
        is_reset_blocked: true,
        is_downgrade_clamped: false,
        core_breakdown: true,
        duration_locked: true,
        trend_dominant: false,
        reset_gate_passed: true,
        indicator_cap: LifecycleState::NEWBORN,
        soft_reset_applied: true,
        defensive_override: true,
    };

    let summary = stock_sentinel::application::radar::build_state_machine_summary(
        Some(MarketState::IGNITION),
        MarketState::DEFENSIVE,
        Some(&audit),
        true,
    );
    let unavailable = stock_sentinel::application::radar::build_state_machine_summary(
        Some(MarketState::ESTABLISHED),
        MarketState::CONFIRMED,
        None,
        false,
    );

    assert_eq!(summary.from_state, "IGNITION");
    assert_eq!(summary.to_state, "DEFENSIVE");
    assert!(summary.reset_confirmed);
    assert!(summary.reset_blocked);
    assert!(summary.soft_reset_applied);
    assert!(summary.duration_locked);
    assert!(summary.defensive_override);
    assert!(summary.core_breakdown);
    assert_eq!(unavailable.from_state, "ESTABLISHED");
    assert_eq!(unavailable.to_state, "DATA_UNAVAILABLE");
}

#[test]
fn radar_application_full_failure_output_is_diagnostic_only() {
    let date = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
    let packet = stock_sentinel::application::radar::build_diagnostic_packet(date);
    let status = stock_sentinel::application::radar::build_full_fetch_failure_status(9);

    assert_eq!(packet.date, date);
    assert!(packet.assets.is_empty());
    assert!(packet.transition_log.is_none());
    assert_eq!(
        status,
        stock_sentinel::core::run_status::DeliveryStatus::Failed {
            reason: "100% data acquisition failure: 9 symbols failed".to_string()
        }
    );
}

#[test]
fn radar_application_decision_outcome_keeps_status_mapping() {
    let date = NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
    let packet = stock_sentinel::application::radar::build_diagnostic_packet(date);
    let outcome = stock_sentinel::application::radar::build_successful_decision_outcome(packet);
    let failed =
        stock_sentinel::application::radar::build_decisioning_failure_status("engine failed");

    assert_eq!(outcome.packet.date, date);
    assert_eq!(
        outcome.decisioning,
        stock_sentinel::core::run_status::DeliveryStatus::Succeeded
    );
    assert_eq!(
        failed,
        stock_sentinel::core::run_status::DeliveryStatus::Failed {
            reason: "engine failed".to_string()
        }
    );
}

#[test]
fn radar_application_entry_policy_matches_existing_pipeline_body_condition() {
    use stock_sentinel::application::radar::DataAcquisitionSummary;

    assert!(!DataAcquisitionSummary::new(0, 0).should_enter_pipeline_body());
    assert!(DataAcquisitionSummary::new(1, 0).should_enter_pipeline_body());
    assert!(DataAcquisitionSummary::new(1, 8).should_enter_pipeline_body());
    assert!(DataAcquisitionSummary::new(0, 9).should_enter_pipeline_body());
}

#[test]
fn radar_application_pipeline_plan_collects_fetch_policies() {
    use stock_sentinel::application::radar::{DataAcquisitionSummary, DataQualityStatus};

    let empty = DataAcquisitionSummary::new(0, 0).pipeline_plan();
    assert!(empty.should_persist_history);
    assert!(!empty.should_enter_pipeline_body);
    assert_eq!(empty.data_quality_status, DataQualityStatus::Ok);

    let partial = DataAcquisitionSummary::new(1, 8).pipeline_plan();
    assert!(partial.should_persist_history);
    assert!(partial.should_enter_pipeline_body);
    assert_eq!(partial.data_quality_status, DataQualityStatus::Warning);

    let full_failure = DataAcquisitionSummary::new(0, 9).pipeline_plan();
    assert!(!full_failure.should_persist_history);
    assert!(full_failure.should_enter_pipeline_body);
    assert_eq!(
        full_failure.data_quality_status,
        DataQualityStatus::Critical
    );
}

#[test]
fn radar_application_run_context_builds_initial_status_metadata() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-05-24T10:15:00+09:00")
        .unwrap()
        .with_timezone(&chrono::Local);
    let context = stock_sentinel::application::radar::RadarRunContext::new("target/run", now);
    let outcome =
        context.initial_run_outcome(stock_sentinel::core::run_status::DeliveryStatus::Skipped);

    assert_eq!(context.save_dir(), std::path::Path::new("target/run"));
    assert_eq!(context.date_string(), "2026-05-24");
    assert_eq!(outcome.date, "2026-05-24");
    assert!(outcome.timestamp.contains("2026-05-24T10:15:00"));
    assert_eq!(
        outcome.evidence_collection,
        stock_sentinel::core::run_status::DeliveryStatus::Skipped
    );
}

#[test]
fn radar_pipeline_use_case_prepares_data_acquisition_payload() {
    let result = stock_sentinel::application::radar::DataAcquisitionResult::new(
        vec!["NVDA"],
        vec!["MSFT".to_string()],
    );
    let prepared = stock_sentinel::application::radar::RadarPipelineUseCase::new()
        .prepare_data_acquisition(result);

    assert_eq!(prepared.successful_items, vec!["NVDA"]);
    assert_eq!(prepared.failed_symbols, vec!["MSFT".to_string()]);
    assert_eq!(
        prepared.summary,
        stock_sentinel::application::radar::DataAcquisitionSummary::new(1, 1)
    );
    assert!(prepared.plan.should_persist_history);
    assert!(prepared.plan.should_enter_pipeline_body);
    assert_eq!(
        prepared.plan.data_quality_status,
        stock_sentinel::application::radar::DataQualityStatus::Warning
    );
}

#[test]
fn radar_pipeline_use_case_collects_provider_results() {
    let results: Vec<(Result<&str, &str>, String)> = vec![
        (Ok("NVDA-history"), "NVDA".to_string()),
        (Err("fetch failed"), "MSFT".to_string()),
    ];
    let data_acquisition = stock_sentinel::application::radar::RadarPipelineUseCase::new()
        .collect_data_acquisition(results);

    assert_eq!(data_acquisition.successful_items, vec!["NVDA-history"]);
    assert_eq!(data_acquisition.failed_symbols, vec!["MSFT".to_string()]);
    assert_eq!(
        data_acquisition.summary(),
        stock_sentinel::application::radar::DataAcquisitionSummary::new(1, 1)
    );
}

#[test]
fn radar_pipeline_use_case_prepares_from_provider_results() {
    let results: Vec<(Result<&str, &str>, String)> = vec![
        (Ok("NVDA-history"), "NVDA".to_string()),
        (Err("fetch failed"), "MSFT".to_string()),
    ];
    let prepared = stock_sentinel::application::radar::RadarPipelineUseCase::new()
        .prepare_from_fetch_results(results);

    assert_eq!(prepared.successful_items, vec!["NVDA-history"]);
    assert_eq!(prepared.failed_symbols, vec!["MSFT".to_string()]);
    assert_eq!(
        prepared.summary,
        stock_sentinel::application::radar::DataAcquisitionSummary::new(1, 1)
    );
    assert!(prepared.plan.should_persist_history);
    assert!(prepared.plan.should_enter_pipeline_body);
}

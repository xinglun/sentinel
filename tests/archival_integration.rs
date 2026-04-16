use serde_json::json;
use std::fs;
use stock_sentinel::core::decision::DecisionPacket;
use stock_sentinel::core::features::MarketFeatures;
use stock_sentinel::core::market_regime::{
    LifecycleState, MarketRegimeSnapshot, MarketState, RiskOverlay,
};
use stock_sentinel::core::persistence::PersistenceLayer;
use stock_sentinel::core::portfolio_policy::PortfolioPolicy;
use stock_sentinel::core::transition_log::TransitionLogger;
use tempfile::tempdir;

#[tokio::test]
async fn test_full_9_asset_archival_package() {
    let tmp_dir = tempdir().expect("Failed to create temp dir");
    let save_dir = tmp_dir.path().to_owned();
    let layer = PersistenceLayer::new(&save_dir);

    let date_str = "2023-01-01";
    let date_naive = chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();

    // 1. Decision Packet (decision_history.jsonl & decision_packet_YYYY-MM-DD.json)
    let market = MarketRegimeSnapshot {
        market_state: MarketState::ESTABLISHED,
        lifecycle_state: LifecycleState::ESTABLISHED,
        risk_overlay: RiskOverlay::NORMAL,
        reasons: vec!["Test".to_string()],
        low_stability_streak: 0,
        duration_in_state: 1,
        transition_audit: None,
    };
    let policy = PortfolioPolicy::from_market_regime(&market);
    let features = MarketFeatures {
        date: date_naive,
        ..Default::default()
    };
    let packet = DecisionPacket::new(
        date_naive,
        features,
        market,
        policy,
        vec![],
        stock_sentinel::core::participation::ParticipationReadiness::default(),
        Vec::new(),
        false,
        stock_sentinel::core::trend_cohesion::TrendCohesionSnapshot::default(),
        None,
    );

    layer.save_packet(&packet).unwrap();
    layer.save_daily_packet(&packet).unwrap();

    // 2. Execution Gate Log (execution_gate_log.jsonl)
    let gate_log = json!({"event": "trade_blocked", "reason": "budget_exceeded"});
    layer.save_execution_gate_log(&gate_log).unwrap();

    // 3. Snapshots (portfolio_snapshot_DATE.json & account_snapshot_DATE.json)
    let snapshot = json!({"assets": [], "total_mv": 10000.0});
    layer.save_portfolio_snapshot(&snapshot, date_str).unwrap();
    layer.save_account_snapshot(&snapshot, date_str).unwrap();

    // 4. Data Quality Log (data_quality_log.jsonl)
    let quality_log = json!({"symbol": "AAPL", "status": "ok"});
    layer.save_data_quality_log(&quality_log).unwrap();

    // 5. Markdown Report (DATE.md)
    layer.save_markdown_report("# Report", date_str).unwrap();

    // 6. Telemetry (telemetry.csv)
    let telemetry_row = stock_sentinel::core::telemetry::TelemetryRow {
        timestamp: "2023-01-01T00:00:00Z".to_string(),
        date: date_str.to_string(),
        provider: "mock".to_string(),
        market_state: packet.market_regime.market_state,
        risk_overlay: packet.market_regime.risk_overlay,
        system_confidence: packet.market_features.system_confidence,
        stability_score: packet.market_features.stability_score,
        dominance_margin: packet.market_features.dominance_margin,
        potential_energy: packet.market_features.potential_energy,
        regime_age: packet.market_features.regime_age,
        up_count: packet.market_features.up_count,
        flat_count: packet.market_features.flat_count,
        down_count: packet.market_features.down_count,
        total_count: packet.market_features.total_count,
        up_weight: packet.market_features.up_weight,
        flat_weight: packet.market_features.flat_weight,
        down_weight: packet.market_features.down_weight,
        total_weight: packet.market_features.total_weight,
        config_hash: "test_hash".to_string(),
        data_quality_status: "OK".to_string(),
    };
    layer.save_telemetry(&telemetry_row).unwrap();

    // 7. Transitions (state_transitions.csv & state_transitions.jsonl)
    let prev_packet = packet.clone(); // Use same packet as mock prev
    let mut curr_packet = packet.clone();
    curr_packet.transition_log = Some(
        stock_sentinel::core::transition_log::StateTransitionLog::compare(
            Some(&prev_packet),
            &curr_packet,
        ),
    );

    let logger = TransitionLogger::new(&save_dir);
    logger
        .log_transition(curr_packet.transition_log.as_ref().unwrap())
        .unwrap();

    // --- VERIFICATION ---
    let expected_files = vec![
        "decision_history.jsonl",
        "decision_packet_2023-01-01.json",
        "execution_gate_log.jsonl",
        "portfolio_snapshot_2023-01-01.json",
        "account_snapshot_2023-01-01.json",
        "data_quality_log.jsonl",
        "2023-01-01.md",
        "telemetry.csv",
        "state_transitions.csv",
        "state_transitions.jsonl",
    ];

    for file in expected_files {
        let path = save_dir.join(file);
        assert!(path.exists(), "Missing expected archival asset: {}", file);

        // HARDENED: Check that file is not empty (matching CLI -s check)
        let metadata = fs::metadata(&path).unwrap();
        assert!(metadata.len() > 0, "Archival asset {} is empty", file);
    }
}

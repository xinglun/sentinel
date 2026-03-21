use stock_sentinel::config::AppConfig;
use stock_sentinel::core::action_matrix::{AssetAction, AssetActionDecision};
use stock_sentinel::core::asset_state::AssetState;
use stock_sentinel::core::decision::DecisionPacket;
use stock_sentinel::core::execution_gate::ExecutionGate;
use stock_sentinel::core::features::MarketFeatures;
use stock_sentinel::core::market_regime::{
    LifecycleState, MarketRegimeSnapshot, MarketState, RiskOverlay,
};
use stock_sentinel::core::portfolio_policy::PortfolioPolicy;

#[test]
fn test_kill_switch_behavior_in_execution_gate() {
    // 1. Config with trading DISABLED
    let config_str = r#"
        version = 1
        watchlist = []
        [output]
        timezone = "UTC"
        format = "json"
        save_to = "./test"
        [trading]
        enabled = false
        global_budget = 10000.0
        [rules]
        actions = { "optimal" = "BUY" }
        [rules.trend]
        lookback_days = 20
        flat_threshold_pct = 0.5
        [rules.deviation_bands]
        optimal = 5.0
    "#;
    let config: AppConfig = toml::from_str(config_str).expect("Valid config should parse");
    let trading_config = config.trading.as_ref().unwrap();

    // 2. Decision packet with a "BUY" signal
    let assets = vec![AssetActionDecision {
        symbol: "TEST".to_string(),
        price: 100.0,
        asset_state: stock_sentinel::core::asset_state::AssetStateSnapshot {
            symbol: "TEST".to_string(),
            state: AssetState::OPTIMAL,
            reasons: vec![],
            recovery_streak: 0,
            last_defend_age: 100,
        },
        action: AssetAction::ACCUMULATE,
        reasons: vec![],
        deviation: None,
        z_score: None,
        trade_enabled: true,
        trade_amount: 1000.0,
        config_multiplier: 1.0,
        prev_action: None,
        action_changed: false,
    }];

    let market = MarketRegimeSnapshot {
        market_state: MarketState::ESTABLISHED,
        lifecycle_state: LifecycleState::ESTABLISHED,
        risk_overlay: RiskOverlay::NORMAL,
        reasons: vec![],
        low_stability_streak: 0,
        duration_in_state: 1,
        transition_audit: None,
    };

    let packet = DecisionPacket::new(
        chrono::Utc::now().date_naive(),
        MarketFeatures::default(),
        market.clone(),
        PortfolioPolicy::from_market_regime(&market),
        assets,
    );

    // 3. Gate the packet
    let result = ExecutionGate::gate_packet(&packet, trading_config, 0.0, 10000.0, 0.0);

    // 4. VERIFY: No trades produced, audit shows TradingDisabled
    assert_eq!(
        result.trades.len(),
        0,
        "No trades should be produced when trading is disabled"
    );
    assert_eq!(result.audits.len(), 1);
    assert_eq!(
        result.audits[0].blocked_by,
        Some("TradingDisabled".to_string())
    );
    assert!(!result.audits[0].passed);
}

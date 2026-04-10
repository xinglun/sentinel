use chrono::{NaiveDate, Utc};
use std::collections::HashMap;
use stock_sentinel::config::{DeviationBasis, ParsedRules, TrendConfig, WatchlistEntry};
use stock_sentinel::core::action_matrix::AssetAction;
use stock_sentinel::core::decision::DecisionPacket;
use stock_sentinel::core::engine::Engine;
use stock_sentinel::core::features::MarketFeatures;
use stock_sentinel::core::market_regime::{
    LifecycleState, MarketRegimeSnapshot, MarketState, RiskOverlay,
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
    use stock_sentinel::core::participation::ParticipationReadiness;
    let top_tier = vec!["MSFT".to_string(), "AAPL".to_string()];
    let prev_packet1 = DecisionPacket::new(
        NaiveDate::from_ymd_opt(2022, 12, 30).unwrap(),
        MarketFeatures::default(),
        prev_market.clone(),
        PortfolioPolicy::from_market_regime(&prev_market),
        vec![],
        ParticipationReadiness {
            core_tier_streak: 1,
            ..Default::default()
        },
        top_tier.clone(),
        false,
        stock_sentinel::core::trend_cohesion::TrendCohesionSnapshot::default(),
    );
    let prev_packet2 = DecisionPacket::new(
        prev_features.date,
        prev_features,
        prev_market.clone(),
        PortfolioPolicy::from_market_regime(&prev_market),
        vec![],
        ParticipationReadiness {
            core_tier_streak: 2,
            ..Default::default()
        },
        top_tier.clone(),
        false,
        stock_sentinel::core::trend_cohesion::TrendCohesionSnapshot::default(),
    );

    let packet = Engine::run_daily_pipeline(
        &histories,
        &rules,
        &[prev_packet1, prev_packet2],
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
    };

    let rules = create_mock_rules();
    let histories = vec![(history, &entry)];

    let packet = Engine::run_daily_pipeline(&histories, &rules, &[], &HashMap::new())
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
        ..Default::default()
    };
    let histories = vec![(history, &entry)];

    // Day 1: Start from scratch (no history)
    let p1 =
        Engine::run_daily_pipeline(&histories, &rules, &[], &HashMap::new()).expect("P1 failed");
    let age1 = p1.market_features.regime_age;

    // Day 2: Pass p1 as history
    let p2 = Engine::run_daily_pipeline(
        &histories,
        &rules,
        std::slice::from_ref(&p1),
        &HashMap::new(),
    )
    .expect("P2 failed");
    let age2 = p2.market_features.regime_age;

    // Day 3: Pass p1, p2 as history
    let p3 = Engine::run_daily_pipeline(&histories, &rules, &[p1, p2], &HashMap::new())
        .expect("P3 failed");
    let age3 = p3.market_features.regime_age;

    // Expect exactly 1 day increment per call
    assert_eq!(age2, age1 + 1, "Age should increment by 1 on Day 2");
    assert_eq!(age3, age2 + 1, "Age should increment by 1 on Day 3");
}

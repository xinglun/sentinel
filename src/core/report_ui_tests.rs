#[cfg(test)]
mod tests {
    use crate::core::action_matrix::{AssetAction, AssetActionDecision};
    use crate::core::asset_state::AssetState;
    use crate::core::decision::{DecisionPacket, TelegramOutput};
    use crate::core::market_regime::{
        LifecycleState, MarketRegimeSnapshot, MarketState, RiskOverlay,
    };
    use crate::core::portfolio_policy::{PortfolioPolicy, RiskAssetsMode};
    use crate::core::report::*;
    use crate::core::runtime_mode::ExecutionMode;
    use chrono::Utc;
    use std::collections::HashMap;

    fn mock_asset(
        symbol: &str,
        action: AssetAction,
        state: AssetState,
        changed: bool,
    ) -> AssetActionDecision {
        AssetActionDecision {
            symbol: symbol.to_string(),
            price: 100.0,
            state,
            action,
            reasons: vec!["Test reason".to_string()],
            deviation: Some(1.5),
            z_score: Some(2.0),
            trade_enabled: true,
            trade_amount: 1000.0,
            config_multiplier: 1.0,
            prev_action: if changed {
                Some(AssetAction::HOLD)
            } else {
                None
            },
            action_changed: changed,
        }
    }

    fn mock_config() -> crate::config::AppConfig {
        crate::config::AppConfig {
            version: 1,
            output: crate::config::OutputConfig {
                timezone: "UTC".to_string(),
                format: "markdown".to_string(),
                save_to: "./reports".to_string(),
                weight_kind: None,
            },
            telegram: None,
            futu: None,
            finnhub: None,
            trading: None,
            provider: None,
            rules: crate::config::RulesConfig {
                trend: crate::config::TrendConfig {
                    lookback_days: 20,
                    flat_threshold_pct: 0.5,
                },
                deviation_bands: Default::default(),
                actions: Default::default(),
                sizing_multipliers: None,
            },
            watchlist: vec![],
        }
    }

    #[test]
    fn test_telegram_v3_ui_bullish() {
        let assets = vec![
            mock_asset("TSLA", AssetAction::ACCUMULATE, AssetState::PULLBACK, true),
            mock_asset("NVDA", AssetAction::HOLD, AssetState::OPTIMAL, false),
        ];
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_features: Default::default(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::ESTABLISHED,
                lifecycle_state: LifecycleState::ESTABLISHED,
                risk_overlay: RiskOverlay::NORMAL,
                reasons: vec![],
            },
            portfolio_policy: PortfolioPolicy {
                risk_assets_mode: RiskAssetsMode::NEUTRAL,
                target_exposure_min: 0.2,
                target_exposure_max: 0.8,
                allow_chase: true,
                allow_pullback_buy: true,
                allow_new_risk: true,
            },
            assets,
            telegram: TelegramOutput {
                headline: "ESTABLISHED | Risk NORMAL".to_string(),
                summary: "Stay long".to_string(),
                bias: "Stay Long".to_string(),
            },
        };

        let card = generate_refined_report(
            &mock_config(),
            &packet,
            0.0,
            &HashMap::new(),
            &ExecutionMode::DryRun,
            vec!["MISSING".to_string()],
        )
        .unwrap()
        .markdown_body;

        // Verify V3 elements
        assert!(card.contains("ESTABLISHED | Risk Normal")); // Header casing
        assert!(card.contains("Bias: Stay Long")); // Bias line
        assert!(card.contains("仓位: 20-80%"));
        assert!(card.contains("TSLA  加仓  PULLBACK [CHANGED]")); // Localized + Tag
        assert!(card.contains("NVDA  持有  OPTIMAL")); // Localized
        assert!(card.contains("Data Notice: MISSING fetch failed")); // Specific symbols
    }

    #[test]
    fn test_telegram_v3_ui_defensive() {
        let assets = vec![mock_asset(
            "SPY",
            AssetAction::AVOID,
            AssetState::DEFEND,
            false,
        )];
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_features: Default::default(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::DEFENSIVE,
                lifecycle_state: LifecycleState::ESTABLISHED,
                risk_overlay: RiskOverlay::DEFENSIVE,
                reasons: vec![],
            },
            portfolio_policy: PortfolioPolicy::from_market_regime(&MarketRegimeSnapshot {
                market_state: MarketState::DEFENSIVE,
                lifecycle_state: LifecycleState::ESTABLISHED,
                risk_overlay: RiskOverlay::DEFENSIVE,
                reasons: vec![],
            }),
            assets,
            telegram: TelegramOutput {
                headline: "DEFENSIVE | Risk DEFENSIVE".to_string(),
                summary: "Stop buying".to_string(),
                bias: "Defense First".to_string(),
            },
        };

        let card = generate_refined_report(
            &mock_config(),
            &packet,
            0.0,
            &HashMap::new(),
            &ExecutionMode::Live,
            vec![],
        )
        .unwrap()
        .markdown_body;

        // Verify Defensive V3 elements
        assert!(card.contains("DEFENSIVE | Risk Defensive"));
        assert!(card.contains("Warning: Circuit Breaker Active")); // Reordered warning
        assert!(card.contains("Bias: Defense First"));
        assert!(card.contains("Priority Actions")); // Reordered section title
        assert!(card.contains("SPY  回避  DEFEND")); // Localized
    }

    #[test]
    fn test_telegram_v3_ui_data_error() {
        let assets = vec![mock_asset(
            "AAPL",
            AssetAction::HOLD,
            AssetState::OPTIMAL,
            false,
        )];
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_features: Default::default(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::ESTABLISHED,
                lifecycle_state: LifecycleState::ESTABLISHED,
                risk_overlay: RiskOverlay::NORMAL,
                reasons: vec![],
            },
            portfolio_policy: PortfolioPolicy {
                target_exposure_min: 0.1,
                target_exposure_max: 0.5,
                allow_chase: false,
                allow_pullback_buy: true,
                allow_new_risk: true,
                risk_assets_mode: RiskAssetsMode::NEUTRAL,
            },
            assets,
            telegram: TelegramOutput {
                headline: "ESTABLISHED".to_string(),
                summary: "Summary".to_string(),
                bias: "Bias".to_string(),
            },
        };

        // Scenario: Multiple failures (Warning)
        let card = generate_refined_report(
            &mock_config(),
            &packet,
            0.0,
            &HashMap::new(),
            &ExecutionMode::DryRun,
            vec!["FIG".to_string(), "U".to_string()],
        )
        .unwrap()
        .markdown_body;

        assert!(card.contains("Data Warning: FIG, U fetch failed"));

        // Scenario: Severe failures (Critical)
        let card_crit = generate_refined_report(
            &mock_config(),
            &packet,
            0.0,
            &HashMap::new(),
            &ExecutionMode::DryRun,
            vec![
                "A".to_string(),
                "B".to_string(),
                "C".to_string(),
                "D".to_string(),
            ],
        )
        .unwrap()
        .markdown_body;

        assert!(card_crit.contains("Data Critical: A, B, C, D fetch failed"));
    }

    #[test]
    fn test_telegram_v3_ui_action_changes() {
        let assets = [
            mock_asset("TSLA", AssetAction::ACCUMULATE, AssetState::PULLBACK, true), // CHANGED
            mock_asset("GOOG", AssetAction::WAIT, AssetState::FORMING, true), // NEW (in mock_asset logic, if prev_action is None)
        ];
        // Override the second one to be NEW (no prev_action)
        let mut goog = mock_asset("GOOG", AssetAction::WAIT, AssetState::FORMING, true);
        goog.prev_action = None;

        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_features: Default::default(),
            market_regime: MarketRegimeSnapshot {
                market_state: MarketState::ESTABLISHED,
                lifecycle_state: LifecycleState::ESTABLISHED,
                risk_overlay: RiskOverlay::NORMAL,
                reasons: vec![],
            },
            portfolio_policy: PortfolioPolicy {
                target_exposure_min: 0.1,
                target_exposure_max: 0.5,
                allow_chase: false,
                allow_pullback_buy: true,
                allow_new_risk: true,
                risk_assets_mode: RiskAssetsMode::NEUTRAL,
            },
            assets: vec![assets[0].clone(), goog],
            telegram: TelegramOutput {
                headline: "ESTABLISHED".to_string(),
                summary: "Summary".to_string(),
                bias: "Bias".to_string(),
            },
        };

        let card = generate_refined_report(
            &mock_config(),
            &packet,
            0.0,
            &HashMap::new(),
            &ExecutionMode::DryRun,
            vec![],
        )
        .unwrap()
        .markdown_body;

        assert!(card.contains("TSLA  加仓  PULLBACK [CHANGED]"));
        assert!(card.contains("GOOG  等待  FORMING [NEW]"));
    }
}

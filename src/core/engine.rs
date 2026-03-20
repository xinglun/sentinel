use crate::config::{ParsedRules, WatchlistEntry};
use crate::core::features::{AssetFeatures, MarketFeatures};
use crate::core::market_regime::MarketRegimeStateMachine;
use crate::data::yahoo_provider::TickerHistory;

use crate::core::asset_state::{AssetState, AssetStateMachine};
use crate::core::portfolio_policy::PortfolioPolicy;

use crate::core::action_matrix::ActionMatrix;
use crate::core::decision::DecisionPacket;
use anyhow::Result;
use chrono::Local;

pub struct Engine;

impl Engine {
    /// Runs the complete modular decision pipeline for a set of assets.
    /// Returns a deterministic DecisionPacket based on current features and optional previous session context.
    pub fn run_daily_pipeline<'a>(
        ticker_histories: &[(TickerHistory<'a>, &WatchlistEntry)],
        rules: &ParsedRules,
        prev_packet: Option<&DecisionPacket>,
    ) -> Result<DecisionPacket> {
        if ticker_histories.is_empty() {
            return Err(anyhow::anyhow!("No ticker history provided for pipeline"));
        }

        // 0. Extract Previous Context
        let _prev_state = prev_packet.map(|p| p.market_regime.market_state);
        let prev_age = prev_packet
            .map(|p| p.market_features.regime_age)
            .unwrap_or(0);

        // 1. Feature Layer: Asset Features
        let mut asset_features = Vec::new();
        for (history, entry) in ticker_histories {
            let f = AssetFeatures::compute(history, entry, rules);

            asset_features.push(f);
        }

        // 2. Market Features (Initial - using prev_packet for context)
        let mut market_features = MarketFeatures::compute(&asset_features, prev_age, prev_packet);

        // 3. Market Regime State Machine (Consolidated Transition)
        let (market_regime, _) = MarketRegimeStateMachine::transition(
            prev_packet.map(|p| &p.market_regime),
            &mut market_features,
            prev_age,
        );

        // 4. Portfolio Policy
        let portfolio_policy = PortfolioPolicy::from_market_regime(&market_regime);

        // 5. Asset Execution State & Action Matrix
        let mut asset_decisions = Vec::new();
        for f in &asset_features {
            let asset_state_snapshot = AssetStateMachine::compute_state(f, rules);

            // Get per-asset constraints from watchlist entry
            let (_h, entry) = ticker_histories
                .iter()
                .find(|(h, _)| h.symbol == f.symbol)
                .unwrap();
            let trade_enabled = entry.trade_enabled.unwrap_or(true);
            let trade_amount = entry.trade_amount.unwrap_or(2000.0); // Safe default

            let state_name = format!("{:?}", asset_state_snapshot.state).to_lowercase();

            // Tiered Multiplier Selection
            let mut action_key = state_name.clone();
            if asset_state_snapshot.state == AssetState::OVERHEAT {
                action_key = if f.z_score.unwrap_or(0.0) >= 2.5 {
                    "overheat_2".to_string()
                } else {
                    "overheat_1".to_string()
                };
            } else if asset_state_snapshot.state == AssetState::CAUTION
                || asset_state_snapshot.state == AssetState::DEFEND
            {
                // Heuristic for fear tiers if needed, for now use fear_1/2 if present or fallback to state name
                action_key = if f.z_score.unwrap_or(0.0) <= -2.0 {
                    "fear_2".to_string()
                } else if f.z_score.unwrap_or(0.0) <= -1.0 {
                    "fear_1".to_string()
                } else {
                    state_name
                };
            }

            let config_multiplier = rules
                .sizing_multipliers
                .as_ref()
                .and_then(|m: &std::collections::HashMap<String, f64>| m.get(&action_key).copied())
                .unwrap_or(1.0);

            let mut decision = ActionMatrix::decide(
                &market_regime,
                &portfolio_policy,
                &asset_state_snapshot,
                f.deviation,
                f.z_score,
                f.close,
                trade_enabled,
                trade_amount,
                config_multiplier,
            );

            // Enrich with previous action context
            let prev_action = prev_packet
                .and_then(|p| p.assets.iter().find(|a| a.symbol == f.symbol))
                .map(|a| a.action);

            decision.prev_action = prev_action;
            decision.action_changed = prev_action.map(|a| a != decision.action).unwrap_or(false);

            asset_decisions.push(decision);
        }

        // 6. Final Decision Packet
        let date = asset_features
            .first()
            .map(|f| f.date)
            .unwrap_or_else(|| Local::now().date_naive());
        let packet = DecisionPacket::new(
            date,
            market_features,
            market_regime,
            portfolio_policy,
            asset_decisions,
        );

        Ok(packet)
    }
}

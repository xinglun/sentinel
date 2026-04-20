use super::models::{BreakoutChange, BreakoutStatus, MarketStateOutput};
use super::transition::StateTransitionManager;
use crate::config::ParsedMarketStateEngineConfig;
use crate::core::decision::DecisionPacket;
use crate::core::features::MarketFeatures;

pub struct DecisionEngine;

impl DecisionEngine {
    pub fn process(
        config: &ParsedMarketStateEngineConfig,
        features: &MarketFeatures,
        has_mainline: bool,
        current_breakouts: &[String],
        prev_packet: Option<&DecisionPacket>,
    ) -> MarketStateOutput {
        let transition_manager = StateTransitionManager::new(config.clone());
        let follower_count = current_breakouts.len();

        let current_state = prev_packet
            .and_then(|p| p.market_state.as_ref())
            .map(|m| m.lifecycle.clone())
            .unwrap_or_default();

        let (next_state, action_status, _transitions) =
            transition_manager.evaluate(&current_state, features, has_mainline, follower_count);

        let mut previous_breakouts = Vec::new();
        if let Some(prev) = prev_packet {
            if let Some(ms) = &prev.market_state {
                for change in &ms.breakout_changes {
                    if change.status == BreakoutStatus::New
                        || change.status == BreakoutStatus::Unchanged
                    {
                        previous_breakouts.push(change.symbol.clone());
                    }
                }
            }
        }

        let mut breakout_changes = Vec::new();

        for symbol in current_breakouts {
            if !previous_breakouts.contains(symbol) {
                breakout_changes.push(BreakoutChange {
                    symbol: symbol.clone(),
                    status: BreakoutStatus::New,
                });
            } else {
                breakout_changes.push(BreakoutChange {
                    symbol: symbol.clone(),
                    status: BreakoutStatus::Unchanged,
                });
            }
        }

        for symbol in &previous_breakouts {
            if !current_breakouts.contains(symbol) {
                breakout_changes.push(BreakoutChange {
                    symbol: symbol.clone(),
                    status: BreakoutStatus::Removed,
                });
            }
        }

        MarketStateOutput {
            lifecycle: next_state,
            action_status,
            breakout_changes,
            stability: features.stability_score,
            continuity_days: features.regime_age,
            has_mainline,
        }
    }
}

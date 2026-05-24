use crate::config::ParsedRules;
use crate::features::radar::application::policy::breakout_detection::BreakoutStatus;
use crate::features::radar::application::policy::decision::DecisionPacket;
use crate::features::radar::application::policy::market_regime::{MarketState, RiskOverlay};
use crate::features::radar::application::policy::trend_cohesion::{
    TrendCohesionStatus, TrendCohesionTopology, TrendRecognitionEvidence,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct GateTransition {
    pub from: bool,
    pub to: bool,
    pub unmet_conditions_changed: bool,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub persisting: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct StatusTransition<T: Serialize + PartialEq> {
    pub from: T,
    pub to: T,
    pub changed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct BreakoutTransition {
    pub symbol: String,
    pub from_status: BreakoutStatus,
    pub to_status: BreakoutStatus,
    pub status_changed: bool,
    pub risk_changed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
pub enum OpportunityMode {
    #[default]
    NoTradeCold,
    NoTradeScout,
    Ready,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct StateTransitionLog {
    pub no_trade_persists: bool,
    pub market_state: StatusTransition<MarketState>,
    pub risk_overlay: StatusTransition<RiskOverlay>,
    pub trend_cohesion_gate: GateTransition,
    pub trend_cohesion_status: StatusTransition<TrendCohesionStatus>,
    pub trend_cohesion_topology: StatusTransition<TrendCohesionTopology>,
    pub breakout_changes: Vec<BreakoutTransition>,
    #[serde(default)]
    pub opportunity_mode: StatusTransition<OpportunityMode>,
    #[serde(default)]
    pub scout_days_without_expansion: usize,
    #[serde(default)]
    pub scout_abort_days: usize,
    #[serde(default)]
    pub scout_reset_triggered: bool,
    #[serde(default)]
    pub breakout_active_count: usize,
    #[serde(default)]
    pub trend_recognition: Option<TrendRecognitionEvidence>,
}

impl StateTransitionLog {
    pub fn compare(prev: Option<&DecisionPacket>, curr: &DecisionPacket) -> Self {
        let defaults = crate::config::ParsedMarketStateEngineRules::default();
        Self::compare_with_abort_days(prev, curr, defaults.scout_abort_days)
    }

    pub fn compare_with_rules(
        prev: Option<&DecisionPacket>,
        curr: &DecisionPacket,
        rules: &ParsedRules,
    ) -> Self {
        Self::compare_with_abort_days(prev, curr, rules.market_state_engine.scout_abort_days)
    }

    fn compare_with_abort_days(
        prev: Option<&DecisionPacket>,
        curr: &DecisionPacket,
        scout_abort_days: usize,
    ) -> Self {
        let no_trade_prev = prev.map(|p| !p.trend_cohesion.gate_passed).unwrap_or(true);
        let no_trade_curr = !curr.trend_cohesion.gate_passed;

        let market_state = StatusTransition {
            from: prev
                .map(|p| p.market_regime.market_state)
                .unwrap_or_default(),
            to: curr.market_regime.market_state,
            changed: prev
                .map(|p| p.market_regime.market_state != curr.market_regime.market_state)
                .unwrap_or(true),
        };

        let risk_overlay = StatusTransition {
            from: prev
                .map(|p| p.market_regime.risk_overlay)
                .unwrap_or_default(),
            to: curr.market_regime.risk_overlay,
            changed: prev
                .map(|p| p.market_regime.risk_overlay != curr.market_regime.risk_overlay)
                .unwrap_or(true),
        };

        let (t_added, t_removed, t_persisting) = Self::diff_reasons(
            prev.map(|p| {
                p.trend_cohesion
                    .unmet_conditions
                    .iter()
                    .map(|c| format!("{:?}", c))
                    .collect()
            })
            .unwrap_or_default(),
            curr.trend_cohesion
                .unmet_conditions
                .iter()
                .map(|c| format!("{:?}", c))
                .collect(),
        );
        let trend_cohesion_gate = GateTransition {
            from: prev.map(|p| p.trend_cohesion.gate_passed).unwrap_or(false),
            to: curr.trend_cohesion.gate_passed,
            unmet_conditions_changed: !t_added.is_empty() || !t_removed.is_empty(),
            added: t_added,
            removed: t_removed,
            persisting: t_persisting,
        };

        let trend_cohesion_status = StatusTransition {
            from: prev.map(|p| p.trend_cohesion.status).unwrap_or_default(),
            to: curr.trend_cohesion.status,
            changed: prev
                .map(|p| p.trend_cohesion.status != curr.trend_cohesion.status)
                .unwrap_or(true),
        };

        let trend_cohesion_topology = StatusTransition {
            from: prev.map(|p| p.trend_cohesion.topology).unwrap_or_default(),
            to: curr.trend_cohesion.topology,
            changed: prev
                .map(|p| p.trend_cohesion.topology != curr.trend_cohesion.topology)
                .unwrap_or(true),
        };

        let mut breakout_changes = Vec::new();
        for curr_asset in &curr.assets {
            let prev_asset =
                prev.and_then(|p| p.assets.iter().find(|a| a.symbol == curr_asset.symbol));

            let from_status = prev_asset.map(|a| a.breakout.status).unwrap_or_default();
            let to_status = curr_asset.breakout.status;
            let status_changed = from_status != to_status;

            let prev_risk = prev_asset
                .map(|a| a.breakout.failed_breakout_risk)
                .unwrap_or(0.0);
            let curr_risk = curr_asset.breakout.failed_breakout_risk;
            let risk_changed = (prev_risk - curr_risk).abs() > 0.1;

            if status_changed || risk_changed {
                breakout_changes.push(BreakoutTransition {
                    symbol: curr_asset.symbol.clone(),
                    from_status,
                    to_status,
                    status_changed,
                    risk_changed,
                });
            }
        }

        let prev_breakout_active_count = prev
            .map(|p| {
                p.assets
                    .iter()
                    .filter(|a| a.breakout.status != BreakoutStatus::NoBreakout)
                    .count()
            })
            .unwrap_or(0);
        let breakout_active_count = curr
            .assets
            .iter()
            .filter(|a| a.breakout.status != BreakoutStatus::NoBreakout)
            .count();
        let breakout_expanded = breakout_active_count >= 2;
        let has_new_breakout_event = breakout_changes
            .iter()
            .any(|b| b.from_status == BreakoutStatus::NoBreakout && b.status_changed);
        let has_expansion_event =
            breakout_expanded && breakout_active_count > prev_breakout_active_count;
        let prev_log = prev.and_then(|p| p.transition_log.as_ref());
        let prev_mode = prev_log
            .map(|log| log.opportunity_mode.to)
            .unwrap_or_default();
        let prev_scout_days = prev_log
            .map(|log| log.scout_days_without_expansion)
            .unwrap_or(0);
        let scout_entry_signal = has_new_breakout_event || has_expansion_event;
        let mut mode_to = if curr.trend_cohesion.gate_passed {
            OpportunityMode::Ready
        } else if (scout_entry_signal || prev_mode == OpportunityMode::NoTradeScout)
            && breakout_active_count > 0
        {
            OpportunityMode::NoTradeScout
        } else {
            OpportunityMode::NoTradeCold
        };
        let mut scout_days_without_expansion = 0usize;
        let mut scout_reset_triggered = false;
        let effective_abort_days = scout_abort_days.max(1);
        if mode_to == OpportunityMode::NoTradeScout {
            if breakout_expanded {
                scout_days_without_expansion = 0;
            } else {
                scout_days_without_expansion = if prev_mode == OpportunityMode::NoTradeScout {
                    prev_scout_days.saturating_add(1)
                } else {
                    1
                };
                if scout_days_without_expansion >= effective_abort_days {
                    mode_to = OpportunityMode::NoTradeCold;
                    scout_days_without_expansion = 0;
                    scout_reset_triggered = true;
                }
            }
        }

        Self {
            no_trade_persists: no_trade_prev && no_trade_curr,
            market_state,
            risk_overlay,
            trend_cohesion_gate,
            trend_cohesion_status,
            trend_cohesion_topology,
            breakout_changes,
            opportunity_mode: StatusTransition {
                from: prev_mode,
                to: mode_to,
                changed: prev_mode != mode_to,
            },
            scout_days_without_expansion,
            scout_abort_days: effective_abort_days,
            scout_reset_triggered,
            breakout_active_count,
            trend_recognition: curr.trend_recognition.clone(),
        }
    }

    fn diff_reasons(
        prev: Vec<String>,
        curr: Vec<String>,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let curr_set: HashSet<String> = curr.into_iter().collect();
        let prev_set: HashSet<String> = prev.into_iter().collect();

        let added = curr_set
            .difference(&prev_set)
            .cloned()
            .collect::<Vec<String>>();
        let removed = prev_set
            .difference(&curr_set)
            .cloned()
            .collect::<Vec<String>>();
        let persisting = curr_set
            .intersection(&prev_set)
            .cloned()
            .collect::<Vec<String>>();

        (added, removed, persisting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::application::policy::action_matrix::AssetActionDecision;
    use crate::features::radar::application::policy::breakout_detection::BreakoutSnapshot;

    fn mock_packet(state: MarketState, trend_gate_passed: bool) -> DecisionPacket {
        let mut curr = DecisionPacket::default();
        curr.market_regime.market_state = state;
        curr.trend_cohesion.gate_passed = trend_gate_passed;
        curr
    }

    #[test]
    fn test_no_trade_persists() {
        let prev = mock_packet(MarketState::DEFENSIVE, false);
        let curr = mock_packet(MarketState::DEFENSIVE, false);
        let log = StateTransitionLog::compare(Some(&prev), &curr);
        assert!(log.no_trade_persists);
        assert!(!log.market_state.changed);
        assert!(!log.trend_cohesion_gate.to);
    }

    #[test]
    fn test_gate_transition() {
        let prev = mock_packet(MarketState::IGNITION, false);
        let curr = mock_packet(MarketState::IGNITION, true);

        let log = StateTransitionLog::compare(Some(&prev), &curr);
        assert!(!log.no_trade_persists);
        assert!(log.trend_cohesion_gate.to);
        assert!(!log.trend_cohesion_gate.from);
    }

    #[test]
    fn test_breakout_transition() {
        let mut prev = mock_packet(MarketState::ESTABLISHED, true);
        prev.assets.push(AssetActionDecision {
            symbol: "AAPL".to_string(),
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::NoBreakout,
                failed_breakout_risk: 10.0,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut curr = mock_packet(MarketState::ESTABLISHED, true);
        curr.assets.push(AssetActionDecision {
            symbol: "AAPL".to_string(),
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::EmergingBreakout,
                failed_breakout_risk: 10.0,
                ..Default::default()
            },
            ..Default::default()
        });

        let log = StateTransitionLog::compare(Some(&prev), &curr);
        assert_eq!(log.breakout_changes.len(), 1);
        assert_eq!(log.breakout_changes[0].symbol, "AAPL");
        assert_eq!(
            log.breakout_changes[0].from_status,
            BreakoutStatus::NoBreakout
        );
        assert_eq!(
            log.breakout_changes[0].to_status,
            BreakoutStatus::EmergingBreakout
        );
        assert!(log.breakout_changes[0].status_changed);
        assert!(!log.breakout_changes[0].risk_changed);
    }

    #[test]
    fn test_first_packet() {
        let curr = mock_packet(MarketState::IGNITION, false);
        let log = StateTransitionLog::compare(None, &curr);
        assert!(log.no_trade_persists);
        assert!(log.market_state.changed); // from default IGNITION to IGNITION is technically same but compare treats None as default
        assert!(!log.trend_cohesion_gate.to);
    }

    #[test]
    fn test_reason_diffing() {
        use crate::features::radar::application::policy::trend_cohesion::{
            TrendCohesionGateCondition, TrendCohesionSnapshot,
        };

        let prev = DecisionPacket {
            trend_cohesion: TrendCohesionSnapshot {
                unmet_conditions: vec![TrendCohesionGateCondition::StabilityThreshold],
                ..Default::default()
            },
            ..Default::default()
        };

        let curr = DecisionPacket {
            trend_cohesion: TrendCohesionSnapshot {
                unmet_conditions: vec![TrendCohesionGateCondition::ContinuityThreshold],
                ..Default::default()
            },
            ..Default::default()
        };

        let log = StateTransitionLog::compare(Some(&prev), &curr);

        // Trend Gate。
        assert_eq!(
            log.trend_cohesion_gate.added,
            vec!["ContinuityThreshold".to_string()]
        );
        assert_eq!(
            log.trend_cohesion_gate.removed,
            vec!["StabilityThreshold".to_string()]
        );
        assert!(log.trend_cohesion_gate.persisting.is_empty());
    }

    #[test]
    fn test_opportunity_mode_switches_to_scout_when_breakout_appears_under_no_trade() {
        let prev = mock_packet(MarketState::IGNITION, false);
        let mut curr = mock_packet(MarketState::IGNITION, false);
        curr.assets.push(AssetActionDecision {
            symbol: "GOOG".to_string(),
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::EmergingBreakout,
                ..Default::default()
            },
            ..Default::default()
        });

        let log = StateTransitionLog::compare(Some(&prev), &curr);
        assert_eq!(log.opportunity_mode.to, OpportunityMode::NoTradeScout);
        assert_eq!(log.scout_days_without_expansion, 1);
        assert!(!log.scout_reset_triggered);
    }

    #[test]
    fn test_opportunity_mode_resets_to_cold_when_scout_expires_without_expansion() {
        let mut prev = mock_packet(MarketState::IGNITION, false);
        prev.transition_log = Some(StateTransitionLog {
            opportunity_mode: StatusTransition {
                from: OpportunityMode::NoTradeCold,
                to: OpportunityMode::NoTradeScout,
                changed: true,
            },
            scout_days_without_expansion: 2,
            scout_abort_days: 3,
            ..Default::default()
        });
        let mut curr = mock_packet(MarketState::IGNITION, false);
        curr.assets.push(AssetActionDecision {
            symbol: "GOOG".to_string(),
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::EmergingBreakout,
                ..Default::default()
            },
            ..Default::default()
        });

        let log = StateTransitionLog::compare(Some(&prev), &curr);
        assert_eq!(log.opportunity_mode.from, OpportunityMode::NoTradeScout);
        assert_eq!(log.opportunity_mode.to, OpportunityMode::NoTradeCold);
        assert!(log.scout_reset_triggered);
        assert_eq!(log.scout_days_without_expansion, 0);
    }

    #[test]
    fn test_opportunity_mode_does_not_reenter_scout_after_reset_without_new_signal() {
        let mut prev = mock_packet(MarketState::IGNITION, false);
        prev.assets.push(AssetActionDecision {
            symbol: "GOOG".to_string(),
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::EmergingBreakout,
                ..Default::default()
            },
            ..Default::default()
        });
        prev.transition_log = Some(StateTransitionLog {
            opportunity_mode: StatusTransition {
                from: OpportunityMode::NoTradeScout,
                to: OpportunityMode::NoTradeCold,
                changed: true,
            },
            scout_days_without_expansion: 0,
            scout_abort_days: 3,
            scout_reset_triggered: true,
            breakout_active_count: 1,
            ..Default::default()
        });
        let mut curr = mock_packet(MarketState::IGNITION, false);
        curr.assets.push(AssetActionDecision {
            symbol: "GOOG".to_string(),
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::EmergingBreakout,
                ..Default::default()
            },
            ..Default::default()
        });

        let log = StateTransitionLog::compare(Some(&prev), &curr);
        assert_eq!(log.opportunity_mode.from, OpportunityMode::NoTradeCold);
        assert_eq!(log.opportunity_mode.to, OpportunityMode::NoTradeCold);
        assert!(!log.scout_reset_triggered);
        assert_eq!(log.scout_days_without_expansion, 0);
    }

    #[test]
    fn test_opportunity_mode_drops_to_cold_when_breakout_disappears() {
        let mut prev = mock_packet(MarketState::IGNITION, false);
        prev.assets.push(AssetActionDecision {
            symbol: "GOOG".to_string(),
            breakout: BreakoutSnapshot {
                status: BreakoutStatus::EmergingBreakout,
                ..Default::default()
            },
            ..Default::default()
        });
        prev.transition_log = Some(StateTransitionLog {
            opportunity_mode: StatusTransition {
                from: OpportunityMode::NoTradeCold,
                to: OpportunityMode::NoTradeScout,
                changed: true,
            },
            scout_days_without_expansion: 1,
            scout_abort_days: 3,
            breakout_active_count: 1,
            ..Default::default()
        });

        let curr = mock_packet(MarketState::IGNITION, false);
        let log = StateTransitionLog::compare(Some(&prev), &curr);
        assert_eq!(log.opportunity_mode.from, OpportunityMode::NoTradeScout);
        assert_eq!(log.opportunity_mode.to, OpportunityMode::NoTradeCold);
        assert_eq!(log.breakout_active_count, 0);
        assert_eq!(log.scout_days_without_expansion, 0);
        assert!(!log.scout_reset_triggered);
    }
}

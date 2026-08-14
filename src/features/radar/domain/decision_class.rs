use crate::features::radar::domain::transition_log::{OpportunityMode, StateTransitionLog};
use serde::{Deserialize, Serialize};

/// Radar が日次に確定する行動許可の分類。
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionClass {
    #[default]
    NoTrade,
    Probe,
    Ready,
}

/// DecisionPacket に保存する分類と安定した理由コード。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionClassification {
    pub class: DecisionClass,
    pub reasons: Vec<String>,
    pub gate_blocked: bool,
}

impl DecisionClassification {
    pub const SNAPSHOT_VERSION: &'static str = "radar-v1.0.0";

    pub fn from_opportunity_mode(mode: OpportunityMode) -> DecisionClass {
        match mode {
            OpportunityMode::NoTradeCold => DecisionClass::NoTrade,
            OpportunityMode::NoTradeScout => DecisionClass::Probe,
            OpportunityMode::Ready => DecisionClass::Ready,
        }
    }

    pub fn from_transition_log(log: &StateTransitionLog) -> Self {
        Self::from_transition_log_with_confidence(log, 100.0)
    }

    pub fn from_transition_log_with_confidence(
        log: &StateTransitionLog,
        system_confidence: f64,
    ) -> Self {
        let class = Self::from_opportunity_mode(log.opportunity_mode.to);
        let mut reasons = Vec::new();

        if class != DecisionClass::Ready {
            if log.trend_cohesion_topology.to
                == crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader
            {
                reasons.push("NO_LEADER".to_string());
            }
            if !log.trend_cohesion_gate.to {
                reasons.push("TREND_GATE_BLOCKED".to_string());
            }
            if log
                .trend_cohesion_topology
                .to
                == crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::FragmentedLeaders
            {
                reasons.push("BREADTH_TOO_NARROW".to_string());
            }
            if !log.breakout_changes.is_empty() || log.breakout_active_count == 0 {
                reasons.push("BREAKOUT_UNCONFIRMED".to_string());
            }
        }

        if log.risk_overlay.to
            != crate::features::shared::domain::market_regime::RiskOverlay::NORMAL
        {
            reasons.push("RISK_OVERLAY_ACTIVE".to_string());
        }
        if system_confidence < 50.0 {
            reasons.push("CONFIDENCE_INSUFFICIENT".to_string());
        }

        reasons.sort();
        reasons.dedup();
        Self {
            class,
            reasons,
            gate_blocked: !log.trend_cohesion_gate.to,
        }
    }

    pub fn universe_id<I, S>(symbols: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut symbols = symbols
            .into_iter()
            .map(|symbol| symbol.as_ref().to_ascii_uppercase())
            .collect::<Vec<_>>();
        symbols.sort();
        symbols.dedup();
        format!("watchlist:{}", symbols.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn universe_id_is_order_independent_and_traceable() {
        assert_eq!(
            DecisionClassification::universe_id(["spy", "AAPL", "spy"]),
            "watchlist:AAPL,SPY"
        );
    }

    #[test]
    fn no_trade_reasons_are_stable_codes() {
        let classification =
            DecisionClassification::from_transition_log(&StateTransitionLog::default());
        assert_eq!(classification.class, DecisionClass::NoTrade);
        assert_eq!(
            classification.reasons,
            vec!["BREAKOUT_UNCONFIRMED", "NO_LEADER", "TREND_GATE_BLOCKED"]
        );
    }

    #[test]
    fn decision_class_serializes_as_contract_code() {
        assert_eq!(
            serde_json::to_string(&DecisionClass::NoTrade).unwrap(),
            "\"NO_TRADE\""
        );
    }

    #[test]
    fn classification_preserves_gate_fact_and_existing_confidence_rule() {
        let classification = DecisionClassification::from_transition_log_with_confidence(
            &StateTransitionLog::default(),
            49.9,
        );
        assert!(classification.gate_blocked);
        assert!(classification
            .reasons
            .contains(&"CONFIDENCE_INSUFFICIENT".to_string()));
    }
}

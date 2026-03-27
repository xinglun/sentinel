use crate::core::asset_state::AssetState;
use crate::core::market_regime::RiskOverlay;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum PositionIntent {
    /// 1. Take all money out
    EXIT = 4,
    /// 2. Reduce position size (e.g. 50%)
    TRIM = 3,
    /// 3. Stop adding, keep current position
    #[default]
    HOLD = 2,
    /// 4. Allow adding to position (Forbidden for Exit layer, only for Synthesizer)
    ADD = 1,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssetExitState {
    #[default]
    None,
    /// Market or Asset structural breakdown
    DefensiveExit,
    /// Asset dropped out of main strength group
    StrengthLoss,
    /// Market participation gate closed
    ParticipationExit,
    /// Volcano state - profit taking
    OverheatProfitTake,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ExitDecision {
    pub position_intent: PositionIntent,
    pub asset_exit_state: AssetExitState,
    pub reasons: Vec<String>,
}

impl ExitDecision {
    #[allow(clippy::too_many_arguments)]
    pub fn compute(
        _asset_symbol: &str,
        current_state: AssetState,
        prev_state: Option<AssetState>,
        state_streak: usize,
        out_of_top_tier_streak: usize,
        risk_overlay: RiskOverlay,
        participation_ready: bool,
        prev_participation_ready: bool,
    ) -> Self {
        let mut reasons = Vec::new();
        let mut intent = PositionIntent::HOLD;
        let mut exit_state = AssetExitState::None;

        // Rule 1: Defensive Exit (Highest Priority)
        if current_state == AssetState::DEFEND
            || risk_overlay == RiskOverlay::DEFENSIVE
            || risk_overlay == RiskOverlay::BROKEN
        {
            intent = PositionIntent::EXIT;
            exit_state = AssetExitState::DefensiveExit;
            reasons.push(format!(
                "Hard Risk: state={:?}, overlay={:?}",
                current_state, risk_overlay
            ));
            return Self {
                position_intent: intent,
                asset_exit_state: exit_state,
                reasons,
            };
        }

        // Rule 2: Strength Loss Exit
        if out_of_top_tier_streak >= 3 {
            intent = PositionIntent::TRIM;
            exit_state = AssetExitState::StrengthLoss;
            reasons.push(format!(
                "Strength Loss: out of top tier for {} days",
                out_of_top_tier_streak
            ));
        } else if let Some(prev) = prev_state {
            if (prev == AssetState::OPTIMAL || prev == AssetState::CRUISE)
                && current_state == AssetState::CAUTION
                && state_streak >= 2
                && intent < PositionIntent::TRIM
            {
                intent = PositionIntent::TRIM;
                exit_state = AssetExitState::StrengthLoss;
                reasons.push("Strength Loss: sustained CAUTION from strength".to_string());
            }
        }

        // Rule 3: Participation Exit
        if prev_participation_ready && !participation_ready && intent < PositionIntent::TRIM {
            // Market gate closed
            // For core assets (in top tier), we might just HOLD, but here we simplify to rule requirement
            // "弱资产 TRIM, 核心强资产 HOLD / FREEZE"
            // Since this compute is per asset, if out_of_top_tier_streak > 0, it's "weaker"
            if out_of_top_tier_streak > 0 {
                intent = PositionIntent::TRIM;
                reasons.push("Market Not Ready: trimming non-core asset".to_string());
            } else {
                intent = PositionIntent::HOLD;
                reasons.push("Market Not Ready: freezing core asset".to_string());
            }
            exit_state = AssetExitState::ParticipationExit;
        }

        // Rule 4: Overheat Profit-Take
        if current_state == AssetState::OVERHEAT && intent < PositionIntent::TRIM {
            intent = PositionIntent::TRIM;
            exit_state = AssetExitState::OverheatProfitTake;
            reasons.push("Overheat: partial profit taking".to_string());
        }

        Self {
            position_intent: intent,
            asset_exit_state: exit_state,
            reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::asset_state::AssetState;

    #[test]
    fn test_defensive_exit_priority() {
        // Defensive exit should override everything else (highest priority)
        let decision = ExitDecision::compute(
            "AAPL",
            AssetState::DEFEND, // Defensive state
            Some(AssetState::OPTIMAL),
            5,
            0,
            RiskOverlay::NORMAL,
            true,
            true,
        );
        assert_eq!(decision.position_intent, PositionIntent::EXIT);
        assert_eq!(decision.asset_exit_state, AssetExitState::DefensiveExit);
    }

    #[test]
    fn test_strength_loss_streak() {
        // Out of top tier for 3+ days triggers TRIM
        let decision = ExitDecision::compute(
            "AAPL",
            AssetState::OPTIMAL,
            Some(AssetState::OPTIMAL),
            10,
            3, // 3 days out of top tier
            RiskOverlay::NORMAL,
            true,
            true,
        );
        assert_eq!(decision.position_intent, PositionIntent::TRIM);
        assert_eq!(decision.asset_exit_state, AssetExitState::StrengthLoss);
    }

    #[test]
    fn test_strength_loss_caution_persistence() {
        // OPTIMAL -> CAUTION for 2+ days triggers TRIM
        let decision = ExitDecision::compute(
            "AAPL",
            AssetState::CAUTION,
            Some(AssetState::OPTIMAL),
            2, // 2nd day in CAUTION
            0,
            RiskOverlay::NORMAL,
            true,
            true,
        );
        assert_eq!(decision.position_intent, PositionIntent::TRIM);
        assert_eq!(decision.asset_exit_state, AssetExitState::StrengthLoss);
    }

    #[test]
    fn test_participation_loss() {
        // Market ready -> not ready triggers TRIM for non-core
        let decision = ExitDecision::compute(
            "AAPL",
            AssetState::OPTIMAL,
            Some(AssetState::OPTIMAL),
            10,
            1, // Not core (out of top tier)
            RiskOverlay::NORMAL,
            false, // Now false
            true,  // Was true
        );
        assert_eq!(decision.position_intent, PositionIntent::TRIM);
        assert_eq!(decision.asset_exit_state, AssetExitState::ParticipationExit);

        // Core assets should stay in HOLD/FREEZE
        let decision_core = ExitDecision::compute(
            "AAPL",
            AssetState::OPTIMAL,
            Some(AssetState::OPTIMAL),
            10,
            0, // Core (in top tier)
            RiskOverlay::NORMAL,
            false,
            true,
        );
        assert_eq!(decision_core.position_intent, PositionIntent::HOLD);
    }

    #[test]
    fn test_overheat_profit_take() {
        let decision = ExitDecision::compute(
            "AAPL",
            AssetState::OVERHEAT,
            Some(AssetState::OPTIMAL),
            1,
            0,
            RiskOverlay::NORMAL,
            true,
            true,
        );
        assert_eq!(decision.position_intent, PositionIntent::TRIM);
        assert_eq!(
            decision.asset_exit_state,
            AssetExitState::OverheatProfitTake
        );
    }

    #[test]
    fn test_exit_decision_never_contains_add() {
        // If no exit rules trigger and everything is ready, allow HOLD (Synthesizer handles ADD)
        let decision = ExitDecision::compute(
            "AAPL",
            AssetState::OPTIMAL,
            Some(AssetState::OPTIMAL),
            5,
            0,
            RiskOverlay::NORMAL,
            true,
            true,
        );
        assert_eq!(decision.position_intent, PositionIntent::HOLD);
    }
}

use crate::features::radar::domain::asset_state::AssetState;
use crate::features::radar::domain::market_regime::RiskOverlay;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum PositionIntent {
    /// 1. 全量 exit する。
    EXIT = 4,
    /// 2. position size を削減する（例: 50%）。
    TRIM = 3,
    /// 3. 追加を止め、現在 position を維持する。
    #[default]
    HOLD = 2,
    /// 4. position 追加を許可する（Exit layer では禁止し、Synthesizer 専用）。
    ADD = 1,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssetExitState {
    #[default]
    None,
    /// market または asset の構造的 breakdown。
    DefensiveExit,
    /// asset が主要 strength group から外れた状態。
    StrengthLoss,
    /// market trend cohesion が失われた状態。
    CohesionExit,
    /// volcano 状態による利益確定。
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
        trend_gate_passed: bool,
        prev_trend_gate_passed: bool,
    ) -> Self {
        let mut reasons = Vec::new();
        let mut intent = PositionIntent::HOLD;
        let mut exit_state = AssetExitState::None;

        // ルール 1: defensive exit（最優先）。
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

        // ルール 2: strength loss exit。
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

        // ルール 3: trend cohesion gate exit。
        if prev_trend_gate_passed && !trend_gate_passed && intent < PositionIntent::TRIM {
            // 追随可能な leader structure gate が閉じた状態。
            // core asset（top tier）は HOLD 相当だが、ここでは rule 要件へ単純化する。
            // 弱い asset は TRIM、core の強い asset は HOLD / FREEZE として扱う。
            // この計算は asset 単位のため、out_of_top_tier_streak > 0 を弱い側とみなす。
            if out_of_top_tier_streak > 0 {
                intent = PositionIntent::TRIM;
                reasons.push("Trend Gate Closed: trimming non-core asset".to_string());
            } else {
                intent = PositionIntent::HOLD;
                reasons.push("Trend Gate Closed: freezing core asset".to_string());
            }
            exit_state = AssetExitState::CohesionExit;
        }

        // ルール 4: overheat profit-take。
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
    use crate::features::radar::domain::asset_state::AssetState;

    #[test]
    fn test_defensive_exit_priority() {
        // defensive exit は最優先で他の判断を上書きする。
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
    fn test_cohesion_loss() {
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
        assert_eq!(decision.asset_exit_state, AssetExitState::CohesionExit);

        // core asset は HOLD / FREEZE に留める。
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
        // exit rule が発火せず全条件が ready の場合は HOLD を許可する（ADD は Synthesizer が扱う）。
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

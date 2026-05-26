use crate::features::radar::domain::action_matrix::AssetAction;
use crate::features::radar::domain::exit::{ExitDecision, PositionIntent};

pub struct IntentSynthesizer;

impl IntentSynthesizer {
    /// ActionMatrix、Trend Cohesion Gate、ExitDecision を統合して最終 PositionIntent を合成する。
    /// 優先順位は EXIT > TRIM > HOLD > ADD とする。
    pub fn synthesize(
        base_action: AssetAction,
        exit_decision: &ExitDecision,
        trend_gate_passed: bool,
    ) -> PositionIntent {
        // 1. Exit layer を最優先し、EXIT と TRIM は他の判断を上書きする。
        if exit_decision.position_intent == PositionIntent::EXIT {
            return PositionIntent::EXIT;
        }
        if exit_decision.position_intent == PositionIntent::TRIM {
            return PositionIntent::TRIM;
        }

        // 2. Trend Cohesion Gate が未通過なら ADD しない。
        if !trend_gate_passed {
            return PositionIntent::HOLD;
        }

        // 3. Action Matrix の判断を統合する。
        match base_action {
            AssetAction::ACCUMULATE => PositionIntent::ADD,
            AssetAction::REDUCE => PositionIntent::TRIM,
            AssetAction::AVOID => PositionIntent::EXIT,
            _ => PositionIntent::HOLD,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::domain::exit::AssetExitState;

    #[test]
    fn test_intent_synthesizer_priority() {
        let exit_trim = ExitDecision {
            position_intent: PositionIntent::TRIM,
            asset_exit_state: AssetExitState::StrengthLoss,
            reasons: vec![],
        };

        // matrix が ACCUMULATE でも Exit TRIM を優先する。
        let intent = IntentSynthesizer::synthesize(AssetAction::ACCUMULATE, &exit_trim, true);
        assert_eq!(intent, PositionIntent::TRIM);

        // market が ready でなければ ACCUMULATE は HOLD になる。
        let exit_none = ExitDecision::default();
        let intent_not_ready =
            IntentSynthesizer::synthesize(AssetAction::ACCUMULATE, &exit_none, false);
        assert_eq!(intent_not_ready, PositionIntent::HOLD);

        // すべて通過した場合、matrix の ACCUMULATE は ADD になる。
        let intent_add = IntentSynthesizer::synthesize(AssetAction::ACCUMULATE, &exit_none, true);
        assert_eq!(intent_add, PositionIntent::ADD);
    }
}

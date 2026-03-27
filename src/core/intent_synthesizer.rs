use crate::core::action_matrix::AssetAction;
use crate::core::exit::{ExitDecision, PositionIntent};

pub struct IntentSynthesizer;

impl IntentSynthesizer {
    /// Synthesizes the final PositionIntent by combining ActionMatrix, Readiness, and ExitDecision.
    /// Priority Rule: EXIT > TRIM > HOLD > ADD
    pub fn synthesize(
        base_action: AssetAction,
        exit_decision: &ExitDecision,
        participation_ready: bool,
    ) -> PositionIntent {
        // 1. Respect Exit Layer first (EXIT and TRIM override everything)
        if exit_decision.position_intent == PositionIntent::EXIT {
            return PositionIntent::EXIT;
        }
        if exit_decision.position_intent == PositionIntent::TRIM {
            return PositionIntent::TRIM;
        }

        // 2. Market Readiness Gate (If not ready, never ADD)
        if !participation_ready {
            return PositionIntent::HOLD;
        }

        // 3. Action Matrix Integration
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
    use crate::core::exit::AssetExitState;

    #[test]
    fn test_intent_synthesizer_priority() {
        let exit_trim = ExitDecision {
            position_intent: PositionIntent::TRIM,
            asset_exit_state: AssetExitState::StrengthLoss,
            reasons: vec![],
        };

        // Even if matrix says ACCUMULATE, Exit TRIM wins
        let intent = IntentSynthesizer::synthesize(AssetAction::ACCUMULATE, &exit_trim, true);
        assert_eq!(intent, PositionIntent::TRIM);

        // If market not ready, ACCUMULATE becomes HOLD
        let exit_none = ExitDecision::default();
        let intent_not_ready =
            IntentSynthesizer::synthesize(AssetAction::ACCUMULATE, &exit_none, false);
        assert_eq!(intent_not_ready, PositionIntent::HOLD);

        // If all clear, matrix ACCUMULATE becomes ADD
        let intent_add = IntentSynthesizer::synthesize(AssetAction::ACCUMULATE, &exit_none, true);
        assert_eq!(intent_add, PositionIntent::ADD);
    }
}

use crate::core::asset_state::AssetState;
use crate::core::exit::{ExitDecision, PositionIntent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UnifiedPositionIntent {
    Add,
    #[default]
    Hold,
    Trim,
    Exit,
    Watch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PositionIntentSource {
    EntryGate,
    ExitGate,
    #[default]
    Synthesized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PositionIntentDecision {
    pub intent: UnifiedPositionIntent,
    #[serde(default)]
    pub reasons: Vec<String>,
    pub source: PositionIntentSource,
}

pub struct UnifiedIntentSynthesizer;

impl UnifiedIntentSynthesizer {
    pub fn synthesize(
        execution_intent: PositionIntent,
        exit_decision: &ExitDecision,
        participation_ready: bool,
        has_position: bool,
        asset_state: AssetState,
    ) -> PositionIntentDecision {
        if execution_intent == PositionIntent::EXIT {
            return PositionIntentDecision {
                intent: UnifiedPositionIntent::Exit,
                reasons: exit_decision.reasons.clone(),
                source: PositionIntentSource::ExitGate,
            };
        }

        if execution_intent == PositionIntent::TRIM {
            return PositionIntentDecision {
                intent: UnifiedPositionIntent::Trim,
                reasons: exit_decision.reasons.clone(),
                source: PositionIntentSource::ExitGate,
            };
        }

        if !participation_ready && !has_position {
            return PositionIntentDecision {
                intent: UnifiedPositionIntent::Watch,
                reasons: vec!["No Trade: watch-only asset".to_string()],
                source: PositionIntentSource::EntryGate,
            };
        }

        if !participation_ready && has_position {
            return PositionIntentDecision {
                intent: match asset_state {
                    AssetState::PULLBACK | AssetState::CAUTION | AssetState::FORMING => {
                        UnifiedPositionIntent::Watch
                    }
                    _ => UnifiedPositionIntent::Hold,
                },
                reasons: vec!["No Trade: existing position retained".to_string()],
                source: PositionIntentSource::Synthesized,
            };
        }

        match execution_intent {
            PositionIntent::ADD => PositionIntentDecision {
                intent: UnifiedPositionIntent::Add,
                reasons: vec!["Participation ready: add allowed".to_string()],
                source: PositionIntentSource::EntryGate,
            },
            PositionIntent::HOLD => PositionIntentDecision {
                intent: if has_position {
                    match asset_state {
                        AssetState::PULLBACK | AssetState::CAUTION | AssetState::FORMING => {
                            UnifiedPositionIntent::Watch
                        }
                        _ => UnifiedPositionIntent::Hold,
                    }
                } else {
                    UnifiedPositionIntent::Watch
                },
                reasons: vec!["No add/trim/exit trigger".to_string()],
                source: PositionIntentSource::Synthesized,
            },
            PositionIntent::TRIM => PositionIntentDecision {
                intent: UnifiedPositionIntent::Trim,
                reasons: exit_decision.reasons.clone(),
                source: PositionIntentSource::ExitGate,
            },
            PositionIntent::EXIT => PositionIntentDecision {
                intent: UnifiedPositionIntent::Exit,
                reasons: exit_decision.reasons.clone(),
                source: PositionIntentSource::ExitGate,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::exit::AssetExitState;

    #[test]
    fn test_unified_intent_no_trade_without_position_becomes_watch() {
        let out = UnifiedIntentSynthesizer::synthesize(
            PositionIntent::HOLD,
            &ExitDecision::default(),
            false,
            false,
            AssetState::FORMING,
        );
        assert_eq!(out.intent, UnifiedPositionIntent::Watch);
    }

    #[test]
    fn test_unified_intent_no_trade_core_position_can_hold() {
        let out = UnifiedIntentSynthesizer::synthesize(
            PositionIntent::HOLD,
            &ExitDecision::default(),
            false,
            true,
            AssetState::OPTIMAL,
        );
        assert_eq!(out.intent, UnifiedPositionIntent::Hold);
    }

    #[test]
    fn test_unified_intent_exit_gate_overrides() {
        let out = UnifiedIntentSynthesizer::synthesize(
            PositionIntent::EXIT,
            &ExitDecision {
                position_intent: PositionIntent::EXIT,
                asset_exit_state: AssetExitState::DefensiveExit,
                reasons: vec!["hard risk".to_string()],
            },
            true,
            true,
            AssetState::DEFEND,
        );
        assert_eq!(out.intent, UnifiedPositionIntent::Exit);
    }
}

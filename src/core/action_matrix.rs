use crate::core::asset_state::{AssetState, AssetStateSnapshot};
use crate::core::market_regime::MarketRegimeSnapshot;
use crate::core::portfolio_policy::PortfolioPolicy;
use serde::{Deserialize, Serialize};

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AssetAction {
    ACCUMULATE,
    HOLD,
    REDUCE,
    FREEZE,
    OBSERVE,
    AVOID,
    WAIT,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssetActionDecision {
    pub symbol: String,
    pub price: f64,
    pub state: AssetState,
    pub action: AssetAction,
    pub reasons: Vec<String>,
    pub deviation: Option<f64>,
    pub z_score: Option<f64>,
    pub trade_enabled: bool,
    pub trade_amount: f64,
    pub config_multiplier: f64,
    pub prev_action: Option<AssetAction>,
    pub action_changed: bool,
}

pub struct ActionMatrix;

impl ActionMatrix {
    // Clippy: Too many arguments (9/7), but this is a core mapping function.
    #[allow(clippy::too_many_arguments)]
    pub fn decide(
        regime: &MarketRegimeSnapshot,
        _policy: &PortfolioPolicy,

        asset_state: &AssetStateSnapshot,
        deviation: Option<f64>,
        z_score: Option<f64>,
        price: f64,
        trade_enabled: bool,
        trade_amount: f64,
        config_multiplier: f64,
    ) -> AssetActionDecision {
        // Roadmap Section 2: Action Matrix Mapping (Hardened 1:1)
        let (action, matrix_reason) = match (regime.market_state, asset_state.state) {
            // Priority 1: Overheat is universal
            (_, AssetState::OVERHEAT) => (
                AssetAction::REDUCE,
                "Matrix: Overheat absolute risk reduction",
            ),

            // Priority 2: Forming is universal
            (_, AssetState::FORMING) => (
                AssetAction::OBSERVE,
                "Matrix: Asset in formation, no action",
            ),

            // Matrix Mapping
            (crate::core::market_regime::MarketState::IGNITION, s) => match s {
                AssetState::OPTIMAL => (
                    AssetAction::ACCUMULATE,
                    "Matrix: Ignition + Optimal -> Aggressive entry",
                ),
                AssetState::CRUISE | AssetState::PULLBACK => {
                    (AssetAction::HOLD, "Matrix: Ignition + Build -> Soft hold")
                }
                AssetState::CAUTION | AssetState::DEFEND => (
                    AssetAction::AVOID,
                    "Matrix: Ignition + Risk -> Avoid weak starts",
                ),
                AssetState::FORMING => (AssetAction::OBSERVE, "Matrix: Forming"),
                _ => (AssetAction::HOLD, "Matrix: Default hold"),
            },

            (crate::core::market_regime::MarketState::NEWBORN, s) => match s {
                AssetState::OPTIMAL | AssetState::PULLBACK => (
                    AssetAction::ACCUMULATE,
                    "Matrix: Newborn + Pullback/Optimal -> Early cycle accumulation",
                ),
                AssetState::CRUISE | AssetState::CAUTION => (
                    AssetAction::HOLD,
                    "Matrix: Newborn + Neutral -> Wait and see",
                ),
                AssetState::DEFEND => (
                    AssetAction::AVOID,
                    "Matrix: Newborn + Defend -> Avoid structural weakness",
                ),
                _ => (AssetAction::HOLD, "Matrix: Default hold"),
            },

            (crate::core::market_regime::MarketState::EARLY_CONFIRMATION, s) => match s {
                AssetState::OPTIMAL | AssetState::PULLBACK => (
                    AssetAction::ACCUMULATE,
                    "Matrix: Early Confirm + Pullback/Optimal -> Confidence build",
                ),
                AssetState::CRUISE | AssetState::CAUTION => (
                    AssetAction::HOLD,
                    "Matrix: Early Confirm + Neutral -> Standard hold",
                ),
                AssetState::DEFEND => (
                    AssetAction::REDUCE,
                    "Matrix: Early Confirm + Defend -> Prudent risk reduction",
                ),
                _ => (AssetAction::HOLD, "Matrix: Default hold"),
            },

            (crate::core::market_regime::MarketState::ESTABLISHED, s) => match s {
                AssetState::OPTIMAL | AssetState::CRUISE | AssetState::CAUTION => (
                    AssetAction::HOLD,
                    "Matrix: Established + High/Neutral -> Maintain Core Position",
                ),
                AssetState::PULLBACK => (
                    AssetAction::ACCUMULATE,
                    "Matrix: Established + Pullback -> Strategic Buy",
                ),
                AssetState::DEFEND => (
                    AssetAction::REDUCE,
                    "Matrix: Established + Defend -> Mid-trend risk management",
                ),
                _ => (AssetAction::HOLD, "Matrix: Default hold"),
            },

            (crate::core::market_regime::MarketState::CONFIRMED, s) => match s {
                AssetState::OPTIMAL | AssetState::CRUISE | AssetState::PULLBACK => (
                    AssetAction::HOLD,
                    "Matrix: Confirmed + Strong -> Max exposure hold",
                ),
                AssetState::CAUTION | AssetState::DEFEND => (
                    AssetAction::REDUCE,
                    "Matrix: Confirmed + Weak -> Late cycle trimming",
                ),
                _ => (AssetAction::HOLD, "Matrix: Default hold"),
            },

            (crate::core::market_regime::MarketState::DEFENSIVE, s) => match s {
                AssetState::OPTIMAL | AssetState::CRUISE => (
                    AssetAction::FREEZE,
                    "Matrix: Defensive + Strong -> Capital preservation (Freeze)",
                ),
                AssetState::PULLBACK | AssetState::CAUTION | AssetState::DEFEND => (
                    AssetAction::AVOID,
                    "Matrix: Defensive + Weak -> Absolute avoidance",
                ),
                _ => (AssetAction::AVOID, "Matrix: Default defensive avoid"),
            },
        };

        let mut reasons = asset_state.reasons.clone();
        reasons.push(matrix_reason.to_string());

        AssetActionDecision {
            symbol: asset_state.symbol.clone(),
            price,
            state: asset_state.state,
            action,
            reasons,
            deviation,
            z_score,
            trade_enabled,
            trade_amount,
            config_multiplier,
            prev_action: None,
            action_changed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::market_regime::{LifecycleState, MarketState, RiskOverlay};

    fn mock_market(state: MarketState) -> MarketRegimeSnapshot {
        MarketRegimeSnapshot {
            market_state: state,
            lifecycle_state: LifecycleState::NONE,
            risk_overlay: RiskOverlay::NORMAL,
            reasons: vec![],
        }
    }

    fn mock_asset(state: AssetState) -> AssetStateSnapshot {
        AssetStateSnapshot {
            symbol: "AAPL".to_string(),
            state,
            reasons: vec![],
        }
    }

    #[test]
    fn test_action_matrix_defensive() {
        let regime = mock_market(MarketState::DEFENSIVE);
        let asset = mock_asset(AssetState::OPTIMAL);
        let policy = PortfolioPolicy::from_market_regime(&regime);
        let decision = ActionMatrix::decide(
            &regime, &policy, &asset, None, None, 150.0, true, 1000.0, 1.0,
        );

        assert_eq!(decision.action, AssetAction::FREEZE);
    }

    #[test]
    fn test_action_matrix_pullback() {
        let regime = mock_market(MarketState::ESTABLISHED);
        let asset = mock_asset(AssetState::PULLBACK);
        let policy = PortfolioPolicy::from_market_regime(&regime);
        let decision = ActionMatrix::decide(
            &regime, &policy, &asset, None, None, 140.0, true, 1000.0, 1.0,
        );

        assert_eq!(decision.action, AssetAction::ACCUMULATE);
    }
}

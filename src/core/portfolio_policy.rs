use crate::core::market_regime::{MarketRegimeSnapshot, MarketState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum RiskAssetsMode {
    AGGRESSIVE,
    NEUTRAL,
    DEFEND,
    HALT,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PortfolioPolicy {
    pub target_exposure_min: f64,
    pub target_exposure_max: f64,
    pub allow_chase: bool,
    pub allow_pullback_buy: bool,
    pub allow_new_risk: bool,
    pub risk_assets_mode: RiskAssetsMode,
}

impl PortfolioPolicy {
    pub fn from_market_regime(snapshot: &MarketRegimeSnapshot) -> Self {
        match snapshot.market_state {
            MarketState::IGNITION => Self {
                target_exposure_min: 0.10,
                target_exposure_max: 0.30,
                allow_chase: false,
                allow_pullback_buy: true,
                allow_new_risk: true,
                risk_assets_mode: RiskAssetsMode::NEUTRAL,
            },
            MarketState::NEWBORN => Self {
                target_exposure_min: 0.20,
                target_exposure_max: 0.40,
                allow_chase: false,
                allow_pullback_buy: true,
                allow_new_risk: true,
                risk_assets_mode: RiskAssetsMode::NEUTRAL,
            },
            MarketState::EARLY_CONFIRMATION => Self {
                target_exposure_min: 0.40,
                target_exposure_max: 0.60,
                allow_chase: false,
                allow_pullback_buy: true,
                allow_new_risk: true,
                risk_assets_mode: RiskAssetsMode::AGGRESSIVE,
            },
            MarketState::ESTABLISHED => Self {
                target_exposure_min: 0.60,
                target_exposure_max: 0.80,
                allow_chase: false,
                allow_pullback_buy: true,
                allow_new_risk: true,
                risk_assets_mode: RiskAssetsMode::AGGRESSIVE,
            },
            MarketState::CONFIRMED => Self {
                target_exposure_min: 0.50, // Slight reduction to lock profits
                target_exposure_max: 0.70,
                allow_chase: false,
                allow_pullback_buy: true,
                allow_new_risk: false,
                risk_assets_mode: RiskAssetsMode::NEUTRAL,
            },
            MarketState::DEFENSIVE => Self {
                target_exposure_min: 0.00,
                target_exposure_max: 0.20,
                allow_chase: false,
                allow_pullback_buy: false,
                allow_new_risk: false,
                risk_assets_mode: RiskAssetsMode::HALT,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::market_regime::{LifecycleState, RiskOverlay};

    fn mock_snapshot(state: MarketState) -> MarketRegimeSnapshot {
        MarketRegimeSnapshot {
            market_state: state,
            lifecycle_state: LifecycleState::NONE,
            risk_overlay: RiskOverlay::NORMAL,
            reasons: vec![],
        }
    }

    #[test]
    fn test_policy_from_defensive() {
        let snap = mock_snapshot(MarketState::DEFENSIVE);
        let policy = PortfolioPolicy::from_market_regime(&snap);
        assert_eq!(policy.target_exposure_max, 0.20);
        assert!(!policy.allow_pullback_buy);
        assert_eq!(policy.risk_assets_mode, RiskAssetsMode::HALT);
    }

    #[test]
    fn test_policy_from_established() {
        let snap = mock_snapshot(MarketState::ESTABLISHED);
        let policy = PortfolioPolicy::from_market_regime(&snap);
        assert_eq!(policy.target_exposure_min, 0.60);
        assert_eq!(policy.target_exposure_max, 0.80);
        assert!(policy.allow_pullback_buy);
        assert_eq!(policy.risk_assets_mode, RiskAssetsMode::AGGRESSIVE);
    }
}

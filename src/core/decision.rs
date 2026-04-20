use crate::core::action_matrix::AssetActionDecision;
use crate::core::features::MarketFeatures;
use crate::core::market_regime::MarketRegimeSnapshot;
use crate::core::market_state::models::MarketStateOutput;
use crate::core::participation::ParticipationReadiness;
use crate::core::portfolio_policy::PortfolioPolicy;
use crate::core::transition_log::StateTransitionLog;
use crate::core::trend_cohesion::TrendCohesionSnapshot;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DecisionPacket {
    pub date: NaiveDate,
    pub market_features: MarketFeatures,
    pub market_regime: MarketRegimeSnapshot,
    pub market_state: Option<MarketStateOutput>,
    pub portfolio_policy: PortfolioPolicy,
    pub assets: Vec<AssetActionDecision>,
    #[serde(default)]
    pub participation: ParticipationReadiness,
    pub top_tier_symbols: Vec<String>,
    #[serde(default, alias = "participation_changed")]
    pub trend_gate_changed: bool,
    #[serde(default)]
    pub trend_cohesion: TrendCohesionSnapshot,
    pub transition_log: Option<StateTransitionLog>,
}

impl Default for DecisionPacket {
    fn default() -> Self {
        Self {
            date: NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
            market_features: Default::default(),
            market_regime: Default::default(),
            market_state: None,
            portfolio_policy: Default::default(),
            assets: Vec::new(),
            participation: Default::default(),
            top_tier_symbols: Vec::new(),
            trend_gate_changed: false,
            trend_cohesion: TrendCohesionSnapshot::default(),
            transition_log: None,
        }
    }
}

impl DecisionPacket {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        date: NaiveDate,
        market_features: MarketFeatures,
        market_regime: MarketRegimeSnapshot,
        market_state: Option<MarketStateOutput>,
        portfolio_policy: PortfolioPolicy,
        assets: Vec<AssetActionDecision>,
        participation: ParticipationReadiness,
        top_tier_symbols: Vec<String>,
        trend_gate_changed: bool,
        trend_cohesion: TrendCohesionSnapshot,
        transition_log: Option<StateTransitionLog>,
    ) -> Self {
        Self {
            date,
            market_features,
            market_regime,
            market_state,
            portfolio_policy,
            assets,
            participation,
            top_tier_symbols,
            trend_gate_changed,
            trend_cohesion,
            transition_log,
        }
    }
}

use crate::core::market_regime::MarketRegimeSnapshot;
use crate::core::portfolio_policy::PortfolioPolicy;
use crate::core::action_matrix::AssetActionDecision;
use crate::core::features::MarketFeatures;
use serde::{Deserialize, Serialize};
use chrono::NaiveDate;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TelegramOutput {
    pub headline: String,
    pub summary: String,
    pub bias: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DecisionPacket {
    pub date: NaiveDate,
    pub market_features: MarketFeatures,
    pub market_regime: MarketRegimeSnapshot,
    pub portfolio_policy: PortfolioPolicy,
    pub assets: Vec<AssetActionDecision>,
    pub telegram: TelegramOutput,
}

impl DecisionPacket {
    pub fn new(
        date: NaiveDate,
        market_features: MarketFeatures,
        market_regime: MarketRegimeSnapshot,
        portfolio_policy: PortfolioPolicy,
        assets: Vec<AssetActionDecision>,
    ) -> Self {
        let telegram = Self::generate_telegram(&market_regime, &portfolio_policy);
        Self {
            date,
            market_features,
            market_regime,
            portfolio_policy,
            assets,
            telegram,
        }
    }

    fn generate_telegram(
        market: &MarketRegimeSnapshot,
        _policy: &PortfolioPolicy,
    ) -> TelegramOutput {
        use crate::core::market_regime::MarketState;

        let headline = format!("{:?} | Risk {:?}", market.market_state, market.risk_overlay);
        
        let summary = match market.market_state {
            MarketState::IGNITION => "轻仓试探，只保留最强标的",
            MarketState::NEWBORN => "只买回撤，禁止追高",
            MarketState::EARLY_CONFIRMATION => "核心持有，回撤加仓",
            MarketState::ESTABLISHED => "持有核心，优先买回撤",
            MarketState::CONFIRMED => "强势持有，浅回撤可加",
            MarketState::DEFENSIVE => "停止加仓，仅保留核心",
        };

        let bias = match market.market_state {
            MarketState::IGNITION => "Probe",
            MarketState::NEWBORN => "Buy Pullbacks",
            MarketState::EARLY_CONFIRMATION => "Build",
            MarketState::ESTABLISHED => "Stay Long",
            MarketState::CONFIRMED => "Hold Strength",
            MarketState::DEFENSIVE => "Defense First",
        };

        TelegramOutput { 
            headline, 
            summary: summary.to_string(),
            bias: bias.to_string(),
        }
    }
}

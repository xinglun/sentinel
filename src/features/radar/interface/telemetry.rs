use crate::features::radar::domain::market_regime::{MarketState, RiskOverlay};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TelemetryRow {
    pub timestamp: String,
    pub date: String,
    pub provider: String,
    pub market_state: MarketState,
    pub risk_overlay: RiskOverlay,
    pub system_confidence: f64,
    pub stability_score: f64,
    pub dominance_margin: f64,
    pub potential_energy: f64,
    pub regime_age: usize,
    pub up_count: usize,
    pub flat_count: usize,
    pub down_count: usize,
    pub total_count: usize,
    pub up_weight: f64,
    pub flat_weight: f64,
    pub down_weight: f64,
    pub total_weight: f64,
    pub config_hash: String,
    pub data_quality_status: String,
}

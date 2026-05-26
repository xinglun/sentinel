use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatedTrade {
    pub symbol: String,
    pub side: TradeSide,
    pub qty: f64,
    pub price: f64,
    pub reason: String,
    pub is_liquidation: bool,
    pub is_trim: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

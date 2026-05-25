use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PositionMismatch {
    pub symbol: String,
    pub local_qty: f64,
    pub broker_qty: f64,
    pub diff: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReconciliationReport {
    pub timestamp: String,
    pub mismatches: Vec<PositionMismatch>,
    pub matching_count: usize,
}

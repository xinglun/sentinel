use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryStatus {
    Succeeded,
    Failed { reason: String },
    Skipped,
}

#[derive(Debug, Serialize)]
pub struct PreflightResult {
    pub status: String, // "Verified", "Warning", "Failed"
    pub sub_quota_used: i32,
    pub sub_quota_total: i32,
    pub market_rights: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PositionMismatch {
    pub symbol: String,
    pub local_qty: f64,
    pub broker_qty: f64,
    pub diff: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ReconciliationReport {
    pub timestamp: String,
    pub mismatches: Vec<PositionMismatch>,
    pub matching_count: usize,
}

#[derive(Debug, Serialize)]
pub struct RunOutcome {
    pub date: String,
    pub timestamp: String,
    pub preflight: Option<PreflightResult>,
    pub decisioning: DeliveryStatus,
    pub archival: DeliveryStatus,
    pub notification: DeliveryStatus,
    pub execution: DeliveryStatus,
    pub reconciliation: DeliveryStatus,
    pub reconciliation_report: Option<ReconciliationReport>,
    pub data_quality: String,
    pub execution_details: Option<serde_json::Value>,
}

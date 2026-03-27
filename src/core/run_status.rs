use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryStatus {
    Succeeded,
    Failed {
        reason: String,
    },
    #[default]
    Skipped,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreflightResult {
    pub status: String,
    pub sub_quota_used: i32,
    pub sub_quota_total: i32,
    pub market_rights: std::collections::HashMap<String, String>,
}

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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StateMachineSummary {
    pub from_state: String,
    pub to_state: String,
    pub reset_confirmed: bool,
    pub reset_blocked: bool,
    pub soft_reset_applied: bool,
    pub duration_locked: bool,
    pub defensive_override: bool,
    pub core_breakdown: bool,
    pub reconciliation_mismatch_count: usize,
    pub preflight_failed: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
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
    pub state_machine: Option<StateMachineSummary>,
    pub data_quality: String,
    pub execution_details: Option<serde_json::Value>,
}

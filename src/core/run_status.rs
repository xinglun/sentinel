use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryStatus {
    Succeeded,
    Failed { reason: String },
    Skipped,
}

#[derive(Debug, Serialize)]
pub struct RunOutcome {
    pub date: String,
    pub timestamp: String,
    pub decisioning: DeliveryStatus,
    pub archival: DeliveryStatus,
    pub notification: DeliveryStatus,
    pub execution: DeliveryStatus,
    pub data_quality: String,
    pub execution_details: Option<serde_json::Value>,
}

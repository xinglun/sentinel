use crate::application::run_status::DeliveryStatus;
use chrono::NaiveDate;
use std::path::Path;

pub const EVIDENCE_COLLECTION_STATUS_FILE: &str = "evidence_collection_status_latest.json";

pub fn parse_evidence_collection_status(value: &serde_json::Value) -> DeliveryStatus {
    let status = value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("skipped");
    match status {
        "succeeded" => DeliveryStatus::Succeeded,
        "failed" => DeliveryStatus::Failed {
            reason: value
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("evidence collection failed")
                .to_string(),
        },
        _ => DeliveryStatus::Skipped,
    }
}

pub fn load_latest_evidence_collection_status(save_dir: &Path) -> DeliveryStatus {
    let path = save_dir.join(EVIDENCE_COLLECTION_STATUS_FILE);
    let Ok(raw) = std::fs::read_to_string(path) else {
        return DeliveryStatus::Skipped;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return DeliveryStatus::Failed {
            reason: "invalid evidence collection status JSON".to_string(),
        };
    };
    parse_evidence_collection_status(&value)
}

pub fn load_run_evidence_collection_status(
    save_dir: &Path,
    date: NaiveDate,
) -> Option<DeliveryStatus> {
    let path = save_dir.join(format!("run_status_{}.json", date));
    let raw = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    let status = value.get("evidence_collection")?;
    serde_json::from_value::<DeliveryStatus>(status.clone()).ok()
}

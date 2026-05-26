use serde::{Deserialize, Serialize};

pub use crate::features::shared::domain::reconciliation::{PositionMismatch, ReconciliationReport};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GrayRhinoProviderStatus {
    pub status: String,
    pub accepted: u64,
    pub rejected: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GrayRhinoCollectionStatus {
    pub status: String,
    pub date: Option<String>,
    pub sec: GrayRhinoProviderStatus,
    pub finnhub: GrayRhinoProviderStatus,
    pub fred: GrayRhinoProviderStatus,
    pub failed_providers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RunOutcome {
    pub date: String,
    pub timestamp: String,
    pub preflight: Option<PreflightResult>,
    pub decisioning: DeliveryStatus,
    #[serde(default)]
    pub evidence_collection: DeliveryStatus,
    #[serde(default)]
    pub gray_rhino_collection: GrayRhinoCollectionStatus,
    #[serde(default)]
    pub gray_rhino_rendering: DeliveryStatus,
    pub archival: DeliveryStatus,
    pub notification: DeliveryStatus,
    pub execution: DeliveryStatus,
    pub reconciliation: DeliveryStatus,
    pub reconciliation_report: Option<ReconciliationReport>,
    pub state_machine: Option<StateMachineSummary>,
    pub data_quality: String,
    pub execution_details: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::{DeliveryStatus, GrayRhinoCollectionStatus, GrayRhinoProviderStatus, RunOutcome};

    #[test]
    fn run_outcome_defaults_missing_evidence_collection_for_legacy_json() {
        let legacy = r#"{
            "date": "2026-05-01",
            "timestamp": "2026-05-01T23:30:00+09:00",
            "preflight": null,
            "decisioning": "succeeded",
            "archival": "succeeded",
            "notification": "skipped",
            "execution": "skipped",
            "reconciliation": "skipped",
            "reconciliation_report": null,
            "state_machine": null,
            "data_quality": "OK",
            "execution_details": null
        }"#;

        let outcome: RunOutcome = serde_json::from_str(legacy).unwrap();
        assert_eq!(outcome.evidence_collection, DeliveryStatus::Skipped);
        assert_eq!(outcome.gray_rhino_collection.status, "");
        assert_eq!(outcome.gray_rhino_rendering, DeliveryStatus::Skipped);
    }

    #[test]
    fn gray_rhino_run_status_records_sensor_status() {
        let outcome = RunOutcome {
            gray_rhino_collection: GrayRhinoCollectionStatus {
                status: "partial_failure".to_string(),
                date: Some("2026-05-25".to_string()),
                fred: GrayRhinoProviderStatus {
                    status: "failed".to_string(),
                    accepted: 0,
                    rejected: 1,
                },
                failed_providers: vec!["fred".to_string()],
                ..Default::default()
            },
            gray_rhino_rendering: DeliveryStatus::Succeeded,
            ..Default::default()
        };
        let encoded = serde_json::to_string(&outcome).unwrap();

        assert!(encoded.contains("gray_rhino_collection"));
        assert!(encoded.contains("partial_failure"));
        assert!(encoded.contains("failed_providers"));
        assert!(encoded.contains("gray_rhino_rendering"));
    }

    #[test]
    fn gray_rhino_collection_status_preserves_partial_provider_coverage() {
        let status = GrayRhinoCollectionStatus {
            status: "partial_failure".to_string(),
            date: Some("2026-05-25".to_string()),
            sec: GrayRhinoProviderStatus {
                status: "succeeded".to_string(),
                accepted: 2,
                rejected: 0,
            },
            fred: GrayRhinoProviderStatus {
                status: "failed".to_string(),
                accepted: 0,
                rejected: 1,
            },
            failed_providers: vec!["fred".to_string()],
            ..Default::default()
        };

        assert_eq!(status.status, "partial_failure");
        assert_eq!(status.sec.accepted, 2);
        assert_eq!(status.fred.rejected, 1);
        assert_eq!(status.failed_providers, vec!["fred"]);
    }
}

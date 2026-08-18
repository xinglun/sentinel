use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::price_volume_structure::PriceVolumeAssessment;
use crate::features::shared::domain::supply_event_context::SupplyEventContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviousSnapshotStatus {
    Available,
    BaselineUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingDaySnapshotWriteDisposition {
    Created,
    SameDayRerun,
}

#[derive(Debug, Clone)]
pub struct PreviousSnapshotResolution {
    pub status: PreviousSnapshotStatus,
    pub current_market_date: chrono::NaiveDate,
    pub previous_market_date: Option<chrono::NaiveDate>,
    pub previous_snapshot_id: Option<String>,
    pub gap_type: Option<String>,
    pub is_same_cycle: bool,
    pub snapshot: Option<DecisionPacket>,
    pub reason: Option<String>,
    pub formal_snapshot: Option<TradingDaySnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradingDaySnapshot {
    pub schema_version: String,
    pub market_date: chrono::NaiveDate,
    pub report_date: chrono::NaiveDate,
    pub as_of_date: chrono::NaiveDate,
    pub generated_at: String,
    pub run_id: String,
    pub cycle_id: String,
    pub snapshot_id: String,
    pub is_valid_trading_day: bool,
    pub source_status: String,
    pub market_state: String,
    pub decision_state: String,
    pub new_position_limit: f64,
    #[serde(default)]
    pub breadth: Option<f64>,
    #[serde(default)]
    pub breadth_classification: Option<String>,
    pub confidence: f64,
    pub supply_phase: String,
    pub risk_state: String,
    pub primary_leader: Option<String>,
    pub secondary_leaders: Vec<String>,
    pub breakouts: serde_json::Value,
    pub stability: f64,
    pub continuity: usize,
    pub cycle_length_days: usize,
    pub reset_event: Option<String>,
    pub data_quality: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObservationHistoryState {
    pub count: usize,
    pub last_market_date: chrono::NaiveDate,
    #[serde(default)]
    pub cycle_id: String,
}

/// 価格・出来高構造の観測を取引判断と分離して保存する record。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub(crate) struct PriceVolumeObservationRecord {
    pub market_date: chrono::NaiveDate,
    pub symbol: String,
    pub assessment: PriceVolumeAssessment,
    #[serde(default)]
    pub supply_context: Option<SupplyEventContext>,
    #[serde(default)]
    pub price_position: Option<String>,
    #[serde(default)]
    pub accumulation_failed: bool,
}

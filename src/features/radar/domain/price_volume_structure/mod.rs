#![allow(dead_code)]

use crate::features::shared::domain::market_data::DailyBar;
use crate::features::shared::domain::supply_event_context::{
    ObservationEffect, SupplyDirection, SupplyEventConfidence, SupplyEventContext,
    SupplyEventContextAvailability,
};
use chrono::NaiveDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum EligibilityStatus {
    Full,
    Partial,
    Insufficient,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum ObservationConfidence {
    High,
    Partial,
    Low,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum BaselineType {
    Standard20d,
    AvailableHistory,
    PostIpo,
    PostEvent,
    PostLockup,
    PostEarnings,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum CandidateLifecycle {
    Candidate,
    Developing,
    Confirmed,
    Unavailable,
    #[default]
    Invalidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum UnavailableReason {
    InsufficientValidHistory,
    MissingVolume,
    MissingOhlcv,
    DataGap,
    CorporateActionConflict,
    ApiFailure,
    MissingSupplyContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PriceVolumeStructure {
    Accumulation,
    AccumulationCandidate,
    HealthyAdvance,
    ExhaustedAdvance,
    Distribution,
    Neutral,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum VolumeDataQuality {
    Healthy,
    Partial,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ParticipationQuality {
    Improving,
    Healthy,
    Weakening,
    Deteriorating,
    Neutral,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SupplyAbsorption {
    Active,
    Candidate,
    None,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum StructurePersistence {
    Candidate,
    Developing,
    Confirmed,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PriceVolumeMetrics {
    pub return_1d: f64,
    pub return_5d: f64,
    pub return_10d: f64,
    pub return_20d: f64,
    pub rvol_5: f64,
    pub rvol_20: f64,
    pub average_volume_5: f64,
    pub average_volume_20: f64,
    pub up_day_average_volume: f64,
    pub down_day_average_volume: f64,
    pub distance_from_20d_high: f64,
    pub distance_from_20d_low: f64,
    pub new_high: bool,
    pub new_low: bool,
    pub atr_normalized_move: Option<f64>,
    pub body_ratio: Option<f64>,
    pub upper_wick_ratio: Option<f64>,
    pub lower_wick_ratio: Option<f64>,
    pub gap_percent: Option<f64>,
    #[serde(default)]
    pub baseline_days: usize,
    #[serde(default)]
    pub baseline_type: BaselineType,
    #[serde(default)]
    pub relative_volume: f64,
    #[serde(default)]
    pub relative_volume_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PriceVolumeObservationBoundary {
    pub decision_weight_percent: u8,
    pub trade_signal: bool,
    pub gate_effect: ObservationEffect,
    pub execution_effect: ObservationEffect,
    pub position_sizing_effect: ObservationEffect,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PriceVolumeAssessment {
    pub structure: PriceVolumeStructure,
    pub participation: ParticipationQuality,
    pub supply_absorption: SupplyAbsorption,
    pub quality: VolumeDataQuality,
    pub persistence: StructurePersistence,
    pub persistence_days: u8,
    pub metrics: Option<PriceVolumeMetrics>,
    #[serde(default)]
    pub secondary_metrics: Option<PriceVolumeMetrics>,
    #[serde(default)]
    pub observation_confidence: ObservationConfidence,
    pub boundary: PriceVolumeObservationBoundary,
    #[serde(default)]
    pub eligibility: EligibilityStatus,
    #[serde(default)]
    pub primary_baseline: BaselineType,
    #[serde(default)]
    pub secondary_baseline: Option<BaselineType>,
    #[serde(default)]
    pub lifecycle: CandidateLifecycle,
    #[serde(default)]
    pub unavailable_reason: Option<UnavailableReason>,
    #[serde(default)]
    pub next_eligibility_condition: Option<String>,
}

pub(crate) struct PriceVolumeInput<'a> {
    pub bars: &'a [DailyBar],
    pub supply_context: Option<&'a SupplyEventContext>,
    pub overheated: bool,
    pub time_cost_rising: bool,
    pub persistence_days: u8,
    pub source_rate_limited: bool,
    pub volume_comparable: bool,
    pub event_baseline: Option<(BaselineType, NaiveDate)>,
    pub secondary_supply_context: Option<&'a SupplyEventContext>,
    pub market_holidays: Option<&'a [NaiveDate]>,
}

mod baseline;
mod classification;
mod eligibility;
mod lifecycle;

#[cfg(test)]
pub(crate) use baseline::boundary_marker as baseline_boundary_marker;
pub(crate) use classification::assess_price_volume_structure;
#[cfg(test)]
pub(crate) use classification::boundary_marker as classification_boundary_marker;
#[cfg(test)]
pub(crate) use eligibility::boundary_marker as eligibility_boundary_marker;
#[cfg(test)]
pub(crate) use eligibility::observation_confidence;
#[cfg(test)]
pub(crate) use lifecycle::boundary_marker as lifecycle_boundary_marker;

#[cfg(test)]
mod tests;

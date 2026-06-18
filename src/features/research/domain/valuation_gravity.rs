use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum GravityStatus {
    DeepUndervalued,
    Undervalued,
    Fair,
    SlightlyExpensive,
    Expensive,
    VeryExpensive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ValuationConfidence {
    VeryHigh,
    High,
    Medium,
    Low,
    VeryLow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ValuationSource {
    AnalystConsensus,
    MarketMultiple,
    ManualOverride,
    Hybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ValuationSourceHealth {
    Succeeded,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ValuationDataQualityReason {
    PriceTargetConsensus,
    MarketMultipleFallback,
    RecommendationFallback,
    MissingCredential,
    EntitlementDenied,
    ProviderFailure,
    InvalidResponse,
    InsufficientEvidence,
    HistoricalSnapshotMissing,
    HistoricalSnapshotReadFailure,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ValuationGravityAssetSnapshot {
    pub symbol: String,
    pub gravity: Option<GravityStatus>,
    pub confidence: Option<ValuationConfidence>,
    pub source: Option<ValuationSource>,
    pub provider: String,
    pub as_of_date: NaiveDate,
    pub source_health: ValuationSourceHealth,
    pub quality_reason: ValuationDataQualityReason,
    pub evidence_count: usize,
    pub relative_ratio: Option<f64>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ValuationGravitySnapshot {
    pub as_of_date: NaiveDate,
    pub assets: Vec<ValuationGravityAssetSnapshot>,
    pub observation_only: bool,
}

impl ValuationGravitySnapshot {
    pub(crate) fn validate_for_replay(
        &self,
        requested_date: NaiveDate,
    ) -> Result<(), &'static str> {
        if self.as_of_date != requested_date {
            return Err("snapshot date does not match requested date");
        }
        if !self.observation_only {
            return Err("snapshot is not observation-only");
        }
        for asset in &self.assets {
            if asset.as_of_date > self.as_of_date {
                return Err("asset date is later than snapshot date");
            }
            if asset.symbol.trim().is_empty() || asset.provider.trim().is_empty() {
                return Err("asset identity is incomplete");
            }
            if asset
                .relative_ratio
                .is_some_and(|ratio| !ratio.is_finite() || ratio <= 0.0)
            {
                return Err("asset relative ratio is invalid");
            }

            let classification_count = [
                asset.gravity.is_some(),
                asset.confidence.is_some(),
                asset.source.is_some(),
            ]
            .into_iter()
            .filter(|present| *present)
            .count();
            if classification_count != 0 && classification_count != 3 {
                return Err("asset classification is incomplete");
            }

            match asset.source {
                Some(ValuationSource::AnalystConsensus)
                    if matches!(
                        asset.quality_reason,
                        ValuationDataQualityReason::PriceTargetConsensus
                            | ValuationDataQualityReason::RecommendationFallback
                    ) && asset.evidence_count > 0
                        && matches!(
                            asset.source_health,
                            ValuationSourceHealth::Succeeded | ValuationSourceHealth::Partial
                        ) => {}
                Some(ValuationSource::MarketMultiple)
                    if asset.quality_reason
                        == ValuationDataQualityReason::MarketMultipleFallback
                        && asset.evidence_count > 0
                        && asset.source_health == ValuationSourceHealth::Partial => {}
                Some(_) => return Err("asset source metadata is inconsistent"),
                None if asset.source_health == ValuationSourceHealth::Unavailable
                    && asset.evidence_count == 0
                    && asset.relative_ratio.is_none()
                    && matches!(
                        asset.quality_reason,
                        ValuationDataQualityReason::MissingCredential
                            | ValuationDataQualityReason::EntitlementDenied
                            | ValuationDataQualityReason::ProviderFailure
                            | ValuationDataQualityReason::InvalidResponse
                            | ValuationDataQualityReason::InsufficientEvidence
                            | ValuationDataQualityReason::HistoricalSnapshotMissing
                            | ValuationDataQualityReason::HistoricalSnapshotReadFailure
                    ) => {}
                None => return Err("unavailable asset metadata is inconsistent"),
            }
        }
        Ok(())
    }
}

pub(crate) fn classify_relative_ratio(ratio: f64) -> Option<GravityStatus> {
    if !ratio.is_finite() || ratio <= 0.0 {
        return None;
    }
    Some(if ratio <= 0.70 {
        GravityStatus::DeepUndervalued
    } else if ratio <= 0.90 {
        GravityStatus::Undervalued
    } else if ratio < 1.10 {
        GravityStatus::Fair
    } else if ratio < 1.25 {
        GravityStatus::SlightlyExpensive
    } else if ratio < 1.50 {
        GravityStatus::Expensive
    } else {
        GravityStatus::VeryExpensive
    })
}

pub(crate) fn classify_recommendation_score(score: f64) -> Option<GravityStatus> {
    if !score.is_finite() || !(-2.0..=2.0).contains(&score) {
        return None;
    }
    Some(if score >= 1.25 {
        GravityStatus::DeepUndervalued
    } else if score >= 0.40 {
        GravityStatus::Undervalued
    } else if score > -0.20 {
        GravityStatus::Fair
    } else if score > -0.65 {
        GravityStatus::SlightlyExpensive
    } else if score > -1.25 {
        GravityStatus::Expensive
    } else {
        GravityStatus::VeryExpensive
    })
}

pub(crate) fn price_target_confidence(
    analyst_count: usize,
    age_days: i64,
    dispersion_ratio: f64,
) -> ValuationConfidence {
    let analyst_points = if analyst_count >= 30 {
        2
    } else if analyst_count >= 10 {
        1
    } else {
        0
    };
    let freshness_points = if age_days <= 30 {
        2
    } else if age_days <= 90 {
        1
    } else {
        0
    };
    let dispersion_points = if dispersion_ratio <= 0.25 {
        2
    } else if dispersion_ratio <= 0.60 {
        1
    } else {
        0
    };
    match analyst_points + freshness_points + dispersion_points {
        6 => ValuationConfidence::VeryHigh,
        5 => ValuationConfidence::High,
        3 | 4 => ValuationConfidence::Medium,
        2 => ValuationConfidence::Low,
        _ => ValuationConfidence::VeryLow,
    }
}

pub(crate) fn market_multiple_confidence(sample_count: usize) -> ValuationConfidence {
    if sample_count >= 5 {
        ValuationConfidence::Low
    } else {
        ValuationConfidence::VeryLow
    }
}

pub(crate) fn recommendation_confidence(
    recommendation_count: usize,
    age_days: i64,
) -> ValuationConfidence {
    if recommendation_count >= 20 && age_days <= 45 {
        ValuationConfidence::Low
    } else {
        ValuationConfidence::VeryLow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_ratio_maps_to_all_six_gravity_bands_without_unknown() {
        assert_eq!(
            classify_relative_ratio(0.70),
            Some(GravityStatus::DeepUndervalued)
        );
        assert_eq!(
            classify_relative_ratio(0.90),
            Some(GravityStatus::Undervalued)
        );
        assert_eq!(classify_relative_ratio(1.00), Some(GravityStatus::Fair));
        assert_eq!(
            classify_relative_ratio(1.10),
            Some(GravityStatus::SlightlyExpensive)
        );
        assert_eq!(
            classify_relative_ratio(1.25),
            Some(GravityStatus::Expensive)
        );
        assert_eq!(
            classify_relative_ratio(1.50),
            Some(GravityStatus::VeryExpensive)
        );
        assert_eq!(classify_relative_ratio(f64::NAN), None);
    }

    #[test]
    fn confidence_reflects_coverage_freshness_and_dispersion() {
        assert_eq!(
            price_target_confidence(35, 10, 0.20),
            ValuationConfidence::VeryHigh
        );
        assert_eq!(
            price_target_confidence(12, 60, 0.40),
            ValuationConfidence::Medium
        );
        assert_eq!(market_multiple_confidence(5), ValuationConfidence::Low);
        assert_eq!(
            recommendation_confidence(8, 60),
            ValuationConfidence::VeryLow
        );
    }
}

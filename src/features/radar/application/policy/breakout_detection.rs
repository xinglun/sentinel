use crate::config::ParsedBreakoutRules;
use crate::features::radar::application::policy::asset_state::{AssetState, AssetStateSnapshot};
use crate::features::radar::application::policy::features::{AssetFeatures, TrendStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum BreakoutStatus {
    #[default]
    NoBreakout,
    EmergingBreakout,
    ConfirmedBreakout,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum BreakoutReason {
    OrdinaryRebound,
    PullbackRepair,
    StructuralBreakout,
    FailedBreakoutRisk,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(default)]
pub struct BreakoutSnapshot {
    pub status: BreakoutStatus,
    pub breakout_eligible: bool,
    pub breakout_strength: f64,
    pub breakout_age: usize,
    pub breakout_quality: f64,
    pub failed_breakout_risk: f64,
    pub reasons: Vec<BreakoutReason>,
}

pub struct BreakoutEvaluator;

impl BreakoutEvaluator {
    pub fn evaluate(
        features: &AssetFeatures,
        asset_state: &AssetStateSnapshot,
        state_streak: usize,
        top_tier_streak: usize,
        previous_state: Option<AssetState>,
        previous_breakout: Option<&BreakoutSnapshot>,
        cfg: &ParsedBreakoutRules,
    ) -> BreakoutSnapshot {
        let z = features.z_score.unwrap_or(0.0);
        let slope = features.slope.unwrap_or(0.0);
        let curvature = features.curvature.unwrap_or(0.0);
        let deviation = features.deviation.unwrap_or(0.0);
        let breakout_eligible = matches!(features.trend_status, TrendStatus::Up)
            || matches!(
                asset_state.state,
                AssetState::PULLBACK
                    | AssetState::FORMING
                    | AssetState::CRUISE
                    | AssetState::OPTIMAL
            )
            || matches!(
                previous_state,
                Some(AssetState::OPTIMAL | AssetState::CRUISE)
            );

        let strength = ((z.max(0.0) * 18.0) + (slope.max(0.0) * 6.0) + (deviation.max(0.0) * 2.0))
            .clamp(0.0, 100.0);
        let quality = ((features.trend_age as f64 * 6.0)
            + (top_tier_streak as f64 * 12.0)
            + (state_streak as f64 * 6.0)
            + if matches!(features.trend_status, TrendStatus::Up) {
                20.0
            } else {
                0.0
            }
            + if curvature >= 0.0 { 10.0 } else { 0.0 })
        .clamp(0.0, 100.0);

        let failed_breakout_risk =
            if matches!(
                previous_state,
                Some(AssetState::OPTIMAL | AssetState::CRUISE)
            ) && matches!(asset_state.state, AssetState::CAUTION | AssetState::DEFEND)
            {
                75.0
            } else if matches!(
                asset_state.state,
                AssetState::OVERHEAT | AssetState::CAUTION
            ) || curvature < cfg.failed_breakout_curvature_threshold
                || slope < cfg.failed_breakout_slope_threshold
            {
                55.0
            } else {
                10.0
            };

        let mut reasons = Vec::new();
        let status = if matches!(features.trend_status, TrendStatus::Up)
            && matches!(asset_state.state, AssetState::OPTIMAL)
            && features.trend_age >= cfg.confirmed_trend_age_threshold
            && top_tier_streak >= cfg.confirmed_top_tier_streak_threshold
            && z >= cfg.confirmed_zscore_threshold
            && slope > cfg.confirmed_min_slope
            && curvature >= cfg.confirmed_min_curvature
        {
            reasons.push(BreakoutReason::StructuralBreakout);
            BreakoutStatus::ConfirmedBreakout
        } else if matches!(features.trend_status, TrendStatus::Up)
            && matches!(asset_state.state, AssetState::CRUISE | AssetState::OPTIMAL)
            && features.trend_age >= cfg.emerging_trend_age_threshold
            && top_tier_streak >= cfg.emerging_top_tier_streak_threshold
            && slope > cfg.emerging_min_slope
            && z >= cfg.emerging_zscore_threshold
        {
            reasons.push(BreakoutReason::StructuralBreakout);
            BreakoutStatus::EmergingBreakout
        } else {
            if matches!(asset_state.state, AssetState::PULLBACK) {
                reasons.push(BreakoutReason::PullbackRepair);
            } else {
                reasons.push(BreakoutReason::OrdinaryRebound);
            }
            BreakoutStatus::NoBreakout
        };

        if failed_breakout_risk >= cfg.failed_breakout_display_threshold {
            reasons.push(BreakoutReason::FailedBreakoutRisk);
        }

        let breakout_age = if matches!(status, BreakoutStatus::NoBreakout) {
            0
        } else if previous_breakout
            .map(|b| {
                matches!(
                    b.status,
                    BreakoutStatus::EmergingBreakout | BreakoutStatus::ConfirmedBreakout
                )
            })
            .unwrap_or(false)
        {
            previous_breakout
                .map(|b| b.breakout_age.max(1) + 1)
                .unwrap_or(1)
        } else {
            1
        };

        BreakoutSnapshot {
            status,
            breakout_eligible,
            breakout_strength: strength,
            breakout_age,
            breakout_quality: quality,
            failed_breakout_risk,
            reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> ParsedBreakoutRules {
        ParsedBreakoutRules::default()
    }

    fn mock_features() -> AssetFeatures {
        AssetFeatures {
            symbol: "AAPL".to_string(),
            trend_status: TrendStatus::Up,
            trend_age: 8,
            slope: Some(1.8),
            curvature: Some(0.4),
            z_score: Some(1.6),
            deviation: Some(3.2),
            ..Default::default()
        }
    }

    fn mock_state(state: AssetState) -> AssetStateSnapshot {
        AssetStateSnapshot {
            symbol: "AAPL".to_string(),
            state,
            ..Default::default()
        }
    }

    #[test]
    fn test_confirmed_breakout() {
        let snapshot = BreakoutEvaluator::evaluate(
            &mock_features(),
            &mock_state(AssetState::OPTIMAL),
            3,
            3,
            Some(AssetState::CRUISE),
            None,
            &rules(),
        );
        assert_eq!(snapshot.status, BreakoutStatus::ConfirmedBreakout);
        assert_eq!(snapshot.breakout_age, 1);
        assert!(snapshot
            .reasons
            .contains(&BreakoutReason::StructuralBreakout));
    }

    #[test]
    fn test_emerging_breakout() {
        let mut features = mock_features();
        features.trend_age = 5;
        features.z_score = Some(0.8);
        let snapshot = BreakoutEvaluator::evaluate(
            &features,
            &mock_state(AssetState::CRUISE),
            2,
            1,
            Some(AssetState::FORMING),
            None,
            &rules(),
        );
        assert_eq!(snapshot.status, BreakoutStatus::EmergingBreakout);
        assert_eq!(snapshot.breakout_age, 1);
    }

    #[test]
    fn test_pullback_repair_not_breakout() {
        let mut features = mock_features();
        features.z_score = Some(-0.2);
        let snapshot = BreakoutEvaluator::evaluate(
            &features,
            &mock_state(AssetState::PULLBACK),
            1,
            0,
            Some(AssetState::CRUISE),
            None,
            &rules(),
        );
        assert_eq!(snapshot.status, BreakoutStatus::NoBreakout);
        assert_eq!(snapshot.breakout_age, 0);
        assert!(snapshot.reasons.contains(&BreakoutReason::PullbackRepair));
    }

    #[test]
    fn test_ordinary_rebound_not_breakout() {
        let mut features = mock_features();
        features.trend_status = TrendStatus::Flat;
        features.slope = Some(0.1);
        features.z_score = Some(0.2);
        let snapshot = BreakoutEvaluator::evaluate(
            &features,
            &mock_state(AssetState::FORMING),
            1,
            0,
            None,
            None,
            &rules(),
        );
        assert_eq!(snapshot.status, BreakoutStatus::NoBreakout);
        assert_eq!(snapshot.breakout_age, 0);
        assert!(snapshot.reasons.contains(&BreakoutReason::OrdinaryRebound));
    }

    #[test]
    fn test_failed_breakout_risk() {
        let mut features = mock_features();
        features.curvature = Some(-1.0);
        features.slope = Some(-0.4);
        let snapshot = BreakoutEvaluator::evaluate(
            &features,
            &mock_state(AssetState::DEFEND),
            1,
            0,
            Some(AssetState::OPTIMAL),
            None,
            &rules(),
        );
        assert!(snapshot.failed_breakout_risk >= 55.0);
        assert!(snapshot
            .reasons
            .contains(&BreakoutReason::FailedBreakoutRisk));
    }

    #[test]
    fn test_breakout_eligibility_true_for_uptrend_or_breakdown_context() {
        let uptrend_snapshot = BreakoutEvaluator::evaluate(
            &mock_features(),
            &mock_state(AssetState::FORMING),
            1,
            0,
            None,
            None,
            &rules(),
        );
        assert!(uptrend_snapshot.breakout_eligible);

        let mut flat_features = mock_features();
        flat_features.trend_status = TrendStatus::Flat;
        let breakdown_snapshot = BreakoutEvaluator::evaluate(
            &flat_features,
            &mock_state(AssetState::DEFEND),
            1,
            0,
            Some(AssetState::CRUISE),
            None,
            &rules(),
        );
        assert!(breakdown_snapshot.breakout_eligible);
    }

    #[test]
    fn test_breakout_eligibility_false_for_non_breakout_context() {
        let mut flat_features = mock_features();
        flat_features.trend_status = TrendStatus::Flat;
        flat_features.slope = Some(-0.2);
        flat_features.curvature = Some(-0.4);
        let snapshot = BreakoutEvaluator::evaluate(
            &flat_features,
            &mock_state(AssetState::CAUTION),
            1,
            0,
            None,
            None,
            &rules(),
        );
        assert!(!snapshot.breakout_eligible);
    }

    #[test]
    fn test_breakout_age_increments_when_breakout_persists() {
        let previous_breakout = BreakoutSnapshot {
            status: BreakoutStatus::EmergingBreakout,
            breakout_age: 5,
            ..Default::default()
        };
        let snapshot = BreakoutEvaluator::evaluate(
            &mock_features(),
            &mock_state(AssetState::OPTIMAL),
            3,
            3,
            Some(AssetState::CRUISE),
            Some(&previous_breakout),
            &rules(),
        );
        assert!(matches!(
            snapshot.status,
            BreakoutStatus::EmergingBreakout | BreakoutStatus::ConfirmedBreakout
        ));
        assert_eq!(snapshot.breakout_age, 6);
    }

    #[test]
    fn test_breakout_age_resets_to_one_on_new_breakout_episode() {
        let previous_breakout = BreakoutSnapshot {
            status: BreakoutStatus::NoBreakout,
            breakout_age: 17,
            ..Default::default()
        };
        let snapshot = BreakoutEvaluator::evaluate(
            &mock_features(),
            &mock_state(AssetState::CRUISE),
            3,
            2,
            Some(AssetState::FORMING),
            Some(&previous_breakout),
            &rules(),
        );
        assert!(matches!(
            snapshot.status,
            BreakoutStatus::EmergingBreakout | BreakoutStatus::ConfirmedBreakout
        ));
        assert_eq!(snapshot.breakout_age, 1);
    }
}

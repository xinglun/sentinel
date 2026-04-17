use crate::core::decision::DecisionPacket;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ParticipationReasonCode {
    StabilityBelowThreshold,
    CoreTierStreakBelowThreshold,
    CoreTierSetChanged,
    FirstDayOfSession,
    #[default]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct ParticipationReadiness {
    pub participation_ready: bool,
    pub stability_ready: bool,
    pub core_tier_streak_ready: bool,
    pub core_tier_streak: usize,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub reason_codes: Vec<ParticipationReasonCode>,
}

impl ParticipationReadiness {
    pub fn compute(
        stability_score: f64,
        current_top_tier: &[String],
        history: &[DecisionPacket],
        stability_threshold: f64,
        continuity_threshold: usize,
    ) -> Self {
        let mut reasons = Vec::new();
        let mut reason_codes = Vec::new();

        // 1. Stability Check (Threshold configurable)
        let stability_ready = stability_score >= stability_threshold;
        if !stability_ready {
            reasons.push(format!(
                "Stability score ({:.1}) below threshold ({})",
                stability_score, stability_threshold
            ));
            reason_codes.push(ParticipationReasonCode::StabilityBelowThreshold);
        }

        // 2. Core Tier Streak Calculation
        let streak = if let Some(prev) = history.last() {
            if !prev.top_tier_symbols.is_empty() && prev.top_tier_symbols == current_top_tier {
                prev.participation.core_tier_streak + 1
            } else {
                if !prev.top_tier_symbols.is_empty() {
                    reasons.push("Core Tier set changed (streak reset)".to_string());
                    reason_codes.push(ParticipationReasonCode::CoreTierSetChanged);
                }
                1
            }
        } else {
            reasons.push("First day of session (no history)".to_string());
            reason_codes.push(ParticipationReasonCode::FirstDayOfSession);
            1
        };

        let core_tier_streak_ready = streak >= continuity_threshold;
        if !core_tier_streak_ready {
            reasons.push(format!(
                "Core Tier streak ({}) below threshold ({})",
                streak, continuity_threshold
            ));
            reason_codes.push(ParticipationReasonCode::CoreTierStreakBelowThreshold);
        }

        let participation_ready = stability_ready && core_tier_streak_ready;

        Self {
            participation_ready,
            stability_ready,
            core_tier_streak_ready,
            core_tier_streak: streak,
            reasons,
            reason_codes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::features::MarketFeatures;
    use crate::core::market_regime::MarketRegimeSnapshot;
    use crate::core::portfolio_policy::PortfolioPolicy;
    use chrono::NaiveDate;

    fn mock_packet(streak: usize, top_tier: Vec<String>) -> DecisionPacket {
        let mut packet = DecisionPacket::new(
            NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            MarketFeatures::default(),
            MarketRegimeSnapshot::default(),
            PortfolioPolicy::default(),
            Vec::new(),
            ParticipationReadiness {
                core_tier_streak: streak,
                ..Default::default()
            },
            top_tier,
            false,
            crate::core::trend_cohesion::TrendCohesionSnapshot::default(),
            None,
        );
        packet.participation.core_tier_streak = streak;
        packet
    }

    #[test]
    fn test_readiness_thresholds() {
        let top_tier = vec!["AAPL".to_string(), "MSFT".to_string(), "NVDA".to_string()];

        // Case 1: Low stability, Low streak
        let res = ParticipationReadiness::compute(5.0, &top_tier, &[], 10.0, 3);
        assert!(!res.participation_ready);
        assert!(!res.stability_ready);
        assert!(!res.core_tier_streak_ready);
        assert_eq!(res.core_tier_streak, 1);

        // Case 2: High stability, Low streak
        let res = ParticipationReadiness::compute(15.0, &top_tier, &[], 10.0, 3);
        assert!(!res.participation_ready);
        assert!(res.stability_ready);
        assert!(!res.core_tier_streak_ready);
        assert_eq!(res.core_tier_streak, 1);

        // Case 3: High stability, High streak (mocked history)
        let history = vec![
            mock_packet(1, top_tier.clone()),
            mock_packet(2, top_tier.clone()),
        ];
        let res = ParticipationReadiness::compute(15.0, &top_tier, &history, 10.0, 3);
        assert!(res.participation_ready);
        assert!(res.stability_ready);
        assert!(res.core_tier_streak_ready);
        assert_eq!(res.core_tier_streak, 3);
    }

    #[test]
    fn test_streak_reset() {
        let tier_a = vec!["AAPL".to_string(), "MSFT".to_string()];
        let tier_b = vec!["AAPL".to_string(), "GOOG".to_string()];

        let history = vec![
            mock_packet(1, tier_a.clone()),
            mock_packet(2, tier_a.clone()),
        ];

        // Same tier -> streak 3
        let res = ParticipationReadiness::compute(15.0, &tier_a, &history, 10.0, 3);
        assert_eq!(res.core_tier_streak, 3);

        // Different tier -> streak reset to 1
        let res = ParticipationReadiness::compute(15.0, &tier_b, &history, 10.0, 3);
        assert_eq!(res.core_tier_streak, 1);
        assert!(res.reasons.iter().any(|r| r.contains("changed")));
    }

    #[test]
    fn test_custom_thresholds_apply_to_decision_and_reasons() {
        let top_tier = vec!["AAPL".to_string(), "MSFT".to_string(), "NVDA".to_string()];

        // Both thresholds fail under 11.0 / 4.
        let res = ParticipationReadiness::compute(10.5, &top_tier, &[], 11.0, 4);
        assert!(!res.participation_ready);
        assert!(!res.stability_ready);
        assert!(!res.core_tier_streak_ready);
        assert!(res.reasons.iter().any(|r| r.contains("threshold (11")));
        assert!(res.reasons.iter().any(|r| r.contains("threshold (4")));
        assert!(res
            .reason_codes
            .contains(&ParticipationReasonCode::StabilityBelowThreshold));
        assert!(res
            .reason_codes
            .contains(&ParticipationReasonCode::CoreTierStreakBelowThreshold));

        // Build to streak 4 and pass both thresholds.
        let history = vec![mock_packet(3, top_tier.clone())];
        let res = ParticipationReadiness::compute(11.2, &top_tier, &history, 11.0, 4);
        assert!(res.participation_ready);
        assert!(res.stability_ready);
        assert!(res.core_tier_streak_ready);
        assert_eq!(res.core_tier_streak, 4);
    }
}

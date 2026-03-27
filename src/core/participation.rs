use crate::core::decision::DecisionPacket;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct ParticipationReadiness {
    pub participation_ready: bool,
    pub stability_ready: bool,
    pub core_tier_streak_ready: bool,
    pub core_tier_streak: usize,
    pub reasons: Vec<String>,
}

impl ParticipationReadiness {
    pub fn compute(
        stability_score: f64,
        current_top_tier: &[String],
        history: &[DecisionPacket],
    ) -> Self {
        let mut reasons = Vec::new();

        // 1. Stability Check (Threshold >= 10.0)
        let stability_ready = stability_score >= 10.0;
        if !stability_ready {
            reasons.push(format!(
                "Stability score ({:.1}) below threshold (10.0)",
                stability_score
            ));
        }

        // 2. Core Tier Streak Calculation
        let streak = if let Some(prev) = history.last() {
            if !prev.top_tier_symbols.is_empty() && prev.top_tier_symbols == current_top_tier {
                prev.participation.core_tier_streak + 1
            } else {
                if !prev.top_tier_symbols.is_empty() {
                    reasons.push("Core Tier set changed (streak reset)".to_string());
                }
                1
            }
        } else {
            reasons.push("First day of session (no history)".to_string());
            1
        };

        let core_tier_streak_ready = streak >= 3;
        if !core_tier_streak_ready {
            reasons.push(format!("Core Tier streak ({}) below threshold (3)", streak));
        }

        let participation_ready = stability_ready && core_tier_streak_ready;

        Self {
            participation_ready,
            stability_ready,
            core_tier_streak_ready,
            core_tier_streak: streak,
            reasons,
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
        );
        packet.participation.core_tier_streak = streak;
        packet
    }

    #[test]
    fn test_readiness_thresholds() {
        let top_tier = vec!["AAPL".to_string(), "MSFT".to_string(), "NVDA".to_string()];

        // Case 1: Low stability, Low streak
        let res = ParticipationReadiness::compute(5.0, &top_tier, &[]);
        assert!(!res.participation_ready);
        assert!(!res.stability_ready);
        assert!(!res.core_tier_streak_ready);
        assert_eq!(res.core_tier_streak, 1);

        // Case 2: High stability, Low streak
        let res = ParticipationReadiness::compute(15.0, &top_tier, &[]);
        assert!(!res.participation_ready);
        assert!(res.stability_ready);
        assert!(!res.core_tier_streak_ready);
        assert_eq!(res.core_tier_streak, 1);

        // Case 3: High stability, High streak (mocked history)
        let history = vec![
            mock_packet(1, top_tier.clone()),
            mock_packet(2, top_tier.clone()),
        ];
        let res = ParticipationReadiness::compute(15.0, &top_tier, &history);
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
        let res = ParticipationReadiness::compute(15.0, &tier_a, &history);
        assert_eq!(res.core_tier_streak, 3);

        // Different tier -> streak reset to 1
        let res = ParticipationReadiness::compute(15.0, &tier_b, &history);
        assert_eq!(res.core_tier_streak, 1);
        assert!(res.reasons.iter().any(|r| r.contains("changed")));
    }
}

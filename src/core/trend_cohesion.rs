use crate::core::decision::DecisionPacket;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrendCohesionStatus {
    #[default]
    NotFormed,
    Forming,
    Cohesive,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrendCohesionCondition {
    NoCandidates,
    LowStability(f64),
    LowStreak(usize),
    HighDispersion(usize),
    HighChurn,
    NoRepeatedLeaders,
    CompactAndStable,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct TrendCohesionSnapshot {
    pub status: TrendCohesionStatus,
    pub candidate_count: usize,
    pub leader_count: usize,
    pub leader_concentration_score: f64,
    pub continuity_quality_score: f64,
    pub dispersion_score: f64,
    pub reasons: Vec<TrendCohesionCondition>,
}

pub struct TrendCohesionEvaluator;

impl TrendCohesionEvaluator {
    pub fn evaluate(
        participation_ready: bool,
        stability_score: f64,
        continuity_streak: usize,
        current_top_tier: &[String],
        history: &[DecisionPacket],
    ) -> TrendCohesionSnapshot {
        let candidate_count = current_top_tier.len();
        let mut reasons = Vec::new();

        // 1. Gather recent history window (up to 2 previous days + current = 3 days).
        let recent_packets: Vec<&DecisionPacket> = history.iter().rev().take(2).collect();
        let past_top_tiers: Vec<&[String]> = recent_packets
            .iter()
            .map(|p| p.top_tier_symbols.as_slice())
            .collect();

        // 2. Identify repeating leaders (in current top tier AND in at least 1 of the prior 2 days)
        let mut leader_count = 0;
        let mut past_symbols: HashSet<&str> = HashSet::new();
        for tier in &past_top_tiers {
            for sym in *tier {
                past_symbols.insert(sym);
            }
        }

        for sym in current_top_tier {
            if past_symbols.contains(sym.as_str()) {
                leader_count += 1;
            }
        }

        // Leader concentration
        let leader_concentration_score = if candidate_count > 0 {
            leader_count as f64 / candidate_count as f64
        } else {
            0.0
        };

        // Continuity Quality (Jaccard similarity with yesterday, if yesterday exists)
        let mut continuity_quality_score = 0.0;
        if let Some(yesterday_tier) = past_top_tiers.first() { // past_top_tiers[0] is the most recent past since we `rev`
            let current_set: HashSet<&str> = current_top_tier.iter().map(|s| s.as_str()).collect();
            let prev_set: HashSet<&str> = yesterday_tier.iter().map(|s| s.as_str()).collect();
            
            let intersection = current_set.intersection(&prev_set).count();
            let union = current_set.union(&prev_set).count();

            if union > 0 {
                continuity_quality_score = (intersection as f64 / union as f64) * 100.0;
            }
        } else if candidate_count > 0 {
            // First day with candidates, technically no churn history, default to 100% or 0%?
            // Conservative: Treat as 0 previous overlap unless candidate count == 0.
            continuity_quality_score = 0.0; 
        } else {
            continuity_quality_score = 0.0;
        }

        // Dispersion score: Simple metric, >= 6 is highly dispersed.
        let dispersion_score = candidate_count as f64;

        // V2 Heuristics
        let mut status = TrendCohesionStatus::Forming;

        let stability_low = stability_score < 10.0;
        let streak_low = continuity_streak < 2;
        let no_candidates = candidate_count == 0;
        let highly_dispersed = candidate_count >= 6;
        let high_churn = continuity_quality_score < 33.0 && !past_top_tiers.is_empty(); // Need history to judge churn reliably
        let no_repeated_leaders = leader_count == 0 && !past_top_tiers.is_empty();

        if no_candidates {
            reasons.push(TrendCohesionCondition::NoCandidates);
        }
        if stability_low {
            reasons.push(TrendCohesionCondition::LowStability(stability_score));
        }
        if streak_low {
            reasons.push(TrendCohesionCondition::LowStreak(continuity_streak));
        }
        if highly_dispersed {
            reasons.push(TrendCohesionCondition::HighDispersion(candidate_count));
        }
        if high_churn {
            reasons.push(TrendCohesionCondition::HighChurn);
        }
        if no_repeated_leaders && candidate_count > 0 {
            reasons.push(TrendCohesionCondition::NoRepeatedLeaders);
        }

        if stability_low || streak_low || no_candidates || highly_dispersed || high_churn || no_repeated_leaders {
            status = TrendCohesionStatus::NotFormed;
        } else if participation_ready 
            && stability_score >= 10.0 
            && continuity_streak >= 3 
            && candidate_count <= 4 
            && leader_count >= 2 
            && continuity_quality_score >= 33.0 
        {
            status = TrendCohesionStatus::Cohesive;
            reasons.push(TrendCohesionCondition::CompactAndStable);
        }

        TrendCohesionSnapshot {
            status,
            candidate_count,
            leader_count,
            leader_concentration_score,
            continuity_quality_score,
            dispersion_score,
            reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn mock_packet(symbols: Vec<&str>) -> DecisionPacket {
        DecisionPacket {
            top_tier_symbols: symbols.into_iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn test_history_leader_repetition() {
        let history = vec![
            mock_packet(vec!["A", "B", "C"]),
            mock_packet(vec!["A", "B", "D"]),
        ];
        let current = vec!["A".to_string(), "B".to_string(), "E".to_string()];

        let snapshot = TrendCohesionEvaluator::evaluate(true, 15.0, 3, &current, &history);
        
        assert_eq!(snapshot.candidate_count, 3);
        assert_eq!(snapshot.leader_count, 2); // A and B repeat
        assert_eq!(snapshot.status, TrendCohesionStatus::Cohesive);
    }

    #[test]
    fn test_churn_degrades_cohesion() {
        let history = vec![
            mock_packet(vec!["X", "Y"]), // Yesterday
        ];
        let current = vec!["A".to_string(), "B".to_string(), "C".to_string()]; // Today (total change)

        let snapshot = TrendCohesionEvaluator::evaluate(true, 15.0, 3, &current, &history);
        
        assert_eq!(snapshot.continuity_quality_score, 0.0);
        assert_eq!(snapshot.status, TrendCohesionStatus::NotFormed); // Highly dispersed implicitly via no repeat leaders + high churn
        assert!(snapshot.reasons.contains(&TrendCohesionCondition::HighChurn));
    }

    #[test]
    fn test_not_formed_heuristics() {
        let current = vec!["A".to_string(), "B".to_string()];
        
        let snap_low_stab = TrendCohesionEvaluator::evaluate(true, 8.0, 3, &current, &[]);
        assert_eq!(snap_low_stab.status, TrendCohesionStatus::NotFormed);

        let snap_dispersed = TrendCohesionEvaluator::evaluate(true, 15.0, 3, &["A".to_string(), "B".to_string(), "C".to_string(), "D".to_string(), "E".to_string(), "F".to_string()], &[]);
        assert_eq!(snap_dispersed.status, TrendCohesionStatus::NotFormed);
    }
}

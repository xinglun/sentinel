use crate::config::ParsedTrendCohesionRules;
use crate::core::decision::DecisionPacket;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrendCohesionStatus {
    #[default]
    NotFormed,
    Forming,
    Cohesive,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrendCohesionTopology {
    #[default]
    NoLeader,
    SingleLeader,
    FragmentedLeaders,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrendCohesionGateCondition {
    StabilityThreshold,
    ContinuityThreshold,
    DirectionalCohesion,
    HighCandidateDispersion,
    UnstableRotation,
    WeakLeadership,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(default)]
pub struct TrendCohesionSnapshot {
    pub status: TrendCohesionStatus,
    pub topology: TrendCohesionTopology,
    pub gate_passed: bool,
    pub stability_score: f64,
    pub continuity_streak: usize,
    pub candidate_count: usize,
    pub leader_count: usize,
    pub cohesion_score: f64,
    pub leader_quality_score: f64,
    pub rotation_quality_score: f64,
    pub candidate_compactness_score: f64,
    pub unmet_conditions: Vec<TrendCohesionGateCondition>,
}

pub struct TrendCohesionEvaluator;

impl TrendCohesionEvaluator {
    pub fn evaluate(
        stability_score: f64,
        continuity_streak: usize,
        current_top_tier: &[String],
        history: &[DecisionPacket],
        cfg: &ParsedTrendCohesionRules,
    ) -> TrendCohesionSnapshot {
        let candidate_count = current_top_tier.len();
        let mut unmet_conditions = Vec::new();

        // 1. Gather a configurable recent history window from persisted packets.
        let recent_packets: Vec<&DecisionPacket> =
            history.iter().rev().take(cfg.history_window_days).collect();
        let past_top_tiers: Vec<&[String]> = recent_packets
            .iter()
            .map(|p| p.top_tier_symbols.as_slice())
            .collect();

        // 2. Identify repeating leaders (in current top tier AND in at least 1 prior packet).
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

        // Leadership concentration proxy: repeated leaders within the active candidate set.
        let repeated_leader_ratio = if candidate_count > 0 {
            leader_count as f64 / candidate_count as f64
        } else {
            0.0
        };

        // Rotation quality (Jaccard similarity with yesterday, if yesterday exists)
        let mut rotation_quality_score = 0.0;
        if let Some(yesterday_tier) = past_top_tiers.first() {
            // past_top_tiers[0] is the most recent past since we `rev`
            let current_set: HashSet<&str> = current_top_tier.iter().map(|s| s.as_str()).collect();
            let prev_set: HashSet<&str> = yesterday_tier.iter().map(|s| s.as_str()).collect();

            let intersection = current_set.intersection(&prev_set).count();
            let union = current_set.union(&prev_set).count();

            if union > 0 {
                rotation_quality_score = (intersection as f64 / union as f64) * 100.0;
            }
        } else if candidate_count > 0 {
            // First day with candidates: not enough history to validate churn, keep neutral.
            rotation_quality_score = 50.0;
        } else {
            rotation_quality_score = 0.0;
        }

        let candidate_compactness_score = match candidate_count {
            0 => 0.0,
            1 => 95.0,
            2 => 90.0,
            3 => 80.0,
            4 => 65.0,
            5 => 50.0,
            6 => 35.0,
            _ => 20.0,
        };

        let leader_depth_score = match leader_count {
            0 => 0.0,
            1 => 55.0,
            2 => 85.0,
            _ => 100.0,
        };

        let stability_component =
            (stability_score / cfg.stability_norm_max * 100.0).clamp(0.0, 100.0);
        let continuity_component =
            (continuity_streak as f64 / cfg.continuity_norm_max as f64 * 100.0).clamp(0.0, 100.0);

        let leader_quality_score = (repeated_leader_ratio * 100.0 * 0.55
            + leader_depth_score * 0.30
            + candidate_compactness_score * 0.15)
            .clamp(0.0, 100.0);

        let cohesion_score = (stability_component * 0.30
            + continuity_component * 0.20
            + leader_quality_score * 0.20
            + rotation_quality_score * 0.15
            + candidate_compactness_score * 0.15)
            .clamp(0.0, 100.0);

        let no_candidates = candidate_count == 0;
        let severe_fragmentation = no_candidates
            || stability_score < cfg.severe_stability_threshold
            || continuity_streak < cfg.severe_continuity_threshold
            || candidate_compactness_score < cfg.severe_compactness_threshold
            || (!past_top_tiers.is_empty()
                && rotation_quality_score < cfg.severe_rotation_threshold)
            || (candidate_count > 0 && leader_quality_score < cfg.severe_leadership_threshold)
            || cohesion_score < cfg.severe_cohesion_threshold;

        let mut gate_passed = true;
        let directional_passed = candidate_count > 0
            && candidate_count <= cfg.directional_max_candidates
            && leader_quality_score >= cfg.directional_leadership_threshold
            && rotation_quality_score >= cfg.directional_rotation_threshold
            && candidate_compactness_score >= cfg.directional_compactness_threshold;

        if stability_score < cfg.gate_stability_threshold {
            unmet_conditions.push(TrendCohesionGateCondition::StabilityThreshold);
            gate_passed = false;
        }
        if continuity_streak < cfg.gate_continuity_threshold {
            unmet_conditions.push(TrendCohesionGateCondition::ContinuityThreshold);
            gate_passed = false;
        }
        if !directional_passed {
            unmet_conditions.push(TrendCohesionGateCondition::DirectionalCohesion);
            gate_passed = false;
        }
        if candidate_compactness_score < cfg.directional_compactness_threshold {
            unmet_conditions.push(TrendCohesionGateCondition::HighCandidateDispersion);
        }
        if !past_top_tiers.is_empty() && rotation_quality_score < cfg.directional_rotation_threshold
        {
            unmet_conditions.push(TrendCohesionGateCondition::UnstableRotation);
        }
        if candidate_count > 0 && leader_quality_score < cfg.directional_leadership_threshold {
            unmet_conditions.push(TrendCohesionGateCondition::WeakLeadership);
        }

        let topology = if no_candidates || leader_count == 0 {
            TrendCohesionTopology::NoLeader
        } else if candidate_count <= cfg.topology_single_max_candidates
            && leader_count == 1
            && candidate_compactness_score >= cfg.topology_single_min_compactness
            && rotation_quality_score >= cfg.topology_single_min_rotation
        {
            TrendCohesionTopology::SingleLeader
        } else {
            TrendCohesionTopology::FragmentedLeaders
        };

        let status = if severe_fragmentation {
            gate_passed = false;
            TrendCohesionStatus::NotFormed
        } else if gate_passed && cohesion_score >= cfg.cohesive_score_threshold {
            TrendCohesionStatus::Cohesive
        } else {
            TrendCohesionStatus::Forming
        };

        TrendCohesionSnapshot {
            status,
            topology,
            gate_passed,
            stability_score,
            continuity_streak,
            candidate_count,
            leader_count,
            cohesion_score,
            leader_quality_score,
            rotation_quality_score,
            candidate_compactness_score,
            unmet_conditions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> ParsedTrendCohesionRules {
        ParsedTrendCohesionRules::default()
    }

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

        let snapshot = TrendCohesionEvaluator::evaluate(15.0, 3, &current, &history, &rules());

        assert_eq!(snapshot.candidate_count, 3);
        assert_eq!(snapshot.leader_count, 2); // A and B repeat
        assert_eq!(snapshot.status, TrendCohesionStatus::Cohesive);
        assert_eq!(snapshot.topology, TrendCohesionTopology::FragmentedLeaders);
        assert!(snapshot.cohesion_score >= 75.0);
        assert!(snapshot.leader_quality_score >= 60.0);
    }

    #[test]
    fn test_churn_degrades_cohesion() {
        let history = vec![
            mock_packet(vec!["X", "Y"]), // Yesterday
        ];
        let current = vec!["A".to_string(), "B".to_string(), "C".to_string()]; // Today (total change)

        let snapshot = TrendCohesionEvaluator::evaluate(15.0, 3, &current, &history, &rules());

        assert_eq!(snapshot.rotation_quality_score, 0.0);
        assert_eq!(snapshot.status, TrendCohesionStatus::NotFormed);
        assert_eq!(snapshot.topology, TrendCohesionTopology::NoLeader);
        assert!(snapshot
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::DirectionalCohesion));
        assert!(snapshot
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::UnstableRotation));
        assert!(snapshot
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::WeakLeadership));
    }

    #[test]
    fn test_not_formed_heuristics() {
        let current = vec!["A".to_string(), "B".to_string()];

        let snap_low_stab = TrendCohesionEvaluator::evaluate(8.0, 3, &current, &[], &rules());
        assert_eq!(snap_low_stab.status, TrendCohesionStatus::NotFormed);
        assert_eq!(snap_low_stab.topology, TrendCohesionTopology::NoLeader);

        let snap_dispersed = TrendCohesionEvaluator::evaluate(
            15.0,
            3,
            &[
                "A".to_string(),
                "B".to_string(),
                "C".to_string(),
                "D".to_string(),
                "E".to_string(),
                "F".to_string(),
            ],
            &[],
            &rules(),
        );
        assert_eq!(snap_dispersed.status, TrendCohesionStatus::NotFormed);
        assert_eq!(snap_dispersed.topology, TrendCohesionTopology::NoLeader);
        assert!(snap_dispersed
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::HighCandidateDispersion));
    }

    #[test]
    fn test_gate_conditions() {
        // Test stability failure only
        let snap1 = TrendCohesionEvaluator::evaluate(
            9.0, // Failed stability
            3,
            &["A".to_string(), "B".to_string()], // Directional ok if it had history... wait, without history leader count is 0
            &[],                                 // Fails directional cohesion
            &rules(),
        );
        assert!(snap1
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::StabilityThreshold));
        assert!(snap1
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::DirectionalCohesion));
        assert!(!snap1.gate_passed);

        // Test continuity failure only
        let history = vec![
            mock_packet(vec!["A", "B", "C"]),
            mock_packet(vec!["A", "B", "D"]),
        ];
        let current = vec!["A".to_string(), "B".to_string()];

        let snap2 = TrendCohesionEvaluator::evaluate(
            15.0,     // Passed stability
            2,        // Failed continuity
            &current, // Passed directional cohesion (candidates <= 4, leaders=2, quality=100%)
            &history,
            &rules(),
        );

        assert!(snap2
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::ContinuityThreshold));
        assert!(!snap2
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::StabilityThreshold));
        assert!(!snap2
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::DirectionalCohesion));
        assert!(!snap2.gate_passed);

        // Gate passes when all are true
        let snap3 = TrendCohesionEvaluator::evaluate(
            15.0,     // Passed stability
            3,        // Passed continuity
            &current, // Passed directional cohesion
            &history,
            &rules(),
        );
        assert!(snap3.unmet_conditions.is_empty());
        assert!(snap3.gate_passed);
    }

    #[test]
    fn test_forming_when_leaders_repeat_but_structure_not_yet_mature() {
        let history = vec![
            mock_packet(vec!["A", "B", "C"]),
            mock_packet(vec!["A", "D", "E"]),
        ];
        let current = vec![
            "A".to_string(),
            "B".to_string(),
            "D".to_string(),
            "E".to_string(),
            "F".to_string(),
        ];

        let snapshot = TrendCohesionEvaluator::evaluate(11.5, 2, &current, &history, &rules());

        assert_eq!(snapshot.status, TrendCohesionStatus::Forming);
        assert!(!snapshot.gate_passed);
        assert!(snapshot.cohesion_score >= 45.0);
        assert!(snapshot.cohesion_score < 75.0);
        assert!(snapshot
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::ContinuityThreshold));
        assert!(snapshot
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::HighCandidateDispersion));
    }

    #[test]
    fn test_topology_single_leader_when_compact_with_one_dominant_name() {
        let history = vec![mock_packet(vec!["A"]), mock_packet(vec!["A", "B"])];
        let current = vec!["A".to_string(), "C".to_string()];

        let snapshot = TrendCohesionEvaluator::evaluate(12.0, 3, &current, &history, &rules());

        assert_eq!(snapshot.topology, TrendCohesionTopology::SingleLeader);
    }

    #[test]
    fn test_topology_fragmented_leaders_when_multiple_leaders_compete() {
        let history = vec![
            mock_packet(vec!["A", "B", "C"]),
            mock_packet(vec!["A", "B", "D"]),
        ];
        let current = vec!["A".to_string(), "B".to_string(), "E".to_string()];

        let snapshot = TrendCohesionEvaluator::evaluate(12.0, 3, &current, &history, &rules());

        assert_eq!(snapshot.topology, TrendCohesionTopology::FragmentedLeaders);
    }
}

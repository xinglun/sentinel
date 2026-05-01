use crate::config::ParsedTrendCohesionRules;
use crate::core::decision::DecisionPacket;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrendCohesionStatus {
    #[default]
    Dispersed,
    Forming,
    Formed,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrendContinuationState {
    #[default]
    None,
    EarlyLeader,
    LeaderConfirmedFollowersLagging,
    Broadening,
    Mature,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceSourceType {
    Manual,      // 手動アノテーション
    OfficialIR,  // 決算速報・公式発表
    NewsMedia,   // 主要ニュースメディア
    PriceAction, // 価格追随（フォロースルー）
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceType {
    CapexPayoff,        // 投資回収の検証
    EarningsValidation, // 業績の裏付け
    OrderVisibility,    // 受注見通しの改善
    FollowThrough,      // 突破後の価格継続性
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AutomatedEvidenceRecord {
    pub source: EvidenceSourceType,
    pub evidence_type: EvidenceType,
    pub confidence: f64,           // 0.0 ~ 1.0 (ソースの信頼性)
    pub description: String,       // 監査用
    pub timestamp_days_ago: usize, // 経過日数
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct SubstantiveEvidence {
    #[serde(default)]
    pub records: Vec<AutomatedEvidenceRecord>,
    // 以下のフィールドは後方互換性と高速アクセスのために維持（集計ロジックによって更新される）
    pub capex_payoff_signal: bool,
    pub earnings_validation: bool,
    pub order_visibility: bool,
    pub event_days_since: usize,
}

impl SubstantiveEvidence {
    /// レコードを元にフラグとスコアを再計算する。
    pub fn aggregate(&mut self) {
        self.capex_payoff_signal = false;
        self.earnings_validation = false;
        self.order_visibility = false;
        let mut min_days = usize::MAX;

        for r in &self.records {
            match r.evidence_type {
                EvidenceType::CapexPayoff => self.capex_payoff_signal = true,
                EvidenceType::EarningsValidation => self.earnings_validation = true,
                EvidenceType::OrderVisibility => self.order_visibility = true,
                EvidenceType::FollowThrough => {} // 今後の拡張用
            }
            if r.timestamp_days_ago < min_days {
                min_days = r.timestamp_days_ago;
            }
        }

        self.event_days_since = if min_days == usize::MAX { 0 } else { min_days };
    }

    /// 特定のタイプの証拠の最大信頼度を取得する。
    pub fn max_confidence(&self, evidence_type: EvidenceType) -> f64 {
        self.records
            .iter()
            .filter(|r| r.evidence_type == evidence_type)
            .map(|r| r.confidence)
            .fold(0.0, f64::max)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct TrendRecognitionEvidence {
    pub state: TrendContinuationState,
    pub diffusion_score: f64,
    pub conviction_score: f64,
    pub lag_state: bool,
    pub single_asset_decay_day: usize,
    pub single_asset_decay_max: usize,
    pub substantive: Option<SubstantiveEvidence>,
}

impl TrendRecognitionEvidence {
    /// 現在のブレイクアウト状況とスカウト状態から、トレンド認識エビデンスを計算する。
    pub fn compute(
        confirmed_count: usize,
        emerging_count: usize,
        scout_days: usize,
        scout_abort_days: usize,
        substantive: Option<SubstantiveEvidence>,
    ) -> Self {
        let active_count = confirmed_count + emerging_count;
        let base_diffusion_score = (confirmed_count as f64 * 1.0) + (emerging_count as f64 * 0.5);

        // 実体的な証拠に基づく確信度の計算
        let mut conviction_score = 0.0;
        if let Some(ref s) = substantive {
            // 各証拠タイプの重み付けと信頼度の積を加算
            let capex_weight = 2.0;
            let earnings_weight = 1.5;
            let order_weight = 1.0;

            if s.records.is_empty() {
                // レコードがない場合は既存のフラグを1.0として扱う（後方互換性）
                if s.capex_payoff_signal {
                    conviction_score += capex_weight;
                }
                if s.earnings_validation {
                    conviction_score += earnings_weight;
                }
                if s.order_visibility {
                    conviction_score += order_weight;
                }
            } else {
                // レコードがある場合は各タイプの最大信頼度を使用
                conviction_score += capex_weight * s.max_confidence(EvidenceType::CapexPayoff);
                conviction_score +=
                    earnings_weight * s.max_confidence(EvidenceType::EarningsValidation);
                conviction_score += order_weight * s.max_confidence(EvidenceType::OrderVisibility);
            }

            // イベントからの経過日数による線形減衰 (T+1=100%, T+5=20%, T+6+=10%)
            let multiplier = if s.event_days_since <= 1 {
                1.0
            } else if s.event_days_since <= 5 {
                1.0 - (s.event_days_since - 1) as f64 * 0.2
            } else {
                0.1 // 低ウェイトを維持
            };
            conviction_score *= multiplier;
        }

        let diffusion_score = base_diffusion_score + conviction_score;

        let state = if active_count == 0 {
            TrendContinuationState::None
        } else if active_count == 1 {
            if confirmed_count == 1 {
                TrendContinuationState::LeaderConfirmedFollowersLagging
            } else {
                TrendContinuationState::EarlyLeader
            }
        } else if emerging_count > 0 {
            TrendContinuationState::Broadening
        } else {
            TrendContinuationState::Mature
        };

        let lag_state = state == TrendContinuationState::LeaderConfirmedFollowersLagging;

        Self {
            state,
            diffusion_score,
            conviction_score,
            lag_state,
            single_asset_decay_day: scout_days,
            single_asset_decay_max: scout_abort_days,
            substantive,
        }
    }
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
    pub stability_ready: bool,
    pub continuity_ready: bool,
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
        current_top_tier: &[String],
        history: &[DecisionPacket],
        cfg: &ParsedTrendCohesionRules,
    ) -> TrendCohesionSnapshot {
        let candidate_count = current_top_tier.len();
        let mut unmet_conditions = Vec::new();

        // 1. ストリーク（継続性）の計算
        let continuity_streak = if let Some(prev) = history.last() {
            if !prev.top_tier_symbols.is_empty() && prev.top_tier_symbols == current_top_tier {
                prev.trend_cohesion.continuity_streak + 1
            } else {
                1
            }
        } else {
            1
        };

        // 2. 永続化されたパケットから、設定可能な直近の履歴ウィンドウを収集。
        let recent_packets: Vec<&DecisionPacket> =
            history.iter().rev().take(cfg.history_window_days).collect();
        let past_top_tiers: Vec<&[String]> = recent_packets
            .iter()
            .map(|p| p.top_tier_symbols.as_slice())
            .collect();

        // 3. 繰り返されるリーダーを特定（現在のトップティアに含まれ、かつ以前のパケットにも少なくとも1回含まれるもの）。
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

        // リーダーシップ集中プロキシ: アクティブな候補セット内で繰り返されるリーダー。
        let repeated_leader_ratio = if candidate_count > 0 {
            leader_count as f64 / candidate_count as f64
        } else {
            0.0
        };

        // ローテーション品質 (昨日とのジャッカード係数、昨日が存在する場合)
        let mut rotation_quality_score = 0.0;
        if let Some(yesterday_tier) = past_top_tiers.first() {
            // `rev` しているため past_top_tiers[0] が直近の過去。
            let current_set: HashSet<&str> = current_top_tier.iter().map(|s| s.as_str()).collect();
            let prev_set: HashSet<&str> = yesterday_tier.iter().map(|s| s.as_str()).collect();

            let intersection = current_set.intersection(&prev_set).count();
            let union = current_set.union(&prev_set).count();

            if union > 0 {
                rotation_quality_score = (intersection as f64 / union as f64) * 100.0;
            }
        } else if candidate_count > 0 {
            // 候補がある初日：チャーンを検証するための十分な履歴がないため、ニュートラルに保つ。
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

        let mut gate_passed = true;
        let directional_passed = candidate_count > 0
            && candidate_count <= cfg.directional_max_candidates
            && leader_quality_score >= cfg.directional_leadership_threshold
            && rotation_quality_score >= cfg.directional_rotation_threshold
            && candidate_compactness_score >= cfg.directional_compactness_threshold;

        let stability_ready = stability_score >= cfg.gate_stability_threshold;
        let continuity_ready = continuity_streak >= cfg.gate_continuity_threshold;

        if !stability_ready {
            unmet_conditions.push(TrendCohesionGateCondition::StabilityThreshold);
            gate_passed = false;
        }
        if !continuity_ready {
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

        let topology = if candidate_count == 0 || leader_count == 0 {
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

        let status = if !gate_passed {
            TrendCohesionStatus::Dispersed
        } else if gate_passed && cohesion_score >= cfg.cohesive_score_threshold {
            TrendCohesionStatus::Formed
        } else {
            TrendCohesionStatus::Forming
        };

        TrendCohesionSnapshot {
            status,
            topology,
            gate_passed,
            stability_ready,
            continuity_ready,
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

    fn mock_packet_with_streak(symbols: Vec<&str>, streak: usize) -> DecisionPacket {
        let mut p = mock_packet(symbols);
        p.trend_cohesion.continuity_streak = streak;
        p
    }

    #[test]
    fn test_history_leader_repetition() {
        let current_symbols = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let history = vec![
            mock_packet(vec!["A", "B", "C"]),
            mock_packet_with_streak(vec!["A", "B", "C"], 2),
        ];
        let current = current_symbols;
        let snapshot = TrendCohesionEvaluator::evaluate(15.0, &current, &history, &rules());

        assert_eq!(snapshot.candidate_count, 3);
        assert_eq!(snapshot.leader_count, 3); // A, B, C 全てが繰り返される
        assert_eq!(snapshot.status, TrendCohesionStatus::Formed);
        assert_eq!(snapshot.topology, TrendCohesionTopology::FragmentedLeaders);
        assert!(snapshot.cohesion_score >= 75.0);
        assert!(snapshot.leader_quality_score >= 60.0);
    }

    #[test]
    fn test_churn_degrades_cohesion() {
        let history = vec![
            mock_packet(vec!["X", "Y"]), // 昨日
        ];
        let current = vec!["A".to_string(), "B".to_string(), "C".to_string()]; // 今日 (全入れ替え)

        let snapshot = TrendCohesionEvaluator::evaluate(15.0, &current, &history, &rules());

        assert_eq!(snapshot.rotation_quality_score, 0.0);
        assert_eq!(snapshot.status, TrendCohesionStatus::Dispersed);
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

        let snap_low_stab = TrendCohesionEvaluator::evaluate(8.0, &current, &[], &rules());
        assert_eq!(snap_low_stab.status, TrendCohesionStatus::Dispersed);
        assert_eq!(snap_low_stab.topology, TrendCohesionTopology::NoLeader);

        let snap_dispersed = TrendCohesionEvaluator::evaluate(
            15.0,
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
        assert_eq!(snap_dispersed.status, TrendCohesionStatus::Dispersed);
        assert_eq!(snap_dispersed.topology, TrendCohesionTopology::NoLeader);
        assert!(snap_dispersed
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::HighCandidateDispersion));
    }

    #[test]
    fn test_gate_conditions() {
        // 安定性のみの失敗をテスト
        let snap1 = TrendCohesionEvaluator::evaluate(
            9.0,                                 // 安定性失敗
            &["A".to_string(), "B".to_string()], // 履歴がないためリーダー数は 0
            &[],                                 // 指向性凝集 (Directional cohesion) に失敗
            &rules(),
        );
        assert!(snap1
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::StabilityThreshold));
        assert!(snap1
            .unmet_conditions
            .contains(&TrendCohesionGateCondition::DirectionalCohesion));
        assert!(!snap1.gate_passed);

        // 継続性のみの失敗をテスト
        let history = vec![
            mock_packet(vec!["A", "B", "C"]),
            mock_packet(vec!["A", "B", "D"]),
        ];
        let current = vec!["A".to_string(), "B".to_string()];

        let snap2 = TrendCohesionEvaluator::evaluate(
            15.0,     // 安定性通過
            &current, // 指向性凝集を通過 (候補数 <= 4, リーダー=2, 品質=100%)
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

        // すべて真の時にゲートを通過
        let history_pass = vec![
            mock_packet(vec!["A", "B"]),
            mock_packet_with_streak(vec!["A", "B"], 2),
        ];
        let snap3 = TrendCohesionEvaluator::evaluate(
            15.0,     // 安定性通過
            &current, // 指向性凝集通過
            &history_pass,
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

        let snapshot = TrendCohesionEvaluator::evaluate(11.5, &current, &history, &rules());

        assert_eq!(snapshot.status, TrendCohesionStatus::Dispersed);
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

        let snapshot = TrendCohesionEvaluator::evaluate(12.0, &current, &history, &rules());

        assert_eq!(snapshot.topology, TrendCohesionTopology::SingleLeader);
    }

    #[test]
    fn test_topology_fragmented_leaders_when_multiple_leaders_compete() {
        let history = vec![
            mock_packet(vec!["A", "B", "C"]),
            mock_packet(vec!["A", "B", "D"]),
        ];
        let current = vec!["A".to_string(), "B".to_string(), "E".to_string()];

        let snapshot = TrendCohesionEvaluator::evaluate(12.0, &current, &history, &rules());

        assert_eq!(snapshot.topology, TrendCohesionTopology::FragmentedLeaders);
    }

    // --- Phase 4: Runtime Stability & Replay Validation ---

    #[test]
    fn test_diffusion_score_sensitivity() {
        // 0:0 -> 0.0
        let ev0 = TrendRecognitionEvidence::compute(0, 0, 0, 3, None);
        assert_eq!(ev0.diffusion_score, 0.0);
        assert_eq!(ev0.state, TrendContinuationState::None);

        // 0:1 (Emerging only) -> 0.5
        let ev1 = TrendRecognitionEvidence::compute(0, 1, 0, 3, None);
        assert_eq!(ev1.diffusion_score, 0.5);
        assert_eq!(ev1.state, TrendContinuationState::EarlyLeader);

        // 1:0 (Confirmed only) -> 1.0
        let ev2 = TrendRecognitionEvidence::compute(1, 0, 0, 3, None);
        assert_eq!(ev2.diffusion_score, 1.0);
        assert_eq!(
            ev2.state,
            TrendContinuationState::LeaderConfirmedFollowersLagging
        );
        assert!(ev2.lag_state);

        // 1:1 (Leader + Follower) -> 1.5
        let ev3 = TrendRecognitionEvidence::compute(1, 1, 0, 3, None);
        assert_eq!(ev3.diffusion_score, 1.5);
        assert_eq!(ev3.state, TrendContinuationState::Broadening);
        assert!(!ev3.lag_state);

        // 2:0 (Two Confirmed) -> 2.0
        let ev4 = TrendRecognitionEvidence::compute(2, 0, 0, 3, None);
        assert_eq!(ev4.diffusion_score, 2.0);
        assert_eq!(ev4.state, TrendContinuationState::Mature);
    }

    #[test]
    fn test_replay_scenario_diffusion_path() {
        // Day 1: Early Leader (Emerging)
        let day1 = TrendRecognitionEvidence::compute(0, 1, 1, 3, None);
        assert_eq!(day1.state, TrendContinuationState::EarlyLeader);
        assert_eq!(day1.single_asset_decay_day, 1);

        // Day 2: Leader Confirmed (Followers Lagging)
        let day2 = TrendRecognitionEvidence::compute(1, 0, 2, 3, None);
        assert_eq!(
            day2.state,
            TrendContinuationState::LeaderConfirmedFollowersLagging
        );
        assert_eq!(day2.single_asset_decay_day, 2);

        // Day 3: Diffusion (Broadening) - Reset Decay
        // 注意: Reset されるのは transition_log 側だが、Evidence に渡される scout_days が 0 になることを検証
        let day3 = TrendRecognitionEvidence::compute(1, 1, 0, 3, None);
        assert_eq!(day3.state, TrendContinuationState::Broadening);
        assert_eq!(day3.single_asset_decay_day, 0);
    }

    #[test]
    fn test_audit_snapshot_consistency_no_drift() {
        // 状態が維持されている間、diffusion_score が不変であることを確認
        let ev_a = TrendRecognitionEvidence::compute(1, 1, 0, 3, None);
        let ev_b = TrendRecognitionEvidence::compute(1, 1, 0, 3, None);
        assert_eq!(ev_a, ev_b);

        // 境界値テスト: Confirmed が増えても Emerging がいれば Broadening
        let ev_c = TrendRecognitionEvidence::compute(2, 1, 0, 3, None);
        assert_eq!(ev_c.state, TrendContinuationState::Broadening);
        assert_eq!(ev_c.diffusion_score, 2.5);
    }

    #[test]
    fn test_substantive_conviction_diffusion() {
        // AI Payoff (+2.0) + Earnings (+1.5) = +3.5
        let substantive = SubstantiveEvidence {
            records: vec![],
            capex_payoff_signal: true,
            earnings_validation: true,
            order_visibility: false,
            event_days_since: 1,
        };

        // Leader confirmed (1.0) + Conviction (3.5) = 4.5
        let ev = TrendRecognitionEvidence::compute(1, 0, 0, 3, Some(substantive));
        assert_eq!(ev.conviction_score, 3.5);
        assert_eq!(ev.diffusion_score, 4.5);
        assert_eq!(
            ev.state,
            TrendContinuationState::LeaderConfirmedFollowersLagging
        );

        // 拡散発生時 (Confirmed + Emerging = 1.5) + Conviction (3.5) = 5.0
        let ev_diffused =
            TrendRecognitionEvidence::compute(1, 1, 0, 3, Some(ev.substantive.unwrap()));
        assert_eq!(ev_diffused.diffusion_score, 5.0);
    }

    #[test]
    fn test_automated_evidence_aggregation() {
        let mut substantive = SubstantiveEvidence {
            records: vec![
                AutomatedEvidenceRecord {
                    source: EvidenceSourceType::NewsMedia,
                    evidence_type: EvidenceType::CapexPayoff,
                    confidence: 0.8,
                    description: "News A".to_string(),
                    timestamp_days_ago: 2,
                },
                AutomatedEvidenceRecord {
                    source: EvidenceSourceType::OfficialIR,
                    evidence_type: EvidenceType::CapexPayoff,
                    confidence: 1.0,
                    description: "Official IR".to_string(),
                    timestamp_days_ago: 1,
                },
                AutomatedEvidenceRecord {
                    source: EvidenceSourceType::Manual,
                    evidence_type: EvidenceType::OrderVisibility,
                    confidence: 0.5,
                    description: "Manual low confidence".to_string(),
                    timestamp_days_ago: 3,
                },
            ],
            ..Default::default()
        };

        substantive.aggregate();
        // 最も新しい(1日)が採用される
        assert_eq!(substantive.event_days_since, 1);
        assert!(substantive.capex_payoff_signal);
        assert!(substantive.order_visibility);

        // Compute 時に Max Confidence が採用されるか検証
        let ev = TrendRecognitionEvidence::compute(1, 0, 0, 3, Some(substantive));
        // Capex (2.0 * 1.0) + Order (1.0 * 0.5) = 2.5
        // T+1 なので減衰なし
        assert_eq!(ev.conviction_score, 2.5);

        // 日数が経過した場合の減衰テスト (T+3 = 1.0 - 2*0.2 = 0.6)
        let mut s_decay = ev.substantive.unwrap();
        s_decay.event_days_since = 3;
        let ev_decay = TrendRecognitionEvidence::compute(1, 0, 0, 3, Some(s_decay));
        let expected = 2.5 * 0.6;
        assert!((ev_decay.conviction_score - expected).abs() < 1e-10);
    }

    #[test]
    fn test_event_follow_through_window() {
        let mut substantive = SubstantiveEvidence {
            records: vec![],
            capex_payoff_signal: true,
            earnings_validation: false,
            order_visibility: false,
            event_days_since: 1,
        };

        // Day 1: Conviction = 2.0 (100%)
        let ev1 = TrendRecognitionEvidence::compute(1, 0, 0, 3, Some(substantive.clone()));
        assert!((ev1.conviction_score - 2.0).abs() < 1e-10);

        // Day 3: Conviction = 2.0 * 0.6 = 1.2 (Linear decay: 1.0 - (3-1)*0.2 = 0.6)
        substantive.event_days_since = 3;
        let ev3 = TrendRecognitionEvidence::compute(1, 0, 0, 3, Some(substantive.clone()));
        assert!((ev3.conviction_score - 1.2).abs() < 1e-10);

        // Day 5: Conviction = 2.0 * 0.2 = 0.4 (Linear decay: 1.0 - (5-1)*0.2 = 0.2)
        substantive.event_days_since = 5;
        let ev5 = TrendRecognitionEvidence::compute(1, 0, 0, 3, Some(substantive.clone()));
        assert!((ev5.conviction_score - 0.4).abs() < 1e-10);

        // Day 6: Maintain low weight (10%) -> 2.0 * 0.1 = 0.2
        substantive.event_days_since = 6;
        let ev6 = TrendRecognitionEvidence::compute(1, 0, 0, 3, Some(substantive));
        assert!((ev6.conviction_score - 0.2).abs() < 1e-10);
        assert!((ev6.diffusion_score - 1.2).abs() < 1e-10); // 1.0 (Confirmed) + 0.2 (Conviction)
    }
}

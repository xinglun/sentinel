use crate::config::ParsedRules;
use crate::core::features::MarketFeatures;
use serde::{Deserialize, Serialize};

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarketState {
    #[default]
    IGNITION,
    NEWBORN,
    EARLY_CONFIRMATION,
    ESTABLISHED,
    CONFIRMED,
    DEFENSIVE,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum LifecycleState {
    #[default]
    NONE,
    IGNITION,
    NEWBORN,
    EARLY_CONFIRMATION,
    ESTABLISHED,
    CONFIRMED,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum RiskOverlay {
    #[default]
    NORMAL,
    DECELERATING,
    DEFENSIVE,
    BROKEN,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MarketRegimeSnapshot {
    pub market_state: MarketState,
    pub lifecycle_state: LifecycleState,
    pub risk_overlay: RiskOverlay,
    pub reasons: Vec<String>,
    pub low_stability_streak: usize,
    pub duration_in_state: usize,
    pub transition_audit: Option<MarketTransitionAudit>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MarketTransitionAudit {
    pub from: LifecycleState,
    pub to: LifecycleState,
    pub is_reset_blocked: bool,
    pub is_downgrade_clamped: bool,
    pub core_breakdown: bool,
    pub duration_locked: bool,
    pub trend_dominant: bool,
    pub reset_gate_passed: bool,
    pub indicator_cap: LifecycleState,
    pub soft_reset_applied: bool,
    pub defensive_override: bool,
}

pub struct MarketRegimeStateMachine {
    pub current_lifecycle: LifecycleState,
    #[allow(dead_code)]
    pub current_risk: RiskOverlay,
}

impl MarketRegimeStateMachine {
    /// 以前のコンテキストに基づいて、次のレジームスナップショットと校正された経過日数を計算します。
    pub fn transition(
        prev_snapshot: Option<&MarketRegimeSnapshot>,
        features: &mut MarketFeatures,
        prev_age: usize,
        rules: &ParsedRules,
    ) -> (MarketRegimeSnapshot, usize) {
        let initial_lifecycle = prev_snapshot
            .map(|s| s.lifecycle_state)
            .unwrap_or(LifecycleState::NONE);
        let initial_risk = prev_snapshot
            .map(|s| s.risk_overlay)
            .unwrap_or(RiskOverlay::NORMAL);

        let sm = Self::new(initial_lifecycle, initial_risk);

        // ハーデニング: バックテストにおける1日の遅延を解消するため、
        // 状態が維持された場合に「今日」到達するであろう経過日数（current_potential_age）を遷移ロジックに使用します。
        let current_potential_age = prev_age + 1;

        // 重要: 判定の前にポテンシャルな経過日数に基づいて成熟度メトリクスを再校正します。
        // これにより、ESTABLISHED -> CONFIRMED などの遷移が正しい日に発生することを保証します。
        features.recalibrate(current_potential_age);
        let (next_snapshot, next_age) =
            sm.compute_next_state(features, current_potential_age, prev_snapshot, rules);

        // 状態が変化した、またはソフトリセットが発生した場合の最終的な再校正
        features.recalibrate(next_age);

        (next_snapshot, next_age)
    }

    pub fn new(lifecycle: LifecycleState, risk: RiskOverlay) -> Self {
        Self {
            current_lifecycle: lifecycle,
            current_risk: risk,
        }
    }

    pub fn compute_next_state(
        &self,
        features: &MarketFeatures,
        regime_age: usize,
        prev_snapshot: Option<&MarketRegimeSnapshot>,
        rules: &ParsedRules,
    ) -> (MarketRegimeSnapshot, usize) {
        let mut reasons = Vec::new();
        let mut next_risk = RiskOverlay::NORMAL;

        let mut low_stability_streak = prev_snapshot.map(|s| s.low_stability_streak).unwrap_or(0);
        if features.stability_score < 10.0 {
            low_stability_streak += 1;
        } else {
            low_stability_streak = 0;
        }

        // --- 1. リスク・オーバーレイ・ロジック（ダウングレード優先） ---
        if features.system_confidence < 50.0 {
            next_risk = RiskOverlay::DEFENSIVE;
            reasons.push(format!(
                "Confidence ({:.1}) < 50",
                features.system_confidence
            ));
        } else if features.flow_acceleration.unwrap_or(0.0) < -0.05 {
            next_risk = RiskOverlay::DECELERATING;
            reasons.push(format!(
                "Flow acceleration ({:.3}) < -0.05",
                features.flow_acceleration.unwrap_or(0.0)
            ));
        }

        // --- 2. ターゲット・ライフサイクルのドラフト（アップグレードおよびダウングレードの評価） ---
        let mut target_lifecycle = self.current_lifecycle;

        // 2.1 アップグレードの可能性
        match self.current_lifecycle {
            LifecycleState::NONE => {
                if features.stability_structural > 0.0 {
                    target_lifecycle = LifecycleState::IGNITION;
                }
            }
            LifecycleState::IGNITION => {
                if features.system_confidence >= 60.0 && regime_age >= 5 {
                    target_lifecycle = LifecycleState::NEWBORN;
                }
            }
            LifecycleState::NEWBORN => {
                if features.system_confidence >= 70.0
                    && regime_age >= 10
                    && features.any_pullback_occurred
                {
                    target_lifecycle = LifecycleState::EARLY_CONFIRMATION;
                }
            }
            LifecycleState::EARLY_CONFIRMATION => {
                if features.system_confidence >= 80.0 && regime_age >= 20 {
                    target_lifecycle = LifecycleState::ESTABLISHED;
                }
            }
            LifecycleState::ESTABLISHED => {
                if features.trend_maturity > 0.8 {
                    target_lifecycle = LifecycleState::CONFIRMED;
                }
            }
            LifecycleState::CONFIRMED => {}
        }

        // 2.2 ダウングレードの可能性（インジケーターの悪化に基づく）
        let indicator_cap = if features.system_confidence < 55.0 || features.core_assets_breakdown {
            LifecycleState::IGNITION
        } else if features.system_confidence < 65.0 {
            LifecycleState::NEWBORN
        } else if features.system_confidence < 75.0 {
            LifecycleState::EARLY_CONFIRMATION
        } else {
            LifecycleState::CONFIRMED
        };

        if self.lifecycle_rank(indicator_cap) < self.lifecycle_rank(target_lifecycle) {
            target_lifecycle = indicator_cap;
            reasons.push(format!(
                "インジケーターが {:?} へのダウングレードを示唆しています",
                target_lifecycle
            ));
        }

        let mut next_lifecycle = target_lifecycle;

        // --- 3. 慣性レイヤー（Inertia Layer）: 期間ロック、ダウングレードおよびリセット・ゲート ---
        let mut is_downgrade_clamped = false;
        let mut reset_gate_passed = false;
        let mut soft_reset_applied = false;
        let mut duration_locked = false;
        let mut is_reset_blocked = false;

        let prev_duration = prev_snapshot.map(|s| s.duration_in_state).unwrap_or(0);

        let is_upgrade =
            self.lifecycle_rank(next_lifecycle) > self.lifecycle_rank(self.current_lifecycle);
        let is_downgrade = self.is_downgrade(self.current_lifecycle, next_lifecycle);
        let is_reset = (next_lifecycle != self.current_lifecycle)
            && is_downgrade
            && next_lifecycle == LifecycleState::IGNITION;
        let is_defensive_override = next_risk == RiskOverlay::DEFENSIVE;

        if next_lifecycle != self.current_lifecycle {
            // 3.1 期間ロック（アップグレードとリセットに適用。NONE およびディフェンシブ・オーバーライドは対象外）
            if (is_upgrade || is_reset)
                && !is_defensive_override
                && self.current_lifecycle != LifecycleState::NONE
                && prev_duration < rules.inertia.min_state_duration
            {
                let change_type = if is_upgrade {
                    "アップグレード"
                } else {
                    "リセット"
                };
                next_lifecycle = self.current_lifecycle;
                duration_locked = true;
                reasons.push(format!("[慣性:期間ロック] {:?} からの {} がブロックされました。最低継続日数 {} 日を満たしていません（現在: {} 日）。", self.current_lifecycle, change_type, rules.inertia.min_state_duration, prev_duration));
            }

            if is_downgrade {
                // リセット・ゲートのチェック（IGNITION への移行試行）
                if next_lifecycle == LifecycleState::IGNITION
                    && self.current_lifecycle != LifecycleState::NEWBORN
                {
                    let reset_allowed = is_defensive_override
                        || self.check_reset_gate(features, low_stability_streak, &mut reasons);
                    if !reset_allowed {
                        // ブロックされたリセット: ダウングレード・ゲートを使用（最大1ステップ）
                        next_lifecycle = self.step_downgrade(self.current_lifecycle);
                        is_reset_blocked = true;
                        reasons.push(format!(
                            "[慣性:リセットブロック] ゲートによりリセットが拒否されました。{:?} へステップダウンします。",
                            next_lifecycle
                        ));
                    } else {
                        reset_gate_passed = true;
                        reasons.push(
                            "[慣性:リセット確定] すべてのリセット・ゲート条件を満たしました。"
                                .to_string(),
                        );
                    }
                } else if self.is_multi_step_downgrade(self.current_lifecycle, next_lifecycle) {
                    // 通常のマルチステップ・ダウングレードをブロック
                    let original_target = next_lifecycle;
                    next_lifecycle = self.step_downgrade(self.current_lifecycle);
                    is_downgrade_clamped = true;
                    reasons.push(format!("[ゲート:ダウングレード] マルチステップ・ダウングレード（{:?} -> {:?}）がブロックされました。{:?} に制限されました。", self.current_lifecycle, original_target, next_lifecycle));
                }
            }
        }

        // --- 4. Age & Stability Persistence ---
        let mut next_age = regime_age;
        if next_lifecycle != self.current_lifecycle {
            let is_downgrade = self.is_downgrade(self.current_lifecycle, next_lifecycle);
            if is_downgrade {
                if next_lifecycle == LifecycleState::IGNITION {
                    next_age = 1;
                } else {
                    // ソフトリセット: 経過日数を1にリセットする代わりに30%削減
                    next_age = (regime_age as f64 * 0.7).max(1.0) as usize;
                    soft_reset_applied = true;
                    reasons.push(format!(
                        "[慣性:ソフトリセット] ダウングレード中に経過日数が {} に削減されました",
                        next_age
                    ));
                }
            }
        }

        // --- 5. 複合状態マッピング（Roadmap 4.2節を参照） ---
        let market_state = match next_risk {
            RiskOverlay::DEFENSIVE | RiskOverlay::BROKEN => MarketState::DEFENSIVE,
            RiskOverlay::NORMAL | RiskOverlay::DECELERATING => match next_lifecycle {
                LifecycleState::NONE => MarketState::DEFENSIVE,
                LifecycleState::IGNITION => MarketState::IGNITION,
                LifecycleState::NEWBORN => MarketState::NEWBORN,
                LifecycleState::EARLY_CONFIRMATION => MarketState::EARLY_CONFIRMATION,
                LifecycleState::ESTABLISHED => MarketState::ESTABLISHED,
                LifecycleState::CONFIRMED => MarketState::CONFIRMED,
            },
        };

        let duration_in_state = if next_lifecycle == self.current_lifecycle {
            prev_duration + 1
        } else {
            1
        };

        let transition_audit = Some(MarketTransitionAudit {
            from: self.current_lifecycle,
            to: next_lifecycle,
            is_reset_blocked,
            is_downgrade_clamped,
            core_breakdown: features.core_assets_breakdown,
            duration_locked,
            trend_dominant: features.trend_dominant,
            reset_gate_passed,
            indicator_cap,
            soft_reset_applied,
            defensive_override: is_defensive_override,
        });

        (
            MarketRegimeSnapshot {
                market_state,
                lifecycle_state: next_lifecycle,
                risk_overlay: next_risk,
                reasons,
                low_stability_streak,
                duration_in_state,
                transition_audit,
            },
            next_age,
        )
    }

    fn is_downgrade(&self, from: LifecycleState, to: LifecycleState) -> bool {
        self.lifecycle_rank(to) < self.lifecycle_rank(from)
    }

    fn is_multi_step_downgrade(&self, from: LifecycleState, to: LifecycleState) -> bool {
        let diff = self.lifecycle_rank(from) as i32 - self.lifecycle_rank(to) as i32;
        diff > 1
    }

    fn step_downgrade(&self, from: LifecycleState) -> LifecycleState {
        match from {
            LifecycleState::CONFIRMED => LifecycleState::ESTABLISHED,
            LifecycleState::ESTABLISHED => LifecycleState::EARLY_CONFIRMATION,
            LifecycleState::EARLY_CONFIRMATION => LifecycleState::NEWBORN,
            LifecycleState::NEWBORN => LifecycleState::IGNITION,
            _ => LifecycleState::IGNITION,
        }
    }

    fn lifecycle_rank(&self, s: LifecycleState) -> usize {
        match s {
            LifecycleState::NONE => 0,
            LifecycleState::IGNITION => 1,
            LifecycleState::NEWBORN => 2,
            LifecycleState::EARLY_CONFIRMATION => 3,
            LifecycleState::ESTABLISHED => 4,
            LifecycleState::CONFIRMED => 5,
        }
    }

    fn check_reset_gate(
        &self,
        features: &MarketFeatures,
        streak: usize,
        reasons: &mut Vec<String>,
    ) -> bool {
        let c1 = !features.trend_dominant;
        let c2 = features.stability_structural < 25.0;
        let c3 = streak >= 3;
        let c4 = features.flow_acceleration.unwrap_or(0.0) <= 0.0;
        let c5 = features.core_assets_breakdown;

        if !c1 {
            reasons.push("[慣性:ゲート失敗] トレンドが支配的（Dominant）です".to_string());
        }
        if !c2 {
            reasons.push("[慣性:ゲート失敗] 構造的安定性が 25 未満ではありません".to_string());
        }
        if !c3 {
            reasons.push(format!(
                "[慣性:ゲート失敗] 安定性ストリーク（{}）が 3 未満です",
                streak
            ));
        }
        if !c4 {
            reasons.push(
                "[慣性:ゲート失敗] フロー加速（Flow acceleration）が 0 を超えています".to_string(),
            );
        }
        if !c5 {
            reasons.push("[慣性:ゲート失敗] コア資産がまだ健全です".to_string());
        }

        c1 && c2 && c3 && c4 && c5
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    fn mock_features(confidence: f64, stability: f64, maturity: f64) -> MarketFeatures {
        MarketFeatures {
            date: Utc::now().date_naive(),
            up_count: 0,
            flat_count: 0,
            down_count: 0,
            total_count: 0,
            up_weight: 0.0,
            flat_weight: 0.0,
            down_weight: 0.0,
            total_weight: 0.0,
            gravity_strength: 0.0,
            potential_energy: 0.0,
            dominance_margin: 0.0,
            system_confidence: confidence,
            trend_dominant: confidence >= 55.0,
            stability_score: 0.0,
            stability_structural: stability,
            stability_temporal: 0.0,
            trend_maturity: maturity,
            universe_integrity: 0.0,
            flow_acceleration: None,
            regime_age: 1,
            any_pullback_occurred: false,
            core_assets_breakdown: false,
        }
    }

    fn mock_rules() -> ParsedRules {
        use crate::config::TrendConfig;
        use std::collections::HashMap;
        ParsedRules {
            trend: TrendConfig {
                lookback_days: 10,
                flat_threshold_pct: 0.1,
            },
            sorted_bands: Vec::new(),
            actions: HashMap::new(),
            sizing_multipliers: None,
            core_assets: Vec::new(),
            inertia: crate::config::ParsedInertia {
                min_state_duration: 3,
                trend_dominant_min_confidence: 55.0,
                core_breakdown_k: 2,
                core_breakdown_avg_deviation: -5.0,
                core_breakdown_breadth_floor: 0.0,
            },
            trend_cohesion: crate::config::ParsedTrendCohesionRules::default(),
            breakout: crate::config::ParsedBreakoutRules::default(),
            market_state_engine: Default::default(),
            sec: None,
        }
    }

    #[test]
    fn test_lifecycle_progression() {
        let rules = mock_rules(); // min_duration = 3
        let sm = MarketRegimeStateMachine::new(LifecycleState::NONE, RiskOverlay::NORMAL);
        let f1 = mock_features(55.0, 0.5, 0.1);
        let (s1, _) = sm.compute_next_state(&f1, 0, None, &rules);
        assert_eq!(s1.lifecycle_state, LifecycleState::IGNITION);

        let sm2 = MarketRegimeStateMachine::new(LifecycleState::IGNITION, RiskOverlay::NORMAL);
        let f2 = mock_features(65.0, 0.5, 0.1);
        let prev = MarketRegimeSnapshot {
            lifecycle_state: LifecycleState::IGNITION,
            duration_in_state: 3,
            ..Default::default()
        };
        let (s2, _) = sm2.compute_next_state(&f2, 6, Some(&prev), &rules);
        assert_eq!(s2.lifecycle_state, LifecycleState::NEWBORN);
    }

    #[test]
    fn test_defensive_downgrade() {
        let rules = mock_rules();
        let sm = MarketRegimeStateMachine::new(LifecycleState::ESTABLISHED, RiskOverlay::NORMAL);
        let f = mock_features(40.0, 0.5, 0.5);
        let (s, _) = sm.compute_next_state(&f, 30, None, &rules);
        assert_eq!(s.market_state, MarketState::DEFENSIVE);
        assert_eq!(s.risk_overlay, RiskOverlay::DEFENSIVE);
        assert!(s
            .reasons
            .iter()
            .any(|r| r.contains("Confidence (40.0) < 50")));
    }

    #[test]
    fn test_duration_lock() {
        let rules = mock_rules(); // min_duration = 3
        let sm = MarketRegimeStateMachine::new(LifecycleState::IGNITION, RiskOverlay::NORMAL);

        // IGNITION 1日目 (duration = 1)
        let prev = MarketRegimeSnapshot {
            lifecycle_state: LifecycleState::IGNITION,
            duration_in_state: 1,
            ..Default::default()
        };

        // NEWBORN へのアップグレードを試行
        let f = mock_features(65.0, 0.5, 0.1);
        let (s, _) = sm.compute_next_state(&f, 6, Some(&prev), &rules);

        // IGNITION にロックされるべき
        assert_eq!(s.lifecycle_state, LifecycleState::IGNITION);
        assert!(s.duration_in_state == 2);
        assert!(s.transition_audit.as_ref().unwrap().duration_locked);

        // IGNITION 3日目 (duration = 3)
        let prev3 = MarketRegimeSnapshot {
            lifecycle_state: LifecycleState::IGNITION,
            duration_in_state: 3,
            ..Default::default()
        };
        let (s3, _) = sm.compute_next_state(&f, 8, Some(&prev3), &rules);

        // ロックが解除されるべき
        assert_eq!(s3.lifecycle_state, LifecycleState::NEWBORN);
        assert!(s3.duration_in_state == 1);
        assert!(!s3.transition_audit.as_ref().unwrap().duration_locked);
    }

    #[test]
    fn test_duration_lock_reset() {
        let rules = mock_rules(); // min_duration = 3
        let sm = MarketRegimeStateMachine::new(LifecycleState::ESTABLISHED, RiskOverlay::NORMAL);

        // ESTABLISHED 1日目 (duration = 1)
        let prev = MarketRegimeSnapshot {
            lifecycle_state: LifecycleState::ESTABLISHED,
            duration_in_state: 1,
            ..Default::default()
        };

        // 1. 信頼度の低下によりリセット（IGNITION へ）を試行（ただし、まだディフェンシブではない）
        let f_reset = mock_features(52.0, 0.5, 0.5); // < 55 は IGNITION へのリセットを意味する
        let (s_reset, _) = sm.compute_next_state(&f_reset, 30, Some(&prev), &rules);

        // ESTABLISHED にロックされるべき（リセットがブロックされる）
        assert_eq!(s_reset.lifecycle_state, LifecycleState::ESTABLISHED);
        assert!(s_reset.transition_audit.as_ref().unwrap().duration_locked);

        // 2. ディフェンシブ・オーバーライドを伴うリセットを試行（信頼度 < 50）
        let f_defensive = mock_features(48.0, 0.5, 0.5); // < 50 は DEFENSIVE リスクを意味する
        let (s_def, _) = sm.compute_next_state(&f_defensive, 30, Some(&prev), &rules);

        // 期間によってロックされるべきではない（ディフェンシブ・オーバーライドが優先される）
        assert_eq!(s_def.lifecycle_state, LifecycleState::IGNITION);
        assert_eq!(s_def.risk_overlay, RiskOverlay::DEFENSIVE);
        assert!(!s_def.transition_audit.as_ref().unwrap().duration_locked);
    }
}

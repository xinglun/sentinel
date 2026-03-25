use crate::config::ParsedRules;
use crate::core::decision::DecisionPacket;
use crate::core::features::AssetFeatures;
use serde::{Deserialize, Serialize};

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssetState {
    OPTIMAL,
    CRUISE,
    PULLBACK,
    CAUTION,
    OVERHEAT,
    DEFEND,
    #[default]
    FORMING,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AssetStateSnapshot {
    pub symbol: String,
    pub state: AssetState,
    pub reasons: Vec<String>,
    pub recovery_streak: usize,
    pub last_defend_age: usize, // Days since last DEFEND state
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AssetStrengthMemory {
    pub symbol: String,
    pub top3_days_last_10: u8,
    pub top5_days_last_10: u8,
    pub defend_days_last_20: u8,
    pub below_cruise_days_last_20: u8,
    pub rolling_strength_rank: f64,
    pub top_tier_locked: bool,
    pub promotion_capped: bool,
    pub consecutive_optimal_signal_history: usize, // Signal was OPTIMAL consecutively in history
    pub consecutive_non_optimal_signal_history: usize, // Signal was NOT OPTIMAL consecutively in history
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AssetStrengthDecision {
    pub symbol: String,
    pub raw_score: f64,
    pub memory_score: f64,
    pub adjusted_score: f64,
    pub min_state: Option<AssetState>,
    pub max_state: Option<AssetState>,
    pub downgrade_friction: bool,
    pub upgrade_friction: bool,
    pub reasons: Vec<String>,
}

pub struct AssetStateMachine;

impl AssetStateMachine {
    pub fn compute_state(
        features: &AssetFeatures,
        rules: &ParsedRules,
        prev_snapshot: Option<&AssetStateSnapshot>,
        memory_decision: Option<&AssetStrengthDecision>,
    ) -> AssetStateSnapshot {
        let mut reasons = Vec::new();

        let mut recovery_streak = prev_snapshot.map(|s| s.recovery_streak).unwrap_or(0);
        let mut last_defend_age = prev_snapshot.map(|s| s.last_defend_age + 1).unwrap_or(100);
        // 1. Check for forming stage based on trend age or historical data (from roadmap)
        if features.trend_age < 5 {
            return AssetStateSnapshot {
                symbol: features.symbol.clone(),
                state: AssetState::FORMING,
                reasons: vec![format!(
                    "Trend age ({}) < 5 (FORMING protection)",
                    features.trend_age
                )],
                recovery_streak: 0,
                last_defend_age: 100,
            };
        }

        // 2. Logic based on Deviation Bands (from Rules)
        let dev = features.deviation.unwrap_or(0.0);
        let _z = features.z_score.unwrap_or(0.0);

        let mut matched_band = if rules.sorted_bands.is_empty() {
            "cruise".to_string()
        } else {
            // Default to the lowest band (last in sorted_bands) if deviation is very low
            rules.sorted_bands.last().unwrap().0.clone()
        };

        for (name, threshold) in &rules.sorted_bands {
            if dev >= *threshold {
                matched_band = name.clone();
                break;
            }
        }

        // Map band names to internal states
        // Roadmap convention: overheat -> OVERHEAT, optimal -> OPTIMAL, pullback -> PULLBACK, etc.
        let mut state = match matched_band.to_lowercase().as_str() {
            n if n.contains("overheat") => AssetState::OVERHEAT,
            n if n.contains("optimal") => AssetState::OPTIMAL,
            n if n.contains("pullback") => AssetState::PULLBACK,
            n if n.contains("cruise") => AssetState::CRUISE,
            n if n.contains("caution") => AssetState::CAUTION,
            n if n.contains("defend") || n.contains("panic") => AssetState::DEFEND,
            _ => AssetState::CRUISE,
        };

        reasons.push(format!("Matched band: {} (dev: {:.2})", matched_band, dev));

        // 3. Z-Score Calibration (Hardening Finding #3)
        // If z-score is deep negative but deviation is in a 'buying' area, ensure it's PULLBACK
        if _z < -2.0 && (state == AssetState::CRUISE || state == AssetState::OPTIMAL) {
            state = AssetState::PULLBACK;
            reasons.push(format!(
                "Z-Score ({:.2}) indicates extreme exhaustion / PULLBACK opportunity",
                _z
            ));
        } else if _z < -3.0 {
            // Truly extreme drop might still be DEFEND if it breaks structural support
            state = AssetState::DEFEND;
            reasons.push(format!("Z-Score ({:.2}) extreme structural break", _z));
        }

        let prev_state = prev_snapshot
            .map(|s| s.state)
            .unwrap_or(AssetState::FORMING);
        let mut next_state = state;

        // 3. Inertia & Step-up Cooldown (only for recovery)
        if Self::is_recovery(prev_state, next_state) {
            recovery_streak += 1;
            last_defend_age = 100; // Reset defend age on recovery sign? Or keep it?
            if !Self::is_recovery_allowed(prev_state, next_state, recovery_streak) {
                let clamped_state = Self::step_recovery(prev_state);
                reasons.push(format!("[Inertia:BlockedRecovery] Step-up from {:?} to {:?} blocked by cooldown (streak: {}). Clamped to {:?}", prev_state, next_state, recovery_streak, clamped_state));
                next_state = clamped_state;
            }
        } else {
            recovery_streak = 0;
        }

        // --- 4. Historical Penalty (20-day window) ---
        if last_defend_age < 20 && Self::is_strong_state(next_state) {
            reasons.push(format!(
                "[Inertia:Penalty] Recent DEFEND event (age: {}) caps state at CRUISE.",
                last_defend_age
            ));
            next_state = AssetState::CRUISE;
        }

        // --- 5. Memory Layer: Top Tier Lock & Promotion Cap ---
        if let Some(mem) = memory_decision {
            if let Some(min) = mem.min_state {
                if Self::state_rank(next_state) < Self::state_rank(min) {
                    reasons.push(format!(
                        "[Memory:Lock] Persistent strength (Top Tier Lock) elevates state from {:?} to {:?}",
                        next_state, min
                    ));
                    next_state = min;
                }
            }
            if let Some(max) = mem.max_state {
                if Self::state_rank(next_state) > Self::state_rank(max) {
                    reasons.push(format!(
                        "[Memory:Cap] Historical weakness (Promotion Cap) restricts state from {:?} to {:?}",
                        next_state, max
                    ));
                    next_state = max;
                }
            }

            // --- 6. State Transition Friction ---
            if mem.downgrade_friction
                && Self::state_rank(next_state) < Self::state_rank(AssetState::OPTIMAL)
            {
                reasons.push(
                    "[Friction:Hold] Strong asset protected; needs 2d failure (1/2). State held at OPTIMAL.".to_string()
                );
                next_state = AssetState::OPTIMAL;
            }

            if mem.upgrade_friction && next_state == AssetState::OPTIMAL {
                reasons.push(
                    "[Friction:Block] Upgrade to OPTIMAL delayed; needs 3d success total. Capped at CRUISE.".to_string()
                );
                next_state = AssetState::CRUISE;
            }

            // Sync reasons from memory layer
            for r in &mem.reasons {
                reasons.push(format!("[Memory:Context] {}", r));
            }
        }

        AssetStateSnapshot {
            symbol: features.symbol.clone(),
            state: next_state,
            reasons,
            recovery_streak,
            last_defend_age,
        }
    }

    pub fn compute_asset_strength_memory(
        symbol: &str,
        history: &[DecisionPacket],
        rules: &ParsedRules,
    ) -> AssetStrengthMemory {
        let mut top3_days_last_10 = 0;
        let mut top5_days_last_10 = 0;
        let mut defend_days_last_20 = 0;
        let mut below_cruise_days_last_20 = 0;
        let mut rank_sum = 0.0;
        let mut rank_count = 0;

        let optimal_threshold = rules
            .sorted_bands
            .iter()
            .find(|(name, _)| name.to_lowercase().contains("optimal"))
            .map(|(_, t)| *t)
            .unwrap_or(f64::MAX);

        let mut consecutive_optimal_signal_history = 0;
        let mut consecutive_non_optimal_signal_history = 0;
        let mut signal_break_optimal = false;
        let mut signal_break_non_optimal = false;

        // history is oldest first, so we reverse to iterate from latest
        for (i, packet) in history.iter().rev().enumerate() {
            if let Some((rank, asset_decision)) = packet
                .assets
                .iter()
                .enumerate()
                .find(|(_, a)| a.symbol == symbol)
            {
                let dev = asset_decision.deviation.unwrap_or(0.0);
                let is_opt_signal = dev >= optimal_threshold;

                // Friction counters (consecutive from latest historical entry)
                if is_opt_signal && !signal_break_optimal {
                    consecutive_optimal_signal_history += 1;
                } else {
                    signal_break_optimal = true;
                }

                if !is_opt_signal && !signal_break_non_optimal {
                    consecutive_non_optimal_signal_history += 1;
                } else {
                    signal_break_non_optimal = true;
                }

                if i < 10 {
                    if rank < 3 {
                        top3_days_last_10 += 1;
                    }
                    if rank < 5 {
                        top5_days_last_10 += 1;
                    }
                    rank_sum += (rank + 1) as f64;
                    rank_count += 1;
                }
                if i < 20 {
                    if asset_decision.asset_state.state == AssetState::DEFEND {
                        defend_days_last_20 += 1;
                    }
                    if Self::state_rank(asset_decision.asset_state.state)
                        < Self::state_rank(AssetState::CRUISE)
                    {
                        below_cruise_days_last_20 += 1;
                    }
                }
            } else {
                // If asset not found in that day's history, break consecutive counters
                signal_break_optimal = true;
                signal_break_non_optimal = true;
            }
        }

        let rolling_strength_rank = if rank_count > 0 {
            rank_sum / rank_count as f64
        } else {
            10.0 // Default to a middle-to-low rank if no history
        };

        AssetStrengthMemory {
            symbol: symbol.to_string(),
            top3_days_last_10,
            top5_days_last_10,
            defend_days_last_20,
            below_cruise_days_last_20,
            rolling_strength_rank,
            top_tier_locked: top3_days_last_10 >= 6,
            promotion_capped: defend_days_last_20 > 0 || below_cruise_days_last_20 > 10,
            consecutive_optimal_signal_history,
            consecutive_non_optimal_signal_history,
        }
    }

    pub fn build_asset_strength_decision(
        current: &AssetFeatures,
        memory: &AssetStrengthMemory,
        prev_state: Option<AssetState>,
        rules: &ParsedRules,
    ) -> AssetStrengthDecision {
        let mut reasons = Vec::new();
        let mut min_state = None;
        let mut max_state = None;

        let raw_score = current.deviation.unwrap_or(0.0);
        // rolling_rank_score: 1.0 rank -> 100 points, 10.0 rank -> 0 points.
        let memory_score = (100.0 - (memory.rolling_strength_rank - 1.0) * 10.0).clamp(0.0, 100.0);
        let adjusted_score = raw_score * 0.7 + memory_score * 0.3;

        if memory.top_tier_locked {
            min_state = Some(AssetState::CRUISE);
            reasons.push(format!(
                "Top Tier Lock: {}/10 days in Top 3",
                memory.top3_days_last_10
            ));
        }

        if memory.promotion_capped {
            max_state = Some(AssetState::CRUISE);
            if memory.defend_days_last_20 > 0 {
                reasons.push(format!(
                    "Promotion Cap: {} DEFEND events in last 20d",
                    memory.defend_days_last_20
                ));
            } else {
                reasons.push(format!(
                    "Promotion Cap: {}/20 days below CRUISE",
                    memory.below_cruise_days_last_20
                ));
            }
        }

        // --- State Transition Friction ---
        let optimal_threshold = rules
            .sorted_bands
            .iter()
            .find(|(name, _)| name.to_lowercase().contains("optimal"))
            .map(|(_, t)| *t)
            .unwrap_or(f64::MAX);

        let current_is_opt = raw_score >= optimal_threshold;
        let mut downgrade_friction = false;
        let mut upgrade_friction = false;

        if let Some(prev) = prev_state {
            // Downgrade Friction: OPTIMAL -> < OPTIMAL needs 2d failure
            if prev == AssetState::OPTIMAL
                && !current_is_opt
                && memory.consecutive_non_optimal_signal_history == 0
            {
                // This is the 1st day of failing optimal threshold
                downgrade_friction = true;
                // Reason will be handled in compute_state for visibility
            }

            // Upgrade Friction: <= CRUISE -> OPTIMAL needs 3d success
            if Self::state_rank(prev) <= Self::state_rank(AssetState::CRUISE)
                && current_is_opt
                && memory.consecutive_optimal_signal_history < 2
            {
                // History has 0 or 1 consecutive optimal days. Today being success makes it 1 or 2 total.
                // We need '>= 2' in history (so today is the 3rd) to allow it.
                upgrade_friction = true;
                // Reason will be handled in compute_state for visibility
            }
        }

        AssetStrengthDecision {
            symbol: memory.symbol.clone(),
            raw_score,
            memory_score,
            adjusted_score,
            min_state,
            max_state,
            downgrade_friction,
            upgrade_friction,
            reasons,
        }
    }

    pub fn rank_assets_with_memory(
        assets: &[AssetFeatures],
        decisions: &std::collections::HashMap<String, AssetStrengthDecision>,
    ) -> Vec<String> {
        let mut ranked: Vec<(&String, f64)> = assets
            .iter()
            .map(|f| {
                let score = decisions
                    .get(&f.symbol)
                    .map(|d| d.adjusted_score)
                    .unwrap_or_else(|| f.deviation.unwrap_or(0.0));
                (&f.symbol, score)
            })
            .collect();

        // Sort by adjusted_score descending
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        ranked.into_iter().map(|(s, _)| s.clone()).collect()
    }

    fn is_improvement(from: AssetState, to: AssetState) -> bool {
        Self::state_rank(to) > Self::state_rank(from)
    }

    fn is_recovery(from: AssetState, to: AssetState) -> bool {
        Self::is_improvement(from, to)
            && (from == AssetState::DEFEND || from == AssetState::CAUTION)
    }

    fn is_strong_state(s: AssetState) -> bool {
        s == AssetState::OPTIMAL || s == AssetState::PULLBACK
    }

    fn is_recovery_allowed(from: AssetState, to: AssetState, streak: usize) -> bool {
        // DEFEND -> CAUTION needs 3 days
        if from == AssetState::DEFEND && streak < 3 {
            return false;
        }
        // CAUTION -> CRUISE needs 2 days (as per final spec)
        if from == AssetState::CAUTION && streak < 2 {
            return false;
        }
        // CRUISE -> OPTIMAL is handled by standard logic but could add more here

        // General: forbid skipping levels
        let rank_diff = Self::state_rank(to) as i32 - Self::state_rank(from) as i32;
        rank_diff <= 1
    }

    fn step_recovery(from: AssetState) -> AssetState {
        match from {
            AssetState::DEFEND => AssetState::CAUTION,
            AssetState::CAUTION => AssetState::CRUISE,
            AssetState::CRUISE => AssetState::OPTIMAL,
            _ => from,
        }
    }

    pub fn state_rank(s: AssetState) -> usize {
        match s {
            AssetState::DEFEND => 0,
            AssetState::CAUTION => 1,
            AssetState::CRUISE => 2,
            AssetState::PULLBACK => 3,
            AssetState::OPTIMAL => 4,
            AssetState::OVERHEAT => 5,
            AssetState::FORMING => 0, // Forming is like base state
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::features::TrendStatus;
    use chrono::Utc;

    fn mock_asset_features(symbol: &str, trend_age: usize, z: f64, dev: f64) -> AssetFeatures {
        AssetFeatures {
            symbol: symbol.to_string(),
            date: Utc::now().date_naive(),
            close: 100.0,
            owner_ma: Some(100.0),
            leash_ma: Some(100.0),
            deviation: Some(dev),
            z_score: Some(z),
            slope: Some(0.0),
            curvature: Some(0.0),
            trend_status: TrendStatus::Up,
            trend_age,
            deviation_percentile: None,
            weight: 1.0,
        }
    }

    #[test]
    fn test_asset_state_forming() {
        let rules = crate::config::ParsedRules {
            trend: crate::config::TrendConfig {
                lookback_days: 0,
                flat_threshold_pct: 0.0,
            },
            sorted_bands: vec![],
            actions: std::collections::HashMap::new(),
            sizing_multipliers: None,
            core_assets: Vec::new(),
            inertia: crate::config::ParsedInertia {
                min_state_duration: 3,
                trend_dominant_min_confidence: 55.0,
                core_breakdown_k: 2,
                core_breakdown_avg_deviation: -5.0,
                core_breakdown_breadth_floor: 0.0,
            },
        };

        let f = mock_asset_features("AAPL", 2, 0.0, 0.0);
        let s = AssetStateMachine::compute_state(&f, &rules, None, None);
        assert_eq!(s.state, AssetState::FORMING);
    }

    #[test]
    fn test_asset_state_optimal() {
        let rules = crate::config::ParsedRules {
            trend: crate::config::TrendConfig {
                lookback_days: 0,
                flat_threshold_pct: 0.0,
            },
            sorted_bands: vec![("optimal".to_string(), 5.0), ("cruise".to_string(), -5.0)],
            actions: std::collections::HashMap::new(),
            sizing_multipliers: None,
            core_assets: Vec::new(),
            inertia: crate::config::ParsedInertia {
                min_state_duration: 3,
                trend_dominant_min_confidence: 55.0,
                core_breakdown_k: 2,
                core_breakdown_avg_deviation: -5.0,
                core_breakdown_breadth_floor: 0.0,
            },
        };

        let f = mock_asset_features("AAPL", 10, 0.5, 6.0);
        let prev = AssetStateSnapshot {
            symbol: "AAPL".to_string(),
            state: AssetState::CRUISE,
            reasons: vec![],
            recovery_streak: 5,
            last_defend_age: 100,
        };
        let s = AssetStateMachine::compute_state(&f, &rules, Some(&prev), None);
        assert_eq!(s.state, AssetState::OPTIMAL);
    }

    #[test]
    fn test_memory_layer_top_tier_lock() {
        use crate::core::action_matrix::{AssetAction, AssetActionDecision};
        use crate::core::decision::TelegramOutput;
        use crate::core::market_regime::{
            LifecycleState, MarketRegimeSnapshot, MarketState, RiskOverlay,
        };
        use crate::core::portfolio_policy::{PortfolioPolicy, RiskAssetsMode};

        // Use bands that would normally result in DEFEND for dev -10.0
        let rules = crate::config::ParsedRules {
            sorted_bands: vec![
                ("OPTIMAL".to_string(), 5.0),
                ("CRUISE".to_string(), 0.0),
                ("DEFEND".to_string(), -5.0),
            ],
            ..Default::default()
        };
        let f = mock_asset_features("NVDA", 10, 0.0, -10.0); // dev -10.0 -> would be DEFEND

        // Mock history: 10 days of being Top 1 (index 0) with dev 10.0
        let mut history = Vec::new();
        for _ in 0..10 {
            let asset_dec = AssetActionDecision {
                symbol: "NVDA".to_string(),
                price: 100.0,
                asset_state: AssetStateSnapshot {
                    symbol: "NVDA".to_string(),
                    state: AssetState::OPTIMAL,
                    reasons: vec![],
                    recovery_streak: 5,
                    last_defend_age: 100,
                },
                action: AssetAction::HOLD,
                reasons: vec![],
                deviation: Some(10.0),
                z_score: Some(1.0),
                trade_enabled: true,
                trade_amount: 1000.0,
                config_multiplier: 1.0,
                prev_action: None,
                action_changed: false,
            };
            let packet = DecisionPacket {
                date: Utc::now().date_naive(),
                market_features: Default::default(),
                market_regime: MarketRegimeSnapshot {
                    market_state: MarketState::ESTABLISHED,
                    lifecycle_state: LifecycleState::ESTABLISHED,
                    risk_overlay: RiskOverlay::NORMAL,
                    reasons: vec![],
                    low_stability_streak: 0,
                    duration_in_state: 1,
                    transition_audit: None,
                },
                portfolio_policy: PortfolioPolicy {
                    target_exposure_min: 0.1,
                    target_exposure_max: 0.3,
                    allow_chase: false,
                    allow_pullback_buy: true,
                    allow_new_risk: true,
                    risk_assets_mode: RiskAssetsMode::NEUTRAL,
                },
                assets: vec![asset_dec], // NVDA is at index 0 (Top 1)
                participation: Default::default(),
                top_tier_symbols: Vec::new(),
                telegram: TelegramOutput {
                    headline: "".to_string(),
                    summary: "".to_string(),
                    bias: "".to_string(),
                },
            };
            history.push(packet);
        }

        let mem = AssetStateMachine::compute_asset_strength_memory("NVDA", &history, &rules);
        assert!(mem.top_tier_locked);
        assert_eq!(mem.top3_days_last_10, 10);

        let decision = AssetStateMachine::build_asset_strength_decision(
            &f,
            &mem,
            Some(AssetState::OPTIMAL),
            &rules,
        );
        assert_eq!(decision.min_state, Some(AssetState::CRUISE));

        let s = AssetStateMachine::compute_state(&f, &rules, None, Some(&decision));
        // It should be held at OPTIMAL by downgrade friction (first day of failure)
        assert_eq!(s.state, AssetState::OPTIMAL);
        assert!(s.reasons.iter().any(|r| r.contains("Friction:Hold")));
    }

    #[test]
    fn test_memory_layer_promotion_cap() {
        use crate::core::action_matrix::{AssetAction, AssetActionDecision};
        use crate::core::decision::TelegramOutput;
        use crate::core::portfolio_policy::{PortfolioPolicy, RiskAssetsMode};
        // Use bands where dev 10.0 -> OPTIMAL
        let rules = crate::config::ParsedRules {
            sorted_bands: vec![
                ("OPTIMAL".to_string(), 5.0),
                ("CRUISE".to_string(), 0.0),
                ("DEFEND".to_string(), -5.0),
            ],
            ..Default::default()
        };

        // Current state: dev 10.0 -> OPTIMAL
        let f = mock_asset_features("MEME", 10, 0.0, 10.0);

        // Mock history: 1 DEFEND in last 20 days
        let mut history = Vec::new();
        let asset_dec = AssetActionDecision {
            symbol: "MEME".to_string(),
            price: 50.0,
            asset_state: AssetStateSnapshot {
                symbol: "MEME".to_string(),
                state: AssetState::DEFEND,
                reasons: vec![],
                recovery_streak: 0,
                last_defend_age: 0,
            },
            action: AssetAction::HOLD,
            reasons: vec![],
            deviation: Some(-10.0),
            z_score: Some(-2.5),
            trade_enabled: true,
            trade_amount: 1000.0,
            config_multiplier: 1.0,
            prev_action: None,
            action_changed: false,
        };
        let packet = DecisionPacket {
            date: Utc::now().date_naive(),
            market_features: Default::default(),
            market_regime: Default::default(),
            portfolio_policy: PortfolioPolicy {
                target_exposure_min: 0.1,
                target_exposure_max: 0.3,
                allow_chase: false,
                allow_pullback_buy: true,
                allow_new_risk: true,
                risk_assets_mode: RiskAssetsMode::NEUTRAL,
            },
            assets: vec![asset_dec],
            participation: Default::default(),
            top_tier_symbols: Vec::new(),
            telegram: TelegramOutput {
                headline: "".to_string(),
                summary: "".to_string(),
                bias: "".to_string(),
            },
        };
        history.push(packet);

        let mem = AssetStateMachine::compute_asset_strength_memory("MEME", &history, &rules);
        assert!(mem.promotion_capped);

        let decision = AssetStateMachine::build_asset_strength_decision(&f, &mem, None, &rules);
        assert_eq!(decision.max_state, Some(AssetState::CRUISE));

        let s = AssetStateMachine::compute_state(&f, &rules, None, Some(&decision));
        // It shouldn't be above CRUISE (it would be OPTIMAL without memory)
        assert_eq!(s.state, AssetState::CRUISE);
        assert!(s.reasons.iter().any(|r| r.contains("Promotion Cap")));
    }

    #[test]
    fn test_friction_downgrade_delay() {
        use crate::core::action_matrix::AssetActionDecision;
        let rules = crate::config::ParsedRules {
            sorted_bands: vec![("OPTIMAL".to_string(), 5.0), ("CRUISE".to_string(), 0.0)],
            ..Default::default()
        };

        let f = mock_asset_features("STABLE", 10, 0.0, -10.0); // dev -10.0 -> would be CRUISE or below

        // History: Yesterday was dev 10.0 (OPTIMAL)
        let mut history = Vec::new();
        let asset_dec = AssetActionDecision {
            symbol: "STABLE".to_string(),
            price: 100.0,
            asset_state: AssetStateSnapshot {
                symbol: "STABLE".to_string(),
                state: AssetState::OPTIMAL,
                ..Default::default()
            },
            action: crate::core::action_matrix::AssetAction::HOLD,
            deviation: Some(10.0), // Meets OPTIMAL
            ..Default::default()
        };
        let packet = DecisionPacket {
            assets: vec![asset_dec],
            ..Default::default()
        };
        history.push(packet);

        let mem = AssetStateMachine::compute_asset_strength_memory("STABLE", &history, &rules);
        assert_eq!(mem.consecutive_optimal_signal_history, 1);
        assert_eq!(mem.consecutive_non_optimal_signal_history, 0);

        let decision = AssetStateMachine::build_asset_strength_decision(
            &f,
            &mem,
            Some(AssetState::OPTIMAL),
            &rules,
        );
        assert!(decision.downgrade_friction);

        let s = AssetStateMachine::compute_state(
            &f,
            &rules,
            Some(&AssetStateSnapshot {
                symbol: "STABLE".to_string(),
                state: AssetState::OPTIMAL,
                ..Default::default()
            }),
            Some(&decision),
        );

        assert_eq!(s.state, AssetState::OPTIMAL);
        assert!(s.reasons.iter().any(|r| r.contains("Friction:Hold")));
    }

    #[test]
    fn test_friction_upgrade_delay() {
        use crate::core::action_matrix::AssetActionDecision;
        let rules = crate::config::ParsedRules {
            sorted_bands: vec![("OPTIMAL".to_string(), 5.0), ("CRUISE".to_string(), 0.0)],
            ..Default::default()
        };

        let f = mock_asset_features("MOVER", 10, 0.0, 10.0); // dev 10.0 -> would be OPTIMAL

        // History: 1 day of success (yesterday)
        let mut history = Vec::new();
        let asset_dec = AssetActionDecision {
            symbol: "MOVER".to_string(),
            price: 100.0,
            asset_state: AssetStateSnapshot {
                symbol: "MOVER".to_string(),
                state: AssetState::CRUISE, // Was held back by friction yesterday too
                ..Default::default()
            },
            action: crate::core::action_matrix::AssetAction::HOLD,
            deviation: Some(10.0), // Met threshold yesterday too (Day 1)
            ..Default::default()
        };
        let packet = DecisionPacket {
            assets: vec![asset_dec],
            ..Default::default()
        };
        history.push(packet);

        let mem = AssetStateMachine::compute_asset_strength_memory("MOVER", &history, &rules);
        assert_eq!(mem.consecutive_optimal_signal_history, 1);

        let decision = AssetStateMachine::build_asset_strength_decision(
            &f,
            &mem,
            Some(AssetState::CRUISE),
            &rules,
        );
        assert!(decision.upgrade_friction);

        let s = AssetStateMachine::compute_state(
            &f,
            &rules,
            Some(&AssetStateSnapshot {
                symbol: "MOVER".to_string(),
                state: AssetState::CRUISE,
                last_defend_age: 100,
                ..Default::default()
            }),
            Some(&decision),
        );

        assert_eq!(s.state, AssetState::CRUISE);
        assert!(s.reasons.iter().any(|r| r.contains("Friction:Block")));

        // Now test Day 3 (2 days of success in history)
        let mut history_v2 = history.clone();
        let asset_dec_2 = AssetActionDecision {
            symbol: "MOVER".to_string(),
            price: 100.0,
            asset_state: AssetStateSnapshot {
                symbol: "MOVER".to_string(),
                state: AssetState::CRUISE, // Still held back
                ..Default::default()
            },
            action: crate::core::action_matrix::AssetAction::HOLD,
            deviation: Some(10.0), // Met threshold again (Day 2)
            ..Default::default()
        };
        let packet_2 = DecisionPacket {
            assets: vec![asset_dec_2],
            ..Default::default()
        };
        history_v2.push(packet_2);

        let mem_v2 = AssetStateMachine::compute_asset_strength_memory("MOVER", &history_v2, &rules);
        assert_eq!(mem_v2.consecutive_optimal_signal_history, 2);

        let decision_v2 = AssetStateMachine::build_asset_strength_decision(
            &f,
            &mem_v2,
            Some(AssetState::CRUISE),
            &rules,
        );
        assert!(!decision_v2.upgrade_friction);

        let s_v2 = AssetStateMachine::compute_state(
            &f,
            &rules,
            Some(&AssetStateSnapshot {
                symbol: "MOVER".to_string(),
                state: AssetState::CRUISE,
                last_defend_age: 100,
                ..Default::default()
            }),
            Some(&decision_v2),
        );

        assert_eq!(s_v2.state, AssetState::OPTIMAL);
    }
}

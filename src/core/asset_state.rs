use crate::config::ParsedRules;
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

pub struct AssetStateMachine;

impl AssetStateMachine {
    pub fn compute_state(
        features: &AssetFeatures,
        rules: &ParsedRules,
        prev_snapshot: Option<&AssetStateSnapshot>,
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
        let z = features.z_score.unwrap_or(0.0);

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
            n if n.contains("caution") => AssetState::CAUTION,
            n if n.contains("defend") || n.contains("panic") => AssetState::DEFEND,
            _ => AssetState::CRUISE,
        };

        reasons.push(format!("Matched band: {} (dev: {:.2})", matched_band, dev));

        // 3. Z-Score Calibration (Hardening Finding #3)
        // If z-score is deep negative but deviation is in a 'buying' area, ensure it's PULLBACK
        if z < -2.0 && (state == AssetState::CRUISE || state == AssetState::OPTIMAL) {
            state = AssetState::PULLBACK;
            reasons.push(format!(
                "Z-Score ({:.2}) indicates extreme exhaustion / PULLBACK opportunity",
                z
            ));
        } else if z < -3.0 {
            // Truly extreme drop might still be DEFEND if it breaks structural support
            state = AssetState::DEFEND;
            reasons.push(format!("Z-Score ({:.2}) extreme structural break", z));
        }

        // --- 4. Inertia Layer: Stepped Recovery & Historical Penalty ---
        let mut next_state = state;
        let prev_state = prev_snapshot
            .map(|s| s.state)
            .unwrap_or(AssetState::FORMING);

        if next_state == AssetState::DEFEND {
            last_defend_age = 0;
            recovery_streak = 0;
        } else {
            // If we are recovering, increment streak
            if Self::is_improvement(prev_state, next_state) {
                recovery_streak += 1;
            } else if next_state == prev_state {
                // Stay at same state, streak continues if it's not a peak state
                recovery_streak += 1;
            } else {
                recovery_streak = 0;
            }
        }

        // Stepped Recovery Gate
        if Self::is_recovery(prev_state, next_state) {
            let allowed_recovery =
                Self::is_recovery_allowed(prev_state, next_state, recovery_streak);
            if !allowed_recovery {
                let clamped_state = Self::step_recovery(prev_state);
                reasons.push(format!("[Inertia:BlockedRecovery] Step-up from {:?} to {:?} blocked by cooldown (streak: {}). Clamped to {:?}", prev_state, next_state, recovery_streak, clamped_state));
                next_state = clamped_state;
            }
        }

        // Historical Penalty (20-day window)
        if last_defend_age < 20 && Self::is_strong_state(next_state) {
            reasons.push(format!(
                "[Inertia:Penalty] Recent DEFEND event (age: {}) caps state at CRUISE.",
                last_defend_age
            ));
            next_state = AssetState::CRUISE;
        }

        AssetStateSnapshot {
            symbol: features.symbol.clone(),
            state: next_state,
            reasons,
            recovery_streak,
            last_defend_age,
        }
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

    fn state_rank(s: AssetState) -> usize {
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
        let s = AssetStateMachine::compute_state(&f, &rules, None);
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
        let s = AssetStateMachine::compute_state(&f, &rules, Some(&prev));
        assert_eq!(s.state, AssetState::OPTIMAL);
    }
}

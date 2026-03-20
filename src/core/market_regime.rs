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
}

pub struct MarketRegimeStateMachine {
    pub current_lifecycle: LifecycleState,
    #[allow(dead_code)]
    pub current_risk: RiskOverlay,
}

impl MarketRegimeStateMachine {
    /// Computes the next regime snapshot and calibrated age based on previous context.
    pub fn transition(
        prev_snapshot: Option<&MarketRegimeSnapshot>,
        features: &mut MarketFeatures,
        prev_age: usize,
    ) -> (MarketRegimeSnapshot, usize) {
        let initial_lifecycle = prev_snapshot
            .map(|s| s.lifecycle_state)
            .unwrap_or(LifecycleState::NONE);
        let initial_risk = prev_snapshot
            .map(|s| s.risk_overlay)
            .unwrap_or(RiskOverlay::NORMAL);

        let sm = Self::new(initial_lifecycle, initial_risk);

        // Hardening: Use current_potential_age (the age it WOULD be today if state remains)
        // for transition logic to eliminate the 1-day lag in backtests.
        let current_potential_age = prev_age + 1;

        // CRITICAL: Recalibrate maturity metrics based on potential age BEFORE decision
        // to ensure transitions like ESTABLISHED -> CONFIRMED happen on the correct day.
        features.recalibrate(current_potential_age);
        let next_snapshot = sm.compute_next_state(features, current_potential_age);

        let next_age = if let Some(ps) = prev_snapshot {
            if ps.market_state == next_snapshot.market_state {
                current_potential_age
            } else {
                1
            }
        } else {
            1
        };

        // Final recalibration if state changed (age reset to 1)
        // or just to ensure total consistency for the final packet.
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
    ) -> MarketRegimeSnapshot {
        let mut reasons = Vec::new();
        let mut next_lifecycle = self.current_lifecycle;
        let mut next_risk = RiskOverlay::NORMAL;

        // --- 1. Risk Overlay Logic (Downgrade priority) ---
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

        // --- 2. Lifecycle Progression Logic ---
        match self.current_lifecycle {
            LifecycleState::NONE => {
                if features.stability_structural > 0.0 {
                    next_lifecycle = LifecycleState::IGNITION;
                    reasons.push("Structural stability detected".to_string());
                }
            }
            LifecycleState::IGNITION => {
                if features.system_confidence >= 60.0 && regime_age >= 5 {
                    next_lifecycle = LifecycleState::NEWBORN;
                    reasons.push("Confidence >= 60 and age >= 5".to_string());
                }
            }
            LifecycleState::NEWBORN => {
                if features.system_confidence >= 70.0
                    && regime_age >= 10
                    && features.any_pullback_occurred
                {
                    next_lifecycle = LifecycleState::EARLY_CONFIRMATION;
                    reasons.push(
                        "Confidence >= 70, age >= 10, and experienced successful pullback"
                            .to_string(),
                    );
                }
            }

            LifecycleState::EARLY_CONFIRMATION => {
                if features.system_confidence >= 80.0 && regime_age >= 20 {
                    next_lifecycle = LifecycleState::ESTABLISHED;
                    reasons.push("Confidence >= 80 and age >= 20".to_string());
                }
            }
            LifecycleState::ESTABLISHED => {
                if features.trend_maturity > 0.8 {
                    next_lifecycle = LifecycleState::CONFIRMED;
                    reasons.push("Trend maturity > 0.8".to_string());
                }
            }
            LifecycleState::CONFIRMED => {
                // Stay or potentially shift to DEFENSIVE risk overlay
            }
        }

        // --- 3. Composite State Mapping (Section 4.2 in Roadmap) ---
        let market_state = match next_risk {
            RiskOverlay::DEFENSIVE | RiskOverlay::BROKEN => MarketState::DEFENSIVE,
            RiskOverlay::NORMAL | RiskOverlay::DECELERATING => {
                match next_lifecycle {
                    LifecycleState::NONE => MarketState::DEFENSIVE, // Fallback
                    LifecycleState::IGNITION => MarketState::IGNITION,
                    LifecycleState::NEWBORN => MarketState::NEWBORN,
                    LifecycleState::EARLY_CONFIRMATION => MarketState::EARLY_CONFIRMATION,
                    LifecycleState::ESTABLISHED => MarketState::ESTABLISHED,
                    LifecycleState::CONFIRMED => MarketState::CONFIRMED,
                }
            }
        };

        MarketRegimeSnapshot {
            market_state,
            lifecycle_state: next_lifecycle,
            risk_overlay: next_risk,
            reasons,
        }
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
            stability_score: 0.0,
            stability_structural: stability,
            stability_temporal: 0.0,
            trend_maturity: maturity,
            universe_integrity: 0.0,
            flow_acceleration: None,
            regime_age: 1,
            any_pullback_occurred: false,
        }
    }

    #[test]
    fn test_lifecycle_progression() {
        let sm = MarketRegimeStateMachine::new(LifecycleState::NONE, RiskOverlay::NORMAL);
        let f1 = mock_features(55.0, 0.5, 0.1);
        let s1 = sm.compute_next_state(&f1, 0);
        assert_eq!(s1.lifecycle_state, LifecycleState::IGNITION);

        let sm2 = MarketRegimeStateMachine::new(LifecycleState::IGNITION, RiskOverlay::NORMAL);
        let f2 = mock_features(65.0, 0.5, 0.1);
        let s2 = sm2.compute_next_state(&f2, 6);
        assert_eq!(s2.lifecycle_state, LifecycleState::NEWBORN);
    }

    #[test]
    fn test_defensive_downgrade() {
        let sm = MarketRegimeStateMachine::new(LifecycleState::ESTABLISHED, RiskOverlay::NORMAL);
        let f = mock_features(40.0, 0.5, 0.5);
        let s = sm.compute_next_state(&f, 30);
        assert_eq!(s.market_state, MarketState::DEFENSIVE);
        assert_eq!(s.risk_overlay, RiskOverlay::DEFENSIVE);
        assert!(s
            .reasons
            .iter()
            .any(|r| r.contains("Confidence (40.0) < 50")));
    }
}

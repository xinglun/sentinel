use crate::core::features::AssetFeatures;
use crate::config::ParsedRules;
use serde::{Deserialize, Serialize};

#[allow(non_camel_case_types)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AssetState {
    OPTIMAL,
    CRUISE,
    PULLBACK,
    CAUTION,
    OVERHEAT,
    DEFEND,
    FORMING,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssetStateSnapshot {
    pub symbol: String,
    pub state: AssetState,
    pub reasons: Vec<String>,
}

pub struct AssetStateMachine;

impl AssetStateMachine {
    pub fn compute_state(
        features: &AssetFeatures,
        rules: &ParsedRules,
    ) -> AssetStateSnapshot {
        let mut reasons = Vec::new();
        // 1. Check for forming stage based on trend age or historical data (from roadmap)
        if features.trend_age < 5 {
            reasons.push(format!("Trend age ({}) < 5", features.trend_age));
            return AssetStateSnapshot { symbol: features.symbol.clone(), state: AssetState::FORMING, reasons };
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
             reasons.push(format!("Z-Score ({:.2}) indicates extreme exhaustion / PULLBACK opportunity", z));
        } else if z < -3.0 {
             // Truly extreme drop might still be DEFEND if it breaks structural support
             state = AssetState::DEFEND;
             reasons.push(format!("Z-Score ({:.2}) extreme structural break", z));
        }


        // TODO: Integrate more complex rules from engine.rs if needed (bear mode confirm etc).
        // For now, this follows the core specification for the transition to Phase 4.

        AssetStateSnapshot {
            symbol: features.symbol.clone(),
            state,
            reasons,
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
            trend: crate::config::TrendConfig { lookback_days: 0, flat_threshold_pct: 0.0 },
            sorted_bands: vec![],
            actions: std::collections::HashMap::new(),
            sizing_multipliers: None,
        };




        let f = mock_asset_features("AAPL", 2, 0.0, 0.0);
        let s = AssetStateMachine::compute_state(&f, &rules);
        assert_eq!(s.state, AssetState::FORMING);
    }

    #[test]
    fn test_asset_state_optimal() {
         let rules = crate::config::ParsedRules {
            trend: crate::config::TrendConfig { lookback_days: 0, flat_threshold_pct: 0.0 },
            sorted_bands: vec![("optimal".to_string(), 5.0), ("cruise".to_string(), -5.0)],
            actions: std::collections::HashMap::new(),
            sizing_multipliers: None,
        };




        let f = mock_asset_features("AAPL", 10, 0.5, 6.0);
        let s = AssetStateMachine::compute_state(&f, &rules);
        assert_eq!(s.state, AssetState::OPTIMAL);
    }

}


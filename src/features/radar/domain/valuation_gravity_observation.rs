use crate::features::research::domain::valuation_gravity::GravityStatus;
use serde::{Deserialize, Serialize};

/// 評価層(Gravity)の方向性を決定パケットへ読み取り専用で写す。
/// Gate、execution、position sizing、decision_weight には一切寄与しない。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValuationGravityObservation {
    pub symbol: String,
    pub gravity: Option<GravityStatus>,
}

impl ValuationGravityObservation {
    /// 強気方向(割安)かどうか。UNAVAILABLE(None)は方向を持たない。
    pub fn is_bullish(&self) -> bool {
        matches!(
            self.gravity,
            Some(GravityStatus::DeepUndervalued) | Some(GravityStatus::Undervalued)
        )
    }

    /// 弱気方向(割高)かどうか。UNAVAILABLE(None)は方向を持たない。
    pub fn is_bearish(&self) -> bool {
        matches!(
            self.gravity,
            Some(GravityStatus::Expensive) | Some(GravityStatus::VeryExpensive)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_gravity_has_no_direction() {
        let observation = ValuationGravityObservation {
            symbol: "GOOG".to_string(),
            gravity: None,
        };
        assert!(!observation.is_bullish());
        assert!(!observation.is_bearish());
    }

    #[test]
    fn deep_undervalued_and_undervalued_are_bullish() {
        for status in [GravityStatus::DeepUndervalued, GravityStatus::Undervalued] {
            let observation = ValuationGravityObservation {
                symbol: "GOOG".to_string(),
                gravity: Some(status),
            };
            assert!(observation.is_bullish());
            assert!(!observation.is_bearish());
        }
    }

    #[test]
    fn expensive_and_very_expensive_are_bearish() {
        for status in [GravityStatus::Expensive, GravityStatus::VeryExpensive] {
            let observation = ValuationGravityObservation {
                symbol: "GOOG".to_string(),
                gravity: Some(status),
            };
            assert!(observation.is_bearish());
            assert!(!observation.is_bullish());
        }
    }

    #[test]
    fn fair_and_slightly_expensive_are_neutral() {
        for status in [GravityStatus::Fair, GravityStatus::SlightlyExpensive] {
            let observation = ValuationGravityObservation {
                symbol: "GOOG".to_string(),
                gravity: Some(status),
            };
            assert!(!observation.is_bullish());
            assert!(!observation.is_bearish());
        }
    }
}

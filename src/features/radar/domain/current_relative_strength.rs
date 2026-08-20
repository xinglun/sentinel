use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelativeStrengthState {
    #[serde(alias = "Strong", alias = "IMPROVING")]
    Improving,
    #[default]
    #[serde(alias = "NEUTRAL")]
    Neutral,
    #[serde(alias = "WEAKENING")]
    Weakening,
}

impl RelativeStrengthState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Improving => "IMPROVING",
            Self::Neutral => "NEUTRAL",
            Self::Weakening => "WEAKENING",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStrength {
    #[serde(alias = "StrongRecovery", alias = "STRONG", alias = "STRONG_RECOVERY")]
    Strong,
    #[serde(alias = "Recovering", alias = "MODERATE", alias = "RECOVERING")]
    Moderate,
    #[serde(alias = "WEAK")]
    Weak,
    #[default]
    #[serde(
        alias = "Neutral",
        alias = "Deteriorating",
        alias = "NONE",
        alias = "DETERIORATING"
    )]
    None,
}

impl RecoveryStrength {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strong => "STRONG",
            Self::Moderate => "MODERATE",
            Self::Weak => "WEAK",
            Self::None => "NONE",
        }
    }
}

const STRONG_RECOVERY_5D_THRESHOLD: f64 = 5.0;
const MODERATE_RECOVERY_5D_THRESHOLD: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentRelativeStrengthObservation {
    pub symbol: String,
    #[serde(default)]
    pub benchmark_symbol: String,
    pub relative_1d_vs_benchmark: Option<f64>,
    pub relative_5d_vs_benchmark: Option<f64>,
    #[serde(default)]
    pub trend_slope: Option<f64>,
    pub price_position: Option<f64>,
    pub volume_participation: Option<f64>,
    #[serde(alias = "status")]
    pub state: RelativeStrengthState,
    #[serde(default)]
    #[serde(alias = "recovery_state")]
    pub recovery_strength: RecoveryStrength,
    pub boundary: String,
}

pub(crate) struct CurrentRelativeStrengthInput {
    pub symbol: String,
    pub benchmark_symbol: String,
    pub relative_1d_vs_benchmark: Option<f64>,
    pub relative_5d_vs_benchmark: Option<f64>,
    pub trend_slope: Option<f64>,
    pub price_position: Option<f64>,
    pub volume_participation: Option<f64>,
}

pub(crate) fn observe_current_relative_strength(
    input: CurrentRelativeStrengthInput,
) -> CurrentRelativeStrengthObservation {
    let improving = input
        .relative_1d_vs_benchmark
        .is_some_and(|value| value > 0.0)
        && input
            .relative_5d_vs_benchmark
            .is_some_and(|value| value > 0.0);
    let weakening = input
        .relative_1d_vs_benchmark
        .is_some_and(|value| value < 0.0)
        && input
            .relative_5d_vs_benchmark
            .is_some_and(|value| value < 0.0);
    let state = if improving {
        RelativeStrengthState::Improving
    } else if weakening {
        RelativeStrengthState::Weakening
    } else {
        RelativeStrengthState::Neutral
    };
    let recovery_strength = derive_recovery_strength(state, input.relative_5d_vs_benchmark);

    CurrentRelativeStrengthObservation {
        symbol: input.symbol,
        benchmark_symbol: input.benchmark_symbol,
        relative_1d_vs_benchmark: input.relative_1d_vs_benchmark,
        relative_5d_vs_benchmark: input.relative_5d_vs_benchmark,
        trend_slope: input.trend_slope,
        price_position: input.price_position,
        volume_participation: input.volume_participation,
        state,
        recovery_strength,
        boundary:
            "Observation only; does not change Leader, Gate, Action Matrix or Position Sizing."
                .to_string(),
    }
}

fn derive_recovery_strength(
    state: RelativeStrengthState,
    relative_5d_vs_benchmark: Option<f64>,
) -> RecoveryStrength {
    if state != RelativeStrengthState::Improving {
        return RecoveryStrength::None;
    }
    let Some(relative_5d) = relative_5d_vs_benchmark else {
        return RecoveryStrength::None;
    };
    if relative_5d >= STRONG_RECOVERY_5D_THRESHOLD {
        RecoveryStrength::Strong
    } else if relative_5d >= MODERATE_RECOVERY_5D_THRESHOLD {
        RecoveryStrength::Moderate
    } else if relative_5d > 0.0 {
        RecoveryStrength::Weak
    } else {
        RecoveryStrength::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_strength_can_be_strong_without_a_confirmed_leader() {
        let observation = observe_current_relative_strength(CurrentRelativeStrengthInput {
            symbol: "NVDA".to_string(),
            benchmark_symbol: "SPY".to_string(),
            relative_1d_vs_benchmark: Some(1.2),
            relative_5d_vs_benchmark: Some(4.5),
            trend_slope: Some(0.5),
            price_position: Some(0.99),
            volume_participation: Some(1.1),
        });

        assert_eq!(observation.state, RelativeStrengthState::Improving);
        assert_eq!(observation.recovery_strength, RecoveryStrength::Moderate);
        assert!(observation.boundary.contains("does not change Leader"));
    }

    #[test]
    fn strong_and_weak_recovery_strength_follow_the_five_day_thresholds() {
        for (symbol, relative_1d, relative_5d, expected_strength) in [
            ("FIG", 4.24, 14.88, RecoveryStrength::Strong),
            ("U", 0.75, 6.43, RecoveryStrength::Strong),
            ("TSLA", 2.52, 6.13, RecoveryStrength::Strong),
            ("PLTR", 0.31, 1.26, RecoveryStrength::Weak),
            ("ISRG", 2.03, 0.11, RecoveryStrength::Weak),
        ] {
            let observation = observe_current_relative_strength(CurrentRelativeStrengthInput {
                symbol: symbol.to_string(),
                benchmark_symbol: "SPY".to_string(),
                relative_1d_vs_benchmark: Some(relative_1d),
                relative_5d_vs_benchmark: Some(relative_5d),
                trend_slope: Some(0.2),
                price_position: None,
                volume_participation: None,
            });

            assert_eq!(observation.state, RelativeStrengthState::Improving);
            assert_eq!(observation.recovery_strength, expected_strength);
        }
    }

    #[test]
    fn weakening_strength_is_not_a_sell_or_leader_transition() {
        let observation = observe_current_relative_strength(CurrentRelativeStrengthInput {
            symbol: "MSFT".to_string(),
            benchmark_symbol: "SPY".to_string(),
            relative_1d_vs_benchmark: Some(-0.4),
            relative_5d_vs_benchmark: Some(-1.5),
            trend_slope: Some(-0.2),
            price_position: Some(0.97),
            volume_participation: Some(0.7),
        });

        assert_eq!(observation.state, RelativeStrengthState::Weakening);
        assert!(!observation.boundary.contains("Sell"));
    }

    #[test]
    fn non_improving_state_has_no_recovery_strength() {
        let observation = observe_current_relative_strength(CurrentRelativeStrengthInput {
            symbol: "SPCX".to_string(),
            benchmark_symbol: "SPY".to_string(),
            relative_1d_vs_benchmark: Some(-1.30),
            relative_5d_vs_benchmark: Some(7.94),
            trend_slope: Some(-0.10),
            price_position: None,
            volume_participation: None,
        });

        assert_eq!(observation.state, RelativeStrengthState::Neutral);
        assert_eq!(observation.recovery_strength, RecoveryStrength::None);
    }

    #[test]
    fn negative_relative_strength_and_negative_slope_are_deteriorating() {
        let observation = observe_current_relative_strength(CurrentRelativeStrengthInput {
            symbol: "WEAK".to_string(),
            benchmark_symbol: "SPY".to_string(),
            relative_1d_vs_benchmark: Some(-1.0),
            relative_5d_vs_benchmark: Some(-4.0),
            trend_slope: Some(-0.20),
            price_position: None,
            volume_participation: None,
        });

        assert_eq!(observation.recovery_strength, RecoveryStrength::None);
    }

    #[test]
    fn modest_positive_relative_strength_is_not_strong_recovery() {
        let observation = observe_current_relative_strength(CurrentRelativeStrengthInput {
            symbol: "PLTR".to_string(),
            benchmark_symbol: "SPY".to_string(),
            relative_1d_vs_benchmark: Some(0.31),
            relative_5d_vs_benchmark: Some(1.26),
            trend_slope: Some(0.20),
            price_position: None,
            volume_participation: None,
        });

        assert_eq!(observation.state, RelativeStrengthState::Improving);
        assert_eq!(observation.recovery_strength, RecoveryStrength::Weak);
    }

    #[test]
    fn legacy_relative_strength_json_remains_readable() {
        let observation: CurrentRelativeStrengthObservation = serde_json::from_str(
            r#"{
                "symbol": "PLTR",
                "relative_1d_vs_benchmark": 0.31,
                "relative_5d_vs_benchmark": 1.26,
                "trend_slope": 0.2,
                "price_position": null,
                "volume_participation": null,
                "status": "Improving",
                "recovery_state": "Neutral",
                "boundary": "Observation only"
            }"#,
        )
        .expect("legacy current relative strength JSON");

        assert_eq!(observation.benchmark_symbol, "");
        assert_eq!(observation.state, RelativeStrengthState::Improving);
        assert_eq!(observation.recovery_strength, RecoveryStrength::None);
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrentRelativeStrengthStatus {
    Improving,
    Strong,
    Neutral,
    Weakening,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelativeStrengthRecoveryState {
    StrongRecovery,
    Recovering,
    #[default]
    Neutral,
    Deteriorating,
}

impl RelativeStrengthRecoveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StrongRecovery => "STRONG_RECOVERY",
            Self::Recovering => "RECOVERING",
            Self::Neutral => "NEUTRAL",
            Self::Deteriorating => "DETERIORATING",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentRelativeStrengthObservation {
    pub symbol: String,
    pub relative_1d_vs_benchmark: Option<f64>,
    pub relative_5d_vs_benchmark: Option<f64>,
    #[serde(default)]
    pub trend_slope: Option<f64>,
    pub price_position: Option<f64>,
    pub volume_participation: Option<f64>,
    pub status: CurrentRelativeStrengthStatus,
    #[serde(default)]
    pub recovery_state: RelativeStrengthRecoveryState,
    pub boundary: String,
}

pub(crate) struct CurrentRelativeStrengthInput {
    pub symbol: String,
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
    let strong = input
        .relative_1d_vs_benchmark
        .is_some_and(|value| value > 0.0)
        && input
            .relative_5d_vs_benchmark
            .is_some_and(|value| value > 0.0)
        && input.price_position.is_some_and(|value| value >= 0.95)
        && input.volume_participation.is_some_and(|value| value >= 1.0);
    let status = if strong {
        CurrentRelativeStrengthStatus::Strong
    } else if improving {
        CurrentRelativeStrengthStatus::Improving
    } else if weakening {
        CurrentRelativeStrengthStatus::Weakening
    } else {
        CurrentRelativeStrengthStatus::Neutral
    };
    let recovery_state = derive_recovery_state(
        input.relative_1d_vs_benchmark,
        input.relative_5d_vs_benchmark,
        input.trend_slope,
    );

    CurrentRelativeStrengthObservation {
        symbol: input.symbol,
        relative_1d_vs_benchmark: input.relative_1d_vs_benchmark,
        relative_5d_vs_benchmark: input.relative_5d_vs_benchmark,
        trend_slope: input.trend_slope,
        price_position: input.price_position,
        volume_participation: input.volume_participation,
        status,
        recovery_state,
        boundary:
            "Observation only; does not change Leader, Gate, Action Matrix or Position Sizing."
                .to_string(),
    }
}

fn derive_recovery_state(
    relative_1d_vs_benchmark: Option<f64>,
    relative_5d_vs_benchmark: Option<f64>,
    trend_slope: Option<f64>,
) -> RelativeStrengthRecoveryState {
    let Some(relative_1d) = relative_1d_vs_benchmark else {
        return RelativeStrengthRecoveryState::Neutral;
    };
    let Some(relative_5d) = relative_5d_vs_benchmark else {
        return RelativeStrengthRecoveryState::Neutral;
    };
    let relative_strength_slope = relative_5d - relative_1d;
    let trend_recovering =
        trend_slope.is_some_and(|value| value > 0.0) || relative_strength_slope > 0.0;

    if relative_1d > 0.0 && relative_5d > 0.0 && trend_recovering {
        RelativeStrengthRecoveryState::StrongRecovery
    } else if relative_5d > 0.0 && trend_recovering {
        RelativeStrengthRecoveryState::Recovering
    } else if relative_1d < 0.0 && relative_5d < 0.0 && trend_slope.is_some_and(|value| value < 0.0)
    {
        RelativeStrengthRecoveryState::Deteriorating
    } else {
        RelativeStrengthRecoveryState::Neutral
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_strength_can_be_strong_without_a_confirmed_leader() {
        let observation = observe_current_relative_strength(CurrentRelativeStrengthInput {
            symbol: "NVDA".to_string(),
            relative_1d_vs_benchmark: Some(1.2),
            relative_5d_vs_benchmark: Some(4.5),
            trend_slope: Some(0.5),
            price_position: Some(0.99),
            volume_participation: Some(1.1),
        });

        assert_eq!(observation.status, CurrentRelativeStrengthStatus::Strong);
        assert!(observation.boundary.contains("does not change Leader"));
    }

    #[test]
    fn weakening_strength_is_not_a_sell_or_leader_transition() {
        let observation = observe_current_relative_strength(CurrentRelativeStrengthInput {
            symbol: "MSFT".to_string(),
            relative_1d_vs_benchmark: Some(-0.4),
            relative_5d_vs_benchmark: Some(-1.5),
            trend_slope: Some(-0.2),
            price_position: Some(0.97),
            volume_participation: Some(0.7),
        });

        assert_eq!(observation.status, CurrentRelativeStrengthStatus::Weakening);
        assert!(!observation.boundary.contains("Sell"));
    }

    #[test]
    fn neutral_classification_can_have_a_recovering_recovery_state() {
        let observation = observe_current_relative_strength(CurrentRelativeStrengthInput {
            symbol: "SPCX".to_string(),
            relative_1d_vs_benchmark: Some(-1.30),
            relative_5d_vs_benchmark: Some(7.94),
            trend_slope: Some(-0.10),
            price_position: None,
            volume_participation: None,
        });

        assert_eq!(observation.status, CurrentRelativeStrengthStatus::Neutral);
        assert_eq!(
            observation.recovery_state,
            RelativeStrengthRecoveryState::Recovering
        );
    }

    #[test]
    fn negative_relative_strength_and_negative_slope_are_deteriorating() {
        let observation = observe_current_relative_strength(CurrentRelativeStrengthInput {
            symbol: "WEAK".to_string(),
            relative_1d_vs_benchmark: Some(-1.0),
            relative_5d_vs_benchmark: Some(-4.0),
            trend_slope: Some(-0.20),
            price_position: None,
            volume_participation: None,
        });

        assert_eq!(
            observation.recovery_state,
            RelativeStrengthRecoveryState::Deteriorating
        );
    }
}

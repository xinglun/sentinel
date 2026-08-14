use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrentRelativeStrengthStatus {
    Improving,
    Strong,
    Neutral,
    Weakening,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentRelativeStrengthObservation {
    pub symbol: String,
    pub relative_1d_vs_benchmark: Option<f64>,
    pub relative_5d_vs_benchmark: Option<f64>,
    pub price_position: Option<f64>,
    pub volume_participation: Option<f64>,
    pub status: CurrentRelativeStrengthStatus,
    pub boundary: String,
}

pub(crate) struct CurrentRelativeStrengthInput {
    pub symbol: String,
    pub relative_1d_vs_benchmark: Option<f64>,
    pub relative_5d_vs_benchmark: Option<f64>,
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

    CurrentRelativeStrengthObservation {
        symbol: input.symbol,
        relative_1d_vs_benchmark: input.relative_1d_vs_benchmark,
        relative_5d_vs_benchmark: input.relative_5d_vs_benchmark,
        price_position: input.price_position,
        volume_participation: input.volume_participation,
        status,
        boundary:
            "Observation only; does not change Leader, Gate, Action Matrix or Position Sizing."
                .to_string(),
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
            price_position: Some(0.97),
            volume_participation: Some(0.7),
        });

        assert_eq!(observation.status, CurrentRelativeStrengthStatus::Weakening);
        assert!(!observation.boundary.contains("Sell"));
    }
}

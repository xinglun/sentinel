use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FlowObservationScope {
    Market,
    Sector,
    Watchlist,
    CoreHolding,
    Asset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FlowDirection {
    Inflow,
    Outflow,
    Mixed,
    Flat,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FlowStrength {
    VeryWeak,
    Weak,
    Neutral,
    Strong,
    VeryStrong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FlowQuality {
    Poor,
    Normal,
    Healthy,
    Excellent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FlowSourceHealth {
    Succeeded,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PriceDirection {
    Up,
    Down,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FlowDivergenceType {
    Positive,
    Negative,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FlowDivergenceSeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FlowBreadthState {
    Supportive,
    Neutral,
    Divergent,
    Stressed,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowObservation {
    pub as_of_date: NaiveDate,
    pub observed_at: NaiveDate,
    pub scope: FlowObservationScope,
    pub subject: String,
    pub provider: String,
    pub source_kind: String,
    pub direction: FlowDirection,
    pub strength: FlowStrength,
    pub quality: FlowQuality,
    pub continuity_days: u16,
    pub net_flow: Option<f64>,
    pub main_net_flow: Option<f64>,
    pub source_health: FlowSourceHealth,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowDivergence {
    pub subject: String,
    pub price_direction: PriceDirection,
    pub flow_direction: FlowDirection,
    pub divergence_type: FlowDivergenceType,
    pub severity: FlowDivergenceSeverity,
    pub explanation_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowBreadth {
    pub market_breadth: FlowBreadthState,
    pub sector_breadth: FlowBreadthState,
    pub watchlist_breadth: FlowBreadthState,
    pub core_holding_breadth: FlowBreadthState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowLayerSnapshot {
    pub as_of_date: NaiveDate,
    pub observations: Vec<FlowObservation>,
    pub divergences: Vec<FlowDivergence>,
    pub breadth: FlowBreadth,
    pub observation_only: bool,
    pub decision_weight_percent: u8,
    pub trend_override_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapitalDynamicsObservation {
    pub as_of_date: NaiveDate,
    pub flow_layer: FlowLayerSnapshot,
}

impl FlowLayerSnapshot {
    // Keep the boundary explicit so future integrations cannot silently turn
    // Flow into a hidden decision input.
    pub fn validate_boundary(&self) -> Result<(), &'static str> {
        if !self.observation_only {
            return Err("flow layer must remain observation-only");
        }
        if self.decision_weight_percent != 0 {
            return Err("flow layer decision weight must remain zero");
        }
        if self.trend_override_allowed {
            return Err("flow layer must not override trend layer");
        }
        for observation in &self.observations {
            if observation.subject.trim().is_empty()
                || observation.provider.trim().is_empty()
                || observation.source_kind.trim().is_empty()
            {
                return Err("flow observation identity is incomplete");
            }
            if observation.observed_at > self.as_of_date || observation.as_of_date > self.as_of_date
            {
                return Err("flow observation date is later than snapshot date");
            }
            if observation.net_flow.is_some_and(|value| !value.is_finite())
                || observation
                    .main_net_flow
                    .is_some_and(|value| !value.is_finite())
            {
                return Err("flow observation contains invalid numeric value");
            }
            if observation.source_health == FlowSourceHealth::Unavailable
                && (observation.net_flow.is_some() || observation.main_net_flow.is_some())
            {
                return Err("unavailable flow observation must not contain flow values");
            }
        }
        for divergence in &self.divergences {
            if divergence.subject.trim().is_empty() || divergence.explanation_key.trim().is_empty()
            {
                return Err("flow divergence identity is incomplete");
            }
            match divergence.divergence_type {
                FlowDivergenceType::Positive | FlowDivergenceType::Negative
                    if matches!(divergence.flow_direction, FlowDirection::Unknown) =>
                {
                    return Err("meaningful divergence must have known flow direction");
                }
                FlowDivergenceType::None => {}
                _ => {}
            }
        }
        Ok(())
    }
}

impl CapitalDynamicsObservation {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.flow_layer.as_of_date != self.as_of_date {
            return Err("capital dynamics snapshot date does not match flow layer date");
        }
        self.flow_layer.validate_boundary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_observation() -> FlowObservation {
        FlowObservation {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
            scope: FlowObservationScope::Asset,
            subject: "NVDA".to_string(),
            provider: "Futu".to_string(),
            source_kind: "CapitalFlow".to_string(),
            direction: FlowDirection::Inflow,
            strength: FlowStrength::Strong,
            quality: FlowQuality::Healthy,
            continuity_days: 5,
            net_flow: Some(12.5),
            main_net_flow: Some(8.2),
            source_health: FlowSourceHealth::Succeeded,
        }
    }

    fn valid_snapshot() -> FlowLayerSnapshot {
        FlowLayerSnapshot {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
            observations: vec![valid_observation()],
            divergences: vec![FlowDivergence {
                subject: "NVDA".to_string(),
                price_direction: PriceDirection::Up,
                flow_direction: FlowDirection::Inflow,
                divergence_type: FlowDivergenceType::None,
                severity: FlowDivergenceSeverity::Low,
                explanation_key: "aligned".to_string(),
            }],
            breadth: FlowBreadth {
                market_breadth: FlowBreadthState::Unavailable,
                sector_breadth: FlowBreadthState::Unavailable,
                watchlist_breadth: FlowBreadthState::Supportive,
                core_holding_breadth: FlowBreadthState::Supportive,
            },
            observation_only: true,
            decision_weight_percent: 0,
            trend_override_allowed: false,
        }
    }

    #[test]
    fn flow_layer_boundary_accepts_observation_only_zero_weight() {
        assert!(valid_snapshot().validate_boundary().is_ok());
    }

    #[test]
    fn flow_layer_boundary_rejects_non_zero_weight() {
        let mut snapshot = valid_snapshot();
        snapshot.decision_weight_percent = 5;
        assert_eq!(
            snapshot.validate_boundary(),
            Err("flow layer decision weight must remain zero")
        );
    }

    #[test]
    fn flow_layer_boundary_rejects_trend_override() {
        let mut snapshot = valid_snapshot();
        snapshot.trend_override_allowed = true;
        assert_eq!(
            snapshot.validate_boundary(),
            Err("flow layer must not override trend layer")
        );
    }

    #[test]
    fn flow_layer_boundary_rejects_unavailable_source_with_values() {
        let mut snapshot = valid_snapshot();
        snapshot.observations[0].source_health = FlowSourceHealth::Unavailable;
        assert_eq!(
            snapshot.validate_boundary(),
            Err("unavailable flow observation must not contain flow values")
        );
    }

    #[test]
    fn capital_dynamics_snapshot_requires_matching_dates() {
        let mut snapshot = valid_snapshot();
        snapshot.as_of_date = NaiveDate::from_ymd_opt(2026, 6, 25).unwrap();
        let observation = CapitalDynamicsObservation {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 26).unwrap(),
            flow_layer: snapshot,
        };
        assert_eq!(
            observation.validate(),
            Err("capital dynamics snapshot date does not match flow layer date")
        );
    }
}

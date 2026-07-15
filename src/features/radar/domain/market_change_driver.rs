use serde::{Deserialize, Serialize};

const LOCAL_RANK_MAX_DELTA: usize = 1;
const LOCAL_SCORE_MAX_DELTA: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeLevel {
    None,
    Minor,
    Moderate,
    Major,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketChangeSnapshot {
    pub primary_leader: String,
    pub breadth_classification: String,
    pub supply_phase: String,
    pub supply_pressure: String,
    pub market_state: String,
    pub risk_state: String,
    pub day_type: String,
    pub confidence: f64,
    pub score: f64,
    pub ranked_leaders: Vec<String>,
    pub breakout_state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketChangeDriver {
    pub change_level: ChangeLevel,
    pub change_drivers: Vec<String>,
    pub unchanged_dimensions: Vec<String>,
    pub summary: String,
}

pub fn build_market_change_driver(
    previous: &MarketChangeSnapshot,
    current: &MarketChangeSnapshot,
) -> MarketChangeDriver {
    let mut major = Vec::new();
    let mut moderate = Vec::new();
    let mut minor = Vec::new();
    let mut unchanged = Vec::new();

    compare_dimension(
        &mut major,
        &mut unchanged,
        "market_state",
        &previous.market_state,
        &current.market_state,
    );
    compare_dimension(
        &mut major,
        &mut unchanged,
        "risk_state",
        &previous.risk_state,
        &current.risk_state,
    );
    compare_dimension(
        &mut major,
        &mut unchanged,
        "day_type",
        &previous.day_type,
        &current.day_type,
    );
    compare_dimension(
        &mut moderate,
        &mut unchanged,
        "primary_leader",
        &previous.primary_leader,
        &current.primary_leader,
    );
    compare_dimension(
        &mut moderate,
        &mut unchanged,
        "breadth_classification",
        &previous.breadth_classification,
        &current.breadth_classification,
    );
    compare_dimension(
        &mut moderate,
        &mut unchanged,
        "supply_phase",
        &previous.supply_phase,
        &current.supply_phase,
    );
    if previous.supply_pressure != current.supply_pressure
        && is_lower_supply_pressure(&previous.supply_pressure, &current.supply_pressure)
    {
        minor.push("supply_pressure_decreased".to_string());
    } else if previous.supply_pressure != current.supply_pressure {
        minor.push("supply_pressure_increased".to_string());
    }

    if (previous.confidence - current.confidence).abs() > f64::EPSILON {
        minor.push("confidence".to_string());
    } else {
        unchanged.push("confidence".to_string());
    }
    if (previous.score - current.score).abs() > f64::EPSILON {
        minor.push("score".to_string());
    } else {
        unchanged.push("score".to_string());
    }
    compare_dimension(
        &mut minor,
        &mut unchanged,
        "breakout_state",
        &previous.breakout_state,
        &current.breakout_state,
    );
    let rank_changed = rank_delta(previous, current);
    if rank_changed {
        minor.push("local_ranking".to_string());
    } else {
        unchanged.push("local_ranking".to_string());
    }

    let (change_level, drivers) = if !major.is_empty() {
        (ChangeLevel::Major, major)
    } else if !moderate.is_empty() {
        (ChangeLevel::Moderate, moderate)
    } else if !minor.is_empty() {
        (ChangeLevel::Minor, minor)
    } else {
        (ChangeLevel::None, Vec::new())
    };
    let summary = if drivers.is_empty() {
        "没有核心维度变化。".to_string()
    } else {
        format!("变化维度：{}。", drivers.join(", "))
    };
    MarketChangeDriver {
        change_level,
        change_drivers: drivers,
        unchanged_dimensions: unchanged,
        summary,
    }
}

fn is_lower_supply_pressure(previous: &str, current: &str) -> bool {
    supply_pressure_rank(current) < supply_pressure_rank(previous)
}

fn supply_pressure_rank(value: &str) -> u8 {
    match value {
        "LOW" => 0,
        "NORMAL" => 1,
        "HIGH" => 2,
        _ => 0,
    }
}

fn compare_dimension(
    changed: &mut Vec<String>,
    unchanged: &mut Vec<String>,
    name: &str,
    previous: &str,
    current: &str,
) {
    if previous == current {
        unchanged.push(name.to_string());
    } else {
        changed.push(name.to_string());
    }
}

fn rank_delta(previous: &MarketChangeSnapshot, current: &MarketChangeSnapshot) -> bool {
    previous.ranked_leaders.len() != current.ranked_leaders.len()
        || previous
            .ranked_leaders
            .iter()
            .enumerate()
            .any(|(index, symbol)| {
                current
                    .ranked_leaders
                    .iter()
                    .position(|candidate| candidate == symbol)
                    .is_some_and(|current_index| {
                        current_index.abs_diff(index) > LOCAL_RANK_MAX_DELTA
                    })
            })
        || previous
            .ranked_leaders
            .iter()
            .any(|symbol| !current.ranked_leaders.contains(symbol))
        || current
            .ranked_leaders
            .iter()
            .any(|symbol| !previous.ranked_leaders.contains(symbol))
        || (previous.score - current.score).abs() >= LOCAL_SCORE_MAX_DELTA
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> MarketChangeSnapshot {
        MarketChangeSnapshot {
            primary_leader: "SPY".to_string(),
            breadth_classification: "NARROW".to_string(),
            supply_phase: "WATCH".to_string(),
            supply_pressure: "LOW".to_string(),
            market_state: "RANGE".to_string(),
            risk_state: "NORMAL".to_string(),
            day_type: "NORMAL".to_string(),
            confidence: 52.4,
            score: 50.0,
            ranked_leaders: vec!["SPY".to_string(), "MSFT".to_string()],
            breakout_state: "SPY:CONFIRMED".to_string(),
        }
    }

    #[test]
    fn confidence_only_change_is_minor() {
        let previous = snapshot();
        let mut current = previous.clone();
        current.confidence = 53.1;

        let change = build_market_change_driver(&previous, &current);

        assert_eq!(change.change_level, ChangeLevel::Minor);
        assert_eq!(change.change_drivers, vec!["confidence"]);
        assert!(change
            .unchanged_dimensions
            .contains(&"primary_leader".to_string()));
    }

    #[test]
    fn market_state_change_takes_major_priority_over_confidence() {
        let previous = snapshot();
        let mut current = previous.clone();
        current.market_state = "TREND".to_string();
        current.confidence += 10.0;

        let change = build_market_change_driver(&previous, &current);

        assert_eq!(change.change_level, ChangeLevel::Major);
    }

    #[test]
    fn primary_leader_change_is_moderate_and_no_dimension_change_is_none() {
        let previous = snapshot();
        let mut current = previous.clone();
        current.primary_leader = "GOOG".to_string();
        assert_eq!(
            build_market_change_driver(&previous, &current).change_level,
            ChangeLevel::Moderate
        );

        assert_eq!(
            build_market_change_driver(&previous, &previous).change_level,
            ChangeLevel::None
        );
    }

    #[test]
    fn confidence_change_does_not_change_score_or_local_ranking() {
        let previous = snapshot();
        let mut current = previous.clone();
        current.confidence = 90.0;

        let change = build_market_change_driver(&previous, &current);

        assert_eq!(change.change_level, ChangeLevel::Minor);
        assert_eq!(change.change_drivers, vec!["confidence"]);
        assert!(!change.change_drivers.contains(&"score".to_string()));
        assert!(!change.change_drivers.contains(&"local_ranking".to_string()));
    }

    #[test]
    fn supply_phase_change_without_pressure_change_is_moderate() {
        let previous = snapshot();
        let mut current = previous.clone();
        current.supply_phase = "ACCUMULATING".to_string();

        let change = build_market_change_driver(&previous, &current);

        assert_eq!(change.change_level, ChangeLevel::Moderate);
        assert!(change.change_drivers.contains(&"supply_phase".to_string()));
    }

    #[test]
    fn supply_pressure_reset_is_minor_with_semantic_driver() {
        let mut previous = snapshot();
        previous.supply_pressure = "NORMAL".to_string();
        let current = snapshot();

        let change = build_market_change_driver(&previous, &current);

        assert_eq!(change.change_level, ChangeLevel::Minor);
        assert_eq!(change.change_drivers, vec!["supply_pressure_decreased"]);
    }

    #[test]
    fn breakout_and_local_ranking_are_only_drivers_when_other_dimensions_are_unchanged() {
        let previous = snapshot();
        let mut current = previous.clone();
        current.breakout_state = "SPY:CONFIRMED,GOOG:EMERGING".to_string();
        current.ranked_leaders = vec!["SPY".to_string(), "GOOG".to_string()];

        let change = build_market_change_driver(&previous, &current);

        assert_eq!(change.change_level, ChangeLevel::Minor);
        assert_eq!(
            change.change_drivers,
            vec!["breakout_state", "local_ranking"]
        );
        assert_eq!(
            change.unchanged_dimensions,
            vec![
                "market_state",
                "risk_state",
                "day_type",
                "primary_leader",
                "breadth_classification",
                "supply_phase",
                "confidence",
                "score"
            ]
        );
    }
}

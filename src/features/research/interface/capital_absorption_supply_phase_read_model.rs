use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoSnapshot, CapitalAbsorptionObservationCoverageState,
    CapitalAbsorptionPotentialSupplyPressureLevel, CapitalAbsorptionPotentialSupplyTrend,
    CapitalAbsorptionSourceHealth,
};
use crate::features::shared::interface::i18n::Language;

use super::capital_absorption_i18n::{
    capital_absorption_boundary, capital_absorption_current_phase_boundary,
    capital_absorption_incomplete_supply_summary, capital_absorption_partial_supply_value,
    capital_absorption_supply_phase_label, capital_absorption_unknown_value,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupplyPhase {
    Idle,
    Accumulating,
    Absorbing,
    Stressed,
    Overwhelmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SupplyEventCounts {
    pub future_queue: usize,
    pub reported: usize,
    pub confirmed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SupplySnapshot {
    pub collection_snapshot_id: Option<String>,
    pub state: String,
    pub trend: String,
    pub pressure: String,
    pub phase: SupplyPhase,
    pub event_counts: SupplyEventCounts,
    pub interpretation: String,
    pub source_health: String,
}

impl SupplySnapshot {
    pub(crate) fn empty() -> Self {
        Self {
            collection_snapshot_id: None,
            state: "UNAVAILABLE".to_string(),
            trend: "UNAVAILABLE".to_string(),
            pressure: "UNAVAILABLE".to_string(),
            phase: SupplyPhase::Idle,
            event_counts: SupplyEventCounts {
                future_queue: 0,
                reported: 0,
                confirmed: 0,
            },
            interpretation: "暂无新增供给风险。".to_string(),
            source_health: "UNAVAILABLE".to_string(),
        }
    }
}

pub(crate) fn build_supply_snapshot(
    snapshot: Option<&CapitalAbsorptionAutoSnapshot>,
) -> SupplySnapshot {
    let Some(snapshot) = snapshot else {
        return SupplySnapshot::empty();
    };
    let observed_pressure = match snapshot.potential_supply_pressure.level {
        CapitalAbsorptionPotentialSupplyPressureLevel::Low => "LOW",
        CapitalAbsorptionPotentialSupplyPressureLevel::Normal => "NORMAL",
        CapitalAbsorptionPotentialSupplyPressureLevel::Elevated => "HIGH",
    };
    let counts = SupplyEventCounts {
        future_queue: snapshot.potential_supply_pressure.future_queue_count,
        reported: snapshot.potential_supply_pressure.reported_count,
        confirmed: snapshot.potential_supply_pressure.confirmed_count,
    };
    let phase = if counts.future_queue == 0
        && counts.reported == 0
        && counts.confirmed == 0
        && observed_pressure == "LOW"
    {
        SupplyPhase::Idle
    } else if counts.confirmed > 0 {
        if observed_pressure == "HIGH" {
            SupplyPhase::Stressed
        } else {
            SupplyPhase::Absorbing
        }
    } else if observed_pressure == "HIGH" {
        SupplyPhase::Stressed
    } else {
        SupplyPhase::Accumulating
    };
    let interpretation = match phase {
        SupplyPhase::Idle => "暂无新增供给风险。",
        SupplyPhase::Accumulating => "新一轮供给正在积累。",
        SupplyPhase::Absorbing => "已确认供给进入市场，当前仍可正常吸收。",
        SupplyPhase::Stressed => "供给显著增加，吸收能力开始恶化。",
        SupplyPhase::Overwhelmed => "供给明显超过需求支持。",
    };
    let source_health = match (snapshot.observation_coverage, snapshot.source_status.status) {
        (
            CapitalAbsorptionObservationCoverageState::Complete,
            CapitalAbsorptionSourceHealth::Succeeded,
        ) => "SUCCEEDED",
        (CapitalAbsorptionObservationCoverageState::Unavailable, _)
        | (_, CapitalAbsorptionSourceHealth::Unavailable) => "UNAVAILABLE",
        _ => "PARTIAL",
    };
    let (state, trend, pressure, interpretation) = match source_health {
        "SUCCEEDED" => (
            match snapshot.status {
                crate::features::research::domain::capital_absorption::CapitalAbsorptionAutoStatus::Normal => "NORMAL",
                crate::features::research::domain::capital_absorption::CapitalAbsorptionAutoStatus::Watch => "WATCH",
            },
            match snapshot.potential_supply_trend {
                CapitalAbsorptionPotentialSupplyTrend::Falling => "FALLING",
                CapitalAbsorptionPotentialSupplyTrend::Stable => "STABLE",
                CapitalAbsorptionPotentialSupplyTrend::Rising => "RISING",
            },
            observed_pressure,
            interpretation,
        ),
        "PARTIAL" => ("PARTIAL", "PARTIAL", "PARTIAL", "来源覆盖不完整，供给阶段不完整。"),
        _ => (
            "UNAVAILABLE",
            "UNAVAILABLE",
            "UNAVAILABLE",
            "来源覆盖不可用，供给阶段未知。",
        ),
    };
    SupplySnapshot {
        collection_snapshot_id: Some(snapshot.collection_snapshot_id.clone()),
        state: state.to_string(),
        trend: trend.to_string(),
        pressure: pressure.to_string(),
        phase,
        event_counts: counts,
        interpretation: interpretation.to_string(),
        source_health: source_health.to_string(),
    }
}

fn supply_phase_value(phase: SupplyPhase, language: Language) -> &'static str {
    match (phase, language) {
        (SupplyPhase::Idle, Language::ZhCn) => "IDLE",
        (SupplyPhase::Accumulating, Language::ZhCn) => "ACCUMULATING",
        (SupplyPhase::Absorbing, Language::ZhCn) => "ABSORBING",
        (SupplyPhase::Stressed, Language::ZhCn) => "STRESSED",
        (SupplyPhase::Overwhelmed, Language::ZhCn) => "OVERWHELMED",
        (SupplyPhase::Idle, Language::EnUs) => "IDLE",
        (SupplyPhase::Accumulating, Language::EnUs) => "ACCUMULATING",
        (SupplyPhase::Absorbing, Language::EnUs) => "ABSORBING",
        (SupplyPhase::Stressed, Language::EnUs) => "STRESSED",
        (SupplyPhase::Overwhelmed, Language::EnUs) => "OVERWHELMED",
        (SupplyPhase::Idle, Language::JaJp) => "IDLE",
        (SupplyPhase::Accumulating, Language::JaJp) => "ACCUMULATING",
        (SupplyPhase::Absorbing, Language::JaJp) => "ABSORBING",
        (SupplyPhase::Stressed, Language::JaJp) => "STRESSED",
        (SupplyPhase::Overwhelmed, Language::JaJp) => "OVERWHELMED",
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SupplyPhaseViewModel {
    pub title: String,
    pub phase_label: String,
    pub phase_value: String,
    pub summary_label: String,
    pub summary_value: String,
    pub boundary: String,
}

pub(crate) fn build_supply_phase_view_model_from_snapshot(
    snapshot: Option<&CapitalAbsorptionAutoSnapshot>,
    language: Language,
) -> SupplyPhaseViewModel {
    let supply = build_supply_snapshot(snapshot);
    build_supply_phase_view_model_from_supply_snapshot(&supply, language)
}

pub(crate) fn build_supply_phase_view_model_from_supply_snapshot(
    supply: &SupplySnapshot,
    language: Language,
) -> SupplyPhaseViewModel {
    let (phase_value, summary_value) = match supply.source_health.as_str() {
        "SUCCEEDED" => (
            supply_phase_value(supply.phase, language).to_string(),
            supply.interpretation.clone(),
        ),
        "PARTIAL" => (
            "PARTIAL".to_string(),
            capital_absorption_partial_supply_value(false, language).to_string(),
        ),
        _ => (
            capital_absorption_unknown_value(language).to_string(),
            capital_absorption_incomplete_supply_summary(language).to_string(),
        ),
    };
    SupplyPhaseViewModel {
        title: capital_absorption_supply_phase_label(language).to_string(),
        phase_label: capital_absorption_supply_phase_label(language).to_string(),
        phase_value,
        summary_label: summary_label(language).to_string(),
        summary_value,
        boundary: format!(
            "{}\n\n{}",
            capital_absorption_current_phase_boundary(language),
            capital_absorption_boundary(language)
        ),
    }
}

fn summary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Summary",
        Language::EnUs => "Summary",
        Language::JaJp => "Summary",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::application::capital_absorption::unavailable_capital_absorption_snapshot;
    use crate::features::research::domain::capital_absorption::{
        build_capital_absorption_snapshot_from_events, CapitalAbsorptionSourceHealth,
        CapitalAbsorptionSourceStatus,
    };

    #[test]
    fn missing_supply_is_unavailable_instead_of_idle() {
        let snapshot = build_supply_snapshot(None);

        assert_eq!(snapshot.state, "UNAVAILABLE");
        assert_eq!(snapshot.trend, "UNAVAILABLE");
        assert_eq!(snapshot.pressure, "UNAVAILABLE");
        assert_eq!(snapshot.phase, SupplyPhase::Idle);
        assert_eq!(snapshot.event_counts.future_queue, 0);
        assert_eq!(snapshot.event_counts.reported, 0);
        assert_eq!(snapshot.event_counts.confirmed, 0);
        assert_eq!(snapshot.source_health, "UNAVAILABLE");
        let view = build_supply_phase_view_model_from_supply_snapshot(&snapshot, Language::EnUs);
        assert_eq!(view.phase_value, "UNKNOWN");
        assert!(view.summary_value.contains("coverage is incomplete"));
    }

    #[test]
    fn partial_supply_remains_partial_and_complete_empty_supply_remains_idle() {
        let partial = build_capital_absorption_snapshot_from_events(
            Vec::new(),
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Partial,
                message: "one source failed".to_string(),
            },
        );
        let partial_supply = build_supply_snapshot(Some(&partial));
        assert_eq!(partial_supply.source_health, "PARTIAL");
        assert_eq!(partial_supply.pressure, "PARTIAL");
        let partial_view =
            build_supply_phase_view_model_from_supply_snapshot(&partial_supply, Language::EnUs);
        assert_eq!(partial_view.phase_value, "PARTIAL");
        assert!(partial_view
            .summary_value
            .contains("Source coverage is partial"));

        let complete_empty = build_capital_absorption_snapshot_from_events(
            Vec::new(),
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "all sources succeeded".to_string(),
            },
        );
        let complete_supply = build_supply_snapshot(Some(&complete_empty));
        assert_eq!(complete_supply.source_health, "SUCCEEDED");
        assert_eq!(complete_supply.pressure, "LOW");
        let complete_view =
            build_supply_phase_view_model_from_supply_snapshot(&complete_supply, Language::EnUs);
        assert_eq!(complete_view.phase_value, "IDLE");
    }

    #[test]
    fn supply_projection_carries_collection_identity_and_unavailable_health() {
        let source = unavailable_capital_absorption_snapshot("HTTP 429 rate limited".to_string());
        let supply = build_supply_snapshot(Some(&source));

        assert_eq!(
            supply.collection_snapshot_id.as_deref(),
            Some(source.collection_snapshot_id.as_str())
        );
        assert_eq!(supply.source_health, "UNAVAILABLE");
        assert_eq!(supply.event_counts.future_queue, 0);
        let view = build_supply_phase_view_model_from_supply_snapshot(&supply, Language::EnUs);
        assert_eq!(view.phase_value, "UNKNOWN");
        assert!(view.summary_value.contains("coverage is incomplete"));
    }
}

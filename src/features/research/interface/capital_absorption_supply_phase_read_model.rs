use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoSnapshot, CapitalAbsorptionPotentialSupplyPressureLevel,
    CapitalAbsorptionPotentialSupplyTrend,
};
use crate::features::shared::interface::i18n::Language;

use super::capital_absorption_i18n::{
    capital_absorption_boundary, capital_absorption_current_phase_boundary,
    capital_absorption_supply_phase_label,
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
            state: "NORMAL".to_string(),
            trend: "STABLE".to_string(),
            pressure: "LOW".to_string(),
            phase: SupplyPhase::Idle,
            event_counts: SupplyEventCounts {
                future_queue: 0,
                reported: 0,
                confirmed: 0,
            },
            interpretation: "暂无新增供给风险。".to_string(),
            source_health: "SUCCEEDED".to_string(),
        }
    }
}

pub(crate) fn build_supply_snapshot(
    snapshot: Option<&CapitalAbsorptionAutoSnapshot>,
) -> SupplySnapshot {
    let Some(snapshot) = snapshot else {
        return SupplySnapshot::empty();
    };
    let pressure = match snapshot.potential_supply_pressure.level {
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
        && pressure == "LOW"
    {
        SupplyPhase::Idle
    } else if counts.confirmed > 0 {
        if pressure == "HIGH" {
            SupplyPhase::Stressed
        } else {
            SupplyPhase::Absorbing
        }
    } else if pressure == "HIGH" {
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
    SupplySnapshot {
        state: match snapshot.status {
            crate::features::research::domain::capital_absorption::CapitalAbsorptionAutoStatus::Normal => "NORMAL",
            crate::features::research::domain::capital_absorption::CapitalAbsorptionAutoStatus::Watch => "WATCH",
        }.to_string(),
        trend: match snapshot.potential_supply_trend {
            CapitalAbsorptionPotentialSupplyTrend::Falling => "FALLING",
            CapitalAbsorptionPotentialSupplyTrend::Stable => "STABLE",
            CapitalAbsorptionPotentialSupplyTrend::Rising => "RISING",
        }.to_string(),
        pressure: pressure.to_string(),
        phase,
        event_counts: counts,
        interpretation: interpretation.to_string(),
        source_health: match snapshot.source_status.status {
            crate::features::research::domain::capital_absorption::CapitalAbsorptionSourceHealth::Succeeded => "SUCCEEDED",
            crate::features::research::domain::capital_absorption::CapitalAbsorptionSourceHealth::Unavailable => "UNAVAILABLE",
        }.to_string(),
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
    SupplyPhaseViewModel {
        title: capital_absorption_supply_phase_label(language).to_string(),
        phase_label: capital_absorption_supply_phase_label(language).to_string(),
        phase_value: supply_phase_value(supply.phase, language).to_string(),
        summary_label: summary_label(language).to_string(),
        summary_value: supply.interpretation.clone(),
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

    #[test]
    fn current_empty_supply_is_idle_with_normalized_facts() {
        let snapshot = SupplySnapshot::empty();

        assert_eq!(snapshot.state, "NORMAL");
        assert_eq!(snapshot.trend, "STABLE");
        assert_eq!(snapshot.pressure, "LOW");
        assert_eq!(snapshot.phase, SupplyPhase::Idle);
        assert_eq!(snapshot.event_counts.future_queue, 0);
        assert_eq!(snapshot.event_counts.reported, 0);
        assert_eq!(snapshot.event_counts.confirmed, 0);
    }
}

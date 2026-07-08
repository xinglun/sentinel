use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoSnapshot, CapitalAbsorptionPotentialSupplyPressureLevel,
    CapitalAbsorptionPotentialSupplyTrend,
};
use crate::features::shared::interface::i18n::Language;

use super::capital_absorption_i18n::{
    capital_absorption_boundary, capital_absorption_current_phase_boundary,
    capital_absorption_supply_phase_label, capital_absorption_supply_phase_value,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct SupplyPhaseViewModel {
    pub title: String,
    pub phase_label: String,
    pub phase_value: String,
    pub summary_label: String,
    pub summary_value: String,
    pub boundary: String,
}

pub(crate) fn build_supply_phase_view_model(
    pressure: CapitalAbsorptionPotentialSupplyPressureLevel,
    trend: CapitalAbsorptionPotentialSupplyTrend,
    language: Language,
) -> SupplyPhaseViewModel {
    let summary_value = supply_phase_summary(pressure, trend, language);
    SupplyPhaseViewModel {
        title: capital_absorption_supply_phase_label(language).to_string(),
        phase_label: capital_absorption_supply_phase_label(language).to_string(),
        phase_value: capital_absorption_supply_phase_value(pressure, language).to_string(),
        summary_label: summary_label(language).to_string(),
        summary_value,
        boundary: format!(
            "{}\n\n{}",
            capital_absorption_current_phase_boundary(language),
            capital_absorption_boundary(language)
        ),
    }
}

pub(crate) fn build_supply_phase_view_model_from_snapshot(
    snapshot: Option<&CapitalAbsorptionAutoSnapshot>,
    language: Language,
) -> SupplyPhaseViewModel {
    let (pressure, trend) = snapshot
        .map(|snapshot| {
            (
                snapshot.potential_supply_pressure.level,
                snapshot.potential_supply_trend,
            )
        })
        .unwrap_or((
            CapitalAbsorptionPotentialSupplyPressureLevel::Low,
            CapitalAbsorptionPotentialSupplyTrend::Stable,
        ));
    build_supply_phase_view_model(pressure, trend, language)
}

fn supply_phase_summary(
    pressure: CapitalAbsorptionPotentialSupplyPressureLevel,
    trend: CapitalAbsorptionPotentialSupplyTrend,
    language: Language,
) -> String {
    let rising = matches!(trend, CapitalAbsorptionPotentialSupplyTrend::Rising);
    match (pressure, rising, language) {
        (CapitalAbsorptionPotentialSupplyPressureLevel::Low, _, Language::ZhCn) => {
            "Supply pressure remains subdued.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Low, _, Language::EnUs) => {
            "Supply pressure remains subdued.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Low, _, Language::JaJp) => {
            "Supply pressure remains subdued.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Normal, true, Language::ZhCn) => {
            "Supply pressure is increasing. Absorption remains manageable.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Normal, true, Language::EnUs) => {
            "Supply pressure is increasing. Absorption remains manageable.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Normal, true, Language::JaJp) => {
            "Supply pressure is increasing. Absorption remains manageable.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Normal, false, Language::ZhCn) => {
            "Supply pressure remains manageable.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Normal, false, Language::EnUs) => {
            "Supply pressure remains manageable.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Normal, false, Language::JaJp) => {
            "Supply pressure remains manageable.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Elevated, _, Language::ZhCn) => {
            "Supply pressure is elevated and absorption is strained.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Elevated, _, Language::EnUs) => {
            "Supply pressure is elevated and absorption is strained.".to_string()
        }
        (CapitalAbsorptionPotentialSupplyPressureLevel::Elevated, _, Language::JaJp) => {
            "Supply pressure is elevated and absorption is strained.".to_string()
        }
    }
}

fn summary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Summary",
        Language::EnUs => "Summary",
        Language::JaJp => "Summary",
    }
}

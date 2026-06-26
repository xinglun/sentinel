use crate::config;
use crate::features::research::application::capital_absorption::CapitalAbsorptionAutoSnapshot;
use crate::features::shared::interface::i18n::Language;

pub(crate) use super::asset_thesis_report::{
    build_asset_thesis_report_from_entries, enabled_asset_thesis_count_from_entries,
    enabled_research_attention_count_from_entries,
};
pub(crate) use super::capital_absorption_report::build_capital_absorption_report_from_config;
pub(crate) use super::capital_dynamics_flow_report::{
    build_flow_layer_report_from_config, build_flow_layer_weekly_summary,
};
pub(crate) use super::capital_dynamics_report::build_capital_dynamics_report;
pub(crate) use super::daily_calibration_i18n::{
    daily_calibration_attention_label, daily_calibration_audit_label, daily_calibration_boundary,
    daily_calibration_capital_dynamics_label, daily_calibration_evidence_none,
    daily_calibration_evidence_observed, daily_calibration_evidence_strong,
    daily_calibration_gray_rhino_label, daily_calibration_macro_gravity_label,
    daily_calibration_question_attention, daily_calibration_question_boundary,
    daily_calibration_question_evidence, daily_calibration_question_gate,
    daily_calibration_question_market, daily_calibration_question_thesis,
    daily_calibration_questions_label, daily_calibration_thesis_label, daily_calibration_title,
    daily_calibration_valuation_gravity_label,
};
pub(crate) use super::macro_gravity_report::{
    build_macro_gravity_report_from_config, credit_stress_label, growth_valuation_impact_label,
    liquidity_condition_label, macro_pressure_label, yield_curve_label,
};
pub(crate) use super::research_attention_report::build_research_attention_report_from_entries;

pub(crate) fn build_research_attention_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    build_research_attention_report_from_entries(app_config.research_attention.as_ref(), language)
}

pub(crate) fn build_asset_thesis_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    build_asset_thesis_report_from_entries(app_config.asset_thesis.as_ref(), language)
}

pub(crate) fn build_macro_gravity_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    build_macro_gravity_report_from_config(app_config.macro_gravity.as_ref(), language)
}

#[allow(dead_code)]
pub(crate) fn build_capital_absorption_report(
    app_config: &config::AppConfig,
    auto_snapshot: Option<&CapitalAbsorptionAutoSnapshot>,
    language: Language,
) -> String {
    build_capital_absorption_report_from_config(
        app_config.capital_absorption.as_ref(),
        auto_snapshot,
        language,
    )
}

pub(crate) fn build_flow_layer_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    build_flow_layer_report_from_config(app_config.capital_dynamics.as_ref(), language)
}

pub(crate) fn enabled_research_attention_count(app_config: &config::AppConfig) -> usize {
    enabled_research_attention_count_from_entries(app_config.research_attention.as_ref())
}

pub(crate) fn enabled_asset_thesis_count(app_config: &config::AppConfig) -> usize {
    enabled_asset_thesis_count_from_entries(app_config.asset_thesis.as_ref())
}

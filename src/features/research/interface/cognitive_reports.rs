use crate::config;
#[cfg(test)]
use crate::features::research::application::capital_absorption::CapitalAbsorptionAutoSnapshot;
use crate::features::shared::interface::i18n::Language;
use anyhow::Result;

pub(crate) use super::asset_thesis_report::{
    build_asset_thesis_report_from_entries, enabled_asset_thesis_count_from_entries,
    enabled_research_attention_count_from_entries,
};
#[cfg(test)]
pub(crate) use super::capital_absorption_report::build_capital_absorption_report_from_config;
use super::capital_absorption_report_builder::build_capital_absorption_report_with_auto;
pub(crate) use super::capital_dynamics_flow_report::{
    build_flow_layer_report_from_config, build_flow_layer_weekly_summary,
};
pub(crate) use super::capital_dynamics_report::build_capital_dynamics_report;
pub(crate) use super::daily_calibration_i18n::{
    daily_calibration_attention_label, daily_calibration_audit_label, daily_calibration_boundary,
    daily_calibration_capital_dynamics_label, daily_calibration_evidence_none,
    daily_calibration_evidence_observed, daily_calibration_evidence_strong,
    daily_calibration_expectation_label, daily_calibration_gray_rhino_label,
    daily_calibration_macro_gravity_label, daily_calibration_question_attention,
    daily_calibration_question_boundary, daily_calibration_question_evidence,
    daily_calibration_question_gate, daily_calibration_question_market,
    daily_calibration_question_thesis, daily_calibration_questions_label,
    daily_calibration_thesis_label, daily_calibration_title,
    daily_calibration_valuation_gravity_label,
};
pub(crate) use super::expectation_report::build_expectation_layer_weekly_summary_with_config_for_market_date;
use super::expectation_report_builder::build_expectation_layer_report_for_market_date;
use super::gray_rhino_report::build_gray_rhino_daily_report_read_only;
pub(crate) use super::macro_gravity_report::{
    build_macro_gravity_report_from_config, credit_stress_label, growth_valuation_impact_label,
    liquidity_condition_label, macro_pressure_label, yield_curve_label,
};
pub(crate) use super::research_attention_report::build_research_attention_report_from_entries;
use super::valuation_gravity_report_builder::build_valuation_gravity_report_with_auto;

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

#[cfg(test)]
#[cfg(test)]
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

pub(crate) async fn build_daily_calibration_report_from_context(
    app_config: &config::AppConfig,
    audit_section: &str,
    questions_section: &str,
    calibration_date: chrono::NaiveDate,
    window_days: usize,
    language: Language,
) -> Result<String> {
    let mut out = String::new();
    out.push_str(daily_calibration_title(language));
    out.push_str("\n\n");
    out.push_str(daily_calibration_audit_label(language));
    out.push_str("\n\n");
    out.push_str(audit_section);
    out.push_str("\n\n");
    out.push_str(daily_calibration_questions_label(language));
    out.push_str("\n\n");
    out.push_str(questions_section);
    out.push_str("\n\n");
    out.push_str(daily_calibration_attention_label(language));
    out.push_str("\n\n");
    out.push_str(&build_research_attention_report(app_config, language));
    out.push_str("\n\n");
    out.push_str(daily_calibration_thesis_label(language));
    out.push_str("\n\n");
    out.push_str(&build_asset_thesis_report(app_config, language));
    out.push_str("\n\n");
    out.push_str(daily_calibration_macro_gravity_label(language));
    out.push_str("\n\n");
    out.push_str(&build_macro_gravity_report(app_config, language));
    out.push_str("\n\n");
    let capital_absorption_report = build_capital_absorption_report_with_auto(
        app_config,
        calibration_date,
        window_days.max(1),
        language,
    )
    .await;
    let flow_report = app_config
        .capital_dynamics
        .as_ref()
        .and_then(config::CapitalDynamicsConfig::flow_layer_snapshot)
        .map(|_| build_flow_layer_report(app_config, language));
    out.push_str(daily_calibration_capital_dynamics_label(language));
    out.push_str("\n\n");
    out.push_str(&build_capital_dynamics_report(
        &capital_absorption_report,
        flow_report.as_deref(),
        language,
    ));
    out.push_str("\n\n");
    out.push_str(daily_calibration_valuation_gravity_label(language));
    out.push_str("\n\n");
    out.push_str(
        &build_valuation_gravity_report_with_auto(app_config, calibration_date, language).await?,
    );
    out.push_str("\n\n");
    out.push_str(daily_calibration_gray_rhino_label(language));
    out.push_str("\n\n");
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    out.push_str(&build_gray_rhino_daily_report_read_only(
        app_config,
        &save_dir,
        calibration_date,
        language,
    )?);
    out.push_str("\n\n");
    out.push_str(daily_calibration_expectation_label(language));
    out.push_str("\n\n");
    out.push_str(&build_expectation_layer_report_for_market_date(
        app_config,
        calibration_date,
        language,
    ));
    out.push_str("\n\n");
    out.push_str(daily_calibration_boundary(language));
    Ok(out)
}

use crate::config::{self, GrayRhinoRiskLevel};
use crate::features::research::acl::gray_rhino_daily_report_factory::build_gray_rhino_daily_report_repository;
use crate::features::research::application::gray_rhino_daily_report::{
    GrayRhinoDailyReportUseCase, GrayRhinoSnapshotPersistence,
};
#[cfg(test)]
use crate::features::research::domain::gray_rhino::{
    evaluate_gray_rhino_escalation, GrayRhinoAssessmentSnapshot,
};
use crate::features::research::domain::gray_rhino::{
    GrayRhinoAssessment, GrayRhinoEscalationInput, RiskLevel,
};
#[cfg(test)]
use crate::features::research::domain::gray_rhino::{
    GrayRhinoEscalation, GrayRhinoObservationSource,
};
use crate::features::research::domain::gray_rhino_candidate::GrayRhinoCandidate;
use crate::features::research::interface::gray_rhino_assessment_renderer::{
    gray_rhino_empty,
    render_gray_rhino_assessment_markdown as render_gray_rhino_assessment_markdown_impl,
};
use crate::features::research::interface::gray_rhino_inline_reference_renderer::{
    render_auto_discovery_inline_reference,
    render_gray_rhino_inline_reference as render_gray_rhino_inline_reference_impl,
};
use crate::features::research::interface::gray_rhino_renderer::{
    render_discovery_ops_view, render_refresh_status,
};
use crate::features::research::interface::gray_rhino_sensor_health_renderer::{
    render_multi_category_sensor_health, render_unclassified_evidence_notice,
};
use crate::features::research::interface::gray_rhino_temporal_survivability_renderer::{
    render_survivability_summary, render_temporal_summary,
};
use crate::features::shared::interface::i18n::Language;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use std::path::Path;

pub(crate) fn build_gray_rhino_escalation_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    let save_dir = Path::new(&app_config.output.save_to);
    let repository = build_gray_rhino_daily_report_repository(save_dir);
    let view_model = GrayRhinoDailyReportUseCase::new(&repository)
        .build(
            input_from_config(app_config),
            &enabled_watch_symbols(app_config),
            Local::now().date_naive(),
            GrayRhinoSnapshotPersistence::ReadOnly,
        )
        .ok();
    view_model
        .and_then(|view_model| view_model.assessment)
        .map(|assessment| render_gray_rhino_assessment_markdown(&assessment, language))
        .unwrap_or_else(|| gray_rhino_empty(language).to_string())
}

pub(crate) fn build_gray_rhino_daily_report(
    app_config: &config::AppConfig,
    save_dir: &Path,
    as_of_date: NaiveDate,
    language: Language,
) -> Result<String> {
    build_gray_rhino_daily_report_with_persistence(
        app_config,
        save_dir,
        as_of_date,
        language,
        GrayRhinoSnapshotPersistence::SaveIfChanged,
    )
}

pub(crate) fn build_gray_rhino_daily_report_read_only(
    app_config: &config::AppConfig,
    save_dir: &Path,
    as_of_date: NaiveDate,
    language: Language,
) -> Result<String> {
    build_gray_rhino_daily_report_with_persistence(
        app_config,
        save_dir,
        as_of_date,
        language,
        GrayRhinoSnapshotPersistence::ReadOnly,
    )
}

fn build_gray_rhino_daily_report_with_persistence(
    app_config: &config::AppConfig,
    save_dir: &Path,
    as_of_date: NaiveDate,
    language: Language,
    snapshot_persistence: GrayRhinoSnapshotPersistence,
) -> Result<String> {
    let watch_symbols = enabled_watch_symbols(app_config);
    let repository = build_gray_rhino_daily_report_repository(save_dir);
    let view_model = GrayRhinoDailyReportUseCase::new(&repository).build(
        input_from_config(app_config),
        &watch_symbols,
        as_of_date,
        snapshot_persistence,
    )?;
    let mut report = if let Some(assessment) = &view_model.assessment {
        render_gray_rhino_assessment_markdown(assessment, language)
    } else {
        gray_rhino_empty(language).to_string()
    };
    let temporal_summary = render_temporal_summary(&view_model.temporal_summary, language);
    if !temporal_summary.is_empty() {
        report.push_str("\n\n");
        report.push_str(&temporal_summary);
    }
    let survivability_summary =
        render_survivability_summary(&view_model.survivability_summary, language);
    if !survivability_summary.is_empty() {
        report.push_str("\n\n");
        report.push_str(&survivability_summary);
    }
    let sensor_health = render_multi_category_sensor_health(&view_model, language);
    if !sensor_health.is_empty() {
        report.push_str("\n\n");
        report.push_str(&sensor_health);
    }
    if view_model.unclassified_record_count > 0 {
        report.push_str("\n\n");
        report.push_str(&render_unclassified_evidence_notice(
            view_model.unclassified_record_count,
            language,
        ));
    }
    if let Some(refresh_status) =
        render_refresh_status(view_model.refresh_status.as_ref(), language)
    {
        report.push_str("\n\n");
        report.push_str(&refresh_status);
    }
    report.push_str("\n\n");
    report.push_str(&render_auto_discovery_inline_reference(
        &watch_symbols,
        &view_model.display_candidates,
        &view_model.monitoring_statuses,
        language,
    ));
    if let Some(discovery_ops_view) =
        render_discovery_ops_view(view_model.discovery_ops_view.as_ref(), language)
    {
        report.push_str("\n\n");
        report.push_str(&discovery_ops_view);
    }
    Ok(report)
}

fn input_from_config(app_config: &config::AppConfig) -> Option<GrayRhinoEscalationInput> {
    let config = app_config
        .gray_rhino_escalation
        .as_ref()
        .filter(|config| config.enable.unwrap_or(true))?;
    Some(GrayRhinoEscalationInput {
        risk_expansion_rate: map_risk_level(config.risk_expansion_rate),
        constraint_growth_rate: map_risk_level(config.constraint_growth_rate),
        dependency_centralization: map_risk_level(config.dependency_centralization),
        awareness_decay: map_risk_level(config.awareness_decay),
        narrative_overconfidence: map_risk_level(config.narrative_overconfidence),
        single_point_fragility: map_risk_level(config.single_point_fragility),
        fallback_survivability_risk: map_risk_level(config.fallback_survivability_risk),
        notes: config.notes.clone().unwrap_or_default(),
    })
}

fn map_risk_level(level: GrayRhinoRiskLevel) -> RiskLevel {
    match level {
        GrayRhinoRiskLevel::Low => RiskLevel::Low,
        GrayRhinoRiskLevel::Moderate => RiskLevel::Moderate,
        GrayRhinoRiskLevel::Elevated => RiskLevel::Elevated,
        GrayRhinoRiskLevel::High => RiskLevel::High,
    }
}

#[cfg(test)]
pub(crate) fn render_gray_rhino_escalation_markdown(
    escalation: &GrayRhinoEscalation,
    language: Language,
) -> String {
    let assessment = GrayRhinoAssessment {
        current: GrayRhinoAssessmentSnapshot {
            schema_version: 1,
            as_of_date: Local::now().date_naive(),
            source: GrayRhinoObservationSource::ManualConfiguration,
            escalation: escalation.clone(),
        },
        previous: None,
    };
    render_gray_rhino_assessment_markdown(&assessment, language)
}

pub(crate) fn render_gray_rhino_assessment_markdown(
    assessment: &GrayRhinoAssessment,
    language: Language,
) -> String {
    render_gray_rhino_assessment_markdown_impl(assessment, language)
}

#[cfg(test)]
pub(crate) fn build_gray_rhino_escalation_telegram_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    build_gray_rhino_escalation_report(app_config, language)
}

pub(crate) fn render_gray_rhino_inline_reference(candidates: &[GrayRhinoCandidate]) -> String {
    render_gray_rhino_inline_reference_impl(candidates)
}

fn enabled_watch_symbols(app_config: &config::AppConfig) -> Vec<String> {
    app_config
        .watchlist
        .iter()
        .filter(|item| item.enable)
        .map(|item| item.symbol.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GrayRhinoRiskLevel;
    use crate::features::research::application::gray_rhino_assessment::build_gray_rhino_assessment;
    use crate::features::research::domain::gray_rhino::GrayRhinoEscalationInput;

    fn normalized_escalation() -> GrayRhinoEscalation {
        evaluate_gray_rhino_escalation(GrayRhinoEscalationInput {
            risk_expansion_rate: RiskLevel::Elevated,
            constraint_growth_rate: RiskLevel::Low,
            dependency_centralization: RiskLevel::High,
            awareness_decay: RiskLevel::High,
            narrative_overconfidence: RiskLevel::Elevated,
            single_point_fragility: RiskLevel::Moderate,
            fallback_survivability_risk: RiskLevel::Moderate,
            notes: vec![
                "Infrastructure concentration continues expanding.".to_string(),
                "Institutional maturity remains flat.".to_string(),
            ],
        })
    }

    #[test]
    fn markdown_output_warns_without_panic_language() {
        let report =
            render_gray_rhino_escalation_markdown(&normalized_escalation(), Language::EnUs);

        assert!(report.contains("Gray Rhino Escalation"));
        assert!(report.contains("State: Normalized"));
        assert!(report.contains("Escalation: Rising"));
        assert!(report.contains("structural escalation monitor"));
        assert!(!report.contains("BUY"));
        assert!(!report.contains("SELL"));
        assert!(!report.contains("execution"));
        assert!(!report.contains("trend_cohesion"));
    }

    #[test]
    fn output_is_available_in_zh_en_ja() {
        for (language, title, state, notice) in [
            (
                Language::ZhCn,
                "灰犀牛升级监控",
                "状态: 风险常态化",
                "不生成交易信号。",
            ),
            (
                Language::EnUs,
                "Gray Rhino Escalation",
                "State: Normalized",
                "It does not generate trading signals.",
            ),
            (
                Language::JaJp,
                "灰色のサイ昇格監視",
                "状態: リスク常態化",
                "取引シグナルを生成しない。",
            ),
        ] {
            let report = render_gray_rhino_escalation_markdown(&normalized_escalation(), language);

            assert!(report.contains(title));
            assert!(report.contains(state));
            assert!(report.contains(notice));
        }
    }

    #[test]
    fn daily_assessment_discloses_source_date_and_changed_dimensions() {
        let previous = GrayRhinoAssessmentSnapshot {
            schema_version: 1,
            as_of_date: NaiveDate::from_ymd_opt(2026, 5, 21).unwrap(),
            source: GrayRhinoObservationSource::ManualConfiguration,
            escalation: evaluate_gray_rhino_escalation(GrayRhinoEscalationInput {
                risk_expansion_rate: RiskLevel::Low,
                constraint_growth_rate: RiskLevel::Moderate,
                dependency_centralization: RiskLevel::Moderate,
                awareness_decay: RiskLevel::Moderate,
                narrative_overconfidence: RiskLevel::Moderate,
                single_point_fragility: RiskLevel::Moderate,
                fallback_survivability_risk: RiskLevel::Moderate,
                notes: Vec::new(),
            }),
        };
        let assessment = build_gray_rhino_assessment(
            GrayRhinoEscalationInput {
                risk_expansion_rate: RiskLevel::Elevated,
                constraint_growth_rate: RiskLevel::Moderate,
                dependency_centralization: RiskLevel::High,
                awareness_decay: RiskLevel::Moderate,
                narrative_overconfidence: RiskLevel::Moderate,
                single_point_fragility: RiskLevel::Moderate,
                fallback_survivability_risk: RiskLevel::Moderate,
                notes: Vec::new(),
            },
            NaiveDate::from_ymd_opt(2026, 5, 22).unwrap(),
            Some(previous),
        );

        let report = render_gray_rhino_assessment_markdown(&assessment, Language::ZhCn);

        assert!(report.contains("评估日期: 2026-05-22"));
        assert!(report.contains("输入来源: 人工结构基线（配置输入）"));
        assert!(report.contains("评估方法: 显式规则判定（可重放）"));
        assert!(report.contains("审计链: 人工结构基线 -> 七项观测 -> 日次快照"));
        assert!(report.contains("变化项: 风险扩张速度 / 依赖集中度"));
        assert!(report.contains("不代表自动事实发现"));
    }

    #[test]
    fn facade_delegates_assessment_rendering_after_renderer_split() {
        let assessment = build_gray_rhino_assessment(
            GrayRhinoEscalationInput {
                risk_expansion_rate: RiskLevel::Elevated,
                constraint_growth_rate: RiskLevel::Moderate,
                dependency_centralization: RiskLevel::High,
                awareness_decay: RiskLevel::Moderate,
                narrative_overconfidence: RiskLevel::Moderate,
                single_point_fragility: RiskLevel::Moderate,
                fallback_survivability_risk: RiskLevel::Moderate,
                notes: Vec::new(),
            },
            NaiveDate::from_ymd_opt(2026, 5, 22).unwrap(),
            None,
        );

        let report = render_gray_rhino_assessment_markdown(&assessment, Language::EnUs);

        assert!(report.contains("Gray Rhino Escalation"));
        assert!(report.contains("Assessment Date: 2026-05-22"));
        assert!(report.contains("Evaluation Method: Explicit rule evaluation (replayable)"));
    }

    #[test]
    fn telegram_output_uses_same_structural_boundary() {
        let app_config = config::AppConfig {
            version: 1,
            output: config::OutputConfig {
                timezone: "UTC".to_string(),
                format: "markdown".to_string(),
                save_to: "./reports".to_string(),
                weight_kind: None,
                language: Some(Language::EnUs),
                compact_transition_evidence_in_no_trade: true,
            },
            telegram: None,
            futu: None,
            finnhub: None,
            fred: None,
            trading: None,
            provider: None,
            rules: config::RulesConfig {
                trend: config::TrendConfig::default(),
                deviation_bands: Default::default(),
                actions: Default::default(),
                sizing_multipliers: None,
                core_assets: None,
                min_state_duration: None,
                inertia: None,
                trend_cohesion: None,
                breakout: None,
                market_state_engine: None,
            },
            watchlist: Vec::new(),
            sec: None,
            research_attention: None,
            asset_thesis: None,
            macro_gravity: None,
            capital_absorption: None,
            capital_dynamics: None,
            gray_rhino_escalation: Some(config::GrayRhinoEscalationConfig {
                risk_expansion_rate: GrayRhinoRiskLevel::Elevated,
                constraint_growth_rate: GrayRhinoRiskLevel::Low,
                dependency_centralization: GrayRhinoRiskLevel::High,
                awareness_decay: GrayRhinoRiskLevel::High,
                narrative_overconfidence: GrayRhinoRiskLevel::Elevated,
                single_point_fragility: GrayRhinoRiskLevel::Moderate,
                fallback_survivability_risk: GrayRhinoRiskLevel::Moderate,
                notes: Some(vec![
                    "Market sensitivity to governance risk is declining.".into()
                ]),
                enable: Some(true),
            }),
            gray_rhino_provider_registry: None,
        };

        let report = build_gray_rhino_escalation_telegram_report(&app_config, Language::EnUs);

        assert!(report.contains("Market sensitivity to governance risk is declining."));
        assert!(report.contains("It does not generate trading signals."));
    }

    #[test]
    fn forbidden_notes_are_not_rendered() {
        let escalation = evaluate_gray_rhino_escalation(GrayRhinoEscalationInput {
            risk_expansion_rate: RiskLevel::Elevated,
            constraint_growth_rate: RiskLevel::Low,
            dependency_centralization: RiskLevel::High,
            awareness_decay: RiskLevel::High,
            narrative_overconfidence: RiskLevel::Elevated,
            single_point_fragility: RiskLevel::Moderate,
            fallback_survivability_risk: RiskLevel::Moderate,
            notes: vec![
                "Infrastructure concentration continues expanding.".to_string(),
                "马上卖出".to_string(),
                "Musk 非常危险".to_string(),
            ],
        });

        let report = render_gray_rhino_escalation_markdown(&escalation, Language::ZhCn);

        assert!(report.contains("Infrastructure concentration continues expanding."));
        assert!(report.contains("已抑制违反结构性观察边界的 notes: 2"));
        assert!(!report.contains("马上卖出"));
        assert!(!report.contains("Musk"));
    }
}

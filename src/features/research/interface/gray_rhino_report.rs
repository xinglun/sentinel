use crate::config::{self, GrayRhinoRiskLevel};
use crate::features::research::acl::gray_rhino_daily_report_factory::build_gray_rhino_daily_report_repository;
use crate::features::research::application::gray_rhino_daily_report::{
    GrayRhinoDailyReportUseCase, GrayRhinoDailyReportViewModel, GrayRhinoSnapshotPersistence,
};
#[cfg(test)]
use crate::features::research::domain::gray_rhino::{
    evaluate_gray_rhino_escalation, GrayRhinoAssessmentSnapshot,
};
use crate::features::research::domain::gray_rhino::{
    GrayRhinoAssessment, GrayRhinoEscalation, GrayRhinoEscalationInput, GrayRhinoObservationSource,
    RhinoEscalationState, RiskLevel,
};
use crate::features::research::domain::gray_rhino_candidate::GrayRhinoCandidate;
use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceCategory, GrayRhinoEvidenceRejection,
};
use crate::features::research::domain::gray_rhino_survivability_policy::{
    DependencyRiskLevel, GrayRhinoSurvivabilitySummary, SurvivabilityLevel,
};
use crate::features::research::domain::gray_rhino_temporal_policy::{
    GrayRhinoTemporalSummary, InstitutionalResponseState, TemperatureLevel, TemperatureVelocity,
    TemporalTrend,
};
use crate::features::research::interface::gray_rhino_inline_reference_renderer::{
    render_auto_discovery_inline_reference,
    render_gray_rhino_inline_reference as render_gray_rhino_inline_reference_impl,
};
use crate::features::research::interface::gray_rhino_renderer::{
    render_backfill_ops_view, render_discovery_ops_view, render_governance_sensor_health,
    render_refresh_status,
};
use crate::features::shared::interface::i18n::Language;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use std::collections::BTreeSet;
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
    let escalation = &assessment.current.escalation;
    let mut out = String::new();
    out.push_str(gray_rhino_title(language));
    out.push_str("\n\n");
    out.push_str(&format!(
        "{}: {}\n",
        assessment_date_heading(language),
        assessment.current.as_of_date
    ));
    out.push_str(&format!(
        "{}: {}\n",
        input_source_heading(language),
        observation_source_label(assessment.current.source, language)
    ));
    out.push_str(&format!(
        "{}: {}\n",
        evaluation_method_heading(language),
        evaluation_method_label(language)
    ));
    out.push_str(&format!(
        "{}: {}\n",
        audit_chain_heading(language),
        audit_chain_label(assessment.current.source, language)
    ));
    out.push_str(&format!(
        "{}: {}\n",
        state_heading(language),
        state_label(escalation.escalation_state, language)
    ));
    out.push_str(&format!(
        "{}: {}\n",
        escalation_heading(language),
        escalation_direction_label(escalation, language)
    ));
    out.push_str(&format!(
        "{}: {}\n",
        comparison_heading(language),
        comparison_label(assessment, language)
    ));
    out.push('\n');
    out.push_str(observation_label(language));
    out.push('\n');
    out.push_str(&format!(
        "- {}: {}\n",
        risk_expansion_label(language),
        risk_level_label(escalation.risk_expansion_rate, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        constraint_growth_label(language),
        risk_level_label(escalation.constraint_growth_rate, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        dependency_centralization_label(language),
        risk_level_label(escalation.dependency_centralization, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        awareness_decay_label(language),
        risk_level_label(escalation.awareness_decay, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        narrative_overconfidence_label(language),
        risk_level_label(escalation.narrative_overconfidence, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        single_point_fragility_label(language),
        risk_level_label(escalation.single_point_fragility, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        fallback_survivability_label(language),
        risk_level_label(escalation.fallback_survivability_risk, language)
    ));

    if !escalation.notes.is_empty() {
        out.push('\n');
        out.push_str(notes_label(language));
        out.push('\n');
        for note in &escalation.notes {
            out.push_str(&format!("- {}\n", note));
        }
    }
    if escalation.suppressed_note_count > 0 {
        out.push('\n');
        out.push_str(&format!(
            "{} {}\n",
            suppressed_notes_label(language),
            escalation.suppressed_note_count
        ));
    }

    out.push('\n');
    out.push_str(source_boundary_label(assessment.current.source, language));
    out.push('\n');
    out.push_str(boundary_label(language));
    out.push('\n');
    out.push_str(non_signal_notice(language));
    out
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

fn gray_rhino_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛升级监控",
        Language::EnUs => "Gray Rhino Escalation",
        Language::JaJp => "灰色のサイ昇格監視",
    }
}

fn gray_rhino_empty(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "灰犀牛升级监控\n\n风险升级评估: 尚无正式证据 / 未启用人工基线。\n\n自动观察候选会在下方独立列出，仅供跟踪。\n\n边界声明: 灰犀牛升级监控仅观察结构性风险升级，不生成交易信号。"
        }
        Language::EnUs => {
            "Gray Rhino Escalation\n\nFormal escalation assessment: no formal evidence / manual baseline is enabled.\n\nAuto-discovered observation candidates are listed below as isolated tracking reference.\n\nGray Rhino Escalation is a structural escalation monitor. It does not generate trading signals."
        }
        Language::JaJp => {
            "灰色のサイ昇格監視\n\n正式な昇格評価: 正式証拠 / 手動ベースラインは未有効です。\n\n自動発見された観測候補は下部に独立した追跡参考として表示します。\n\n境界声明: 灰色のサイ昇格監視は構造的リスクの昇格だけを観測し、取引シグナルを生成しない。"
        }
    }
}

fn state_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "状态",
        Language::EnUs => "State",
        Language::JaJp => "状態",
    }
}

fn escalation_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "升级趋势",
        Language::EnUs => "Escalation",
        Language::JaJp => "昇格傾向",
    }
}

fn state_label(state: RhinoEscalationState, language: Language) -> &'static str {
    match (state, language) {
        (RhinoEscalationState::Background, Language::ZhCn) => "背景观察",
        (RhinoEscalationState::Visible, Language::ZhCn) => "风险可见",
        (RhinoEscalationState::Expanding, Language::ZhCn) => "风险扩张",
        (RhinoEscalationState::Normalized, Language::ZhCn) => "风险常态化",
        (RhinoEscalationState::Critical, Language::ZhCn) => "临界监控",
        (RhinoEscalationState::Background, Language::EnUs) => "Background",
        (RhinoEscalationState::Visible, Language::EnUs) => "Visible",
        (RhinoEscalationState::Expanding, Language::EnUs) => "Expanding",
        (RhinoEscalationState::Normalized, Language::EnUs) => "Normalized",
        (RhinoEscalationState::Critical, Language::EnUs) => "Critical",
        (RhinoEscalationState::Background, Language::JaJp) => "背景観測",
        (RhinoEscalationState::Visible, Language::JaJp) => "リスク可視化",
        (RhinoEscalationState::Expanding, Language::JaJp) => "リスク拡張",
        (RhinoEscalationState::Normalized, Language::JaJp) => "リスク常態化",
        (RhinoEscalationState::Critical, Language::JaJp) => "臨界監視",
    }
}

fn assessment_date_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "评估日期",
        Language::EnUs => "Assessment Date",
        Language::JaJp => "評価日",
    }
}

fn input_source_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "输入来源",
        Language::EnUs => "Input Source",
        Language::JaJp => "入力由来",
    }
}

fn observation_source_label(
    source: GrayRhinoObservationSource,
    language: Language,
) -> &'static str {
    match (source, language) {
        (GrayRhinoObservationSource::ManualConfiguration, Language::ZhCn) => {
            "人工结构基线（配置输入）"
        }
        (GrayRhinoObservationSource::ManualConfiguration, Language::EnUs) => {
            "Manual structural baseline (configuration input)"
        }
        (GrayRhinoObservationSource::ManualConfiguration, Language::JaJp) => {
            "手動構造ベースライン（設定入力）"
        }
        (GrayRhinoObservationSource::EvidenceStore, Language::ZhCn) => {
            "Evidence-backed sensor store（结构化 evidence）"
        }
        (GrayRhinoObservationSource::EvidenceStore, Language::EnUs) => {
            "Evidence-backed sensor store"
        }
        (GrayRhinoObservationSource::EvidenceStore, Language::JaJp) => {
            "Evidence-backed sensor store（構造化 evidence）"
        }
    }
}

fn comparison_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "相比前次日次评估",
        Language::EnUs => "Versus Previous Daily Assessment",
        Language::JaJp => "前回日次評価との比較",
    }
}

fn evaluation_method_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "评估方法",
        Language::EnUs => "Evaluation Method",
        Language::JaJp => "評価方法",
    }
}

fn evaluation_method_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "显式规则判定（可重放）",
        Language::EnUs => "Explicit rule evaluation (replayable)",
        Language::JaJp => "明示ルール判定（再生可能）",
    }
}

fn audit_chain_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "审计链",
        Language::EnUs => "Audit Chain",
        Language::JaJp => "監査チェーン",
    }
}

fn audit_chain_label(source: GrayRhinoObservationSource, language: Language) -> &'static str {
    match (source, language) {
        (GrayRhinoObservationSource::ManualConfiguration, Language::ZhCn) => {
            "人工结构基线 -> 七项观测 -> 日次快照"
        }
        (GrayRhinoObservationSource::ManualConfiguration, Language::EnUs) => {
            "Manual structural baseline -> seven observations -> daily snapshot"
        }
        (GrayRhinoObservationSource::ManualConfiguration, Language::JaJp) => {
            "手動構造ベースライン -> 7 観測項目 -> 日次 snapshot"
        }
        (GrayRhinoObservationSource::EvidenceStore, Language::ZhCn) => {
            "结构化 EvidenceStore -> directional risk_effect -> 日次快照"
        }
        (GrayRhinoObservationSource::EvidenceStore, Language::EnUs) => {
            "Structured EvidenceStore -> directional risk_effect -> daily snapshot"
        }
        (GrayRhinoObservationSource::EvidenceStore, Language::JaJp) => {
            "構造化 EvidenceStore -> directional risk_effect -> 日次 snapshot"
        }
    }
}

fn render_multi_category_sensor_health(
    view_model: &GrayRhinoDailyReportViewModel,
    language: Language,
) -> String {
    let records = &view_model.evidence_records;
    let scoreable_records = &view_model.scoreable_evidence_records;
    let excluded_count = records.len().saturating_sub(scoreable_records.len())
        + view_model.rejected_evidence_records.len();
    let governance = render_governance_sensor_health(&view_model.governance_audits, language);
    if records.is_empty()
        && view_model.rejected_evidence_records.is_empty()
        && governance.is_empty()
    {
        return String::new();
    }
    let mut out = String::new();
    out.push_str(sensor_health_heading(language));
    out.push('\n');
    let categories = sensor_health_categories(language);
    let ready_count = categories
        .iter()
        .filter(|category| {
            scoreable_records
                .iter()
                .any(|record| category.matches(record.category))
        })
        .count();
    out.push_str(&format!(
        "- {}: {:.1}% ({}/{})\n",
        readiness_score_label(language),
        ready_count as f64 / categories.len() as f64 * 100.0,
        ready_count,
        categories.len()
    ));
    let average_confidence = if scoreable_records.is_empty() {
        0.0
    } else {
        scoreable_records
            .iter()
            .map(|record| record.confidence)
            .sum::<f64>()
            / scoreable_records.len() as f64
    };
    let source_diversity = scoreable_records
        .iter()
        .map(|record| record.source.publisher.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let quality_label = if ready_count >= 3 && average_confidence >= 0.75 {
        readiness_ready_label(language)
    } else if ready_count >= 2 && average_confidence >= 0.6 {
        readiness_partial_label(language)
    } else {
        readiness_insufficient_label(language)
    };
    out.push_str(&format!(
        "- {}: {quality_label} ({} {:.2}, {} {})\n",
        quality_score_label(language),
        average_confidence_label(language),
        average_confidence,
        source_diversity_label(language),
        source_diversity
    ));
    out.push_str(evidence_quality_dimensions_label(language));
    out.push('\n');
    if excluded_count > 0 {
        out.push_str(&format!(
            "- {}: {} ({})\n",
            excluded_evidence_count_label(language),
            excluded_count,
            excluded_evidence_reason_label(language)
        ));
        for rejection in &view_model.rejected_evidence_records {
            out.push_str(&format!(
                "  - {}: {}\n",
                rejection.source_title,
                evidence_rejection_reason_label(rejection.reason, language)
            ));
        }
    }
    for category in categories {
        let count = scoreable_records
            .iter()
            .filter(|record| category.matches(record.category))
            .count();
        let readiness = if count > 0 {
            readiness_ready_label(language)
        } else {
            readiness_insufficient_label(language)
        };
        out.push_str(&format!(
            "- {}: {count} {}, {}={readiness}\n",
            category.label,
            evidence_record_count_label(language),
            readiness_label(language)
        ));
    }
    if !governance.is_empty() {
        out.push('\n');
        out.push_str(&governance);
    }
    out.push('\n');
    out.push_str(evidence_explanation_graph_label(language));
    out.push('\n');
    out.push_str(evidence_explanation_graph_body(language));
    if let Some(ops_view) =
        render_backfill_ops_view(view_model.backfill_ops_view.as_ref(), language)
    {
        out.push('\n');
        out.push_str(&ops_view);
    }
    out
}

fn render_temporal_summary(summary: &GrayRhinoTemporalSummary, language: Language) -> String {
    let has_velocity = summary.escalation_velocity.is_some();
    let has_evidence_motion = summary.evidence_acceleration.recent_count > 0
        || summary.evidence_acceleration.prior_count > 0;
    let has_institutional_response =
        summary.institutional_response.state != InstitutionalResponseState::NoData;
    if !has_velocity && !has_evidence_motion && !has_institutional_response {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(temporal_summary_heading(language));
    out.push('\n');
    out.push_str(&format!(
        "- {}: {}\n",
        temperature_label(language),
        temperature_level_label(summary.temperature, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        temperature_velocity_label(language),
        temperature_velocity_value_label(summary.velocity, language)
    ));
    if let Some(velocity) = &summary.escalation_velocity {
        out.push_str(&format!(
            "- {}: {} ({} {}, {} {}, {} {})\n",
            escalation_velocity_label(language),
            temporal_trend_label(velocity.trend),
            delta_score_label(language),
            velocity.delta_score,
            delta_days_label(language),
            velocity.delta_days,
            changed_dimensions_count_label(language),
            velocity.changed_dimension_count
        ));
    }
    if has_evidence_motion {
        out.push_str(&format!(
            "- {}: {} ({} {}, {} {})\n",
            evidence_acceleration_label(language),
            temporal_trend_label(summary.evidence_acceleration.trend),
            recent_window_count_label(language),
            summary.evidence_acceleration.recent_count,
            prior_window_count_label(language),
            summary.evidence_acceleration.prior_count
        ));
    }
    if has_institutional_response {
        out.push_str(&format!(
            "- {}: {} ({} {}, {} {})\n",
            institutional_response_label(language),
            institutional_response_state_label(summary.institutional_response.state),
            mitigating_evidence_count_label(language),
            summary.institutional_response.mitigating_count,
            amplifying_gap_count_label(language),
            summary.institutional_response.amplifying_count
        ));
    }
    out.push_str(temporal_summary_boundary(language));
    out
}

fn render_survivability_summary(
    summary: &GrayRhinoSurvivabilitySummary,
    language: Language,
) -> String {
    let has_observed_dimension = summary.compute_control.level != SurvivabilityLevel::Unknown
        || summary.governance_resilience.level != SurvivabilityLevel::Unknown
        || summary.dependency_risk.level != DependencyRiskLevel::Unknown
        || summary.retry_capacity.level != SurvivabilityLevel::Unknown;
    if !has_observed_dimension {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(survivability_summary_heading(language));
    out.push('\n');
    out.push_str(&format!(
        "- {}: {}\n",
        capital_access_label(language),
        survivability_level_label(summary.capital_access)
    ));
    out.push_str(&format!(
        "- {}: {} ({} {}, {} {})\n",
        compute_control_label(language),
        survivability_level_label(summary.compute_control.level),
        mitigating_evidence_count_label(language),
        summary.compute_control.mitigating_count,
        amplifying_gap_count_label(language),
        summary.compute_control.amplifying_count
    ));
    out.push_str(&format!(
        "- {}: {} ({} {}, {} {})\n",
        governance_resilience_label(language),
        survivability_level_label(summary.governance_resilience.level),
        mitigating_evidence_count_label(language),
        summary.governance_resilience.mitigating_count,
        amplifying_gap_count_label(language),
        summary.governance_resilience.amplifying_count
    ));
    out.push_str(&format!(
        "- {}: {} ({} {}, {} {})\n",
        dependency_risk_label(language),
        dependency_risk_level_label(summary.dependency_risk.level),
        mitigating_evidence_count_label(language),
        summary.dependency_risk.mitigating_count,
        amplifying_gap_count_label(language),
        summary.dependency_risk.amplifying_count
    ));
    out.push_str(&format!(
        "- {}: {} ({} {}, {} {})\n",
        retry_capacity_label(language),
        survivability_level_label(summary.retry_capacity.level),
        mitigating_evidence_count_label(language),
        summary.retry_capacity.mitigating_count,
        amplifying_gap_count_label(language),
        summary.retry_capacity.amplifying_count
    ));
    out.push_str(survivability_summary_boundary(language));
    out
}

struct SensorHealthCategory {
    category: GrayRhinoEvidenceCategory,
    label: &'static str,
}

impl SensorHealthCategory {
    fn matches(&self, category: GrayRhinoEvidenceCategory) -> bool {
        self.category == category
    }
}

fn sensor_health_categories(language: Language) -> Vec<SensorHealthCategory> {
    vec![
        SensorHealthCategory {
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            label: match language {
                Language::ZhCn => "治理集中",
                Language::EnUs => "Governance Concentration",
                Language::JaJp => "ガバナンス集中",
            },
        },
        SensorHealthCategory {
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            label: match language {
                Language::ZhCn => "依赖集中",
                Language::EnUs => "Dependency Concentration",
                Language::JaJp => "依存集中",
            },
        },
        SensorHealthCategory {
            category: GrayRhinoEvidenceCategory::InstitutionalMaturity,
            label: match language {
                Language::ZhCn => "制度成熟度",
                Language::EnUs => "Institutional Maturity",
                Language::JaJp => "制度成熟度",
            },
        },
        SensorHealthCategory {
            category: GrayRhinoEvidenceCategory::Redundancy,
            label: match language {
                Language::ZhCn => "冗余能力",
                Language::EnUs => "Redundancy",
                Language::JaJp => "冗長性",
            },
        },
    ]
}

fn sensor_health_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛传感器健康度",
        Language::EnUs => "Gray Rhino Sensor Health",
        Language::JaJp => "灰色のサイセンサー健全性",
    }
}

fn temporal_summary_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛温度变化（只读参考）",
        Language::EnUs => "Gray Rhino Temperature Change (Reference Only)",
        Language::JaJp => "灰色のサイ温度変化（参照のみ）",
    }
}

fn survivability_summary_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "生存能力评估（只读参考）",
        Language::EnUs => "Survivability Assessment (Reference Only)",
        Language::JaJp => "生存能力評価（参照のみ）",
    }
}

fn capital_access_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资本可得性",
        Language::EnUs => "Capital access",
        Language::JaJp => "資本アクセス",
    }
}

fn compute_control_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "算力控制",
        Language::EnUs => "Compute control",
        Language::JaJp => "計算資源コントロール",
    }
}

fn governance_resilience_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "治理韧性",
        Language::EnUs => "Governance resilience",
        Language::JaJp => "ガバナンス耐性",
    }
}

fn dependency_risk_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "依赖风险",
        Language::EnUs => "Dependency risk",
        Language::JaJp => "依存リスク",
    }
}

fn retry_capacity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "再试能力",
        Language::EnUs => "Retry capacity",
        Language::JaJp => "再試行能力",
    }
}

fn escalation_velocity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "升级速度",
        Language::EnUs => "Escalation velocity",
        Language::JaJp => "エスカレーション速度",
    }
}

fn evidence_acceleration_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "证据加速度",
        Language::EnUs => "Evidence acceleration",
        Language::JaJp => "証拠加速度",
    }
}

fn temperature_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "温度",
        Language::EnUs => "Temperature",
        Language::JaJp => "温度",
    }
}

fn temperature_velocity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "速度",
        Language::EnUs => "Velocity",
        Language::JaJp => "速度",
    }
}

fn temperature_level_label(level: TemperatureLevel, language: Language) -> &'static str {
    match (level, language) {
        (TemperatureLevel::Low, Language::ZhCn) => "低",
        (TemperatureLevel::Medium, Language::ZhCn) => "中",
        (TemperatureLevel::High, Language::ZhCn) => "高",
        (TemperatureLevel::Critical, Language::ZhCn) => "临界",
        (TemperatureLevel::Low, Language::EnUs) => "Low",
        (TemperatureLevel::Medium, Language::EnUs) => "Medium",
        (TemperatureLevel::High, Language::EnUs) => "High",
        (TemperatureLevel::Critical, Language::EnUs) => "Critical",
        (TemperatureLevel::Low, Language::JaJp) => "低",
        (TemperatureLevel::Medium, Language::JaJp) => "中",
        (TemperatureLevel::High, Language::JaJp) => "高",
        (TemperatureLevel::Critical, Language::JaJp) => "臨界",
    }
}

fn temperature_velocity_value_label(
    velocity: TemperatureVelocity,
    language: Language,
) -> &'static str {
    match (velocity, language) {
        (TemperatureVelocity::Falling, Language::ZhCn) => "下降",
        (TemperatureVelocity::Stable, Language::ZhCn) => "稳定",
        (TemperatureVelocity::Rising, Language::ZhCn) => "上升",
        (TemperatureVelocity::Accelerating, Language::ZhCn) => "加速",
        (TemperatureVelocity::Falling, Language::EnUs) => "Falling",
        (TemperatureVelocity::Stable, Language::EnUs) => "Stable",
        (TemperatureVelocity::Rising, Language::EnUs) => "Rising",
        (TemperatureVelocity::Accelerating, Language::EnUs) => "Accelerating",
        (TemperatureVelocity::Falling, Language::JaJp) => "低下",
        (TemperatureVelocity::Stable, Language::JaJp) => "安定",
        (TemperatureVelocity::Rising, Language::JaJp) => "上昇",
        (TemperatureVelocity::Accelerating, Language::JaJp) => "加速",
    }
}

fn institutional_response_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "制度响应",
        Language::EnUs => "Institutional response",
        Language::JaJp => "制度対応",
    }
}

fn delta_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "分数变化",
        Language::EnUs => "delta score",
        Language::JaJp => "スコア変化",
    }
}

fn delta_days_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "天数",
        Language::EnUs => "days",
        Language::JaJp => "日数",
    }
}

fn changed_dimensions_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "变化维度",
        Language::EnUs => "changed dimensions",
        Language::JaJp => "変化次元",
    }
}

fn recent_window_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "近期",
        Language::EnUs => "recent",
        Language::JaJp => "直近",
    }
}

fn prior_window_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "前窗",
        Language::EnUs => "prior",
        Language::JaJp => "前期間",
    }
}

fn mitigating_evidence_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "缓解证据",
        Language::EnUs => "mitigating evidence",
        Language::JaJp => "緩和証拠",
    }
}

fn amplifying_gap_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "放大缺口",
        Language::EnUs => "amplifying gaps",
        Language::JaJp => "増幅ギャップ",
    }
}

fn temporal_summary_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: 温度变化只说明结构风险是否升温，不更新 Gate、execution、trend 或交易状态。"
        }
        Language::EnUs => {
            "Boundary: temperature change only describes structural risk motion; it does not update Gate, execution, trend, or trading state."
        }
        Language::JaJp => {
            "境界: 温度変化は構造リスクの動きだけを示し、Gate、execution、trend、取引状態を更新しない。"
        }
    }
}

fn survivability_summary_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: 生存能力评估只说明错误后的恢复余地，不生成乐观叙事、估值结论或交易动作。"
        }
        Language::EnUs => {
            "Boundary: survivability assessment only describes recovery capacity after mistakes; it does not generate optimistic narrative, valuation conclusions, or trading actions."
        }
        Language::JaJp => {
            "境界: 生存能力評価は誤り後の回復余地だけを示し、楽観 narrative、valuation 結論、取引行動を生成しない。"
        }
    }
}

fn temporal_trend_label(trend: TemporalTrend) -> &'static str {
    match trend {
        TemporalTrend::Rising => "RISING",
        TemporalTrend::Stable => "STABLE",
        TemporalTrend::Falling => "FALLING",
    }
}

fn survivability_level_label(level: SurvivabilityLevel) -> &'static str {
    match level {
        SurvivabilityLevel::Extreme => "EXTREME",
        SurvivabilityLevel::High => "HIGH",
        SurvivabilityLevel::Medium => "MEDIUM",
        SurvivabilityLevel::Low => "LOW",
        SurvivabilityLevel::Unknown => "UNKNOWN",
    }
}

fn dependency_risk_level_label(level: DependencyRiskLevel) -> &'static str {
    match level {
        DependencyRiskLevel::High => "HIGH",
        DependencyRiskLevel::Medium => "MEDIUM",
        DependencyRiskLevel::Low => "LOW",
        DependencyRiskLevel::Unknown => "UNKNOWN",
    }
}

fn institutional_response_state_label(state: InstitutionalResponseState) -> &'static str {
    match state {
        InstitutionalResponseState::Strong => "STRONG",
        InstitutionalResponseState::Adequate => "ADEQUATE",
        InstitutionalResponseState::Weak => "WEAK",
        InstitutionalResponseState::NoData => "NO_DATA",
    }
}

fn readiness_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "准备度评分",
        Language::EnUs => "Readiness score",
        Language::JaJp => "準備度スコア",
    }
}

fn quality_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "质量评分",
        Language::EnUs => "Quality score",
        Language::JaJp => "品質スコア",
    }
}

fn average_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "平均置信度",
        Language::EnUs => "avg confidence",
        Language::JaJp => "平均信頼度",
    }
}

fn source_diversity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源多样性",
        Language::EnUs => "source diversity",
        Language::JaJp => "由来の多様性",
    }
}

fn evidence_quality_dimensions_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 证据质量维度: 可追溯性 / 完整性 / 新鲜度 / 置信度 / 来源多样性 / 拒绝比例",
        Language::EnUs => "- Evidence quality dimensions: traceability / completeness / freshness / confidence / source diversity / rejection ratio",
        Language::JaJp => "- 証拠品質次元: 追跡可能性 / 完全性 / 鮮度 / 信頼度 / 由来の多様性 / 拒否比率",
    }
}

fn evidence_record_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "条证据记录",
        Language::EnUs => "evidence record(s)",
        Language::JaJp => "件の証拠記録",
    }
}

fn readiness_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "准备度",
        Language::EnUs => "readiness",
        Language::JaJp => "準備度",
    }
}

fn excluded_evidence_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "不可评分证据记录",
        Language::EnUs => "Non-scoreable evidence records",
        Language::JaJp => "採点対象外の証拠記録",
    }
}

fn excluded_evidence_reason_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "缺少主体或风险作用不可用于正式评分",
        Language::EnUs => "missing subject or risk effect is not scoreable",
        Language::JaJp => "主体欠落またはリスク作用が正式採点対象外",
    }
}

fn evidence_rejection_reason_label(
    reason: GrayRhinoEvidenceRejection,
    language: Language,
) -> &'static str {
    match (reason, language) {
        (GrayRhinoEvidenceRejection::MissingSubject, Language::ZhCn) => "缺少主体",
        (GrayRhinoEvidenceRejection::MissingSubject, Language::EnUs) => "missing subject",
        (GrayRhinoEvidenceRejection::MissingSubject, Language::JaJp) => "主体が欠落",
        (GrayRhinoEvidenceRejection::MissingSourceReference, Language::ZhCn) => "缺少来源引用",
        (GrayRhinoEvidenceRejection::MissingSourceReference, Language::EnUs) => {
            "missing source reference"
        }
        (GrayRhinoEvidenceRejection::MissingSourceReference, Language::JaJp) => "出典参照が欠落",
        (GrayRhinoEvidenceRejection::MissingSourceTitle, Language::ZhCn) => "缺少来源标题",
        (GrayRhinoEvidenceRejection::MissingSourceTitle, Language::EnUs) => "missing source title",
        (GrayRhinoEvidenceRejection::MissingSourceTitle, Language::JaJp) => "出典タイトルが欠落",
        (GrayRhinoEvidenceRejection::MissingPublisher, Language::ZhCn) => "缺少发布方",
        (GrayRhinoEvidenceRejection::MissingPublisher, Language::EnUs) => "missing publisher",
        (GrayRhinoEvidenceRejection::MissingPublisher, Language::JaJp) => "発行元が欠落",
        (GrayRhinoEvidenceRejection::MissingExtractionNote, Language::ZhCn) => "缺少提取说明",
        (GrayRhinoEvidenceRejection::MissingExtractionNote, Language::EnUs) => {
            "missing extraction note"
        }
        (GrayRhinoEvidenceRejection::MissingExtractionNote, Language::JaJp) => "抽出メモが欠落",
        (GrayRhinoEvidenceRejection::MissingStructuralFact, Language::ZhCn) => "缺少结构事实",
        (GrayRhinoEvidenceRejection::MissingStructuralFact, Language::EnUs) => {
            "missing structural fact"
        }
        (GrayRhinoEvidenceRejection::MissingStructuralFact, Language::JaJp) => "構造的事実が欠落",
        (GrayRhinoEvidenceRejection::ConfidenceOutOfRange, Language::ZhCn) => "置信度超出范围",
        (GrayRhinoEvidenceRejection::ConfidenceOutOfRange, Language::EnUs) => {
            "confidence out of range"
        }
        (GrayRhinoEvidenceRejection::ConfidenceOutOfRange, Language::JaJp) => "信頼度が範囲外",
        (GrayRhinoEvidenceRejection::NarrativeOnly, Language::ZhCn) => "仅为叙事性表述",
        (GrayRhinoEvidenceRejection::NarrativeOnly, Language::EnUs) => "narrative-only record",
        (GrayRhinoEvidenceRejection::NarrativeOnly, Language::JaJp) => "叙述のみの記録",
        (GrayRhinoEvidenceRejection::ForbiddenBoundaryTerm, Language::ZhCn) => "包含禁止边界词",
        (GrayRhinoEvidenceRejection::ForbiddenBoundaryTerm, Language::EnUs) => {
            "forbidden boundary term"
        }
        (GrayRhinoEvidenceRejection::ForbiddenBoundaryTerm, Language::JaJp) => "禁止された境界語",
        (GrayRhinoEvidenceRejection::UnsupportedSourceType, Language::ZhCn) => "来源类型不支持",
        (GrayRhinoEvidenceRejection::UnsupportedSourceType, Language::EnUs) => {
            "unsupported source type"
        }
        (GrayRhinoEvidenceRejection::UnsupportedSourceType, Language::JaJp) => "未対応の出典種別",
        (GrayRhinoEvidenceRejection::MissingGovernanceMetric, Language::ZhCn) => "缺少治理指标",
        (GrayRhinoEvidenceRejection::MissingGovernanceMetric, Language::EnUs) => {
            "missing governance metric"
        }
        (GrayRhinoEvidenceRejection::MissingGovernanceMetric, Language::JaJp) => {
            "ガバナンス指標が欠落"
        }
        (GrayRhinoEvidenceRejection::InvalidGovernanceMetric, Language::ZhCn) => "治理指标无效",
        (GrayRhinoEvidenceRejection::InvalidGovernanceMetric, Language::EnUs) => {
            "invalid governance metric"
        }
        (GrayRhinoEvidenceRejection::InvalidGovernanceMetric, Language::JaJp) => {
            "ガバナンス指標が無効"
        }
        (GrayRhinoEvidenceRejection::MissingDependencyMetric, Language::ZhCn) => "缺少依赖指标",
        (GrayRhinoEvidenceRejection::MissingDependencyMetric, Language::EnUs) => {
            "missing dependency metric"
        }
        (GrayRhinoEvidenceRejection::MissingDependencyMetric, Language::JaJp) => "依存指標が欠落",
        (GrayRhinoEvidenceRejection::InvalidDependencyMetric, Language::ZhCn) => "依赖指标无效",
        (GrayRhinoEvidenceRejection::InvalidDependencyMetric, Language::EnUs) => {
            "invalid dependency metric"
        }
        (GrayRhinoEvidenceRejection::InvalidDependencyMetric, Language::JaJp) => "依存指標が無効",
        (GrayRhinoEvidenceRejection::MissingInstitutionalMetric, Language::ZhCn) => {
            "缺少制度成熟度指标"
        }
        (GrayRhinoEvidenceRejection::MissingInstitutionalMetric, Language::EnUs) => {
            "missing institutional metric"
        }
        (GrayRhinoEvidenceRejection::MissingInstitutionalMetric, Language::JaJp) => {
            "制度成熟度指標が欠落"
        }
        (GrayRhinoEvidenceRejection::InvalidInstitutionalMetric, Language::ZhCn) => {
            "制度成熟度指标无效"
        }
        (GrayRhinoEvidenceRejection::InvalidInstitutionalMetric, Language::EnUs) => {
            "invalid institutional metric"
        }
        (GrayRhinoEvidenceRejection::InvalidInstitutionalMetric, Language::JaJp) => {
            "制度成熟度指標が無効"
        }
        (GrayRhinoEvidenceRejection::MissingRedundancyMetric, Language::ZhCn) => "缺少冗余指标",
        (GrayRhinoEvidenceRejection::MissingRedundancyMetric, Language::EnUs) => {
            "missing redundancy metric"
        }
        (GrayRhinoEvidenceRejection::MissingRedundancyMetric, Language::JaJp) => "冗長性指標が欠落",
        (GrayRhinoEvidenceRejection::InvalidRedundancyMetric, Language::ZhCn) => "冗余指标无效",
        (GrayRhinoEvidenceRejection::InvalidRedundancyMetric, Language::EnUs) => {
            "invalid redundancy metric"
        }
        (GrayRhinoEvidenceRejection::InvalidRedundancyMetric, Language::JaJp) => "冗長性指標が無効",
    }
}

fn readiness_ready_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "就绪",
        Language::EnUs => "ready",
        Language::JaJp => "準備完了",
    }
}

fn readiness_partial_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "部分就绪",
        Language::EnUs => "partial",
        Language::JaJp => "部分的に準備",
    }
}

fn readiness_insufficient_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "不足",
        Language::EnUs => "insufficient",
        Language::JaJp => "不足",
    }
}

fn evidence_explanation_graph_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "证据解释图",
        Language::EnUs => "Evidence Explanation Graph",
        Language::JaJp => "証拠説明グラフ",
    }
}

fn evidence_explanation_graph_body(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 依赖集中 -> 依赖集中证据 -> 供应商 / 云服务 / 基础设施披露\n- 后备生存风险 -> 依赖集中 + 冗余缺口 -> 后备与故障切换证据\n- 约束成长 -> 制度成熟度 -> 审计、监督、合规与继任证据\n- 风险扩张 -> 治理集中 + 依赖集中 -> 结构集中证据\n",
        Language::EnUs => "- dependency_centralization -> DependencyConcentration -> supplier/cloud/infrastructure disclosures\n- fallback_survivability_risk -> DependencyConcentration + Redundancy gap -> fallback and failover evidence\n- constraint_growth_rate -> InstitutionalMaturity -> audit, oversight, compliance maturity evidence\n- risk_expansion_rate -> GovernanceConcentration + DependencyConcentration -> structural concentration evidence\n",
        Language::JaJp => "- 依存集中 -> 依存集中証拠 -> 供給元 / クラウド / インフラ開示\n- 代替生存リスク -> 依存集中 + 冗長性不足 -> 代替とフェイルオーバー証拠\n- 制約成長 -> 制度成熟度 -> 監査、監督、コンプライアンス、継承証拠\n- リスク拡張 -> ガバナンス集中 + 依存集中 -> 構造集中証拠\n",
    }
}

fn render_unclassified_evidence_notice(count: usize, language: Language) -> String {
    match language {
        Language::ZhCn => format!(
            "旧证据记录不可评分\n- 缺少风险作用的记录数: {count}\n- 处理: 已载入但不参与正式升级评分，请重新投影或重新采集。"
        ),
        Language::EnUs => format!(
            "Unclassified legacy evidence\n- records missing risk_effect: {count}\n- handling: loaded but excluded from formal escalation scoring until re-projected or re-collected."
        ),
        Language::JaJp => format!(
            "未分類の旧証拠\n- リスク作用が欠落した記録数: {count}\n- 処理: 読み込みは行うが、再投影または再収集まで正式な昇格採点から除外する。"
        ),
    }
}

fn enabled_watch_symbols(app_config: &config::AppConfig) -> Vec<String> {
    app_config
        .watchlist
        .iter()
        .filter(|entry| entry.enable)
        .map(|entry| entry.symbol.clone())
        .collect()
}

fn comparison_label(assessment: &GrayRhinoAssessment, language: Language) -> String {
    let Some(previous) = assessment.previous.as_ref() else {
        return match language {
            Language::ZhCn => "首次记录（无前次快照）".to_string(),
            Language::EnUs => "First record (no prior snapshot)".to_string(),
            Language::JaJp => "初回記録（前回 snapshot なし）".to_string(),
        };
    };
    let changed = assessment.changed_dimension_keys();
    let state_change =
        if previous.escalation.escalation_state == assessment.current.escalation.escalation_state {
            match language {
                Language::ZhCn => "状态不变".to_string(),
                Language::EnUs => "State unchanged".to_string(),
                Language::JaJp => "状態変化なし".to_string(),
            }
        } else {
            format!(
                "{} -> {}",
                state_label(previous.escalation.escalation_state, language),
                state_label(assessment.current.escalation.escalation_state, language)
            )
        };
    if changed.is_empty() {
        return state_change;
    }
    let labels = changed
        .iter()
        .map(|key| dimension_key_label(key, language))
        .collect::<Vec<_>>()
        .join(" / ");
    match language {
        Language::ZhCn => format!("{state_change}；变化项: {labels}"),
        Language::EnUs => format!("{state_change}; changed: {labels}"),
        Language::JaJp => format!("{state_change}；変化項目: {labels}"),
    }
}

fn dimension_key_label(key: &str, language: Language) -> &'static str {
    match key {
        "risk_expansion_rate" => risk_expansion_label(language),
        "constraint_growth_rate" => constraint_growth_label(language),
        "dependency_centralization" => dependency_centralization_label(language),
        "awareness_decay" => awareness_decay_label(language),
        "narrative_overconfidence" => narrative_overconfidence_label(language),
        "single_point_fragility" => single_point_fragility_label(language),
        "fallback_survivability_risk" => fallback_survivability_label(language),
        _ => "",
    }
}

fn risk_level_label(level: RiskLevel, language: Language) -> &'static str {
    match (level, language) {
        (RiskLevel::Low, Language::ZhCn) => "低",
        (RiskLevel::Moderate, Language::ZhCn) => "中等",
        (RiskLevel::Elevated, Language::ZhCn) => "偏高",
        (RiskLevel::High, Language::ZhCn) => "高",
        (RiskLevel::Low, Language::EnUs) => "LOW",
        (RiskLevel::Moderate, Language::EnUs) => "MODERATE",
        (RiskLevel::Elevated, Language::EnUs) => "ELEVATED",
        (RiskLevel::High, Language::EnUs) => "HIGH",
        (RiskLevel::Low, Language::JaJp) => "低",
        (RiskLevel::Moderate, Language::JaJp) => "中程度",
        (RiskLevel::Elevated, Language::JaJp) => "高まり",
        (RiskLevel::High, Language::JaJp) => "高",
    }
}

fn escalation_direction_label(
    escalation: &GrayRhinoEscalation,
    language: Language,
) -> &'static str {
    match escalation.escalation_state {
        RhinoEscalationState::Critical => match language {
            Language::ZhCn => "临界监控",
            Language::EnUs => "Critical Watch",
            Language::JaJp => "臨界監視",
        },
        RhinoEscalationState::Normalized | RhinoEscalationState::Expanding => match language {
            Language::ZhCn => "上升",
            Language::EnUs => "Rising",
            Language::JaJp => "上昇",
        },
        RhinoEscalationState::Visible => match language {
            Language::ZhCn => "可见",
            Language::EnUs => "Visible",
            Language::JaJp => "可視",
        },
        RhinoEscalationState::Background => match language {
            Language::ZhCn => "背景",
            Language::EnUs => "Background",
            Language::JaJp => "背景",
        },
    }
}

fn observation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察项:",
        Language::EnUs => "Observation:",
        Language::JaJp => "観測項目:",
    }
}

fn risk_expansion_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "风险扩张速度",
        Language::EnUs => "Risk Expansion Rate",
        Language::JaJp => "リスク拡張速度",
    }
}

fn constraint_growth_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "制度成长速度",
        Language::EnUs => "Constraint Growth Rate",
        Language::JaJp => "制度成熟度の成長速度",
    }
}

fn dependency_centralization_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "依赖集中度",
        Language::EnUs => "Dependency Centralization",
        Language::JaJp => "依存集中度",
    }
}

fn awareness_decay_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "风险感知衰减",
        Language::EnUs => "Public Awareness Decay",
        Language::JaJp => "リスク感知の低下",
    }
}

fn narrative_overconfidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "叙事过度自信",
        Language::EnUs => "Narrative Overconfidence",
        Language::JaJp => "ナラティブ過信",
    }
}

fn single_point_fragility_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "单点脆弱性",
        Language::EnUs => "Single Point Fragility",
        Language::JaJp => "単一点脆弱性",
    }
}

fn fallback_survivability_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "后备生存性风险",
        Language::EnUs => "Fallback Survivability Risk",
        Language::JaJp => "フォールバック生存性リスク",
    }
}

fn notes_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察备注:",
        Language::EnUs => "Notes:",
        Language::JaJp => "観測メモ:",
    }
}

fn suppressed_notes_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已抑制违反结构性观察边界的 notes:",
        Language::EnUs => "Suppressed notes outside the structural observation boundary:",
        Language::JaJp => "構造観測境界の外にある notes を抑制:",
    }
}

fn boundary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: 该层只观察长期结构风险是否从背景风险进入危险临界区，不覆盖 Reality Layer、Tactical Layer 或 Execution Layer。"
        }
        Language::EnUs => {
            "Boundary: this layer only observes whether long-term structural risk is moving from background risk toward a dangerous threshold; it does not override the Reality Layer, Tactical Layer, or Execution Layer."
        }
        Language::JaJp => {
            "境界: このレイヤーは長期構造リスクが背景リスクから危険な臨界域へ移るかだけを観測し、Reality Layer、Tactical Layer、Execution Layer を上書きしない。"
        }
    }
}

fn source_boundary_label(source: GrayRhinoObservationSource, language: Language) -> &'static str {
    match (source, language) {
        (GrayRhinoObservationSource::ManualConfiguration, Language::ZhCn) => {
            "数据边界: 当前来源为人工配置的结构基线，尚未接入专用灰犀牛外部证据源，不代表自动事实发现。"
        }
        (GrayRhinoObservationSource::ManualConfiguration, Language::EnUs) => {
            "Data boundary: the current source is a manually configured structural baseline; no dedicated external Gray Rhino evidence source is connected, so this is not automated fact discovery."
        }
        (GrayRhinoObservationSource::ManualConfiguration, Language::JaJp) => {
            "データ境界: 現在の由来は手動設定した構造ベースラインであり、灰色のサイ専用の外部証拠源は未接続のため、自動的な事実発見を表さない。"
        }
        (GrayRhinoObservationSource::EvidenceStore, Language::ZhCn) => {
            "数据边界: 当前正式评估来自结构化 EvidenceStore；仅用于灰犀牛升级观察，不改变交易、闸门、趋势或市场状态。"
        }
        (GrayRhinoObservationSource::EvidenceStore, Language::EnUs) => {
            "Data boundary: the current formal assessment comes from the structured EvidenceStore; it is used only for Gray Rhino escalation observation and does not change trading, Gate, trend, or market state."
        }
        (GrayRhinoObservationSource::EvidenceStore, Language::JaJp) => {
            "データ境界: 現在の正式評価は構造化 EvidenceStore に由来し、灰色のサイ昇格観測にのみ使う。取引、ゲート、トレンド、市場状態は変更しない。"
        }
    }
}

fn non_signal_notice(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界声明: 灰犀牛升级监控仅观察结构性风险升级，不生成交易信号。"
        }
        Language::EnUs => {
            "Gray Rhino Escalation is a structural escalation monitor. It does not generate trading signals."
        }
        Language::JaJp => {
            "境界声明: 灰色のサイ昇格監視は構造的リスクの昇格だけを観測し、取引シグナルを生成しない。"
        }
    }
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

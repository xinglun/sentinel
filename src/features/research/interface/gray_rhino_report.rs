use crate::config::{self, GrayRhinoRiskLevel};
use crate::features::research::acl::gray_rhino_daily_report_factory::build_gray_rhino_daily_report_repository;
use crate::features::research::application::gray_rhino_daily_report::{
    BackfillOpsSummary, DiscoveryOpsSummary, GrayRhinoDailyReportUseCase,
    GrayRhinoDailyReportViewModel, GrayRhinoRefreshStatus, GrayRhinoSnapshotPersistence,
};
use crate::features::research::application::gray_rhino_monitoring_state::{
    GrayRhinoMonitoringDirection, GrayRhinoMonitoringStatus,
};
#[cfg(test)]
use crate::features::research::domain::gray_rhino::{
    evaluate_gray_rhino_escalation, GrayRhinoAssessmentSnapshot,
};
use crate::features::research::domain::gray_rhino::{
    GrayRhinoAssessment, GrayRhinoEscalation, GrayRhinoEscalationInput, GrayRhinoObservationSource,
    RhinoEscalationState, RiskLevel,
};
use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateKind, GrayRhinoCandidateScope, GrayRhinoCandidateState,
};
use crate::features::research::domain::gray_rhino_evidence::GrayRhinoEvidenceCategory;
use crate::features::shared::interface::i18n::Language;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use std::collections::{BTreeMap, BTreeSet};
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
        app_config,
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
    if candidates.is_empty() {
        return "Gray Rhino Inline Reference: none auto-discovered.\nBoundary: reference only; no trading, Gate, trend, or market-state mutation.".to_string();
    }
    let mut out = String::from("Gray Rhino Inline Reference (semantic isolation)\n");
    for candidate in candidates {
        out.push_str(&format!(
            "- {} / {:?} / {:?} / {:?}: {}\n",
            candidate.subject,
            candidate.scope,
            candidate.kind,
            candidate.state,
            candidate.evidence.join(" ")
        ));
        if !candidate.watch_triggers.is_empty() {
            out.push_str(&format!(
                "  Trigger watch: {}\n",
                candidate.watch_triggers.join(" / ")
            ));
        }
    }
    out.push_str("Boundary: reference only; no trading, Gate, trend, or market-state mutation.");
    out
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
    let governance = render_governance_sensor_health(&view_model.governance_audits, language);
    if records.is_empty() && governance.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push_str(sensor_health_heading(language));
    out.push('\n');
    let categories = sensor_health_categories(language);
    let ready_count = categories
        .iter()
        .filter(|category| {
            records
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
    let average_confidence = if records.is_empty() {
        0.0
    } else {
        records.iter().map(|record| record.confidence).sum::<f64>() / records.len() as f64
    };
    let source_diversity = records
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
    for category in categories {
        let count = records
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

fn render_backfill_ops_view(
    value: Option<&BackfillOpsSummary>,
    language: Language,
) -> Option<String> {
    let value = value?;
    let mut out = String::new();
    out.push_str(backfill_ops_title(language));
    out.push_str(&format!(
        "- {}: {}\n",
        latest_run_label(language),
        value.run_id
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        source_count_label(language),
        value.source_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        failed_sources_label(language),
        value.rejected
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        stale_sources_label(language),
        value.stale_sources
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        drift_sources_label(language),
        value.drift_sources
    ));
    Some(out)
}

fn render_discovery_ops_view(
    value: Option<&DiscoveryOpsSummary>,
    language: Language,
) -> Option<String> {
    let value = value?;
    let mut out = String::new();
    out.push_str(auto_discovery_ops_title(language));
    out.push_str(&format!(
        "- {}: {}\n",
        latest_run_label(language),
        value.run_id
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        source_count_label(language),
        value.source_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        candidate_count_label(language),
        value.candidate_count
    ));
    Some(out)
}

fn render_refresh_status(
    value: Option<&GrayRhinoRefreshStatus>,
    language: Language,
) -> Option<String> {
    let value = value?;
    let mut out = String::new();
    out.push_str(refresh_status_title(language));
    out.push_str(&format!(
        "- {}: {}\n",
        refresh_overall_status_label(language),
        refresh_status_value_label(&value.status, language)
    ));
    out.push_str(&format!(
        "- SEC: {} / Finnhub: {} / FRED: {}\n",
        refresh_status_value_label(&value.sec, language),
        refresh_status_value_label(&value.finnhub, language),
        refresh_status_value_label(&value.fred, language)
    ));
    out.push_str(&format!(
        "- {}: SEC {}/{} / Finnhub {}/{} / FRED {}/{}\n",
        refresh_coverage_label(language),
        value.sec_accepted,
        value.sec_accepted + value.sec_rejected,
        value.finnhub_accepted,
        value.finnhub_accepted + value.finnhub_rejected,
        value.fred_accepted,
        value.fred_accepted + value.fred_rejected
    ));
    if !value.failed_providers.trim().is_empty() {
        out.push_str(&format!(
            "- {}: {}\n",
            failed_providers_label(language),
            value.failed_providers.trim()
        ));
    }
    if let Some(date) = &value.date {
        out.push_str(&format!("- {}: {}\n", refresh_date_label(language), date));
    }
    if let Some(reason) = &value.reason {
        out.push_str(&format!(
            "- {}: {}\n",
            refresh_reason_label(language),
            reason
        ));
    }
    out.push_str(refresh_status_boundary(language));
    Some(out)
}

fn render_auto_discovery_inline_reference(
    app_config: &config::AppConfig,
    display_candidates: &[GrayRhinoCandidate],
    monitoring_statuses: &[GrayRhinoMonitoringStatus],
    language: Language,
) -> String {
    format!(
        "{}\n\n{}\n\n{}",
        render_gray_rhino_compact_summary(display_candidates, monitoring_statuses, language),
        render_watchlist_inline_candidates(app_config, display_candidates, language),
        render_watchlist_inline_monitoring(app_config, monitoring_statuses, language)
    )
}

fn render_gray_rhino_compact_summary(
    candidates: &[GrayRhinoCandidate],
    statuses: &[GrayRhinoMonitoringStatus],
    language: Language,
) -> String {
    let market_active = candidates
        .iter()
        .filter(|candidate| candidate.scope == GrayRhinoCandidateScope::Market)
        .count();
    let company_subjects = candidates
        .iter()
        .filter(|candidate| candidate.scope == GrayRhinoCandidateScope::Company)
        .map(|candidate| candidate.subject.to_uppercase())
        .collect::<BTreeSet<_>>();
    let intensifying_subjects = statuses
        .iter()
        .filter(|status| {
            status.scope == GrayRhinoCandidateScope::Company
                && status.direction == GrayRhinoMonitoringDirection::Intensifying
        })
        .map(|status| status.subject.to_uppercase())
        .collect::<BTreeSet<_>>();

    let company_summary = if company_subjects.is_empty() {
        none_label(language).to_string()
    } else {
        company_subjects.into_iter().collect::<Vec<_>>().join(", ")
    };
    let intensifying_summary = if intensifying_subjects.is_empty() {
        none_label(language).to_string()
    } else {
        intensifying_subjects
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!(
        "{}\n- {}: {market_active}\n- {}: {company_summary}\n- {}: {intensifying_summary}\n{}",
        gray_rhino_summary_title(language),
        market_active_label(language),
        company_active_label(language),
        company_intensifying_label(language),
        summary_boundary_label(language)
    )
}

fn render_watchlist_inline_candidates(
    app_config: &config::AppConfig,
    candidates: &[GrayRhinoCandidate],
    language: Language,
) -> String {
    if candidates.is_empty() {
        return format!(
            "{}: {}\n{}",
            inline_reference_title(language),
            none_auto_discovered_label(language),
            reference_boundary_label(language)
        );
    }

    let mut out = format!("{}\n", inline_reference_title(language));
    let market_candidates = candidates
        .iter()
        .filter(|candidate| candidate.scope == GrayRhinoCandidateScope::Market)
        .collect::<Vec<_>>();
    out.push_str(market_reference_title(language));
    out.push('\n');
    if market_candidates.is_empty() {
        out.push_str(&format!("- {}\n", none_label(language)));
    } else {
        for candidate in market_candidates {
            append_candidate_line(&mut out, candidate, language);
        }
    }

    out.push('\n');
    out.push_str(watchlist_reference_title(language));
    out.push('\n');
    let by_subject = group_company_candidates(candidates);
    let watch_symbols = enabled_watch_symbols(app_config);
    let watch_symbol_keys = watch_symbols
        .iter()
        .map(|symbol| symbol.to_uppercase())
        .collect::<BTreeSet<_>>();
    for symbol in &watch_symbols {
        out.push_str(&format!("- {symbol}\n"));
        if let Some(items) = by_subject.get(&symbol.to_uppercase()) {
            for candidate in items {
                append_candidate_line(&mut out, candidate, language);
            }
        } else {
            out.push_str(&format!(
                "  {}: {}\n",
                company_gray_rhino_label(language),
                none_label(language)
            ));
        }
    }
    let other_subjects = by_subject
        .keys()
        .filter(|subject| !watch_symbol_keys.contains(*subject))
        .collect::<Vec<_>>();
    if !other_subjects.is_empty() {
        out.push('\n');
        out.push_str(other_company_reference_title(language));
        out.push('\n');
        for subject in other_subjects {
            out.push_str(&format!("- {subject}\n"));
            if let Some(items) = by_subject.get(subject) {
                for candidate in items {
                    append_candidate_line(&mut out, candidate, language);
                }
            }
        }
    }
    out.push_str(reference_boundary_label(language));
    out
}

fn render_watchlist_inline_monitoring(
    app_config: &config::AppConfig,
    statuses: &[GrayRhinoMonitoringStatus],
    language: Language,
) -> String {
    if statuses.is_empty() {
        return format!(
            "{}: {}.\n{}",
            monitoring_status_title(language),
            none_label(language),
            reference_boundary_label(language)
        );
    }

    let mut out = format!("{}\n", monitoring_state_title(language));
    let market_statuses = statuses
        .iter()
        .filter(|status| status.scope == GrayRhinoCandidateScope::Market)
        .collect::<Vec<_>>();
    out.push_str(market_reference_title(language));
    out.push('\n');
    if market_statuses.is_empty() {
        out.push_str(&format!("- {}\n", none_label(language)));
    } else {
        for status in market_statuses {
            append_monitoring_line(&mut out, status, language);
        }
    }

    out.push('\n');
    out.push_str(watchlist_monitoring_title(language));
    out.push('\n');
    let by_subject = group_company_statuses(statuses);
    let watch_symbols = enabled_watch_symbols(app_config);
    let watch_symbol_keys = watch_symbols
        .iter()
        .map(|symbol| symbol.to_uppercase())
        .collect::<BTreeSet<_>>();
    for symbol in &watch_symbols {
        out.push_str(&format!("- {symbol}\n"));
        if let Some(items) = by_subject.get(&symbol.to_uppercase()) {
            for status in items {
                append_monitoring_line(&mut out, status, language);
            }
        } else {
            out.push_str(&format!(
                "  {}: {}\n",
                company_gray_rhino_monitoring_label(language),
                none_label(language)
            ));
        }
    }
    let other_subjects = by_subject
        .keys()
        .filter(|subject| !watch_symbol_keys.contains(*subject))
        .collect::<Vec<_>>();
    if !other_subjects.is_empty() {
        out.push('\n');
        out.push_str(other_company_monitoring_title(language));
        out.push('\n');
        for subject in other_subjects {
            out.push_str(&format!("- {subject}\n"));
            if let Some(items) = by_subject.get(subject) {
                for status in items {
                    append_monitoring_line(&mut out, status, language);
                }
            }
        }
    }
    out.push_str(reference_boundary_label(language));
    out
}

fn append_candidate_line(out: &mut String, candidate: &GrayRhinoCandidate, language: Language) {
    out.push_str(&format!(
        "  - {} / {} / {} / {}: {}\n",
        candidate.subject,
        candidate_scope_label(candidate.scope, language),
        candidate_kind_label(candidate.kind, language),
        candidate_state_label(candidate.state, language),
        candidate
            .evidence
            .iter()
            .map(|item| localized_structural_text(item, language))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    if !candidate.watch_triggers.is_empty() {
        out.push_str(&format!(
            "    {}: {}\n",
            trigger_watch_label(language),
            candidate
                .watch_triggers
                .iter()
                .map(|item| localized_structural_text(item, language))
                .collect::<Vec<_>>()
                .join(" / ")
        ));
    }
}

fn append_monitoring_line(
    out: &mut String,
    status: &GrayRhinoMonitoringStatus,
    language: Language,
) {
    out.push_str(&format!(
        "  - {} / {} / {}: {} ({}, {}: {}, {}: {}, {}: {})\n",
        status.subject,
        candidate_scope_label(status.scope, language),
        candidate_kind_label(status.kind, language),
        candidate_state_label(status.current_state, language),
        monitoring_direction_label(status.direction, language),
        observations_label(language),
        status.observation_count,
        latest_label(language),
        status.latest_observed_at,
        stale_days_label(language),
        status.stale_days
    ));
    if let Some(previous_state) = status.previous_state {
        out.push_str(&format!(
            "    {}: {}\n",
            previous_state_label(language),
            candidate_state_label(previous_state, language)
        ));
    }
}

fn gray_rhino_summary_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛摘要（语义隔离）",
        Language::EnUs => "Gray Rhino Summary (semantic isolation)",
        Language::JaJp => "灰色のサイ要約（意味的に隔離）",
    }
}

fn market_active_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "市场活跃候选",
        Language::EnUs => "Market active candidates",
        Language::JaJp => "市場の有効候補",
    }
}

fn company_active_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司活跃候选",
        Language::EnUs => "Company active candidates",
        Language::JaJp => "企業の有効候補",
    }
}

fn company_intensifying_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司升温观察",
        Language::EnUs => "Company intensifying watch",
        Language::JaJp => "企業の強まり観測",
    }
}

fn inline_reference_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛内联参考（语义隔离）",
        Language::EnUs => "Gray Rhino Inline Reference (semantic isolation)",
        Language::JaJp => "灰色のサイ内訳参考（意味的に隔離）",
    }
}

fn market_reference_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "市场参考",
        Language::EnUs => "Market Reference",
        Language::JaJp => "市場参考",
    }
}

fn watchlist_reference_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察列表内联参考",
        Language::EnUs => "Watchlist Inline Reference",
        Language::JaJp => "監視リスト内訳参考",
    }
}

fn other_company_reference_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "其他公司参考",
        Language::EnUs => "Other Company Reference",
        Language::JaJp => "その他企業参考",
    }
}

fn monitoring_status_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛监控状态",
        Language::EnUs => "Gray Rhino Monitoring Status",
        Language::JaJp => "灰色のサイ監視状態",
    }
}

fn monitoring_state_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛监控状态（语义隔离）",
        Language::EnUs => "Gray Rhino Monitoring State (semantic isolation)",
        Language::JaJp => "灰色のサイ監視状態（意味的に隔離）",
    }
}

fn watchlist_monitoring_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察列表内联监控",
        Language::EnUs => "Watchlist Inline Monitoring",
        Language::JaJp => "監視リスト内訳監視",
    }
}

fn other_company_monitoring_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "其他公司监控",
        Language::EnUs => "Other Company Monitoring",
        Language::JaJp => "その他企業監視",
    }
}

fn reference_boundary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "边界声明: 仅作结构风险参考；不改变交易、闸门、趋势或市场状态。",
        Language::EnUs => {
            "Boundary: reference only; no trading, Gate, trend, or market-state mutation."
        }
        Language::JaJp => {
            "境界声明: 構造リスクの参考のみで、取引、ゲート、トレンド、市場状態は変更しない。"
        }
    }
}

fn summary_boundary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "边界声明: 摘要仅供参考；不改变交易、闸门、趋势或市场状态。",
        Language::EnUs => {
            "Boundary: summary only; no trading, Gate, trend, or market-state mutation."
        }
        Language::JaJp => {
            "境界声明: 要約は参考のみで、取引、ゲート、トレンド、市場状態は変更しない。"
        }
    }
}

fn none_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无",
        Language::EnUs => "none",
        Language::JaJp => "なし",
    }
}

fn none_auto_discovered_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未发现自动候选",
        Language::EnUs => "none auto-discovered",
        Language::JaJp => "自動発見候補なし",
    }
}

fn company_gray_rhino_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司灰犀牛",
        Language::EnUs => "Company Gray Rhino",
        Language::JaJp => "企業灰色のサイ",
    }
}

fn company_gray_rhino_monitoring_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司灰犀牛监控",
        Language::EnUs => "Company Gray Rhino monitoring",
        Language::JaJp => "企業灰色のサイ監視",
    }
}

fn trigger_watch_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "触发观察",
        Language::EnUs => "Trigger watch",
        Language::JaJp => "トリガー観測",
    }
}

fn observations_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观测次数",
        Language::EnUs => "observations",
        Language::JaJp => "観測回数",
    }
}

fn latest_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "最新",
        Language::EnUs => "latest",
        Language::JaJp => "最新",
    }
}

fn stale_days_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "陈旧天数",
        Language::EnUs => "stale_days",
        Language::JaJp => "古さ（日）",
    }
}

fn previous_state_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "前次状态",
        Language::EnUs => "Previous state",
        Language::JaJp => "前回状態",
    }
}

fn candidate_scope_label(scope: GrayRhinoCandidateScope, language: Language) -> &'static str {
    match (scope, language) {
        (GrayRhinoCandidateScope::Company, Language::ZhCn) => "公司",
        (GrayRhinoCandidateScope::Market, Language::ZhCn) => "市场",
        (GrayRhinoCandidateScope::Company, Language::EnUs) => "Company",
        (GrayRhinoCandidateScope::Market, Language::EnUs) => "Market",
        (GrayRhinoCandidateScope::Company, Language::JaJp) => "企業",
        (GrayRhinoCandidateScope::Market, Language::JaJp) => "市場",
    }
}

fn candidate_kind_label(kind: GrayRhinoCandidateKind, language: Language) -> &'static str {
    match (kind, language) {
        (GrayRhinoCandidateKind::GovernanceConcentration, Language::ZhCn) => "治理集中",
        (GrayRhinoCandidateKind::DependencyConcentration, Language::ZhCn) => "依赖集中",
        (GrayRhinoCandidateKind::InstitutionalMaturityGap, Language::ZhCn) => "制度成熟缺口",
        (GrayRhinoCandidateKind::RedundancyGap, Language::ZhCn) => "冗余缺口",
        (GrayRhinoCandidateKind::MarketConcentration, Language::ZhCn) => "市场集中",
        (GrayRhinoCandidateKind::NarrativeCrowding, Language::ZhCn) => "叙事拥挤",
        (GrayRhinoCandidateKind::LiquidityFragility, Language::ZhCn) => "流动性脆弱",
        (GrayRhinoCandidateKind::CapexPaybackFragility, Language::ZhCn) => "资本开支回收脆弱",
        (GrayRhinoCandidateKind::GovernanceConcentration, Language::EnUs) => {
            "Governance Concentration"
        }
        (GrayRhinoCandidateKind::DependencyConcentration, Language::EnUs) => {
            "Dependency Concentration"
        }
        (GrayRhinoCandidateKind::InstitutionalMaturityGap, Language::EnUs) => {
            "Institutional Maturity Gap"
        }
        (GrayRhinoCandidateKind::RedundancyGap, Language::EnUs) => "Redundancy Gap",
        (GrayRhinoCandidateKind::MarketConcentration, Language::EnUs) => "Market Concentration",
        (GrayRhinoCandidateKind::NarrativeCrowding, Language::EnUs) => "Narrative Crowding",
        (GrayRhinoCandidateKind::LiquidityFragility, Language::EnUs) => "Liquidity Fragility",
        (GrayRhinoCandidateKind::CapexPaybackFragility, Language::EnUs) => {
            "Capex Payback Fragility"
        }
        (GrayRhinoCandidateKind::GovernanceConcentration, Language::JaJp) => "ガバナンス集中",
        (GrayRhinoCandidateKind::DependencyConcentration, Language::JaJp) => "依存集中",
        (GrayRhinoCandidateKind::InstitutionalMaturityGap, Language::JaJp) => "制度成熟度ギャップ",
        (GrayRhinoCandidateKind::RedundancyGap, Language::JaJp) => "冗長性ギャップ",
        (GrayRhinoCandidateKind::MarketConcentration, Language::JaJp) => "市場集中",
        (GrayRhinoCandidateKind::NarrativeCrowding, Language::JaJp) => "ナラティブ過密",
        (GrayRhinoCandidateKind::LiquidityFragility, Language::JaJp) => "流動性脆弱性",
        (GrayRhinoCandidateKind::CapexPaybackFragility, Language::JaJp) => "設備投資回収脆弱性",
    }
}

fn candidate_state_label(state: GrayRhinoCandidateState, language: Language) -> &'static str {
    match (state, language) {
        (GrayRhinoCandidateState::Background, Language::ZhCn) => "背景观察",
        (GrayRhinoCandidateState::Visible, Language::ZhCn) => "可见",
        (GrayRhinoCandidateState::Expanding, Language::ZhCn) => "扩张",
        (GrayRhinoCandidateState::Critical, Language::ZhCn) => "临界",
        (GrayRhinoCandidateState::Cooling, Language::ZhCn) => "降温",
        (GrayRhinoCandidateState::Resolved, Language::ZhCn) => "解除",
        (GrayRhinoCandidateState::Background, Language::EnUs) => "Background",
        (GrayRhinoCandidateState::Visible, Language::EnUs) => "Visible",
        (GrayRhinoCandidateState::Expanding, Language::EnUs) => "Expanding",
        (GrayRhinoCandidateState::Critical, Language::EnUs) => "Critical",
        (GrayRhinoCandidateState::Cooling, Language::EnUs) => "Cooling",
        (GrayRhinoCandidateState::Resolved, Language::EnUs) => "Resolved",
        (GrayRhinoCandidateState::Background, Language::JaJp) => "背景観測",
        (GrayRhinoCandidateState::Visible, Language::JaJp) => "可視",
        (GrayRhinoCandidateState::Expanding, Language::JaJp) => "拡張",
        (GrayRhinoCandidateState::Critical, Language::JaJp) => "臨界",
        (GrayRhinoCandidateState::Cooling, Language::JaJp) => "低下",
        (GrayRhinoCandidateState::Resolved, Language::JaJp) => "解消",
    }
}

fn monitoring_direction_label(
    direction: GrayRhinoMonitoringDirection,
    language: Language,
) -> &'static str {
    match (direction, language) {
        (GrayRhinoMonitoringDirection::New, Language::ZhCn) => "新增",
        (GrayRhinoMonitoringDirection::Stable, Language::ZhCn) => "稳定",
        (GrayRhinoMonitoringDirection::Intensifying, Language::ZhCn) => "升温",
        (GrayRhinoMonitoringDirection::Cooling, Language::ZhCn) => "降温",
        (GrayRhinoMonitoringDirection::Resolved, Language::ZhCn) => "解除",
        (GrayRhinoMonitoringDirection::New, Language::EnUs) => "New",
        (GrayRhinoMonitoringDirection::Stable, Language::EnUs) => "Stable",
        (GrayRhinoMonitoringDirection::Intensifying, Language::EnUs) => "Intensifying",
        (GrayRhinoMonitoringDirection::Cooling, Language::EnUs) => "Cooling",
        (GrayRhinoMonitoringDirection::Resolved, Language::EnUs) => "Resolved",
        (GrayRhinoMonitoringDirection::New, Language::JaJp) => "新規",
        (GrayRhinoMonitoringDirection::Stable, Language::JaJp) => "安定",
        (GrayRhinoMonitoringDirection::Intensifying, Language::JaJp) => "強まり",
        (GrayRhinoMonitoringDirection::Cooling, Language::JaJp) => "低下",
        (GrayRhinoMonitoringDirection::Resolved, Language::JaJp) => "解消",
    }
}

fn localized_structural_text(value: &str, language: Language) -> String {
    if matches!(language, Language::EnUs) {
        return value.to_string();
    }
    let lower = value.to_lowercase();
    let translated = match language {
        Language::ZhCn => {
            if lower.contains("market-level structural concentration") {
                Some("检测到市场层面的结构集中。")
            } else if lower.contains("liquidity or rate-pressure fragility") {
                Some("检测到流动性或利率压力脆弱性。")
            } else if lower.contains("capex payback fragility") {
                Some("检测到资本开支回收脆弱性。")
            } else if lower.contains("narrative crowding") {
                Some("检测到叙事拥挤。")
            } else if lower.contains("single dependency") || lower.contains("missing fallback") {
                Some("检测到单一依赖或后备路径缺失。")
            } else if lower.contains("founder") && lower.contains("voting control") {
                Some("检测到创始人或单一主体投票控制。")
            } else if lower.contains("governance check-and-balance weakness") {
                Some("检测到治理制衡弱点。")
            } else if lower.contains("ipo voting terms") {
                Some("IPO 投票条款")
            } else if lower.contains("board composition changes") {
                Some("董事会构成变化")
            } else if lower.contains("related-party transactions") {
                Some("关联交易")
            } else if lower.contains("founder control changes") {
                Some("创始人控制权变化")
            } else if lower.contains("supplier disruption") {
                Some("供应商中断")
            } else if lower.contains("cloud outage") {
                Some("云服务中断")
            } else if lower.contains("fallback disclosure change") {
                Some("后备路径披露变化")
            } else if lower.contains("breadth deterioration") {
                Some("市场广度恶化")
            } else if lower.contains("liquidity tightening") {
                Some("流动性收紧")
            } else if lower.contains("capex payback disappointment") {
                Some("资本开支回收不及预期")
            } else if lower.contains("yield curve deterioration") {
                Some("收益率曲线恶化")
            } else if lower.contains("credit spread widening") {
                Some("信用利差扩大")
            } else if lower.contains("central-bank liquidity shift") {
                Some("央行流动性变化")
            } else if lower.contains("utilization gap") {
                Some("利用率缺口")
            } else if lower.contains("earnings disappointment") {
                Some("盈利不及预期")
            } else if lower.contains("capex guidance revision") {
                Some("资本开支指引修正")
            } else if lower.contains("headline concentration") {
                Some("新闻标题集中")
            } else if lower.contains("single-theme leadership") {
                Some("单一主题领涨")
            } else if lower.contains("positioning reversal") {
                Some("仓位反转")
            } else {
                None
            }
        }
        Language::JaJp => {
            if lower.contains("market-level structural concentration") {
                Some("市場レベルの構造集中を検出。")
            } else if lower.contains("liquidity or rate-pressure fragility") {
                Some("流動性または金利圧力の脆弱性を検出。")
            } else if lower.contains("capex payback fragility") {
                Some("設備投資回収の脆弱性を検出。")
            } else if lower.contains("narrative crowding") {
                Some("ナラティブ過密を検出。")
            } else if lower.contains("single dependency") || lower.contains("missing fallback") {
                Some("単一依存または代替経路の不足を検出。")
            } else if lower.contains("founder") && lower.contains("voting control") {
                Some("創業者または単一主体の議決権支配を検出。")
            } else if lower.contains("governance check-and-balance weakness") {
                Some("ガバナンスの牽制不足を検出。")
            } else if lower.contains("ipo voting terms") {
                Some("IPO 議決権条件")
            } else if lower.contains("board composition changes") {
                Some("取締役会構成の変化")
            } else if lower.contains("related-party transactions") {
                Some("関連当事者取引")
            } else if lower.contains("founder control changes") {
                Some("創業者支配の変化")
            } else if lower.contains("supplier disruption") {
                Some("supplier 障害")
            } else if lower.contains("cloud outage") {
                Some("cloud 障害")
            } else if lower.contains("fallback disclosure change") {
                Some("代替経路開示の変化")
            } else if lower.contains("breadth deterioration") {
                Some("市場 breadth 悪化")
            } else if lower.contains("liquidity tightening") {
                Some("流動性引き締まり")
            } else if lower.contains("capex payback disappointment") {
                Some("設備投資回収の未達")
            } else if lower.contains("yield curve deterioration") {
                Some("イールドカーブ悪化")
            } else if lower.contains("credit spread widening") {
                Some("信用スプレッド拡大")
            } else if lower.contains("central-bank liquidity shift") {
                Some("中央銀行流動性の変化")
            } else if lower.contains("utilization gap") {
                Some("稼働率ギャップ")
            } else if lower.contains("earnings disappointment") {
                Some("利益未達")
            } else if lower.contains("capex guidance revision") {
                Some("設備投資 guidance 修正")
            } else if lower.contains("headline concentration") {
                Some("headline 集中")
            } else if lower.contains("single-theme leadership") {
                Some("単一テーマ主導")
            } else if lower.contains("positioning reversal") {
                Some("positioning 反転")
            } else {
                None
            }
        }
        Language::EnUs => None,
    };
    translated.unwrap_or(value).to_string()
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

fn backfill_ops_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "回填运维视图\n",
        Language::EnUs => "Backfill Ops View\n",
        Language::JaJp => "回填運用ビュー\n",
    }
}

fn failed_sources_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "失败来源数",
        Language::EnUs => "failed_sources",
        Language::JaJp => "失敗した由来数",
    }
}

fn stale_sources_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "陈旧来源数",
        Language::EnUs => "stale_sources",
        Language::JaJp => "古い由来数",
    }
}

fn drift_sources_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "漂移来源数",
        Language::EnUs => "drift_sources",
        Language::JaJp => "漂移した由来数",
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

fn auto_discovery_ops_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "自动发现运维视图\n",
        Language::EnUs => "Auto Discovery Ops View\n",
        Language::JaJp => "自動発見運用ビュー\n",
    }
}

fn latest_run_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "最新运行",
        Language::EnUs => "latest_run",
        Language::JaJp => "最新実行",
    }
}

fn source_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源数",
        Language::EnUs => "source_count",
        Language::JaJp => "由来数",
    }
}

fn candidate_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "候选数",
        Language::EnUs => "candidate_count",
        Language::JaJp => "候補数",
    }
}

fn refresh_status_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛采集状态\n",
        Language::EnUs => "Gray Rhino Refresh Status\n",
        Language::JaJp => "灰色のサイ収集状態\n",
    }
}

fn refresh_overall_status_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "整体状态",
        Language::EnUs => "overall_status",
        Language::JaJp => "全体状態",
    }
}

fn refresh_status_value_label(value: &str, language: Language) -> &'static str {
    match (value, language) {
        ("succeeded", Language::ZhCn) => "成功",
        ("partial_failure", Language::ZhCn) => "部分失败",
        ("failed", Language::ZhCn) => "失败",
        ("skipped", Language::ZhCn) => "跳过",
        ("succeeded", Language::EnUs) => "succeeded",
        ("partial_failure", Language::EnUs) => "partial_failure",
        ("failed", Language::EnUs) => "failed",
        ("skipped", Language::EnUs) => "skipped",
        ("succeeded", Language::JaJp) => "成功",
        ("partial_failure", Language::JaJp) => "部分失敗",
        ("failed", Language::JaJp) => "失敗",
        ("skipped", Language::JaJp) => "未実行",
        _ => "unknown",
    }
}

fn refresh_coverage_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "覆盖率",
        Language::EnUs => "coverage",
        Language::JaJp => "取得カバー率",
    }
}

fn failed_providers_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "失败来源",
        Language::EnUs => "failed_providers",
        Language::JaJp => "失敗した取得元",
    }
}

fn refresh_date_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "采集日期",
        Language::EnUs => "refresh_date",
        Language::JaJp => "収集日",
    }
}

fn refresh_reason_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "原因",
        Language::EnUs => "reason",
        Language::JaJp => "理由",
    }
}

fn refresh_status_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界声明: 采集状态仅说明自动情报新鲜度；不改变交易、闸门、趋势或市场状态。"
        }
        Language::EnUs => {
            "Boundary: refresh status only explains intelligence freshness; it does not change trading, Gate, trend, or market state."
        }
        Language::JaJp => {
            "境界: 収集状態は自動情報の鮮度だけを説明し、取引、ゲート、トレンド、市場状態を変更しない。"
        }
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

fn group_company_candidates(
    candidates: &[GrayRhinoCandidate],
) -> BTreeMap<String, Vec<&GrayRhinoCandidate>> {
    let mut by_subject: BTreeMap<String, Vec<&GrayRhinoCandidate>> = BTreeMap::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.scope == GrayRhinoCandidateScope::Company)
    {
        by_subject
            .entry(candidate.subject.to_uppercase())
            .or_default()
            .push(candidate);
    }
    by_subject
}

fn group_company_statuses(
    statuses: &[GrayRhinoMonitoringStatus],
) -> BTreeMap<String, Vec<&GrayRhinoMonitoringStatus>> {
    let mut by_subject: BTreeMap<String, Vec<&GrayRhinoMonitoringStatus>> = BTreeMap::new();
    for status in statuses
        .iter()
        .filter(|status| status.scope == GrayRhinoCandidateScope::Company)
    {
        by_subject
            .entry(status.subject.to_uppercase())
            .or_default()
            .push(status);
    }
    by_subject
}

fn render_governance_sensor_health(
    audits: &[crate::features::research::domain::governance_source::GovernanceExtractionAuditRecord],
    language: Language,
) -> String {
    if audits.is_empty() {
        return String::new();
    }
    let source_count = audits.len();
    let accepted_count = audits.iter().filter(|audit| audit.accepted).count();
    let rejected_count = source_count.saturating_sub(accepted_count);
    let latest_observed = audits.iter().map(|audit| audit.observed_at).max();
    let coverage_ratio = accepted_count as f64 / source_count as f64;

    let mut out = String::new();
    out.push_str(governance_sensor_health_heading(language));
    out.push('\n');
    out.push_str(&format!(
        "- {}: {}\n",
        governance_sensor_source_count_label(language),
        source_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        governance_sensor_accepted_label(language),
        accepted_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        governance_sensor_rejected_label(language),
        rejected_count
    ));
    out.push_str(&format!(
        "- {}: {:.1}%\n",
        governance_sensor_coverage_label(language),
        coverage_ratio * 100.0
    ));
    if let Some(latest) = latest_observed {
        out.push_str(&format!(
            "- {}: {}\n",
            governance_sensor_latest_label(language),
            latest
        ));
    }
    out.push_str(governance_sensor_boundary_label(language));
    out
}

fn governance_sensor_health_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "治理传感器健康度",
        Language::EnUs => "Governance Sensor Health",
        Language::JaJp => "ガバナンスセンサー健全性",
    }
}

fn governance_sensor_source_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源数",
        Language::EnUs => "Source count",
        Language::JaJp => "由来数",
    }
}

fn governance_sensor_accepted_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已接受",
        Language::EnUs => "Accepted",
        Language::JaJp => "受理済み",
    }
}

fn governance_sensor_rejected_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已拒绝",
        Language::EnUs => "Rejected",
        Language::JaJp => "拒否済み",
    }
}

fn governance_sensor_coverage_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "覆盖率",
        Language::EnUs => "Coverage ratio",
        Language::JaJp => "カバー率",
    }
}

fn governance_sensor_latest_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "最新观测日",
        Language::EnUs => "Latest observed date",
        Language::JaJp => "最新観測日",
    }
}

fn governance_sensor_boundary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界声明: 治理传感器健康度仅用于证据覆盖检查，不更新升级状态、交易执行或交易状态。"
        }
        Language::EnUs => {
            "Boundary: Governance sensor health only; no escalation, Gate, execution, or trading state is updated."
        }
        Language::JaJp => {
            "境界声明: ガバナンスセンサー健全性は証拠カバー率の確認のみで、昇格状態、実行、取引状態を更新しない。"
        }
    }
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

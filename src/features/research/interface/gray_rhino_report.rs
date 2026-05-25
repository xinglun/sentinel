use crate::config::{self, GrayRhinoRiskLevel};
use crate::features::research::application::dependency_evidence::DependencyEvidenceRepository;
use crate::features::research::application::governance_evidence::GovernanceEvidenceRepository;
use crate::features::research::application::governance_source_pipeline::GovernanceSourceAuditRepository;
use crate::features::research::application::gray_rhino_assessment::{
    build_evidence_backed_gray_rhino_assessment, build_gray_rhino_assessment,
};
use crate::features::research::application::gray_rhino_discovery::{
    discover_gray_rhino_candidates, render_gray_rhino_inline_reference, GrayRhinoDiscoveryInput,
};
use crate::features::research::application::gray_rhino_monitoring_state::{
    evaluate_gray_rhino_monitoring_states, render_gray_rhino_monitoring_states,
};
use crate::features::research::application::institutional_evidence::InstitutionalEvidenceRepository;
use crate::features::research::application::redundancy_evidence::RedundancyEvidenceRepository;
#[cfg(test)]
use crate::features::research::domain::gray_rhino::{
    evaluate_gray_rhino_escalation, GrayRhinoAssessmentSnapshot,
};
use crate::features::research::domain::gray_rhino::{
    GrayRhinoAssessment, GrayRhinoEscalation, GrayRhinoEscalationInput, GrayRhinoObservationSource,
    RhinoEscalationState, RiskLevel,
};
use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateScope,
};
use crate::features::research::infrastructure::gray_rhino_candidate_store::GrayRhinoCandidateStore;
use crate::features::research::infrastructure::gray_rhino_evidence_store::GrayRhinoEvidenceStore;
use crate::features::research::infrastructure::gray_rhino_snapshot_store::GrayRhinoSnapshotStore;
use crate::features::shared::interface::i18n::Language;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) fn build_gray_rhino_escalation_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    let save_dir = Path::new(&app_config.output.save_to);
    if let Ok(records) = load_gray_rhino_evidence_records(save_dir) {
        if let Some(assessment) =
            build_evidence_backed_gray_rhino_assessment(&records, Local::now().date_naive(), None)
        {
            return render_gray_rhino_assessment_markdown(&assessment, language);
        }
    }
    let Some(input) = input_from_config(app_config) else {
        return gray_rhino_empty(language).to_string();
    };

    let assessment = build_gray_rhino_assessment(input, Local::now().date_naive(), None);
    render_gray_rhino_assessment_markdown(&assessment, language)
}

pub(crate) fn build_gray_rhino_daily_report(
    app_config: &config::AppConfig,
    save_dir: &Path,
    as_of_date: NaiveDate,
    language: Language,
) -> Result<String> {
    let store = GrayRhinoSnapshotStore::new(save_dir);
    let previous = store.load_latest_before(as_of_date)?;
    let records = load_gray_rhino_evidence_records(save_dir)?;
    let assessment = if let Some(assessment) =
        build_evidence_backed_gray_rhino_assessment(&records, as_of_date, previous.clone())
    {
        assessment
    } else {
        let Some(input) = input_from_config(app_config) else {
            let mut report = gray_rhino_empty(language).to_string();
            report.push_str("\n\n");
            report.push_str(&render_auto_discovery_inline_reference(
                app_config, save_dir, as_of_date,
            ));
            if let Some(discovery_ops_view) = render_discovery_ops_view(save_dir) {
                report.push_str("\n\n");
                report.push_str(&discovery_ops_view);
            }
            return Ok(report);
        };
        build_gray_rhino_assessment(input, as_of_date, previous)
    };
    store.save_if_changed(&assessment.current)?;
    let sensor_health = render_multi_category_sensor_health(save_dir, language)?;
    let mut report = render_gray_rhino_assessment_markdown(&assessment, language);
    if !sensor_health.is_empty() {
        report.push_str("\n\n");
        report.push_str(&sensor_health);
    }
    report.push_str("\n\n");
    report.push_str(&render_auto_discovery_inline_reference(
        app_config, save_dir, as_of_date,
    ));
    if let Some(discovery_ops_view) = render_discovery_ops_view(save_dir) {
        report.push_str("\n\n");
        report.push_str(&discovery_ops_view);
    }
    Ok(report)
}

fn load_gray_rhino_evidence_records(
    save_dir: &Path,
) -> Result<Vec<crate::features::research::domain::gray_rhino_evidence::GrayRhinoEvidenceRecord>> {
    let store = GrayRhinoEvidenceStore::new(save_dir);
    let mut records = store.load_governance_evidence()?;
    records.extend(store.load_dependency_evidence()?);
    records.extend(store.load_institutional_evidence()?);
    records.extend(store.load_redundancy_evidence()?);
    Ok(records)
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
        audit_chain_label(language)
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
    out.push_str(source_boundary_label(language));
    out.push('\n');
    out.push_str(boundary_label(language));
    out.push('\n');
    out.push_str(non_signal_notice(language));
    out
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

#[cfg(test)]
pub(crate) fn build_gray_rhino_escalation_telegram_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    build_gray_rhino_escalation_report(app_config, language)
}

fn map_risk_level(level: GrayRhinoRiskLevel) -> RiskLevel {
    match level {
        GrayRhinoRiskLevel::Low => RiskLevel::Low,
        GrayRhinoRiskLevel::Moderate => RiskLevel::Moderate,
        GrayRhinoRiskLevel::Elevated => RiskLevel::Elevated,
        GrayRhinoRiskLevel::High => RiskLevel::High,
    }
}

fn gray_rhino_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛升级监控（Gray Rhino Escalation）",
        Language::EnUs => "Gray Rhino Escalation",
        Language::JaJp => "灰色のサイ昇格監視（Gray Rhino Escalation）",
    }
}

fn gray_rhino_empty(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "灰犀牛升级监控（Gray Rhino Escalation）\n\n未配置灰犀牛风险升级观察项。\n\n当前未启用观察项，因此本节不参与日报判断。\n\n边界声明: 灰犀牛升级监控仅观察结构性风险升级，不生成交易信号。"
        }
        Language::EnUs => {
            "Gray Rhino Escalation\n\nNo gray rhino escalation monitor is configured.\n\nNo observation item is enabled, so this section does not participate in daily report judgment.\n\nGray Rhino Escalation is a structural escalation monitor. It does not generate trading signals."
        }
        Language::JaJp => {
            "灰色のサイ昇格監視（Gray Rhino Escalation）\n\n灰色のサイのリスク昇格観測項目は未設定です。\n\n現在有効な観測項目がないため、このセクションは日次判断に参加しない。\n\n境界声明: 灰色のサイ昇格監視は構造的リスクの昇格だけを観測し、取引シグナルを生成しない。"
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

fn audit_chain_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "人工结构基线 -> 七项观测 -> 日次快照",
        Language::EnUs => "Manual structural baseline -> seven observations -> daily snapshot",
        Language::JaJp => "手動構造ベースライン -> 7 観測項目 -> 日次 snapshot",
    }
}

fn render_multi_category_sensor_health(save_dir: &Path, language: Language) -> Result<String> {
    let records = load_gray_rhino_evidence_records(save_dir)?;
    let governance = render_governance_sensor_health(save_dir, language)?;
    if records.is_empty() && governance.is_empty() {
        return Ok(String::new());
    }
    let mut out = String::new();
    out.push_str(match language {
        Language::ZhCn => "Gray Rhino sensor health",
        Language::EnUs => "Gray Rhino Sensor Health",
        Language::JaJp => "Gray Rhino sensor health",
    });
    out.push('\n');
    let categories = [
        "GovernanceConcentration",
        "DependencyConcentration",
        "InstitutionalMaturity",
        "Redundancy",
    ];
    let ready_count = categories
        .iter()
        .filter(|category| {
            records
                .iter()
                .any(|record| format!("{:?}", record.category) == **category)
        })
        .count();
    out.push_str(&format!(
        "- Readiness score: {:.1}% ({}/{})\n",
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
        "ready"
    } else if ready_count >= 2 && average_confidence >= 0.6 {
        "partial"
    } else {
        "insufficient"
    };
    out.push_str(&format!(
        "- Quality score: {quality_label} (avg confidence {:.2}, source diversity {})\n",
        average_confidence, source_diversity
    ));
    out.push_str("- Evidence quality dimensions: traceability / completeness / freshness / confidence / source diversity / rejection ratio\n");
    for category in categories {
        let count = records
            .iter()
            .filter(|record| format!("{:?}", record.category) == category)
            .count();
        let readiness = if count > 0 { "ready" } else { "insufficient" };
        out.push_str(&format!(
            "- {category}: {count} evidence record(s), readiness={readiness}\n"
        ));
    }
    if !governance.is_empty() {
        out.push('\n');
        out.push_str(&governance);
    }
    out.push('\n');
    out.push_str("Evidence Explanation Graph\n");
    out.push_str("- dependency_centralization -> DependencyConcentration -> supplier/cloud/infrastructure disclosures\n");
    out.push_str("- fallback_survivability_risk -> DependencyConcentration + Redundancy gap -> fallback and failover evidence\n");
    out.push_str("- constraint_growth_rate -> InstitutionalMaturity -> audit, oversight, compliance maturity evidence\n");
    out.push_str("- risk_expansion_rate -> GovernanceConcentration + DependencyConcentration -> structural concentration evidence\n");
    if let Some(ops_view) = render_backfill_ops_view(save_dir) {
        out.push('\n');
        out.push_str(&ops_view);
    }
    if let Some(discovery_ops_view) = render_discovery_ops_view(save_dir) {
        out.push('\n');
        out.push_str(&discovery_ops_view);
    }
    Ok(out)
}

fn render_backfill_ops_view(save_dir: &Path) -> Option<String> {
    let path = save_dir.join("gray_rhino_backfill_runs.jsonl");
    let raw = std::fs::read_to_string(path).ok()?;
    let latest = raw.lines().rev().find(|line| !line.trim().is_empty())?;
    let value: serde_json::Value = serde_json::from_str(latest).ok()?;
    let mut out = String::new();
    out.push_str("Backfill Ops View\n");
    out.push_str(&format!(
        "- latest_run: {}\n",
        value
            .get("run_id")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
    ));
    out.push_str(&format!(
        "- source_count: {}\n",
        value
            .get("source_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    ));
    out.push_str(&format!(
        "- failed_sources: {}\n",
        value
            .get("rejected")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    ));
    out.push_str(&format!(
        "- stale_sources: {}\n",
        value
            .get("stale_sources")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    ));
    out.push_str(&format!(
        "- drift_sources: {}\n",
        value
            .get("drift_sources")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    ));
    Some(out)
}

fn render_discovery_ops_view(save_dir: &Path) -> Option<String> {
    let path = save_dir.join("gray_rhino_discovery_runs.jsonl");
    let raw = std::fs::read_to_string(path).ok()?;
    let latest = raw.lines().rev().find(|line| !line.trim().is_empty())?;
    let value: serde_json::Value = serde_json::from_str(latest).ok()?;
    let mut out = String::new();
    out.push_str("Auto Discovery Ops View\n");
    out.push_str(&format!(
        "- latest_run: {}\n",
        value
            .get("run_id")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
    ));
    out.push_str(&format!(
        "- source_count: {}\n",
        value
            .get("source_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    ));
    out.push_str(&format!(
        "- candidate_count: {}\n",
        value
            .get("candidate_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    ));
    Some(out)
}

fn render_auto_discovery_inline_reference(
    app_config: &config::AppConfig,
    save_dir: &Path,
    as_of_date: NaiveDate,
) -> String {
    let candidates = collect_auto_discovered_candidates(app_config, save_dir, as_of_date);
    let display_candidates = dedupe_candidates(candidates.clone());
    let monitoring_statuses = evaluate_gray_rhino_monitoring_states(&candidates, as_of_date);
    format!(
        "{}\n\n{}",
        render_gray_rhino_inline_reference(&display_candidates),
        render_gray_rhino_monitoring_states(&monitoring_statuses)
    )
}

fn collect_auto_discovered_candidates(
    app_config: &config::AppConfig,
    save_dir: &Path,
    as_of_date: NaiveDate,
) -> Vec<GrayRhinoCandidate> {
    let source_roots = [
        save_dir.join("gray_rhino_sources"),
        save_dir.join("gray_rhino_raw_sources"),
    ];
    let mut files = Vec::new();
    for root in source_roots {
        collect_text_files(&root, &mut files);
    }
    let watch_symbols: Vec<String> = app_config
        .watchlist
        .iter()
        .filter(|entry| entry.enable)
        .map(|entry| entry.symbol.clone())
        .collect();
    let default_subject = watch_symbols
        .first()
        .cloned()
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let mut candidates = Vec::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let path_text = path.to_string_lossy().to_string();
        let path_components = path
            .components()
            .filter_map(|component| component.as_os_str().to_str())
            .map(|component| component.to_uppercase())
            .collect::<Vec<_>>();
        let subject = watch_symbols
            .iter()
            .find(|symbol| {
                let symbol = symbol.to_uppercase();
                path_components.iter().any(|component| {
                    component == &symbol || component.starts_with(&format!("{symbol}_"))
                })
            })
            .cloned()
            .or_else(|| {
                path.parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| default_subject.clone());
        let source_is_typed_company_cache = path.components().any(|component| {
            matches!(
                component.as_os_str().to_str(),
                Some("governance" | "narrative")
            )
        });
        if source_is_typed_company_cache
            && !watch_symbols
                .iter()
                .any(|watch_symbol| watch_symbol.eq_ignore_ascii_case(&subject))
        {
            continue;
        }
        candidates.extend(discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject,
            source_title: path_text,
            observed_at: as_of_date,
            text,
        }));
    }
    if let Ok(persisted_candidates) = GrayRhinoCandidateStore::new(save_dir).load_candidates() {
        candidates.extend(
            persisted_candidates
                .into_iter()
                .filter(|candidate| candidate_in_current_report_scope(candidate, &watch_symbols)),
        );
    }
    candidates
}

fn candidate_in_current_report_scope(
    candidate: &GrayRhinoCandidate,
    watch_symbols: &[String],
) -> bool {
    candidate.scope == GrayRhinoCandidateScope::Market
        || watch_symbols
            .iter()
            .any(|symbol| symbol.eq_ignore_ascii_case(&candidate.subject))
}

fn dedupe_candidates(candidates: Vec<GrayRhinoCandidate>) -> Vec<GrayRhinoCandidate> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for candidate in candidates {
        let key = format!(
            "{}::{:?}::{:?}",
            candidate.subject, candidate.scope, candidate.kind
        );
        if seen.insert(key) {
            deduped.push(candidate);
        }
    }
    deduped
}

fn collect_text_files(path: &Path, out: &mut Vec<PathBuf>) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if metadata.is_file() {
        if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("txt" | "md" | "html" | "htm")
        ) {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_text_files(&entry.path(), out);
    }
}

fn render_governance_sensor_health(save_dir: &Path, language: Language) -> Result<String> {
    let store = GrayRhinoEvidenceStore::new(save_dir);
    let audits = store.load_governance_extraction_audits()?;
    if audits.is_empty() {
        return Ok(String::new());
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
    Ok(out)
}

fn governance_sensor_health_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Governance sensor health",
        Language::EnUs => "Governance Sensor Health",
        Language::JaJp => "Governance sensor health",
    }
}

fn governance_sensor_source_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "source 数",
        Language::EnUs => "Source count",
        Language::JaJp => "source 数",
    }
}

fn governance_sensor_accepted_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "accepted",
        Language::EnUs => "Accepted",
        Language::JaJp => "accepted",
    }
}

fn governance_sensor_rejected_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "rejected",
        Language::EnUs => "Rejected",
        Language::JaJp => "rejected",
    }
}

fn governance_sensor_coverage_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "coverage ratio",
        Language::EnUs => "Coverage ratio",
        Language::JaJp => "coverage ratio",
    }
}

fn governance_sensor_latest_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "latest observed date",
        Language::EnUs => "Latest observed date",
        Language::JaJp => "latest observed date",
    }
}

fn governance_sensor_boundary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "Boundary: Governance sensor health only; no escalation, Gate, execution, or trading state is updated."
        }
        Language::EnUs => {
            "Boundary: Governance sensor health only; no escalation, Gate, execution, or trading state is updated."
        }
        Language::JaJp => {
            "Boundary: Governance sensor health only; no escalation, Gate, execution, or trading state is updated."
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

fn source_boundary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "数据边界: 当前来源为人工配置的结构基线，尚未接入专用灰犀牛外部证据源，不代表自动事实发现。"
        }
        Language::EnUs => {
            "Data boundary: the current source is a manually configured structural baseline; no dedicated external Gray Rhino evidence source is connected, so this is not automated fact discovery."
        }
        Language::JaJp => {
            "データ境界: 現在の由来は手動設定した構造ベースラインであり、灰色のサイ専用の外部 evidence source は未接続のため、自動的な事実発見を表さない。"
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
        for (language, state, notice) in [
            (Language::ZhCn, "状态: 风险常态化", "不生成交易信号。"),
            (
                Language::EnUs,
                "State: Normalized",
                "It does not generate trading signals.",
            ),
            (
                Language::JaJp,
                "状態: リスク常態化",
                "取引シグナルを生成しない。",
            ),
        ] {
            let report = render_gray_rhino_escalation_markdown(&normalized_escalation(), language);

            assert!(report.contains("Gray Rhino Escalation"));
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

use crate::features::research::domain::gray_rhino::{
    GrayRhinoAssessment, GrayRhinoEscalation, GrayRhinoObservationSource, RhinoEscalationState,
    RiskLevel,
};
use crate::features::shared::interface::i18n::Language;

pub(super) fn render_gray_rhino_assessment_markdown(
    assessment: &GrayRhinoAssessment,
    language: Language,
) -> String {
    let escalation = &assessment.current.escalation;
    if is_idle_background_assessment(assessment) {
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
            state_heading(language),
            state_label(escalation.escalation_state, language)
        ));
        out.push_str(&format!("{}\n", idle_deterioration_message(language)));
        out.push_str(&format!("{}\n", idle_monitoring_message(language)));
        out.push('\n');
        out.push_str(boundary_label(language));
        out.push('\n');
        out.push_str(non_signal_notice(language));
        return out;
    }

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

fn is_idle_background_assessment(assessment: &GrayRhinoAssessment) -> bool {
    let escalation = &assessment.current.escalation;
    escalation.escalation_state == RhinoEscalationState::Background
        && escalation.risk_expansion_rate == RiskLevel::Low
        && escalation.constraint_growth_rate == RiskLevel::Low
        && escalation.dependency_centralization == RiskLevel::Low
        && escalation.awareness_decay == RiskLevel::Low
        && escalation.narrative_overconfidence == RiskLevel::Low
        && escalation.single_point_fragility == RiskLevel::Low
        && escalation.fallback_survivability_risk == RiskLevel::Low
        && escalation.notes.is_empty()
        && escalation.suppressed_note_count == 0
}

fn idle_deterioration_message(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "No structural deterioration observed.",
        Language::EnUs => "No structural deterioration observed.",
        Language::JaJp => "No structural deterioration observed.",
    }
}

fn idle_monitoring_message(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Current monitoring remains idle.",
        Language::EnUs => "Current monitoring remains idle.",
        Language::JaJp => "Current monitoring remains idle.",
    }
}

pub(super) fn gray_rhino_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛升级监控",
        Language::EnUs => "Gray Rhino Escalation",
        Language::JaJp => "灰色のサイ昇格監視",
    }
}

pub(super) fn gray_rhino_empty(language: Language) -> &'static str {
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

pub(super) fn state_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "状态",
        Language::EnUs => "State",
        Language::JaJp => "状態",
    }
}

pub(super) fn escalation_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "升级趋势",
        Language::EnUs => "Escalation",
        Language::JaJp => "昇格傾向",
    }
}

pub(super) fn state_label(state: RhinoEscalationState, language: Language) -> &'static str {
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

pub(super) fn assessment_date_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "评估日期",
        Language::EnUs => "Assessment Date",
        Language::JaJp => "評価日",
    }
}

pub(super) fn input_source_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "输入来源",
        Language::EnUs => "Input Source",
        Language::JaJp => "入力由来",
    }
}

pub(super) fn observation_source_label(
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

pub(super) fn comparison_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "相比前次日次评估",
        Language::EnUs => "Versus Previous Daily Assessment",
        Language::JaJp => "前回日次評価との比較",
    }
}

pub(super) fn evaluation_method_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "评估方法",
        Language::EnUs => "Evaluation Method",
        Language::JaJp => "評価方法",
    }
}

pub(super) fn evaluation_method_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "显式规则判定（可重放）",
        Language::EnUs => "Explicit rule evaluation (replayable)",
        Language::JaJp => "明示ルール判定（再生可能）",
    }
}

pub(super) fn audit_chain_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "审计链",
        Language::EnUs => "Audit Chain",
        Language::JaJp => "監査チェーン",
    }
}

pub(super) fn audit_chain_label(
    source: GrayRhinoObservationSource,
    language: Language,
) -> &'static str {
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

pub(super) fn comparison_label(assessment: &GrayRhinoAssessment, language: Language) -> String {
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

pub(super) fn dimension_key_label(key: &str, language: Language) -> &'static str {
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

pub(super) fn risk_level_label(level: RiskLevel, language: Language) -> &'static str {
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

pub(super) fn escalation_direction_label(
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

pub(super) fn observation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察项:",
        Language::EnUs => "Observation:",
        Language::JaJp => "観測項目:",
    }
}

pub(super) fn risk_expansion_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "风险扩张速度",
        Language::EnUs => "Risk Expansion Rate",
        Language::JaJp => "リスク拡張速度",
    }
}

pub(super) fn constraint_growth_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "制度成长速度",
        Language::EnUs => "Constraint Growth Rate",
        Language::JaJp => "制度成熟度の成長速度",
    }
}

pub(super) fn dependency_centralization_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "依赖集中度",
        Language::EnUs => "Dependency Centralization",
        Language::JaJp => "依存集中度",
    }
}

pub(super) fn awareness_decay_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "风险感知衰减",
        Language::EnUs => "Public Awareness Decay",
        Language::JaJp => "リスク感知の低下",
    }
}

pub(super) fn narrative_overconfidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "叙事过度自信",
        Language::EnUs => "Narrative Overconfidence",
        Language::JaJp => "ナラティブ過信",
    }
}

pub(super) fn single_point_fragility_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "单点脆弱性",
        Language::EnUs => "Single Point Fragility",
        Language::JaJp => "単一点脆弱性",
    }
}

pub(super) fn fallback_survivability_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "后备生存性风险",
        Language::EnUs => "Fallback Survivability Risk",
        Language::JaJp => "フォールバック生存性リスク",
    }
}

pub(super) fn notes_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察备注:",
        Language::EnUs => "Notes:",
        Language::JaJp => "観測メモ:",
    }
}

pub(super) fn suppressed_notes_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已抑制违反结构性观察边界的 notes:",
        Language::EnUs => "Suppressed notes outside the structural observation boundary:",
        Language::JaJp => "構造観測境界の外にある notes を抑制:",
    }
}

pub(super) fn boundary_label(language: Language) -> &'static str {
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

pub(super) fn source_boundary_label(
    source: GrayRhinoObservationSource,
    language: Language,
) -> &'static str {
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

pub(super) fn non_signal_notice(language: Language) -> &'static str {
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
    use crate::features::research::domain::gray_rhino::{
        GrayRhinoAssessment, GrayRhinoAssessmentSnapshot, GrayRhinoEscalation,
        GrayRhinoObservationSource, RhinoEscalationState, RiskLevel,
    };
    use chrono::NaiveDate;

    #[test]
    fn background_assessment_renders_idle_copy_without_detail_rows() {
        let assessment = GrayRhinoAssessment {
            current: GrayRhinoAssessmentSnapshot {
                schema_version: 1,
                as_of_date: NaiveDate::from_ymd_opt(2026, 7, 8).unwrap(),
                source: GrayRhinoObservationSource::ManualConfiguration,
                escalation: GrayRhinoEscalation {
                    escalation_state: RhinoEscalationState::Background,
                    risk_expansion_rate: RiskLevel::Low,
                    constraint_growth_rate: RiskLevel::Low,
                    dependency_centralization: RiskLevel::Low,
                    awareness_decay: RiskLevel::Low,
                    narrative_overconfidence: RiskLevel::Low,
                    single_point_fragility: RiskLevel::Low,
                    fallback_survivability_risk: RiskLevel::Low,
                    notes: vec![],
                    suppressed_note_count: 0,
                },
            },
            previous: None,
        };

        let report = render_gray_rhino_assessment_markdown(&assessment, Language::EnUs);

        assert!(report.contains("No structural deterioration observed."));
        assert!(report.contains("Current monitoring remains idle."));
        assert!(!report.contains("Risk Expansion"));
        assert!(!report.contains("Constraint Growth"));
        assert!(!report.contains("Notes"));
    }
}

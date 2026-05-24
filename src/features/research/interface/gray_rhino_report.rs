use crate::config::{self, GrayRhinoRiskLevel};
use crate::features::research::domain::gray_rhino::{
    evaluate_gray_rhino_escalation, GrayRhinoEscalation, GrayRhinoEscalationInput,
    RhinoEscalationState, RiskLevel,
};
use crate::features::shared::interface::i18n::Language;

pub(crate) fn build_gray_rhino_escalation_report(
    app_config: &config::AppConfig,
    language: Language,
) -> String {
    let Some(config) = app_config
        .gray_rhino_escalation
        .as_ref()
        .filter(|config| config.enable.unwrap_or(true))
    else {
        return gray_rhino_empty(language).to_string();
    };

    let escalation = evaluate_gray_rhino_escalation(GrayRhinoEscalationInput {
        risk_expansion_rate: map_risk_level(config.risk_expansion_rate),
        constraint_growth_rate: map_risk_level(config.constraint_growth_rate),
        dependency_centralization: map_risk_level(config.dependency_centralization),
        awareness_decay: map_risk_level(config.awareness_decay),
        narrative_overconfidence: map_risk_level(config.narrative_overconfidence),
        single_point_fragility: map_risk_level(config.single_point_fragility),
        fallback_survivability_risk: map_risk_level(config.fallback_survivability_risk),
        notes: config.notes.clone().unwrap_or_default(),
    });

    render_gray_rhino_escalation_markdown(&escalation, language)
}

pub(crate) fn render_gray_rhino_escalation_markdown(
    escalation: &GrayRhinoEscalation,
    language: Language,
) -> String {
    let mut out = String::new();
    out.push_str(gray_rhino_title(language));
    out.push_str("\n\n");
    out.push_str(&format!(
        "State: {}\n",
        state_label(escalation.escalation_state)
    ));
    out.push_str(&format!(
        "Escalation: {}\n",
        escalation_direction_label(escalation, language)
    ));
    out.push_str(&format!(
        "Escalation Score: {}\n\n",
        escalation.escalation_score()
    ));
    out.push_str(observation_label(language));
    out.push('\n');
    out.push_str(&format!(
        "- {}: {}\n",
        risk_expansion_label(language),
        risk_level_label(escalation.risk_expansion_rate)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        constraint_growth_label(language),
        risk_level_label(escalation.constraint_growth_rate)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        dependency_centralization_label(language),
        risk_level_label(escalation.dependency_centralization)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        awareness_decay_label(language),
        risk_level_label(escalation.awareness_decay)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        narrative_overconfidence_label(language),
        risk_level_label(escalation.narrative_overconfidence)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        single_point_fragility_label(language),
        risk_level_label(escalation.single_point_fragility)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        fallback_survivability_label(language),
        risk_level_label(escalation.fallback_survivability_risk)
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
            "灰犀牛升级监控（Gray Rhino Escalation）\n\n未配置灰犀牛风险升级观察项。\n\n当前未启用观察项，因此本节不参与日报判断。\n\nGray Rhino Escalation 是结构性升级监控层，不生成交易信号。"
        }
        Language::EnUs => {
            "Gray Rhino Escalation\n\nNo gray rhino escalation monitor is configured.\n\nNo observation item is enabled, so this section does not participate in daily report judgment.\n\nGray Rhino Escalation is a structural escalation monitor. It does not generate trading signals."
        }
        Language::JaJp => {
            "灰色のサイ昇格監視（Gray Rhino Escalation）\n\n灰色のサイのリスク昇格観測項目は未設定です。\n\n現在有効な観測項目がないため、このセクションは日次判断に参加しない。\n\nGray Rhino Escalation は構造的な昇格監視レイヤーであり、取引シグナルを生成しない。"
        }
    }
}

fn state_label(state: RhinoEscalationState) -> &'static str {
    match state {
        RhinoEscalationState::Background => "Background",
        RhinoEscalationState::Visible => "Visible",
        RhinoEscalationState::Expanding => "Expanding",
        RhinoEscalationState::Normalized => "Normalized",
        RhinoEscalationState::Critical => "Critical",
    }
}

fn risk_level_label(level: RiskLevel) -> &'static str {
    match level {
        RiskLevel::Low => "LOW",
        RiskLevel::Moderate => "MODERATE",
        RiskLevel::Elevated => "ELEVATED",
        RiskLevel::High => "HIGH",
    }
}

fn escalation_direction_label(
    escalation: &GrayRhinoEscalation,
    language: Language,
) -> &'static str {
    match escalation.escalation_state {
        RhinoEscalationState::Critical => match language {
            Language::ZhCn => "Critical Watch",
            Language::EnUs => "Critical Watch",
            Language::JaJp => "Critical Watch",
        },
        RhinoEscalationState::Normalized | RhinoEscalationState::Expanding => match language {
            Language::ZhCn => "Rising",
            Language::EnUs => "Rising",
            Language::JaJp => "Rising",
        },
        RhinoEscalationState::Visible => match language {
            Language::ZhCn => "Visible",
            Language::EnUs => "Visible",
            Language::JaJp => "Visible",
        },
        RhinoEscalationState::Background => match language {
            Language::ZhCn => "Background",
            Language::EnUs => "Background",
            Language::JaJp => "Background",
        },
    }
}

fn observation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Observation:",
        Language::EnUs => "Observation:",
        Language::JaJp => "Observation:",
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
        Language::ZhCn => "fallback 生存性风险",
        Language::EnUs => "Fallback Survivability Risk",
        Language::JaJp => "フォールバック生存性リスク",
    }
}

fn notes_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Notes:",
        Language::EnUs => "Notes:",
        Language::JaJp => "Notes:",
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

fn non_signal_notice(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "Gray Rhino Escalation is a structural escalation monitor. It does not generate trading signals."
        }
        Language::EnUs => {
            "Gray Rhino Escalation is a structural escalation monitor. It does not generate trading signals."
        }
        Language::JaJp => {
            "Gray Rhino Escalation is a structural escalation monitor. It does not generate trading signals."
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
        for language in [Language::ZhCn, Language::EnUs, Language::JaJp] {
            let report = render_gray_rhino_escalation_markdown(&normalized_escalation(), language);

            assert!(report.contains("Gray Rhino Escalation"));
            assert!(report.contains("State: Normalized"));
            assert!(report.contains("It does not generate trading signals."));
        }
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

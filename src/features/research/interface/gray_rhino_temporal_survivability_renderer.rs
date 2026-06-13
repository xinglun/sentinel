use crate::features::research::domain::gray_rhino_survivability_policy::{
    DependencyRiskLevel, GrayRhinoSurvivabilitySummary, SurvivabilityLevel,
};
use crate::features::research::domain::gray_rhino_temporal_policy::{
    GrayRhinoTemporalSummary, InstitutionalResponseState, TemperatureLevel, TemperatureVelocity,
    TemporalTrend,
};
use crate::features::shared::interface::i18n::Language;

/// 時系列サマリーを描画する。
pub(super) fn render_temporal_summary(
    summary: &GrayRhinoTemporalSummary,
    language: Language,
) -> String {
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
            temporal_trend_label(velocity.trend, language),
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
            temporal_trend_label(summary.evidence_acceleration.trend, language),
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
            institutional_response_state_label(summary.institutional_response.state, language),
            mitigating_evidence_count_label(language),
            summary.institutional_response.mitigating_count,
            amplifying_gap_count_label(language),
            summary.institutional_response.amplifying_count
        ));
    }
    out.push_str(temporal_summary_boundary(language));
    out
}

/// 生存能力サマリーを描画する。
pub(super) fn render_survivability_summary(
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
        survivability_level_label(summary.capital_access, language)
    ));
    out.push_str(&format!(
        "- {}: {} ({} {}, {} {})\n",
        compute_control_label(language),
        survivability_level_label(summary.compute_control.level, language),
        mitigating_evidence_count_label(language),
        summary.compute_control.mitigating_count,
        amplifying_gap_count_label(language),
        summary.compute_control.amplifying_count
    ));
    out.push_str(&format!(
        "- {}: {} ({} {}, {} {})\n",
        governance_resilience_label(language),
        survivability_level_label(summary.governance_resilience.level, language),
        mitigating_evidence_count_label(language),
        summary.governance_resilience.mitigating_count,
        amplifying_gap_count_label(language),
        summary.governance_resilience.amplifying_count
    ));
    out.push_str(&format!(
        "- {}: {} ({} {}, {} {})\n",
        dependency_risk_label(language),
        dependency_risk_level_label(summary.dependency_risk.level, language),
        mitigating_evidence_count_label(language),
        summary.dependency_risk.mitigating_count,
        amplifying_gap_count_label(language),
        summary.dependency_risk.amplifying_count
    ));
    out.push_str(&format!(
        "- {}: {} ({} {}, {} {})\n",
        retry_capacity_label(language),
        survivability_level_label(summary.retry_capacity.level, language),
        mitigating_evidence_count_label(language),
        summary.retry_capacity.mitigating_count,
        amplifying_gap_count_label(language),
        summary.retry_capacity.amplifying_count
    ));
    out.push_str(survivability_summary_boundary(language));
    out
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

/// 緩和証拠カウントラベル。センサー健全性レンダラーからも使用される。
pub(super) fn mitigating_evidence_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "缓解证据",
        Language::EnUs => "mitigating evidence",
        Language::JaJp => "緩和証拠",
    }
}

/// 増幅ギャップカウントラベル。センサー健全性レンダラーからも使用される。
pub(super) fn amplifying_gap_count_label(language: Language) -> &'static str {
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

fn temporal_trend_label(trend: TemporalTrend, language: Language) -> &'static str {
    match (trend, language) {
        (TemporalTrend::Rising, Language::ZhCn) => "上升",
        (TemporalTrend::Stable, Language::ZhCn) => "稳定",
        (TemporalTrend::Falling, Language::ZhCn) => "下降",
        (TemporalTrend::Rising, Language::EnUs) => "Rising",
        (TemporalTrend::Stable, Language::EnUs) => "Stable",
        (TemporalTrend::Falling, Language::EnUs) => "Falling",
        (TemporalTrend::Rising, Language::JaJp) => "上昇",
        (TemporalTrend::Stable, Language::JaJp) => "安定",
        (TemporalTrend::Falling, Language::JaJp) => "低下",
    }
}

fn survivability_level_label(level: SurvivabilityLevel, language: Language) -> &'static str {
    match (level, language) {
        (SurvivabilityLevel::Extreme, Language::ZhCn) => "极高",
        (SurvivabilityLevel::High, Language::ZhCn) => "高",
        (SurvivabilityLevel::Medium, Language::ZhCn) => "中",
        (SurvivabilityLevel::Low, Language::ZhCn) => "低",
        (SurvivabilityLevel::Unknown, Language::ZhCn) => "未知",
        (SurvivabilityLevel::Extreme, Language::EnUs) => "Extreme",
        (SurvivabilityLevel::High, Language::EnUs) => "High",
        (SurvivabilityLevel::Medium, Language::EnUs) => "Medium",
        (SurvivabilityLevel::Low, Language::EnUs) => "Low",
        (SurvivabilityLevel::Unknown, Language::EnUs) => "Unknown",
        (SurvivabilityLevel::Extreme, Language::JaJp) => "極めて高い",
        (SurvivabilityLevel::High, Language::JaJp) => "高",
        (SurvivabilityLevel::Medium, Language::JaJp) => "中",
        (SurvivabilityLevel::Low, Language::JaJp) => "低",
        (SurvivabilityLevel::Unknown, Language::JaJp) => "不明",
    }
}

fn dependency_risk_level_label(level: DependencyRiskLevel, language: Language) -> &'static str {
    match (level, language) {
        (DependencyRiskLevel::High, Language::ZhCn) => "高",
        (DependencyRiskLevel::Medium, Language::ZhCn) => "中",
        (DependencyRiskLevel::Low, Language::ZhCn) => "低",
        (DependencyRiskLevel::Unknown, Language::ZhCn) => "未知",
        (DependencyRiskLevel::High, Language::EnUs) => "High",
        (DependencyRiskLevel::Medium, Language::EnUs) => "Medium",
        (DependencyRiskLevel::Low, Language::EnUs) => "Low",
        (DependencyRiskLevel::Unknown, Language::EnUs) => "Unknown",
        (DependencyRiskLevel::High, Language::JaJp) => "高",
        (DependencyRiskLevel::Medium, Language::JaJp) => "中",
        (DependencyRiskLevel::Low, Language::JaJp) => "低",
        (DependencyRiskLevel::Unknown, Language::JaJp) => "不明",
    }
}

fn institutional_response_state_label(
    state: InstitutionalResponseState,
    language: Language,
) -> &'static str {
    match (state, language) {
        (InstitutionalResponseState::Strong, Language::ZhCn) => "强",
        (InstitutionalResponseState::Adequate, Language::ZhCn) => "充分",
        (InstitutionalResponseState::Weak, Language::ZhCn) => "弱",
        (InstitutionalResponseState::NoData, Language::ZhCn) => "无数据",
        (InstitutionalResponseState::Strong, Language::EnUs) => "Strong",
        (InstitutionalResponseState::Adequate, Language::EnUs) => "Adequate",
        (InstitutionalResponseState::Weak, Language::EnUs) => "Weak",
        (InstitutionalResponseState::NoData, Language::EnUs) => "No data",
        (InstitutionalResponseState::Strong, Language::JaJp) => "強い",
        (InstitutionalResponseState::Adequate, Language::JaJp) => "十分",
        (InstitutionalResponseState::Weak, Language::JaJp) => "弱い",
        (InstitutionalResponseState::NoData, Language::JaJp) => "データなし",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino_survivability_policy::{
        DependencyRiskDimension, SurvivabilityDimension,
    };
    use crate::features::research::domain::gray_rhino_temporal_policy::{
        EscalationVelocity, EvidenceAcceleration, InstitutionalResponseSummary,
    };

    #[test]
    fn temporal_summary_localizes_state_values() {
        let summary = GrayRhinoTemporalSummary {
            temperature: TemperatureLevel::High,
            velocity: TemperatureVelocity::Accelerating,
            escalation_velocity: Some(EscalationVelocity {
                delta_score: 2,
                delta_days: 3,
                changed_dimension_count: 2,
                trend: TemporalTrend::Rising,
            }),
            evidence_acceleration: EvidenceAcceleration {
                recent_count: 1,
                prior_count: 3,
                trend: TemporalTrend::Falling,
            },
            institutional_response: InstitutionalResponseSummary {
                mitigating_count: 2,
                amplifying_count: 1,
                state: InstitutionalResponseState::Strong,
            },
        };

        let zh = render_temporal_summary(&summary, Language::ZhCn);
        assert!(zh.contains("上升"));
        assert!(zh.contains("下降"));
        assert!(zh.contains("强"));
        assert!(!zh.contains("RISING"));
        assert!(!zh.contains("STRONG"));

        let ja = render_temporal_summary(&summary, Language::JaJp);
        assert!(ja.contains("上昇"));
        assert!(ja.contains("低下"));
        assert!(ja.contains("強い"));
        assert!(!ja.contains("RISING"));
        assert!(!ja.contains("STRONG"));
    }

    #[test]
    fn survivability_summary_localizes_state_values() {
        let summary = GrayRhinoSurvivabilitySummary {
            capital_access: SurvivabilityLevel::Unknown,
            compute_control: SurvivabilityDimension {
                level: SurvivabilityLevel::High,
                mitigating_count: 2,
                amplifying_count: 0,
            },
            governance_resilience: SurvivabilityDimension {
                level: SurvivabilityLevel::Medium,
                mitigating_count: 1,
                amplifying_count: 1,
            },
            dependency_risk: DependencyRiskDimension {
                level: DependencyRiskLevel::Medium,
                mitigating_count: 1,
                amplifying_count: 1,
            },
            retry_capacity: SurvivabilityDimension {
                level: SurvivabilityLevel::Extreme,
                mitigating_count: 3,
                amplifying_count: 0,
            },
        };

        let zh = render_survivability_summary(&summary, Language::ZhCn);
        assert!(zh.contains("极高"));
        assert!(zh.contains("高"));
        assert!(zh.contains("中"));
        assert!(!zh.contains("EXTREME"));
        assert!(!zh.contains("HIGH"));
        assert!(!zh.contains("MEDIUM"));

        let en = render_survivability_summary(&summary, Language::EnUs);
        assert!(en.contains("Extreme"));
        assert!(en.contains("High"));
        assert!(en.contains("Medium"));
    }
}

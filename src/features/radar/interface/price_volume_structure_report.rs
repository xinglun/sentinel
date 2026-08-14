use crate::features::radar::domain::price_volume_structure::{
    ParticipationQuality, PriceVolumeAssessment, PriceVolumeStructure, SupplyAbsorption,
    VolumeDataQuality,
};
use crate::features::shared::domain::supply_event_context::{SupplyEventContext, SupplyEventType};
use crate::features::shared::interface::i18n::Language;

pub(crate) struct PriceVolumeReportEntry {
    pub symbol: String,
    pub assessment: PriceVolumeAssessment,
    pub supply_context: Option<SupplyEventContext>,
    pub overheated: bool,
    pub accumulation_failed: bool,
}

pub(crate) fn render_price_volume_structure_report(
    entries: &[PriceVolumeReportEntry],
    language: Language,
) -> String {
    let title = match language {
        Language::ZhCn => "📊 Price-Volume Structure",
        Language::EnUs => "📊 Price-Volume Structure",
        Language::JaJp => "📊 Price-Volume Structure",
    };
    let boundary = match language {
        Language::ZhCn => "Observation only · Decision Weight: 0% · Trade Signal: false",
        Language::EnUs => "Observation only · Decision Weight: 0% · Trade Signal: false",
        Language::JaJp => "Observation only · Decision Weight: 0% · Trade Signal: false",
    };
    let mut out = format!("## {title}\n\n{boundary}\n");
    for entry in entries {
        let assessment = &entry.assessment;
        out.push_str(&format!("\n### {}\n", entry.symbol));
        out.push_str(&format!(
            "- Structure: {}\n",
            structure_label(assessment.structure)
        ));
        out.push_str(&format!(
            "- Participation Quality: {}\n",
            participation_label(assessment.participation)
        ));
        out.push_str(&format!(
            "- Volume Data Quality: {}\n",
            quality_label(assessment.quality)
        ));
        if let Some(metrics) = assessment.metrics.as_ref() {
            out.push_str(&format!("- Relative Volume: {:.2}x\n", metrics.rvol_20));
            out.push_str("- Baseline: STANDARD_20D\n");
            out.push_str("- Baseline Sessions: 20\n");
            out.push_str(&format!(
                "- Secondary Relative Volume: {:.2}x\n",
                metrics.rvol_5
            ));
            out.push_str("- Secondary Baseline: SHORT_5D\n");
            out.push_str("- Secondary Baseline Sessions: 5\n");
            out.push_str(&format!(
                "- Price Behavior: 5d {:+.2}% · 20d high {:+.2}%\n",
                metrics.return_5d, metrics.distance_from_20d_high
            ));
        } else {
            out.push_str("- Relative Volume: UNAVAILABLE\n");
            out.push_str("- Baseline: UNAVAILABLE\n");
            out.push_str("- Baseline Sessions: UNAVAILABLE\n");
            out.push_str("- Price Behavior: UNAVAILABLE\n");
        }
        if let Some(context) = entry.supply_context.as_ref().filter(|context| context.availability == crate::features::shared::domain::supply_event_context::SupplyEventContextAvailability::Available) {
            out.push_str(&format!("- Supply Context: {}\n", supply_event_label(context.event_type)));
            out.push_str(&format!("- Supply Date: {}\n", context.event_date.map(|date| date.to_string()).unwrap_or_else(|| "UNAVAILABLE".to_string())));
            out.push_str(&format!("- Supply Direction: {:?}\n", context.supply_direction));
            out.push_str(&format!("- Supply Confidence: {:?}\n", context.confidence));
        } else {
            out.push_str("- Supply Context: UNAVAILABLE\n");
            out.push_str("- Supply Context Status: UNAVAILABLE\n");
            out.push_str("- Supply Context Reason: SUPPLY_CONTEXT_MISSING\n");
        }
        if entry.overheated {
            out.push_str("- Price Position: OVERHEATED\n");
        }
        if assessment.supply_absorption == SupplyAbsorption::Active {
            out.push_str("- Supply Absorption: ACTIVE\n");
        }
        out.push_str(&format!(
            "- Persistence: {:?} ({} days)\n",
            assessment.persistence, assessment.persistence_days
        ));
        out.push_str(&format!(
            "- Interpretation: {}\n",
            interpretation(assessment, entry.accumulation_failed, language)
        ));
    }
    out
}

fn supply_event_label(value: SupplyEventType) -> &'static str {
    match value {
        SupplyEventType::Ipo => "IPO",
        SupplyEventType::LockupExpiry => "LOCKUP_EXPIRY",
        SupplyEventType::SecondaryOffering => "SECONDARY_OFFERING",
        SupplyEventType::FollowOnOffering => "FOLLOW_ON_OFFERING",
        SupplyEventType::InsiderSelling => "INSIDER_SELLING",
        SupplyEventType::EmployeeLiquidityEvent => "EMPLOYEE_LIQUIDITY_EVENT",
        SupplyEventType::ConvertibleIssuance => "CONVERTIBLE_ISSUANCE",
        SupplyEventType::IndexInclusion => "INDEX_INCLUSION",
        SupplyEventType::IndexExclusion => "INDEX_EXCLUSION",
        SupplyEventType::MajorShareholderSale => "MAJOR_SHAREHOLDER_SALE",
        SupplyEventType::ShareUnlock => "SHARE_UNLOCK",
        SupplyEventType::Unknown => "UNKNOWN",
    }
}

fn structure_label(value: PriceVolumeStructure) -> &'static str {
    match value {
        PriceVolumeStructure::Accumulation => "ACCUMULATION",
        PriceVolumeStructure::AccumulationCandidate => "ACCUMULATION_CANDIDATE",
        PriceVolumeStructure::HealthyAdvance => "HEALTHY_ADVANCE",
        PriceVolumeStructure::ExhaustedAdvance => "EXHAUSTED_ADVANCE",
        PriceVolumeStructure::Distribution => "DISTRIBUTION",
        PriceVolumeStructure::Neutral => "NEUTRAL",
        PriceVolumeStructure::Unavailable => "UNAVAILABLE",
    }
}
fn participation_label(value: ParticipationQuality) -> &'static str {
    match value {
        ParticipationQuality::Improving => "IMPROVING",
        ParticipationQuality::Healthy => "HEALTHY",
        ParticipationQuality::Weakening => "WEAKENING",
        ParticipationQuality::Deteriorating => "DETERIORATING",
        ParticipationQuality::Neutral => "NEUTRAL",
        ParticipationQuality::Unavailable => "UNAVAILABLE",
    }
}
fn quality_label(value: VolumeDataQuality) -> &'static str {
    match value {
        VolumeDataQuality::Healthy => "HEALTHY",
        VolumeDataQuality::Partial => "PARTIAL",
        VolumeDataQuality::Degraded => "DEGRADED",
        VolumeDataQuality::Unavailable => "UNAVAILABLE",
    }
}
fn interpretation(
    assessment: &PriceVolumeAssessment,
    accumulation_failed: bool,
    language: Language,
) -> &'static str {
    if accumulation_failed {
        return match language {
            Language::ZhCn => "ACCUMULATION_FAILED：此前的供给吸收观察未延续，当前仅记录为观察失败，不构成交易指令。",
            _ => "ACCUMULATION_FAILED: the prior supply-absorption observation did not persist; this remains observation only.",
        };
    }
    match (assessment.structure, language) {
        (PriceVolumeStructure::Accumulation, Language::ZhCn) => "潜在供给增加后成交量放大而价格未明显走弱，当前观察到供给吸收增强；这不确认机构买入。",
        (PriceVolumeStructure::ExhaustedAdvance, Language::ZhCn) => "价格仍在高位，但参与度正在减弱，短期可能需要横盘或回撤消化。",
        (PriceVolumeStructure::Distribution, Language::ZhCn) => "卖方主动性增强，筹码结构正在恶化；这不是卖出指令。",
        (PriceVolumeStructure::HealthyAdvance, Language::ZhCn) => "上涨得到参与度支持，趋势质量较高；这不是买入指令。",
        (PriceVolumeStructure::Unavailable, Language::ZhCn) => "成交量或价格历史不足，无法可靠判断价量结构。",
        (_, Language::ZhCn) => "当前价量关系没有形成可确认结构。",
        (PriceVolumeStructure::Accumulation, _) => "Higher turnover without material price weakness is observing supply absorption; this does not confirm institutional buying.",
        (PriceVolumeStructure::ExhaustedAdvance, _) => "Price remains elevated while participation is weakening; consolidation or a pullback may be needed.",
        (PriceVolumeStructure::Distribution, _) => "Seller initiative is strengthening; this is an observation, not a sell instruction.",
        (PriceVolumeStructure::HealthyAdvance, _) => "Participation supports the advance; this is not a buy instruction.",
        (PriceVolumeStructure::Unavailable, _) => "Volume or price history is insufficient for a reliable structure assessment.",
        (_, _) => "The current price-volume relationship does not confirm a structure.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::domain::price_volume_structure::{
        PriceVolumeObservationBoundary, StructurePersistence,
    };
    use crate::features::shared::domain::supply_event_context::ObservationEffect;
    fn assessment(
        structure: PriceVolumeStructure,
        participation: ParticipationQuality,
        supply_absorption: SupplyAbsorption,
    ) -> PriceVolumeAssessment {
        PriceVolumeAssessment {
            structure,
            participation,
            supply_absorption,
            quality: VolumeDataQuality::Healthy,
            persistence: StructurePersistence::Confirmed,
            persistence_days: 3,
            metrics: None,
            boundary: PriceVolumeObservationBoundary {
                decision_weight_percent: 0,
                trade_signal: false,
                gate_effect: ObservationEffect::None,
                execution_effect: ObservationEffect::None,
                position_sizing_effect: ObservationEffect::None,
            },
            secondary_metrics: None,
            observation_confidence: Default::default(),
            eligibility: Default::default(),
            primary_baseline: Default::default(),
            secondary_baseline: None,
            lifecycle: Default::default(),
            unavailable_reason: None,
            next_eligibility_condition: None,
        }
    }
    #[test]
    fn report_keeps_spacex_absorption_as_observation_not_institutional_confirmation() {
        let report = render_price_volume_structure_report(
            &[PriceVolumeReportEntry {
                symbol: "SPCX".to_string(),
                assessment: assessment(
                    PriceVolumeStructure::Accumulation,
                    ParticipationQuality::Improving,
                    SupplyAbsorption::Active,
                ),
                supply_context: None,
                overheated: false,
                accumulation_failed: false,
            }],
            Language::ZhCn,
        );
        assert!(report.contains("Supply Absorption: ACTIVE"));
        assert!(report.contains("Relative Volume: UNAVAILABLE"));
        assert!(report.contains("Price Behavior: UNAVAILABLE"));
        assert!(report.contains("Supply Context: UNAVAILABLE"));
        assert!(report.contains("Supply Context Status: UNAVAILABLE"));
        assert!(report.contains("Supply Context Reason: SUPPLY_CONTEXT_MISSING"));
        assert!(report.contains("Baseline: UNAVAILABLE"));
        assert!(report.contains("Baseline Sessions: UNAVAILABLE"));
        assert!(report.contains("Decision Weight: 0%"));
        assert!(report.contains("Supply Context: UNAVAILABLE"));
        assert!(!report.contains("机构买入确认"));
        assert!(!report.contains("买入指令"));
    }
    #[test]
    fn report_keeps_microsoft_exhaustion_as_non_sell_observation() {
        let report = render_price_volume_structure_report(
            &[PriceVolumeReportEntry {
                symbol: "MSFT".to_string(),
                assessment: assessment(
                    PriceVolumeStructure::ExhaustedAdvance,
                    ParticipationQuality::Weakening,
                    SupplyAbsorption::None,
                ),
                supply_context: None,
                overheated: true,
                accumulation_failed: false,
            }],
            Language::ZhCn,
        );
        assert!(report.contains("EXHAUSTED_ADVANCE"));
        assert!(report.contains("WEAKENING"));
        assert!(report.contains("Price Position: OVERHEATED"));
        assert!(!report.contains("卖出指令"));
    }

    #[test]
    fn unavailable_context_remains_explicit() {
        let report = render_price_volume_structure_report(&[], Language::ZhCn);
        assert!(report.contains("Price-Volume Structure"));
    }
}

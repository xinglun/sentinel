use crate::features::radar::domain::price_volume_structure::{
    BaselineType, CandidateLifecycle, EligibilityStatus, ObservationConfidence,
    ParticipationQuality, PriceVolumeAssessment, PriceVolumeStructure, SupplyAbsorption,
    UnavailableReason, VolumeDataQuality,
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
            "- Eligibility: {}\n",
            eligibility_label(assessment.eligibility)
        ));
        out.push_str(&format!(
            "- Observation Confidence: {}\n",
            observation_confidence_label(assessment.observation_confidence)
        ));
        out.push_str(&format!(
            "- Lifecycle: {}\n",
            lifecycle_label(assessment.lifecycle)
        ));
        out.push_str(&format!(
            "- Participation Quality: {}\n",
            participation_label(assessment.participation)
        ));
        out.push_str(&format!(
            "- Volume Data Quality: {}\n",
            quality_label(assessment.quality)
        ));
        out.push_str("- Decision Weight: 0%\n");
        out.push_str("- Trade Signal: false\n");
        if let Some(metrics) = assessment.metrics.as_ref() {
            out.push_str(&format!(
                "- Relative Volume: {:.2}x ({})\n",
                metrics.relative_volume, metrics.relative_volume_label
            ));
            out.push_str(&format!("- Baseline Days: {}\n", metrics.baseline_days));
            out.push_str(&format!(
                "- Price Behavior: 5d {:+.2}% · 20d high {:+.2}%\n",
                metrics.return_5d, metrics.distance_from_20d_high
            ));
        } else {
            out.push_str("- Relative Volume: UNAVAILABLE\n");
            out.push_str("- Price Behavior: UNAVAILABLE\n");
        }
        if assessment.secondary_baseline.is_some() {
            if let Some(metrics) = assessment.secondary_metrics.as_ref() {
                out.push_str(&format!(
                    "- Secondary Relative Volume: {:.2}x ({})\n",
                    metrics.relative_volume, metrics.relative_volume_label
                ));
                out.push_str(&format!(
                    "- Secondary Baseline Days: {}\n",
                    metrics.baseline_days
                ));
            } else {
                out.push_str("- Secondary Relative Volume: UNAVAILABLE\n");
                out.push_str("- Secondary Baseline Days: UNAVAILABLE\n");
            }
        }
        out.push_str(&format!(
            "- Primary Baseline: {}\n",
            baseline_label(assessment.primary_baseline)
        ));
        if let Some(secondary) = assessment.secondary_baseline {
            out.push_str(&format!(
                "- Secondary Baseline: {}\n",
                baseline_label(secondary)
            ));
        }
        if let Some(reason) = assessment.unavailable_reason {
            out.push_str(&format!(
                "- Unavailable Reason: {}\n",
                unavailable_reason_label(reason)
            ));
        }
        if let Some(condition) = assessment.next_eligibility_condition.as_deref() {
            out.push_str(&format!("- Next Eligibility Condition: {condition}\n"));
        }
        if let Some(context) = entry.supply_context.as_ref().filter(|context| context.availability == crate::features::shared::domain::supply_event_context::SupplyEventContextAvailability::Available) {
            out.push_str(&format!("- Supply Context: {}\n", supply_event_label(context.event_type)));
            out.push_str(&format!("- Supply Date: {}\n", context.event_date.map(|date| date.to_string()).unwrap_or_else(|| "UNAVAILABLE".to_string())));
            out.push_str(&format!("- Supply Direction: {:?}\n", context.supply_direction));
            out.push_str(&format!("- Supply Confidence: {:?}\n", context.confidence));
        } else {
            out.push_str("- Supply Context: UNAVAILABLE\n");
        }
        if entry.overheated {
            out.push_str("- Price Position: OVERHEATED\n");
        }
        match assessment.supply_absorption {
            SupplyAbsorption::Active => out.push_str("- Supply Absorption: ACTIVE\n"),
            SupplyAbsorption::Candidate => out.push_str("- Supply Absorption: CANDIDATE\n"),
            _ => {}
        }
        out.push_str(&format!(
            "- Persistence Observation: {:?} ({} days)\n",
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

fn eligibility_label(value: EligibilityStatus) -> &'static str {
    match value {
        EligibilityStatus::Full => "FULL",
        EligibilityStatus::Partial => "PARTIAL",
        EligibilityStatus::Insufficient => "INSUFFICIENT",
        EligibilityStatus::Unavailable => "UNAVAILABLE",
    }
}

fn observation_confidence_label(value: ObservationConfidence) -> &'static str {
    match value {
        ObservationConfidence::High => "HIGH",
        ObservationConfidence::Partial => "PARTIAL",
        ObservationConfidence::Low => "LOW",
        ObservationConfidence::Unavailable => "UNAVAILABLE",
    }
}

fn lifecycle_label(value: CandidateLifecycle) -> &'static str {
    match value {
        CandidateLifecycle::Candidate => "CANDIDATE",
        CandidateLifecycle::Developing => "DEVELOPING",
        CandidateLifecycle::Confirmed => "CONFIRMED",
        CandidateLifecycle::Unavailable => "UNAVAILABLE",
        CandidateLifecycle::Invalidated => "INVALIDATED",
    }
}

fn baseline_label(value: BaselineType) -> &'static str {
    match value {
        BaselineType::Standard20d => "STANDARD_20D",
        BaselineType::AvailableHistory => "AVAILABLE_HISTORY",
        BaselineType::PostIpo => "POST_IPO",
        BaselineType::PostEvent => "POST_EVENT",
        BaselineType::PostLockup => "POST_LOCKUP",
        BaselineType::PostEarnings => "POST_EARNINGS",
        BaselineType::Unavailable => "UNAVAILABLE",
    }
}

fn unavailable_reason_label(value: UnavailableReason) -> &'static str {
    match value {
        UnavailableReason::InsufficientValidHistory => "INSUFFICIENT_VALID_HISTORY",
        UnavailableReason::MissingVolume => "MISSING_VOLUME",
        UnavailableReason::MissingOhlcv => "MISSING_OHLCV",
        UnavailableReason::DataGap => "DATA_GAP",
        UnavailableReason::CorporateActionConflict => "CORPORATE_ACTION_CONFLICT",
        UnavailableReason::ApiFailure => "API_FAILURE",
        UnavailableReason::MissingSupplyContext => "SUPPLY_CONTEXT_MISSING",
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
        (PriceVolumeStructure::AccumulationCandidate, Language::ZhCn) => "价量显示潜在供给吸收特征，但缺少 Supply Context，当前仅为候选观察。",
        (PriceVolumeStructure::ExhaustedAdvance, Language::ZhCn) => "价格仍在高位，但参与度正在减弱，短期可能需要横盘或回撤消化。",
        (PriceVolumeStructure::Distribution, Language::ZhCn) => "卖方主动性增强，筹码结构正在恶化；这不是卖出指令。",
        (PriceVolumeStructure::HealthyAdvance, Language::ZhCn) => "上涨得到参与度支持，趋势质量较高；这不是买入指令。",
        (PriceVolumeStructure::Unavailable, Language::ZhCn) => "成交量或价格历史不足，无法可靠判断价量结构。",
        (_, Language::ZhCn) => "当前价量关系没有形成可确认结构。",
        (PriceVolumeStructure::Accumulation, _) => "Higher turnover without material price weakness is observing supply absorption; this does not confirm institutional buying.",
        (PriceVolumeStructure::AccumulationCandidate, _) => "Price-volume behavior suggests potential absorption, but Supply Context is missing; this remains a candidate observation.",
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
        BaselineType, CandidateLifecycle, EligibilityStatus, ObservationConfidence,
        PriceVolumeObservationBoundary, StructurePersistence, UnavailableReason,
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
            secondary_metrics: None,
            observation_confidence: ObservationConfidence::High,
            boundary: PriceVolumeObservationBoundary {
                decision_weight_percent: 0,
                trade_signal: false,
                gate_effect: ObservationEffect::None,
                execution_effect: ObservationEffect::None,
                position_sizing_effect: ObservationEffect::None,
            },
            eligibility: EligibilityStatus::Full,
            primary_baseline: BaselineType::Standard20d,
            secondary_baseline: None,
            lifecycle: CandidateLifecycle::Confirmed,
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

    #[test]
    fn report_discloses_partial_lifecycle_baseline_reason_and_next_condition() {
        let mut assessment = assessment(
            PriceVolumeStructure::Unavailable,
            ParticipationQuality::Unavailable,
            SupplyAbsorption::Unavailable,
        );
        assessment.eligibility = EligibilityStatus::Partial;
        assessment.observation_confidence = ObservationConfidence::Partial;
        assessment.primary_baseline = BaselineType::PostIpo;
        assessment.secondary_baseline = Some(BaselineType::PostLockup);
        assessment.lifecycle = CandidateLifecycle::Candidate;
        assessment.unavailable_reason = Some(UnavailableReason::InsufficientValidHistory);
        assessment.next_eligibility_condition =
            Some("Need 2 additional valid OHLCV sessions.".to_string());

        let report = render_price_volume_structure_report(
            &[PriceVolumeReportEntry {
                symbol: "NEWCO".to_string(),
                assessment,
                supply_context: None,
                overheated: false,
                accumulation_failed: false,
            }],
            Language::ZhCn,
        );

        assert!(report.contains("Eligibility: PARTIAL"));
        assert!(report.contains("Observation Confidence: PARTIAL"));
        assert!(report.contains("Lifecycle: CANDIDATE"));
        assert!(report.contains("Primary Baseline: POST_IPO"));
        assert!(report.contains("Secondary Baseline: POST_LOCKUP"));
        assert!(report.contains("Unavailable Reason: INSUFFICIENT_VALID_HISTORY"));
        assert!(
            report.contains("Next Eligibility Condition: Need 2 additional valid OHLCV sessions.")
        );
    }

    #[test]
    fn report_discloses_observation_confidence_without_changing_boundary() {
        let mut assessment = assessment(
            PriceVolumeStructure::Neutral,
            ParticipationQuality::Neutral,
            SupplyAbsorption::None,
        );
        assessment.observation_confidence = ObservationConfidence::Low;

        let report = render_price_volume_structure_report(
            &[PriceVolumeReportEntry {
                symbol: "NEWCO".to_string(),
                assessment,
                supply_context: None,
                overheated: false,
                accumulation_failed: false,
            }],
            Language::ZhCn,
        );

        assert!(report.contains("Observation Confidence: LOW"));
        assert!(report.contains("Decision Weight: 0%"));
        assert!(report.contains("Trade Signal: false"));
        assert_eq!(report.matches("Decision Weight: 0%").count(), 2);
        assert_eq!(report.matches("Trade Signal: false").count(), 2);
    }

    #[test]
    fn report_discloses_secondary_baseline_metric_slot() {
        let mut assessment = assessment(
            PriceVolumeStructure::HealthyAdvance,
            ParticipationQuality::Healthy,
            SupplyAbsorption::None,
        );
        assessment.secondary_baseline = Some(BaselineType::PostEarnings);

        let report = render_price_volume_structure_report(
            &[PriceVolumeReportEntry {
                symbol: "MSFT".to_string(),
                assessment,
                supply_context: None,
                overheated: false,
                accumulation_failed: false,
            }],
            Language::ZhCn,
        );

        assert!(report.contains("Secondary Baseline: POST_EARNINGS"));
        assert!(report.contains("Secondary Relative Volume: UNAVAILABLE"));
    }

    #[test]
    fn report_discloses_accumulation_candidate_without_supply_context() {
        let mut assessment = assessment(
            PriceVolumeStructure::AccumulationCandidate,
            ParticipationQuality::Improving,
            SupplyAbsorption::None,
        );
        assessment.eligibility = EligibilityStatus::Partial;
        assessment.primary_baseline = BaselineType::AvailableHistory;
        assessment.lifecycle = CandidateLifecycle::Candidate;
        assessment.unavailable_reason = Some(UnavailableReason::MissingSupplyContext);

        let report = render_price_volume_structure_report(
            &[PriceVolumeReportEntry {
                symbol: "GENERIC_NEWCO".to_string(),
                assessment,
                supply_context: None,
                overheated: false,
                accumulation_failed: false,
            }],
            Language::ZhCn,
        );

        assert!(report.contains("Structure: ACCUMULATION_CANDIDATE"));
        assert!(report.contains("Supply Context: UNAVAILABLE"));
        assert!(report.contains("Unavailable Reason: SUPPLY_CONTEXT_MISSING"));
        assert!(!report.contains("Supply Absorption: ACTIVE"));
    }
}

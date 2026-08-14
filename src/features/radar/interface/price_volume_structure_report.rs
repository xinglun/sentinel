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
            "- Participation Quality: {}\n",
            participation_label(assessment.participation)
        ));
        out.push_str(&format!(
            "- Volume Data Quality: {}\n",
            quality_label(assessment.quality)
        ));
        out.push_str(&format!(
            "- Eligibility: {}\n",
            eligibility_label(assessment.eligibility)
        ));
        out.push_str(&format!(
            "- Observation Confidence: {}\n",
            confidence_label(assessment.observation_confidence)
        ));
        out.push_str(&format!(
            "- Structure Hypothesis: {}\n",
            structure_label(assessment.structure)
        ));
        out.push_str(&format!(
            "- Candidate Lifecycle: {}\n",
            lifecycle_label(assessment.lifecycle)
        ));
        if let Some(metrics) = assessment.metrics.as_ref() {
            out.push_str(&format!(
                "- Relative Volume: {:.2}x\n",
                metrics.relative_volume
            ));
            out.push_str(&format!(
                "- Primary Baseline: {}\n",
                baseline_label(assessment.primary_baseline)
            ));
            out.push_str(&format!("- Baseline Sessions: {}\n", metrics.baseline_days));
            if let Some(secondary) = assessment.secondary_metrics.as_ref() {
                out.push_str(&format!(
                    "- Secondary Relative Volume: {:.2}x\n",
                    secondary.relative_volume
                ));
                out.push_str(&format!(
                    "- Secondary Baseline: {}\n",
                    baseline_label(
                        assessment
                            .secondary_baseline
                            .unwrap_or(secondary.baseline_type)
                    )
                ));
                out.push_str(&format!(
                    "- Secondary Baseline Sessions: {}\n",
                    secondary.baseline_days
                ));
            } else {
                out.push_str("- Secondary Relative Volume: UNAVAILABLE\n");
                out.push_str("- Secondary Baseline: UNAVAILABLE\n");
                out.push_str("- Secondary Baseline Sessions: UNAVAILABLE\n");
            }
            out.push_str(&format!(
                "- Price Behavior: 5d {:+.2}% · 20d high {:+.2}%\n",
                metrics.return_5d, metrics.distance_from_20d_high
            ));
        } else {
            out.push_str("- Relative Volume: UNAVAILABLE\n");
            out.push_str("- Primary Baseline: UNAVAILABLE\n");
            out.push_str("- Baseline Sessions: UNAVAILABLE\n");
            out.push_str("- Secondary Relative Volume: UNAVAILABLE\n");
            out.push_str("- Secondary Baseline: UNAVAILABLE\n");
            out.push_str("- Secondary Baseline Sessions: UNAVAILABLE\n");
            out.push_str("- Price Behavior: UNAVAILABLE\n");
        }
        out.push_str(&format!(
            "- Structure Persistence: {} ({} days)\n",
            persistence_label(assessment.persistence),
            assessment.persistence_days
        ));
        out.push_str(&format!(
            "- Observation Persistence: {} days\n",
            assessment.persistence_days
        ));
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
            out.push_str("- Supply Context Status: UNAVAILABLE\n");
            let reason = if entry
                .supply_context
                .as_ref()
                .is_some_and(|context| context.event_type == SupplyEventType::Unknown)
            {
                "NO_MAPPED_SUPPLY_EVENT"
            } else {
                "SUPPLY_CONTEXT_MISSING"
            };
            out.push_str(&format!("- Supply Context Reason: {reason}\n"));
        }
        if entry.overheated {
            out.push_str("- Price Position: OVERHEATED\n");
        }
        if assessment.supply_absorption == SupplyAbsorption::Active {
            out.push_str("- Supply Absorption: ACTIVE\n");
        }
        out.push_str(&format!(
            "- Interpretation: {}\n",
            interpretation(assessment, entry.accumulation_failed, language)
        ));
    }
    out
}

fn eligibility_label(value: EligibilityStatus) -> &'static str {
    match value {
        EligibilityStatus::Full => "FULL",
        EligibilityStatus::Partial => "PARTIAL",
        EligibilityStatus::Insufficient => "INSUFFICIENT",
        EligibilityStatus::Unavailable => "UNAVAILABLE",
    }
}

fn confidence_label(value: ObservationConfidence) -> &'static str {
    match value {
        ObservationConfidence::High => "HIGH",
        ObservationConfidence::Partial => "PARTIAL",
        ObservationConfidence::Low => "LOW",
        ObservationConfidence::Unavailable => "UNAVAILABLE",
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

fn lifecycle_label(value: CandidateLifecycle) -> &'static str {
    match value {
        CandidateLifecycle::Candidate => "CANDIDATE",
        CandidateLifecycle::Developing => "DEVELOPING",
        CandidateLifecycle::Confirmed => "CONFIRMED",
        CandidateLifecycle::Unavailable => "UNAVAILABLE",
        CandidateLifecycle::Invalidated => "INVALIDATED",
    }
}

fn persistence_label(
    value: crate::features::radar::domain::price_volume_structure::StructurePersistence,
) -> &'static str {
    match value {
        crate::features::radar::domain::price_volume_structure::StructurePersistence::Candidate => "CANDIDATE",
        crate::features::radar::domain::price_volume_structure::StructurePersistence::Developing => "DEVELOPING",
        crate::features::radar::domain::price_volume_structure::StructurePersistence::Confirmed => "CONFIRMED",
        crate::features::radar::domain::price_volume_structure::StructurePersistence::Unavailable => "UNAVAILABLE",
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
        UnavailableReason::MissingSupplyContext => "MISSING_SUPPLY_CONTEXT",
    }
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
        BaselineType, CandidateLifecycle, EligibilityStatus, ObservationConfidence,
        PriceVolumeMetrics, PriceVolumeObservationBoundary, StructurePersistence,
        UnavailableReason,
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
        assert!(report.contains("Primary Baseline: UNAVAILABLE"));
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

    #[test]
    fn report_distinguishes_unmapped_supply_context_reason() {
        let report = render_price_volume_structure_report(
            &[PriceVolumeReportEntry {
                symbol: "SPCX".to_string(),
                assessment: assessment(
                    PriceVolumeStructure::Unavailable,
                    ParticipationQuality::Unavailable,
                    SupplyAbsorption::Unavailable,
                ),
                supply_context: Some(SupplyEventContext::unavailable("SPCX".to_string())),
                overheated: false,
                accumulation_failed: false,
            }],
            Language::EnUs,
        );

        assert!(report.contains("Supply Context Reason: NO_MAPPED_SUPPLY_EVENT"));
    }

    #[test]
    fn report_renders_dynamic_eligibility_baselines_and_observation_states() {
        let mut value = assessment(
            PriceVolumeStructure::AccumulationCandidate,
            ParticipationQuality::Improving,
            SupplyAbsorption::Candidate,
        );
        value.metrics = Some(PriceVolumeMetrics {
            return_1d: 0.01,
            return_5d: 0.03,
            return_10d: 0.05,
            return_20d: 0.08,
            rvol_5: 1.4,
            rvol_20: 1.8,
            average_volume_5: 100.0,
            average_volume_20: 80.0,
            up_day_average_volume: 110.0,
            down_day_average_volume: 70.0,
            distance_from_20d_high: -0.02,
            distance_from_20d_low: 0.25,
            new_high: false,
            new_low: false,
            atr_normalized_move: None,
            body_ratio: None,
            upper_wick_ratio: None,
            lower_wick_ratio: None,
            gap_percent: None,
            baseline_days: 8,
            baseline_type: BaselineType::PostIpo,
            relative_volume: 1.8,
            relative_volume_label: "ELEVATED".to_string(),
        });
        value.secondary_metrics = Some(PriceVolumeMetrics {
            baseline_days: 3,
            baseline_type: BaselineType::AvailableHistory,
            rvol_5: 1.2,
            rvol_20: 1.2,
            relative_volume: 1.2,
            relative_volume_label: "NORMAL".to_string(),
            ..value.metrics.clone().expect("primary metrics")
        });
        value.observation_confidence = ObservationConfidence::Partial;
        value.eligibility = EligibilityStatus::Partial;
        value.primary_baseline = BaselineType::PostIpo;
        value.secondary_baseline = Some(BaselineType::AvailableHistory);
        value.lifecycle = CandidateLifecycle::Developing;
        value.persistence = StructurePersistence::Candidate;
        value.persistence_days = 2;
        value.unavailable_reason = Some(UnavailableReason::InsufficientValidHistory);
        value.next_eligibility_condition = Some("2 more valid sessions".to_string());

        let report = render_price_volume_structure_report(
            &[PriceVolumeReportEntry {
                symbol: "SPCX".to_string(),
                assessment: value,
                supply_context: None,
                overheated: false,
                accumulation_failed: false,
            }],
            Language::EnUs,
        );

        assert!(report.contains("Eligibility: PARTIAL"));
        assert!(report.contains("Observation Confidence: PARTIAL"));
        assert!(report.contains("Primary Baseline: POST_IPO"));
        assert!(report.contains("Baseline Sessions: 8"));
        assert!(report.contains("Secondary Baseline: AVAILABLE_HISTORY"));
        assert!(report.contains("Secondary Baseline Sessions: 3"));
        assert!(report.contains("Candidate Lifecycle: DEVELOPING"));
        assert!(report.contains("Structure Persistence: CANDIDATE (2 days)"));
        assert!(report.contains("Observation Persistence: 2 days"));
        assert!(report.contains("Unavailable Reason: INSUFFICIENT_VALID_HISTORY"));
        assert!(report.contains("Next Eligibility Condition: 2 more valid sessions"));
        assert!(report.contains("Secondary Relative Volume: 1.20x"));
    }

    #[test]
    fn report_keeps_named_symbol_baselines_explicit() {
        let metrics = |baseline_type, baseline_days, relative_volume| PriceVolumeMetrics {
            return_1d: 0.0,
            return_5d: 0.0,
            return_10d: 0.0,
            return_20d: 0.0,
            rvol_5: relative_volume,
            rvol_20: relative_volume,
            average_volume_5: 100.0,
            average_volume_20: 100.0,
            up_day_average_volume: 100.0,
            down_day_average_volume: 100.0,
            distance_from_20d_high: 0.0,
            distance_from_20d_low: 0.0,
            new_high: false,
            new_low: false,
            atr_normalized_move: None,
            body_ratio: None,
            upper_wick_ratio: None,
            lower_wick_ratio: None,
            gap_percent: None,
            baseline_days,
            baseline_type,
            relative_volume,
            relative_volume_label: "NORMAL".to_string(),
        };
        let mut spcx = assessment(
            PriceVolumeStructure::AccumulationCandidate,
            ParticipationQuality::Improving,
            SupplyAbsorption::Candidate,
        );
        spcx.metrics = Some(metrics(BaselineType::PostIpo, 8, 1.4));
        spcx.secondary_metrics = Some(metrics(BaselineType::PostLockup, 3, 1.1));
        spcx.primary_baseline = BaselineType::PostIpo;
        spcx.secondary_baseline = Some(BaselineType::PostLockup);

        let mut msft = assessment(
            PriceVolumeStructure::ExhaustedAdvance,
            ParticipationQuality::Weakening,
            SupplyAbsorption::None,
        );
        msft.metrics = Some(metrics(BaselineType::Standard20d, 20, 0.9));
        msft.secondary_metrics = Some(metrics(BaselineType::PostEarnings, 5, 1.0));
        msft.primary_baseline = BaselineType::Standard20d;
        msft.secondary_baseline = Some(BaselineType::PostEarnings);

        let report = render_price_volume_structure_report(
            &[
                PriceVolumeReportEntry {
                    symbol: "SPCX".to_string(),
                    assessment: spcx,
                    supply_context: None,
                    overheated: false,
                    accumulation_failed: false,
                },
                PriceVolumeReportEntry {
                    symbol: "MSFT/PLTR".to_string(),
                    assessment: msft,
                    supply_context: None,
                    overheated: false,
                    accumulation_failed: false,
                },
            ],
            Language::EnUs,
        );

        assert!(report.contains("### SPCX"));
        assert!(report.contains("Primary Baseline: POST_IPO"));
        assert!(report.contains("Secondary Baseline: POST_LOCKUP"));
        assert!(report.contains("### MSFT/PLTR"));
        assert!(report.contains("Primary Baseline: STANDARD_20D"));
        assert!(report.contains("Secondary Baseline: POST_EARNINGS"));
    }
}

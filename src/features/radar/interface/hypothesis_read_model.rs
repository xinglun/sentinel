use crate::features::radar::domain::hypothesis_governance_policy::{
    derive_hypothesis_governance, HypothesisConfidenceDecayKey, HypothesisConsensusKey,
    HypothesisMarketCyclePhase, HypothesisNarrativeSaturationKey, HypothesisPricingKey,
    HypothesisRealityOverridePriorityKey,
};
use crate::features::radar::domain::trend_cohesion::{AutomatedEvidenceRecord, EvidenceType};
use crate::features::radar::interface::presentation::{
    HypothesisBeneficiaryViewModel, HypothesisCandidateViewModel, HypothesisConfidence,
    HypothesisEvidenceNodeViewModel, HypothesisFailureRiskViewModel, HypothesisLayerViewModel,
    HypothesisValidationCheckViewModel, MarketCyclePosition,
};
use crate::features::shared::interface::i18n::DisplayDictionary;
use chrono::NaiveDate;

#[derive(Debug, Clone, Copy)]
pub(crate) struct HypothesisEvidencePresence {
    pub(crate) capex: bool,
    pub(crate) earnings: bool,
    pub(crate) order: bool,
}

pub(crate) struct HypothesisReadModelInput<'a> {
    pub(crate) substantive_signals: &'a [String],
    pub(crate) substantive_records: &'a [AutomatedEvidenceRecord],
    pub(crate) evidence_presence: HypothesisEvidencePresence,
    pub(crate) conviction_score: Option<f64>,
    pub(crate) market_cycle_position: MarketCyclePosition,
    pub(crate) as_of_date: NaiveDate,
    pub(crate) dict: &'a DisplayDictionary,
}

pub(crate) fn build_hypothesis_layer(
    input: HypothesisReadModelInput<'_>,
) -> Option<HypothesisLayerViewModel> {
    debug_assert_eq!(
        input.substantive_signals.len(),
        [
            input.evidence_presence.capex,
            input.evidence_presence.earnings,
            input.evidence_presence.order,
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
    );
    let enough_reality_evidence = input.evidence_presence.capex
        && (input.evidence_presence.earnings || input.evidence_presence.order)
        && input.conviction_score.unwrap_or(0.0) >= 3.0;

    if !enough_reality_evidence {
        return None;
    }

    let validation_checks = build_hypothesis_validation_checks(input.evidence_presence, input.dict);
    let validation_passed = validation_checks
        .iter()
        .filter(|check| check.passed)
        .count();
    let candidate = build_profit_pool_migration_hypothesis(
        input.market_cycle_position,
        hypothesis_age_days(input.substantive_records, input.as_of_date),
        validation_passed,
        validation_checks,
        input.dict,
    );
    if candidate.failure_risks.is_empty() {
        return None;
    }

    Some(HypothesisLayerViewModel {
        title: input.dict.hypothesis.title.clone(),
        notice: input.dict.hypothesis.notice.clone(),
        candidates: vec![candidate],
    })
}

fn build_profit_pool_migration_hypothesis(
    market_cycle_position: MarketCyclePosition,
    age_days: Option<i64>,
    validation_passed: usize,
    validation_checks: Vec<HypothesisValidationCheckViewModel>,
    dict: &DisplayDictionary,
) -> HypothesisCandidateViewModel {
    let h = &dict.hypothesis;
    let derived =
        derive_hypothesis_governance(hypothesis_market_cycle_phase(market_cycle_position));
    let consensus_state = match derived.consensus {
        HypothesisConsensusKey::Crowded => h.consensus_crowded.clone(),
        HypothesisConsensusKey::Consensus => h.consensus_consensus.clone(),
        HypothesisConsensusKey::Emerging => h.consensus_emerging.clone(),
    };
    let pricing_state = match derived.pricing {
        HypothesisPricingKey::Overpriced => h.pricing_overpriced.clone(),
        HypothesisPricingKey::FullyPriced => h.pricing_fully_priced.clone(),
        HypothesisPricingKey::PartiallyPriced => h.pricing_partially_priced.clone(),
    };
    let narrative_saturation = match derived.narrative_saturation {
        HypothesisNarrativeSaturationKey::Saturated => h.narrative_saturation_saturated.clone(),
        HypothesisNarrativeSaturationKey::Crowded => h.narrative_saturation_crowded.clone(),
        HypothesisNarrativeSaturationKey::Developing => h.narrative_saturation_developing.clone(),
    };
    let reality_override_notice = match derived.reality_override_priority {
        HypothesisRealityOverridePriorityKey::Critical
        | HypothesisRealityOverridePriorityKey::Elevated => h.reality_override_required.clone(),
        HypothesisRealityOverridePriorityKey::Watch => h.reality_override_watch.clone(),
    };
    let reality_override_priority = match derived.reality_override_priority {
        HypothesisRealityOverridePriorityKey::Critical => {
            h.reality_override_priority_critical.clone()
        }
        HypothesisRealityOverridePriorityKey::Elevated => {
            h.reality_override_priority_elevated.clone()
        }
        HypothesisRealityOverridePriorityKey::Watch => h.reality_override_priority_watch.clone(),
    };
    let confidence_decay_notice = match derived.confidence_decay {
        HypothesisConfidenceDecayKey::Required => h.confidence_decay_required.clone(),
        HypothesisConfidenceDecayKey::Watch => h.confidence_decay_watch.clone(),
    };

    HypothesisCandidateViewModel {
        title: h.title_profit_pool_migration.clone(),
        hypothesis_type: h.type_profit_pool_migration.clone(),
        summary: h.summary_profit_pool_migration.clone(),
        consensus_state,
        pricing_state,
        confidence: HypothesisConfidence::Developing,
        confidence_label: h.confidence_developing.clone(),
        time_horizon: h.horizon_long.clone(),
        materialization_window: h.materialization_window_12_36_months.clone(),
        tactical_isolation_notice: h.tactical_isolation_long_term.clone(),
        narrative_saturation,
        reality_override_notice,
        reality_override_priority,
        confidence_decay_notice,
        age_days,
        age_label: age_days
            .map(|days| format!("{days} {}", h.age_days_unit))
            .unwrap_or_else(|| h.age_unknown.clone()),
        validation_summary: format!("{validation_passed}/{}", validation_checks.len()),
        validation_checks,
        evidence_chain: vec![
            HypothesisEvidenceNodeViewModel {
                label: h.evidence_capex_expansion.clone(),
                evidence_type: "CapitalAllocationSignal".to_string(),
                strength: h.strength_strong.clone(),
                source_layer: h.source_evidence_record.clone(),
                note: h.evidence_capex_expansion.clone(),
            },
            HypothesisEvidenceNodeViewModel {
                label: h.evidence_cost_reduction.clone(),
                evidence_type: "CostCurveShift".to_string(),
                strength: h.strength_moderate.clone(),
                source_layer: h.source_industry_data.clone(),
                note: h.evidence_cost_reduction.clone(),
            },
            HypothesisEvidenceNodeViewModel {
                label: h.evidence_pricing_power.clone(),
                evidence_type: "PricingPower".to_string(),
                strength: h.strength_moderate.clone(),
                source_layer: h.source_industry_data.clone(),
                note: h.evidence_pricing_power.clone(),
            },
            HypothesisEvidenceNodeViewModel {
                label: h.evidence_platform_adoption.clone(),
                evidence_type: "PlatformAdoption".to_string(),
                strength: h.strength_moderate.clone(),
                source_layer: h.source_evidence_record.clone(),
                note: h.evidence_platform_adoption.clone(),
            },
            HypothesisEvidenceNodeViewModel {
                label: h.evidence_revenue_validation.clone(),
                evidence_type: "RevenueValidation".to_string(),
                strength: h.strength_moderate.clone(),
                source_layer: h.source_evidence_record.clone(),
                note: h.evidence_revenue_validation.clone(),
            },
        ],
        candidate_beneficiaries: vec![
            HypothesisBeneficiaryViewModel {
                symbol: "MSFT".to_string(),
                role: h.beneficiary_primary.clone(),
                rationale: h.beneficiary_msft_rationale.clone(),
            },
            HypothesisBeneficiaryViewModel {
                symbol: "AMZN".to_string(),
                role: h.beneficiary_secondary.clone(),
                rationale: h.beneficiary_amzn_rationale.clone(),
            },
            HypothesisBeneficiaryViewModel {
                symbol: "GOOG".to_string(),
                role: h.beneficiary_secondary.clone(),
                rationale: h.beneficiary_goog_rationale.clone(),
            },
        ],
        failure_risks: vec![
            HypothesisFailureRiskViewModel {
                label: h.failure_capex_delay.clone(),
                description: h.failure_capex_delay_desc.clone(),
                severity: h.severity_high.clone(),
            },
            HypothesisFailureRiskViewModel {
                label: h.failure_pricing_competition.clone(),
                description: h.failure_pricing_competition_desc.clone(),
                severity: h.severity_medium.clone(),
            },
            HypothesisFailureRiskViewModel {
                label: h.failure_adoption_shortfall.clone(),
                description: h.failure_adoption_shortfall_desc.clone(),
                severity: h.severity_medium.clone(),
            },
            HypothesisFailureRiskViewModel {
                label: h.failure_macro_gravity.clone(),
                description: h.failure_macro_gravity_desc.clone(),
                severity: h.severity_medium.clone(),
            },
        ],
        responsibility_notice: h.responsibility_notice.clone(),
    }
}

fn build_hypothesis_validation_checks(
    evidence_presence: HypothesisEvidencePresence,
    dict: &DisplayDictionary,
) -> Vec<HypothesisValidationCheckViewModel> {
    let h = &dict.hypothesis;
    vec![
        HypothesisValidationCheckViewModel {
            label: h.validation_capex_payoff.clone(),
            passed: evidence_presence.capex,
        },
        HypothesisValidationCheckViewModel {
            label: h.validation_earnings_quality.clone(),
            passed: evidence_presence.earnings,
        },
        HypothesisValidationCheckViewModel {
            label: h.validation_order_visibility.clone(),
            passed: evidence_presence.order,
        },
        HypothesisValidationCheckViewModel {
            label: h.validation_platform_adoption.clone(),
            passed: false,
        },
        HypothesisValidationCheckViewModel {
            label: h.validation_pricing_power.clone(),
            passed: false,
        },
    ]
}

fn hypothesis_age_days(records: &[AutomatedEvidenceRecord], as_of_date: NaiveDate) -> Option<i64> {
    records
        .iter()
        .filter(|record| {
            matches!(
                record.evidence_type,
                EvidenceType::CapexPayoff
                    | EvidenceType::EarningsValidation
                    | EvidenceType::OrderVisibility
            )
        })
        .filter_map(|record| NaiveDate::parse_from_str(&record.event_date, "%Y-%m-%d").ok())
        .filter(|event_date| *event_date <= as_of_date)
        .map(|event_date| (as_of_date - event_date).num_days())
        .max()
}

fn hypothesis_market_cycle_phase(position: MarketCyclePosition) -> HypothesisMarketCyclePhase {
    match position {
        MarketCyclePosition::EarlyFormation => HypothesisMarketCyclePhase::EarlyFormation,
        MarketCyclePosition::MidConfirmation => HypothesisMarketCyclePhase::MidConfirmation,
        MarketCyclePosition::LateAcceptance => HypothesisMarketCyclePhase::LateAcceptance,
        MarketCyclePosition::CrowdedExpectation => HypothesisMarketCyclePhase::CrowdedExpectation,
        MarketCyclePosition::DistributionWarning => HypothesisMarketCyclePhase::DistributionWarning,
        MarketCyclePosition::Unknown => HypothesisMarketCyclePhase::Unknown,
    }
}

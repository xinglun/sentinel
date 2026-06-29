use crate::features::radar::interface::presentation::{
    InterpretationExpectationQuality, InterpretationExpectationQualityReason,
    InterpretationGravityDataQuality, InterpretationGravityDataQualityReason,
    InterpretationLayerViewModel, InterpretationPattern, InterpretationTrendState,
    StateTransitionViewModel,
};
use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoSnapshot, CapitalAbsorptionPotentialSupplyPressureLevel,
};
use crate::features::research::application::valuation_gravity::{
    ValuationGravityObservation, ValuationPersistenceHealth, ValuationPersistenceReason,
};
use crate::features::research::domain::valuation_gravity::GravityStatus;
use crate::features::research::interface::expectation_report_builder::ExpectationLayerSnapshot;
use crate::features::shared::interface::i18n::{DisplayDictionary, Language};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InterpretationNarrativeSignal {
    pub trend_state: InterpretationTrendState,
    pub expectation_quality: InterpretationExpectationQuality,
    pub expectation_quality_reason: InterpretationExpectationQualityReason,
    pub gravity_data_quality: InterpretationGravityDataQuality,
    pub gravity_data_quality_reason: InterpretationGravityDataQualityReason,
    pub gravity_status: Option<GravityStatus>,
    pub supply_pressure: bool,
}

impl Default for InterpretationNarrativeSignal {
    fn default() -> Self {
        Self {
            trend_state: InterpretationTrendState::Weak,
            expectation_quality: InterpretationExpectationQuality::Unavailable,
            expectation_quality_reason: InterpretationExpectationQualityReason::SystemUnavailable,
            gravity_data_quality: InterpretationGravityDataQuality::Unavailable,
            gravity_data_quality_reason:
                InterpretationGravityDataQualityReason::ProviderUnavailable,
            gravity_status: None,
            supply_pressure: false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct InterpretationLayerReadModelInput<'a> {
    pub subjects: &'a [String],
    pub signal: InterpretationNarrativeSignal,
    pub language: Language,
    pub dict: &'a DisplayDictionary,
}

pub(crate) fn build_interpretation_layer_view_model(
    input: InterpretationLayerReadModelInput<'_>,
) -> InterpretationLayerViewModel {
    let interpretation = &input.dict.interpretation;
    let pattern = classify_interpretation_pattern(&input.signal);
    let subjects_value = render_subjects(input.subjects);

    InterpretationLayerViewModel {
        title: interpretation.title.clone(),
        notice: interpretation.notice.clone(),
        current_decision_weight_label: interpretation.current_decision_weight_label.clone(),
        current_decision_weight_value: "0%".to_string(),
        expectation_quality_label: interpretation.expectation_quality_label.clone(),
        expectation_quality_value: expectation_quality_label(input.signal.expectation_quality),
        expectation_quality_reason_label: interpretation.expectation_quality_reason_label.clone(),
        expectation_quality_reason_value: expectation_quality_reason_label(
            input.signal.expectation_quality_reason,
            input.language,
        ),
        gravity_data_quality_label: interpretation.gravity_data_quality_label.clone(),
        gravity_data_quality_value: gravity_data_quality_label(input.signal.gravity_data_quality),
        gravity_data_quality_reason_label: interpretation.gravity_data_quality_reason_label.clone(),
        gravity_data_quality_reason_value: gravity_data_quality_reason_label(
            input.signal.gravity_data_quality_reason,
        ),
        narrative_pattern_label: interpretation.narrative_pattern_label.clone(),
        narrative_pattern_value: narrative_pattern_label(pattern, input.language),
        subjects_label: interpretation.subjects_label.clone(),
        subjects_value,
        narrative_summary_label: interpretation.narrative_summary_label.clone(),
        narrative_summary_value: narrative_summary(pattern, input.language),
        boundary: interpretation.boundary.clone(),
    }
}

pub(crate) fn derive_expectation_quality(
    snapshot: &ExpectationLayerSnapshot,
) -> (
    InterpretationExpectationQuality,
    InterpretationExpectationQualityReason,
) {
    if snapshot.observations.is_empty() {
        return (
            InterpretationExpectationQuality::Unavailable,
            InterpretationExpectationQualityReason::SystemUnavailable,
        );
    }

    let available_confidences = snapshot
        .observations
        .iter()
        .filter(|observation| {
            observation.source_health
                == crate::features::research::domain::expectation::SourceHealth::Succeeded
        })
        .filter_map(|observation| observation.confidence)
        .collect::<Vec<_>>();
    if available_confidences.is_empty() {
        let reason = if snapshot.observations.iter().any(|observation| {
            observation.source_health
                == crate::features::research::domain::expectation::SourceHealth::Succeeded
        }) {
            InterpretationExpectationQualityReason::MarketConsensusUnavailable
        } else {
            InterpretationExpectationQualityReason::SystemUnavailable
        };
        return (InterpretationExpectationQuality::Unavailable, reason);
    }

    let average_confidence =
        available_confidences.iter().sum::<f64>() / available_confidences.len() as f64;
    let all_succeeded = snapshot.observations.iter().all(|observation| {
        observation.source_health
            == crate::features::research::domain::expectation::SourceHealth::Succeeded
    });

    if all_succeeded && average_confidence >= 0.80 {
        (
            InterpretationExpectationQuality::High,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
        )
    } else if average_confidence >= 0.60 {
        (
            InterpretationExpectationQuality::Medium,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
        )
    } else {
        (
            InterpretationExpectationQuality::Low,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
        )
    }
}

pub(crate) fn derive_gravity_data_quality(
    observation: &ValuationGravityObservation,
) -> InterpretationGravityDataQuality {
    if observation.snapshot.assets.is_empty() {
        return InterpretationGravityDataQuality::Unavailable;
    }

    let available_assets = observation
        .snapshot
        .assets
        .iter()
        .filter(|asset| asset.source_health != crate::features::research::domain::valuation_gravity::ValuationSourceHealth::Unavailable)
        .count();
    if available_assets == 0 {
        return InterpretationGravityDataQuality::Unavailable;
    }

    if available_assets != observation.snapshot.assets.len()
        || observation.persistence_health == ValuationPersistenceHealth::Missing
        || observation.persistence_reason == ValuationPersistenceReason::HistoricalSnapshotMissing
    {
        InterpretationGravityDataQuality::Partial
    } else {
        InterpretationGravityDataQuality::Ready
    }
}

pub(crate) fn derive_gravity_data_quality_reason(
    observation: &ValuationGravityObservation,
) -> InterpretationGravityDataQualityReason {
    if observation.persistence_health == ValuationPersistenceHealth::Missing
        || observation.persistence_reason == ValuationPersistenceReason::HistoricalSnapshotMissing
    {
        return InterpretationGravityDataQualityReason::HistoricalSnapshotMissing;
    }

    if observation
        .snapshot
        .assets
        .iter()
        .any(|asset| matches!(
            asset.quality_reason,
            crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::HistoricalSnapshotMissing
                | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::HistoricalSnapshotReadFailure
        ))
    {
        return InterpretationGravityDataQualityReason::HistoricalSnapshotMissing;
    }

    if observation.snapshot.assets.iter().any(|asset| matches!(
        asset.quality_reason,
        crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::InsufficientEvidence
            | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::PriceTargetConsensus
            | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::MarketMultipleFallback
            | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::RecommendationFallback
    )) {
        return InterpretationGravityDataQualityReason::ConsensusUnavailable;
    }

    if observation.snapshot.assets.iter().any(|asset| matches!(
        asset.quality_reason,
        crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::MissingCredential
            | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::EntitlementDenied
            | crate::features::research::domain::valuation_gravity::ValuationDataQualityReason::ProviderFailure
    )) {
        return InterpretationGravityDataQualityReason::ProviderUnavailable;
    }

    InterpretationGravityDataQualityReason::SourceTemporarilyUnavailable
}

pub(crate) fn derive_trend_state(
    transition_evidence: Option<&StateTransitionViewModel>,
) -> InterpretationTrendState {
    let Some(evidence) = transition_evidence else {
        return InterpretationTrendState::Weak;
    };

    if evidence.no_trade_persists
        || !evidence.breakout_changes.is_empty()
        || evidence.trend_cohesion_status_change.is_some()
        || evidence.trend_cohesion_topology_change.is_some()
    {
        return InterpretationTrendState::PostRallyConsolidation;
    }

    if evidence.trend_recognition_state.is_some() || evidence.has_significant_change {
        InterpretationTrendState::Stable
    } else {
        InterpretationTrendState::Weak
    }
}

pub(crate) fn has_supply_pressure(snapshot: &CapitalAbsorptionAutoSnapshot) -> bool {
    matches!(
        snapshot.potential_supply_pressure.level,
        CapitalAbsorptionPotentialSupplyPressureLevel::Elevated
    ) || snapshot.potential_supply_pressure.future_queue_count > 0
        || snapshot.potential_supply_pressure.near_term_supply_count > 0
}

pub(crate) fn collect_subjects(
    expectation_snapshot: &ExpectationLayerSnapshot,
    gravity_observation: &ValuationGravityObservation,
) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut subjects = Vec::new();
    for subject in expectation_snapshot
        .observations
        .iter()
        .map(|observation| observation.subject.clone())
        .chain(
            gravity_observation
                .snapshot
                .assets
                .iter()
                .map(|asset| asset.symbol.clone()),
        )
    {
        let subject: String = subject;
        if seen.insert(subject.clone()) {
            subjects.push(subject);
        }
    }
    subjects
}

pub(crate) fn classify_interpretation_pattern(
    signal: &InterpretationNarrativeSignal,
) -> InterpretationPattern {
    if signal.supply_pressure {
        return InterpretationPattern::SupplyPressure;
    }

    if matches!(
        signal.trend_state,
        InterpretationTrendState::PostRallyConsolidation
    ) {
        return InterpretationPattern::PostRallyConsolidation;
    }

    if matches!(signal.trend_state, InterpretationTrendState::Stable)
        && matches!(
            signal.gravity_data_quality,
            InterpretationGravityDataQuality::Ready
        )
        && matches!(
            signal.gravity_status,
            Some(GravityStatus::Fair)
                | Some(GravityStatus::Undervalued)
                | Some(GravityStatus::DeepUndervalued)
        )
        && matches!(
            signal.expectation_quality,
            InterpretationExpectationQuality::High | InterpretationExpectationQuality::Medium
        )
    {
        return InterpretationPattern::FundamentalPricing;
    }

    InterpretationPattern::EventWaiting
}

fn render_subjects(subjects: &[String]) -> String {
    let mut seen = HashSet::<String>::new();
    let mut unique = Vec::new();
    for subject in subjects.iter().map(|subject| subject.trim()) {
        if !subject.is_empty() && seen.insert(subject.to_string()) {
            unique.push(subject.to_string());
        }
    }
    if unique.is_empty() {
        return "N/A".to_string();
    }
    unique.join(", ")
}

fn expectation_quality_label(value: InterpretationExpectationQuality) -> String {
    match value {
        InterpretationExpectationQuality::High => "HIGH".to_string(),
        InterpretationExpectationQuality::Medium => "MEDIUM".to_string(),
        InterpretationExpectationQuality::Low => "LOW".to_string(),
        InterpretationExpectationQuality::Unavailable => "UNAVAILABLE".to_string(),
    }
}

fn expectation_quality_reason_label(
    value: InterpretationExpectationQualityReason,
    language: Language,
) -> String {
    match (value, language) {
        (InterpretationExpectationQualityReason::MarketConsensusAvailable, Language::ZhCn) => {
            "市场预期已形成".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusAvailable, Language::EnUs) => {
            "Market consensus available".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusAvailable, Language::JaJp) => {
            "市場コンセンサスあり".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusUnavailable, Language::ZhCn) => {
            "市场没有一致预期".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusUnavailable, Language::EnUs) => {
            "Market consensus unavailable".to_string()
        }
        (InterpretationExpectationQualityReason::MarketConsensusUnavailable, Language::JaJp) => {
            "市場コンセンサスなし".to_string()
        }
        (InterpretationExpectationQualityReason::SystemUnavailable, Language::ZhCn) => {
            "系统未取得预期数据".to_string()
        }
        (InterpretationExpectationQualityReason::SystemUnavailable, Language::EnUs) => {
            "System unavailable".to_string()
        }
        (InterpretationExpectationQualityReason::SystemUnavailable, Language::JaJp) => {
            "システム未取得".to_string()
        }
    }
}

fn gravity_data_quality_label(value: InterpretationGravityDataQuality) -> String {
    match value {
        InterpretationGravityDataQuality::Ready => "READY".to_string(),
        InterpretationGravityDataQuality::Partial => "PARTIAL".to_string(),
        InterpretationGravityDataQuality::Unavailable => "UNAVAILABLE".to_string(),
    }
}

fn gravity_data_quality_reason_label(value: InterpretationGravityDataQualityReason) -> String {
    match value {
        InterpretationGravityDataQualityReason::ProviderUnavailable => {
            "Provider unavailable".to_string()
        }
        InterpretationGravityDataQualityReason::HistoricalSnapshotMissing => {
            "Historical snapshot missing".to_string()
        }
        InterpretationGravityDataQualityReason::ConsensusUnavailable => {
            "Consensus unavailable".to_string()
        }
        InterpretationGravityDataQualityReason::SourceTemporarilyUnavailable => {
            "Source temporarily unavailable".to_string()
        }
    }
}

fn narrative_pattern_label(pattern: InterpretationPattern, language: Language) -> String {
    match (pattern, language) {
        (InterpretationPattern::EventWaiting, Language::ZhCn) => "事件等待".to_string(),
        (InterpretationPattern::EventWaiting, Language::EnUs) => "Event waiting".to_string(),
        (InterpretationPattern::EventWaiting, Language::JaJp) => "イベント待ち".to_string(),
        (InterpretationPattern::FundamentalPricing, Language::ZhCn) => "基本面定价".to_string(),
        (InterpretationPattern::FundamentalPricing, Language::EnUs) => {
            "Fundamental pricing".to_string()
        }
        (InterpretationPattern::FundamentalPricing, Language::JaJp) => {
            "ファンダメンタル定価".to_string()
        }
        (InterpretationPattern::PostRallyConsolidation, Language::ZhCn) => "上涨后整理".to_string(),
        (InterpretationPattern::PostRallyConsolidation, Language::EnUs) => {
            "Post-rally consolidation".to_string()
        }
        (InterpretationPattern::PostRallyConsolidation, Language::JaJp) => {
            "上昇後の整理".to_string()
        }
        (InterpretationPattern::SupplyPressure, Language::ZhCn) => "供给压力".to_string(),
        (InterpretationPattern::SupplyPressure, Language::EnUs) => "Supply pressure".to_string(),
        (InterpretationPattern::SupplyPressure, Language::JaJp) => "供給圧力".to_string(),
    }
}

fn narrative_summary(pattern: InterpretationPattern, language: Language) -> String {
    match (pattern, language) {
        (InterpretationPattern::EventWaiting, Language::ZhCn) => {
            "当前弱势更像是事件等待，而不是长期 Thesis 的崩坏。".to_string()
        }
        (InterpretationPattern::EventWaiting, Language::EnUs) => {
            "The current weakness looks more like event waiting than a break in the long-term thesis.".to_string()
        }
        (InterpretationPattern::EventWaiting, Language::JaJp) => {
            "現在の弱さは、長期 Thesis の崩れというよりイベント待ちに近い。".to_string()
        }
        (InterpretationPattern::FundamentalPricing, Language::ZhCn) => {
            "市场已经开始按基本面定价，价格更多反映现实兑现而非远期想象。".to_string()
        }
        (InterpretationPattern::FundamentalPricing, Language::EnUs) => {
            "The market is already pricing fundamentals, with price reflecting realization more than distant expectations.".to_string()
        }
        (InterpretationPattern::FundamentalPricing, Language::JaJp) => {
            "市場はすでに基本面を織り込み始めており、価格は遠い期待より実現を映している。".to_string()
        }
        (InterpretationPattern::PostRallyConsolidation, Language::ZhCn) => {
            "这更像是上涨后的整理，而不是长结构恶化的证据。".to_string()
        }
        (InterpretationPattern::PostRallyConsolidation, Language::EnUs) => {
            "This looks more like post-rally consolidation than evidence of long-structure deterioration.".to_string()
        }
        (InterpretationPattern::PostRallyConsolidation, Language::JaJp) => {
            "これは長期構造の悪化というより、上昇後の整理に近い。".to_string()
        }
        (InterpretationPattern::SupplyPressure, Language::ZhCn) => {
            "新的供给压力正在影响价格解释，但它本身不自动转化为交易结论。".to_string()
        }
        (InterpretationPattern::SupplyPressure, Language::EnUs) => {
            "New supply pressure is affecting the price story, but it does not automatically turn into a trading conclusion.".to_string()
        }
        (InterpretationPattern::SupplyPressure, Language::JaJp) => {
            "新しい供給圧力が価格説明に効いているが、それ自体を売買結論へ自動変換しない。".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::shared::interface::i18n::get_dictionary;

    fn signal(
        trend_state: InterpretationTrendState,
        expectation_quality: InterpretationExpectationQuality,
        expectation_quality_reason: InterpretationExpectationQualityReason,
        gravity_data_quality: InterpretationGravityDataQuality,
        gravity_status: Option<GravityStatus>,
        supply_pressure: bool,
    ) -> InterpretationNarrativeSignal {
        InterpretationNarrativeSignal {
            trend_state,
            expectation_quality,
            expectation_quality_reason,
            gravity_data_quality,
            gravity_data_quality_reason:
                InterpretationGravityDataQualityReason::ProviderUnavailable,
            gravity_status,
            supply_pressure,
        }
    }

    #[test]
    fn tsla_style_event_waiting_selects_event_waiting_pattern() {
        let pattern = classify_interpretation_pattern(&signal(
            InterpretationTrendState::Weak,
            InterpretationExpectationQuality::High,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
            InterpretationGravityDataQuality::Ready,
            Some(GravityStatus::Fair),
            false,
        ));

        assert_eq!(pattern, InterpretationPattern::EventWaiting);
    }

    #[test]
    fn goog_style_fundamental_pricing_selects_fundamental_pricing_pattern() {
        let pattern = classify_interpretation_pattern(&signal(
            InterpretationTrendState::Stable,
            InterpretationExpectationQuality::Medium,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
            InterpretationGravityDataQuality::Ready,
            Some(GravityStatus::Fair),
            false,
        ));

        assert_eq!(pattern, InterpretationPattern::FundamentalPricing);
    }

    #[test]
    fn nvda_style_post_rally_consolidation_selects_post_rally_pattern() {
        let pattern = classify_interpretation_pattern(&signal(
            InterpretationTrendState::PostRallyConsolidation,
            InterpretationExpectationQuality::High,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
            InterpretationGravityDataQuality::Ready,
            Some(GravityStatus::Expensive),
            false,
        ));

        assert_eq!(pattern, InterpretationPattern::PostRallyConsolidation);
    }

    #[test]
    fn supply_pressure_wins_over_other_signals() {
        let pattern = classify_interpretation_pattern(&signal(
            InterpretationTrendState::Stable,
            InterpretationExpectationQuality::High,
            InterpretationExpectationQualityReason::MarketConsensusAvailable,
            InterpretationGravityDataQuality::Ready,
            Some(GravityStatus::Fair),
            true,
        ));

        assert_eq!(pattern, InterpretationPattern::SupplyPressure);
    }

    #[test]
    fn view_model_renders_boundary_and_zero_weight() {
        let dict = get_dictionary(Language::EnUs);
        let subjects = vec!["TSLA".to_string(), "GOOG".to_string(), "NVDA".to_string()];
        let view_model = build_interpretation_layer_view_model(InterpretationLayerReadModelInput {
            subjects: &subjects,
            signal: signal(
                InterpretationTrendState::Stable,
                InterpretationExpectationQuality::High,
                InterpretationExpectationQualityReason::MarketConsensusAvailable,
                InterpretationGravityDataQuality::Ready,
                Some(GravityStatus::Fair),
                false,
            ),
            language: Language::EnUs,
            dict: &dict,
        });

        assert_eq!(view_model.current_decision_weight_value, "0%");
        assert_eq!(
            view_model.expectation_quality_reason_value,
            "Market consensus available"
        );
        assert!(view_model.boundary.contains("Observation Layer"));
        assert!(view_model.subjects_value.contains("TSLA"));
        assert!(view_model
            .narrative_summary_value
            .contains("pricing fundamentals"));
    }

    #[test]
    fn expectation_quality_reason_distinguishes_system_and_market_unavailability() {
        use crate::features::research::domain::expectation::{
            ExpectationEventType, ExpectationObservation, ExpectationPressure, RevisionDirection,
            SourceHealth, SurpriseState,
        };

        let as_of_date = chrono::NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
        let system_unavailable_snapshot = ExpectationLayerSnapshot {
            as_of_date,
            decision_weight_percent: 0,
            trade_signal: false,
            gate_effect: "none".to_string(),
            execution_effect: "none".to_string(),
            position_sizing_effect: "none".to_string(),
            observations: vec![],
        };
        let (quality, reason) = derive_expectation_quality(&system_unavailable_snapshot);
        assert_eq!(quality, InterpretationExpectationQuality::Unavailable);
        assert_eq!(
            reason,
            InterpretationExpectationQualityReason::SystemUnavailable
        );

        let market_unavailable_snapshot = ExpectationLayerSnapshot {
            as_of_date,
            decision_weight_percent: 0,
            trade_signal: false,
            gate_effect: "none".to_string(),
            execution_effect: "none".to_string(),
            position_sizing_effect: "none".to_string(),
            observations: vec![ExpectationObservation {
                subject: "TSLA".to_string(),
                period: "2026Q2".to_string(),
                event_type: ExpectationEventType::DeliveryConsensus,
                expected_value: "~401k deliveries".to_string(),
                actual_value: "未発表".to_string(),
                unit: "deliveries".to_string(),
                consensus_source: "fixture".to_string(),
                estimate_count: 0,
                estimate_high: None,
                estimate_low: None,
                estimate_median: None,
                estimate_average: None,
                revision_direction: RevisionDirection::Stable,
                surprise_state: SurpriseState::NotReleased,
                expectation_pressure: ExpectationPressure::Normal,
                confidence: None,
                source_health: SourceHealth::Succeeded,
                interpretation: "市場没有一致预期".to_string(),
                as_of_date,
                observed_at: as_of_date,
            }],
        };
        let (quality, reason) = derive_expectation_quality(&market_unavailable_snapshot);
        assert_eq!(quality, InterpretationExpectationQuality::Unavailable);
        assert_eq!(
            reason,
            InterpretationExpectationQualityReason::MarketConsensusUnavailable
        );
    }

    #[test]
    fn trend_state_uses_transition_read_model_only() {
        let weak = derive_trend_state(None);
        assert_eq!(weak, InterpretationTrendState::Weak);

        let stable = derive_trend_state(Some(&StateTransitionViewModel {
            trend_recognition_state: Some("STRUCTURAL_PERSISTENCE".to_string()),
            ..Default::default()
        }));
        assert_eq!(stable, InterpretationTrendState::Stable);

        let post_rally = derive_trend_state(Some(&StateTransitionViewModel {
            no_trade_persists: true,
            ..Default::default()
        }));
        assert_eq!(post_rally, InterpretationTrendState::PostRallyConsolidation);
    }
}

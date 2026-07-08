use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::interface::presentation::{
    MarketCyclePosition, MarketInterpretationViewModel, PresentationPacket, TrendBreadthMode,
};
use crate::features::shared::interface::i18n::Language;

pub(crate) fn build_market_interpretation_view_model(
    packet: &DecisionPacket,
    pres_packet: &PresentationPacket,
    language: Language,
) -> Option<MarketInterpretationViewModel> {
    let interpretation_layer = pres_packet.interpretation_layer.as_ref()?;
    let transition_evidence = pres_packet.transition_evidence.as_ref();
    let trend_breadth_mode = transition_evidence
        .map(|evidence| evidence.trend_breadth_mode)
        .unwrap_or_default();
    let market_cycle_position = transition_evidence
        .map(|evidence| evidence.market_cycle_position)
        .unwrap_or_default();
    let flow_acceleration = packet.market_features.flow_acceleration.unwrap_or(0.0);

    let primary_context = interpretation_layer
        .signal_context_primary_context_value
        .as_str();
    let exceptional_factors = exceptional_factors(
        primary_context,
        trend_breadth_mode,
        market_cycle_position,
        flow_acceleration,
        language,
    );
    let day_type = if exceptional_factors.is_empty() {
        day_type_normal(language)
    } else {
        day_type_exceptional(language)
    };

    let primary_values = select_primary_symbols(&pres_packet.top_actions);
    let supporting_values = select_supporting_symbols(&pres_packet.top_actions, &primary_values);
    let weakening_values = select_weakening_symbols(pres_packet);
    let leadership_consistency =
        leadership_consistency(&primary_values, &supporting_values, &weakening_values);

    let leadership_breadth_value = leadership_breadth(
        trend_breadth_mode,
        primary_values.len(),
        supporting_values.len(),
        weakening_values.len(),
        primary_context,
        language,
    );

    let (breadth_score, concentration_score, rotation_score, concentration_label_text) =
        concentration_scores(trend_breadth_mode, market_cycle_position, language);
    let classification_value = if leadership_consistency.is_valid {
        concentration_label_text.clone()
    } else {
        leadership_unavailable_value(language).to_string()
    };
    let rotation_type = rotation_type(&RotationTypeInput {
        primary_context,
        trend_breadth_mode,
        market_cycle_position,
        primary: &primary_values,
        supporting: &supporting_values,
        weakening: &weakening_values,
        flow_acceleration,
        language,
    });

    let rotation_from_values = if weakening_values.is_empty() {
        primary_values.clone()
    } else {
        weakening_values.clone()
    };
    let mut rotation_to_values = primary_values.clone();
    let additional_supporting: Vec<String> = supporting_values
        .iter()
        .filter(|symbol| !rotation_to_values.contains(symbol))
        .cloned()
        .collect();
    rotation_to_values.extend(additional_supporting);

    let rotation_interpretation_value = rotation_interpretation(
        &rotation_type,
        trend_breadth_mode,
        market_cycle_position,
        flow_acceleration,
        language,
    );
    let narrative_values = market_interpretation_narrative_values(
        day_type,
        pres_packet
            .interpretation_layer
            .as_ref()
            .map(|layer| layer.signal_context_next_observation_value.as_str())
            .unwrap_or_default(),
        language,
    );

    let interpretation_priority_values = interpretation_priority(&InterpretationPriorityInput {
        trend_confidence: interpretation_layer.trend_confidence_value.as_str(),
        supply_confidence: interpretation_layer.supply_confidence_value.as_str(),
        macro_confidence: interpretation_layer.signal_context_quality_value.as_str(),
        flow_confidence: interpretation_layer.flow_confidence_value.as_str(),
        expectation_confidence: interpretation_layer.expectation_confidence_value.as_str(),
        trend_breadth_mode,
        market_cycle_position,
        exceptional_factors: &exceptional_factors,
        language,
    });

    Some(MarketInterpretationViewModel {
        title: market_interpretation_title(language).to_string(),
        notice: market_interpretation_notice(language).to_string(),
        current_decision_weight_label: current_decision_weight_label(language).to_string(),
        current_decision_weight_value: "0%".to_string(),
        narrative_label: narrative_label(language).to_string(),
        narrative_values,
        day_type_label: day_type_label(language).to_string(),
        day_type_value: day_type.to_string(),
        day_type_reason_label: day_type_reason_label(language).to_string(),
        day_type_reason_value: day_type_reason(
            primary_context,
            trend_breadth_mode,
            market_cycle_position,
            flow_acceleration,
            language,
        )
        .to_string(),
        exceptional_factors_label: exceptional_factors_label(language).to_string(),
        exceptional_factors_values: exceptional_factors,
        leadership_label: leadership_label(language).to_string(),
        leadership_classification_label: leadership_classification_label(language).to_string(),
        leadership_classification_value: classification_value,
        primary_label: primary_label(language).to_string(),
        primary_values: primary_values.clone(),
        supporting_label: supporting_label(language).to_string(),
        supporting_values: supporting_values.clone(),
        weakening_label: weakening_label(language).to_string(),
        weakening_values: weakening_values.clone(),
        leadership_metrics_label: leadership_metrics_label(language).to_string(),
        leadership_breadth_label: leadership_breadth_label(language).to_string(),
        leadership_breadth_value,
        concentration_label: concentration_label_text,
        breadth_score_label: breadth_score_label(language).to_string(),
        breadth_score_value: breadth_score.to_string(),
        concentration_score_label: concentration_score_label(language).to_string(),
        concentration_score_value: concentration_score.to_string(),
        rotation_score_label: rotation_score_label(language).to_string(),
        rotation_score_value: rotation_score.to_string(),
        rotation_label: rotation_label(language).to_string(),
        rotation_type_value: rotation_type,
        rotation_from_label: rotation_from_label(language).to_string(),
        rotation_from_values,
        rotation_to_label: rotation_to_label(language).to_string(),
        rotation_to_values,
        rotation_interpretation_label: rotation_interpretation_label(language).to_string(),
        rotation_interpretation_value,
        confidence_label: confidence_label(language).to_string(),
        trend_confidence_label: trend_confidence_label(language).to_string(),
        trend_confidence_value: interpretation_layer.trend_confidence_value.clone(),
        macro_confidence_label: macro_confidence_label(language).to_string(),
        macro_confidence_value: interpretation_layer.signal_context_quality_value.clone(),
        supply_confidence_label: supply_confidence_label(language).to_string(),
        supply_confidence_value: interpretation_layer.supply_confidence_value.clone(),
        expectation_confidence_label: expectation_confidence_label(language).to_string(),
        expectation_confidence_value: interpretation_layer.expectation_confidence_value.clone(),
        gravity_confidence_label: gravity_confidence_label(language).to_string(),
        gravity_confidence_value: interpretation_layer.gravity_confidence_value.clone(),
        flow_confidence_label: flow_confidence_label(language).to_string(),
        flow_confidence_value: interpretation_layer.flow_confidence_value.clone(),
        overall_confidence_label: overall_confidence_label(language).to_string(),
        overall_confidence_value: interpretation_layer.interpretation_quality_value.clone(),
        interpretation_priority_label: interpretation_priority_label(language).to_string(),
        interpretation_priority_values,
        observation_only_label: observation_only_label(language).to_string(),
        observation_only_value: "true".to_string(),
        boundary: market_interpretation_boundary(language).to_string(),
    })
}

struct LeadershipConsistency {
    is_valid: bool,
}

fn leadership_consistency(
    primary: &[String],
    supporting: &[String],
    weakening: &[String],
) -> LeadershipConsistency {
    let has_overlap = !intersection(primary, supporting).is_empty()
        || !intersection(primary, weakening).is_empty()
        || !intersection(supporting, weakening).is_empty();
    LeadershipConsistency {
        is_valid: !has_overlap,
    }
}

fn intersection(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|symbol| right.iter().any(|item| item == *symbol))
        .cloned()
        .collect()
}

fn leadership_unavailable_value(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership unavailable",
        Language::EnUs => "Leadership unavailable",
        Language::JaJp => "Leadership unavailable",
    }
}

fn leadership_classification_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership Classification",
        Language::EnUs => "Leadership Classification",
        Language::JaJp => "Leadership Classification",
    }
}

fn leadership_metrics_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership Metrics",
        Language::EnUs => "Leadership Metrics",
        Language::JaJp => "Leadership Metrics",
    }
}

fn narrative_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Narrative",
        Language::EnUs => "Narrative",
        Language::JaJp => "Narrative",
    }
}

fn market_interpretation_narrative_values(
    day_type: &str,
    next_observation: &str,
    language: Language,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(match (day_type, language) {
        ("normal", Language::ZhCn) => "今天是正常趋势延续。".to_string(),
        ("normal", Language::EnUs) => "Today is a normal trend continuation.".to_string(),
        ("normal", Language::JaJp) => "今日は通常のトレンド継続です。".to_string(),
        ("exceptional", Language::ZhCn) => "今天属于例外驱动日。".to_string(),
        ("exceptional", Language::EnUs) => "Today is an exception-driven day.".to_string(),
        ("exceptional", Language::JaJp) => "今日は例外駆動の日です。".to_string(),
        _ => "Today is a normal trend continuation.".to_string(),
    });
    if !next_observation.is_empty() {
        lines.push(next_observation.to_string());
    }
    lines.push(match language {
        Language::ZhCn => "没有结构性恶化证据。".to_string(),
        Language::EnUs => "No structural deterioration evidence is visible.".to_string(),
        Language::JaJp => "構造的悪化の証拠は見えていません。".to_string(),
    });
    lines
}

fn select_primary_symbols(
    top_actions: &[crate::features::radar::interface::display::TopActionViewModel],
) -> Vec<String> {
    top_actions
        .iter()
        .take(1)
        .map(|action| action.symbol.clone())
        .collect()
}

fn select_supporting_symbols(
    top_actions: &[crate::features::radar::interface::display::TopActionViewModel],
    primary: &[String],
) -> Vec<String> {
    top_actions
        .iter()
        .skip(primary.len())
        .take(2)
        .map(|action| action.symbol.clone())
        .collect()
}

fn select_weakening_symbols(pres_packet: &PresentationPacket) -> Vec<String> {
    let mut weakening = Vec::new();
    for item in &pres_packet.exit_summary.items {
        if !matches!(
            item.intent,
            crate::features::radar::interface::presentation::ExitDisplayIntent::Exit
                | crate::features::radar::interface::presentation::ExitDisplayIntent::Trim
        ) {
            continue;
        }
        weakening.push(item.symbol.clone());
    }
    for item in &pres_packet.risk_opportunities {
        if !weakening.contains(&item.symbol) {
            weakening.push(item.symbol.clone());
        }
    }
    weakening
}

fn exceptional_factors(
    primary_context: &str,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> Vec<String> {
    let mut factors = Vec::new();
    if let Some(value) = exceptional_factor_from_primary_context(primary_context, language) {
        factors.push(value);
    }
    if matches!(trend_breadth_mode, TrendBreadthMode::StructuralDefense) {
        factors.push(exceptional_factor_structural_defense(language));
    }
    if matches!(
        market_cycle_position,
        MarketCyclePosition::DistributionWarning
    ) {
        factors.push(exceptional_factor_distribution(language));
    }
    if flow_acceleration.abs() >= 0.10 {
        factors.push(exceptional_factor_abnormal_flow(language));
    }
    factors.sort();
    factors.dedup();
    factors
}

fn exceptional_factor_from_primary_context(
    primary_context: &str,
    language: Language,
) -> Option<String> {
    let factor = match primary_context {
        "Macro Event" => Some(exceptional_factor_macro_surprise(language)),
        "Index Reconstitution" => Some(exceptional_factor_index_reconstitution(language)),
        "ETF Rebalance" => Some(exceptional_factor_etf_rebalance(language)),
        "Pre-Earnings Waiting" => Some(exceptional_factor_major_earnings(language)),
        "Major Event Waiting" => Some(exceptional_factor_unusual_rotation(language)),
        "Holiday Liquidity" => Some(exceptional_factor_unusual_rotation(language)),
        "Quarter-end Rebalancing" | "Month-end Rebalancing" => {
            Some(exceptional_factor_etf_rebalance(language))
        }
        "None" => None,
        _ => None,
    }?;
    Some(factor)
}

fn leadership_breadth(
    trend_breadth_mode: TrendBreadthMode,
    primary_count: usize,
    supporting_count: usize,
    weakening_count: usize,
    primary_context: &str,
    language: Language,
) -> String {
    if matches!(trend_breadth_mode, TrendBreadthMode::BroadExpansion) && weakening_count == 0 {
        return leadership_breadth_broad(language).to_string();
    }
    if matches!(trend_breadth_mode, TrendBreadthMode::StructuralDefense) {
        return leadership_breadth_defensive(language).to_string();
    }
    if weakening_count > 0
        || primary_context == "Major Event Waiting"
        || primary_context == "Pre-Earnings Waiting"
    {
        return leadership_breadth_rotation(language).to_string();
    }
    if primary_count <= 1 && supporting_count <= 2 {
        return leadership_breadth_narrow(language).to_string();
    }
    leadership_breadth_broad(language).to_string()
}

fn concentration_scores(
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    language: Language,
) -> (u8, u8, u8, String) {
    match (trend_breadth_mode, market_cycle_position) {
        (TrendBreadthMode::BroadExpansion, _) => {
            (78, 34, 14, concentration_label_broad(language).to_string())
        }
        (TrendBreadthMode::NarrowLeadership, MarketCyclePosition::CrowdedExpectation) => (
            35,
            82,
            18,
            concentration_label_very_narrow(language).to_string(),
        ),
        (TrendBreadthMode::NarrowLeadership, _) => {
            (38, 80, 20, concentration_label_narrow(language).to_string())
        }
        (TrendBreadthMode::FragileRotation, MarketCyclePosition::DistributionWarning) => (
            24,
            78,
            56,
            concentration_label_rotation(language).to_string(),
        ),
        (TrendBreadthMode::FragileRotation, _) => (
            30,
            72,
            48,
            concentration_label_rotation(language).to_string(),
        ),
        (TrendBreadthMode::StructuralDefense, _) => (
            20,
            76,
            30,
            concentration_label_defensive(language).to_string(),
        ),
    }
}

struct RotationTypeInput<'a> {
    primary_context: &'a str,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    primary: &'a [String],
    supporting: &'a [String],
    weakening: &'a [String],
    flow_acceleration: f64,
    language: Language,
}

fn rotation_type(input: &RotationTypeInput<'_>) -> String {
    if matches!(
        input.primary_context,
        "Index Reconstitution" | "ETF Rebalance"
    ) {
        return rotation_type_index(input.language).to_string();
    }
    if matches!(input.primary_context, "Macro Event") {
        return rotation_type_macro(input.language).to_string();
    }
    if matches!(
        input.trend_breadth_mode,
        TrendBreadthMode::StructuralDefense
    ) {
        return rotation_type_defensive(input.language).to_string();
    }
    if matches!(input.trend_breadth_mode, TrendBreadthMode::BroadExpansion) {
        return rotation_type_broad(input.language).to_string();
    }
    if input
        .weakening
        .iter()
        .any(|symbol| symbol == "NVDA" || symbol == "PLTR")
        && input.primary.iter().any(|symbol| symbol == "SPY")
        && input.flow_acceleration.abs() < 0.10
    {
        return rotation_type_mega_cap(input.language).to_string();
    }
    if matches!(
        input.market_cycle_position,
        MarketCyclePosition::DistributionWarning
    ) {
        return rotation_type_defensive(input.language).to_string();
    }
    if !input.supporting.is_empty() || !input.weakening.is_empty() {
        return rotation_type_sector(input.language).to_string();
    }
    rotation_type_none(input.language).to_string()
}

fn rotation_interpretation(
    rotation_type: &str,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> String {
    match rotation_type {
        "index_reconstitution" => rotation_interpretation_index(language).to_string(),
        "etf_rebalance" => rotation_interpretation_etf(language).to_string(),
        "macro_repricing" => rotation_interpretation_macro(language).to_string(),
        "mega_cap_internal_rotation" => rotation_interpretation_mega_cap(language).to_string(),
        "defensive_rotation" => rotation_interpretation_defensive(language).to_string(),
        "broad_participation" => rotation_interpretation_broad(language).to_string(),
        "sector_or_index_rotation" => rotation_interpretation_sector(language).to_string(),
        _ => match (trend_breadth_mode, market_cycle_position) {
            (TrendBreadthMode::BroadExpansion, _) => {
                rotation_interpretation_broad(language).to_string()
            }
            (TrendBreadthMode::StructuralDefense, _) => {
                rotation_interpretation_defensive(language).to_string()
            }
            (TrendBreadthMode::FragileRotation, MarketCyclePosition::DistributionWarning) => {
                rotation_interpretation_defensive(language).to_string()
            }
            _ if flow_acceleration < -0.10 => {
                rotation_interpretation_withdrawal(language).to_string()
            }
            _ => rotation_interpretation_none(language).to_string(),
        },
    }
}

struct InterpretationPriorityInput<'a> {
    trend_confidence: &'a str,
    supply_confidence: &'a str,
    macro_confidence: &'a str,
    flow_confidence: &'a str,
    expectation_confidence: &'a str,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    exceptional_factors: &'a [String],
    language: Language,
}

fn interpretation_priority(input: &InterpretationPriorityInput<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "{}: {}",
        interpretation_priority_trend_label(input.language),
        trend_stars(input.trend_confidence)
    ));
    if !input.supply_confidence.eq_ignore_ascii_case("UNAVAILABLE") {
        lines.push(format!(
            "{}: {}",
            interpretation_priority_supply_label(input.language),
            "★★"
        ));
    }
    if !input.macro_confidence.eq_ignore_ascii_case("UNAVAILABLE")
        || !input.exceptional_factors.is_empty()
    {
        lines.push(format!(
            "{}: {}",
            interpretation_priority_macro_label(input.language),
            "★"
        ));
    }
    if !input.flow_confidence.eq_ignore_ascii_case("UNAVAILABLE")
        && !matches!(input.trend_breadth_mode, TrendBreadthMode::BroadExpansion)
    {
        lines.push(format!(
            "{}: {}",
            interpretation_priority_flow_label(input.language),
            "☆"
        ));
    }
    if !input
        .expectation_confidence
        .eq_ignore_ascii_case("UNAVAILABLE")
        || matches!(
            input.market_cycle_position,
            MarketCyclePosition::CrowdedExpectation
        )
    {
        lines.push(format!(
            "{}: {}",
            interpretation_priority_expectation_label(input.language),
            "☆"
        ));
    }
    lines
}

fn trend_stars(value: &str) -> String {
    match value.to_ascii_uppercase().as_str() {
        "HIGH" => "★★★★★".to_string(),
        "MEDIUM" => "★★★".to_string(),
        "LOW" => "★".to_string(),
        _ => "☆".to_string(),
    }
}

fn current_decision_weight_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Current decision weight",
        Language::EnUs => "Current decision weight",
        Language::JaJp => "Current decision weight",
    }
}

fn market_interpretation_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "🧭 市场解释层",
        Language::EnUs => "🧭 Market Interpretation Layer",
        Language::JaJp => "🧭 市場解釈レイヤー",
    }
}

fn market_interpretation_notice(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "仅作解释输出。Decision Weight 固定为 0%，不会进入 Gate / Execution / Trader / Action Matrix / Position Sizing。",
        Language::EnUs => "Observation only. Decision Weight is fixed at 0%, and this layer does not enter Gate / Execution / Trader / Action Matrix / Position Sizing.",
        Language::JaJp => "説明出力のみ。Decision Weight は 0% に固定され、Gate / Execution / Trader / Action Matrix / Position Sizing には入らない。",
    }
}

fn day_type_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "dayType",
        Language::EnUs => "dayType",
        Language::JaJp => "dayType",
    }
}

fn day_type_reason_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "reason",
        Language::EnUs => "reason",
        Language::JaJp => "reason",
    }
}

fn exceptional_factors_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "exceptionalFactors",
        Language::EnUs => "exceptionalFactors",
        Language::JaJp => "exceptionalFactors",
    }
}

fn leadership_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership",
        Language::EnUs => "Leadership",
        Language::JaJp => "Leadership",
    }
}

fn primary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "primary",
        Language::EnUs => "primary",
        Language::JaJp => "primary",
    }
}

fn supporting_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "supporting",
        Language::EnUs => "supporting",
        Language::JaJp => "supporting",
    }
}

fn weakening_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "weakening",
        Language::EnUs => "weakening",
        Language::JaJp => "weakening",
    }
}

fn leadership_breadth_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "leadershipBreadth",
        Language::EnUs => "leadershipBreadth",
        Language::JaJp => "leadershipBreadth",
    }
}

fn breadth_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "breadthScore",
        Language::EnUs => "breadthScore",
        Language::JaJp => "breadthScore",
    }
}

fn concentration_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "concentrationScore",
        Language::EnUs => "concentrationScore",
        Language::JaJp => "concentrationScore",
    }
}

fn rotation_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "rotationScore",
        Language::EnUs => "rotationScore",
        Language::JaJp => "rotationScore",
    }
}

fn rotation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Rotation Observation",
        Language::EnUs => "Rotation Observation",
        Language::JaJp => "Rotation Observation",
    }
}

fn rotation_from_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "from",
        Language::EnUs => "from",
        Language::JaJp => "from",
    }
}

fn rotation_to_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "to",
        Language::EnUs => "to",
        Language::JaJp => "to",
    }
}

fn rotation_interpretation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "interpretation",
        Language::EnUs => "interpretation",
        Language::JaJp => "interpretation",
    }
}

fn confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Observation Confidence",
        Language::EnUs => "Observation Confidence",
        Language::JaJp => "Observation Confidence",
    }
}

fn trend_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "trend",
        Language::EnUs => "trend",
        Language::JaJp => "trend",
    }
}

fn macro_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "macro",
        Language::EnUs => "macro",
        Language::JaJp => "macro",
    }
}

fn supply_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "supply",
        Language::EnUs => "supply",
        Language::JaJp => "supply",
    }
}

fn expectation_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "expectation",
        Language::EnUs => "expectation",
        Language::JaJp => "expectation",
    }
}

fn gravity_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "gravity",
        Language::EnUs => "gravity",
        Language::JaJp => "gravity",
    }
}

fn flow_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "flow",
        Language::EnUs => "flow",
        Language::JaJp => "flow",
    }
}

fn overall_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "overall",
        Language::EnUs => "overall",
        Language::JaJp => "overall",
    }
}

fn interpretation_priority_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Interpretation Priority",
        Language::EnUs => "Interpretation Priority",
        Language::JaJp => "Interpretation Priority",
    }
}

fn interpretation_priority_trend_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Trend",
        Language::EnUs => "Trend",
        Language::JaJp => "Trend",
    }
}

fn interpretation_priority_supply_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Supply",
        Language::EnUs => "Supply",
        Language::JaJp => "Supply",
    }
}

fn interpretation_priority_macro_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Macro",
        Language::EnUs => "Macro",
        Language::JaJp => "Macro",
    }
}

fn interpretation_priority_flow_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Flow",
        Language::EnUs => "Flow",
        Language::JaJp => "Flow",
    }
}

fn interpretation_priority_expectation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Expectation",
        Language::EnUs => "Expectation",
        Language::JaJp => "Expectation",
    }
}

fn observation_only_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "observationOnly",
        Language::EnUs => "observationOnly",
        Language::JaJp => "observationOnly",
    }
}

fn day_type_normal(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "normal",
        Language::EnUs => "normal",
        Language::JaJp => "normal",
    }
}

fn day_type_exceptional(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "exceptional",
        Language::EnUs => "exceptional",
        Language::JaJp => "exceptional",
    }
}

fn day_type_reason(
    primary_context: &str,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> &'static str {
    match (
        primary_context,
        trend_breadth_mode,
        market_cycle_position,
        flow_acceleration,
    ) {
        ("Macro Event", _, _, _) => match language {
            Language::ZhCn => "macro_surprise",
            Language::EnUs => "macro_surprise",
            Language::JaJp => "macro_surprise",
        },
        ("Index Reconstitution", _, _, _) => match language {
            Language::ZhCn => "index_reconstitution",
            Language::EnUs => "index_reconstitution",
            Language::JaJp => "index_reconstitution",
        },
        ("ETF Rebalance", _, _, _) => match language {
            Language::ZhCn => "etf_rebalance",
            Language::EnUs => "etf_rebalance",
            Language::JaJp => "etf_rebalance",
        },
        ("Pre-Earnings Waiting", _, _, _) => match language {
            Language::ZhCn => "major_earnings_surprise",
            Language::EnUs => "major_earnings_surprise",
            Language::JaJp => "major_earnings_surprise",
        },
        ("Major Event Waiting", _, _, _) => match language {
            Language::ZhCn => "unusual_rotation",
            Language::EnUs => "unusual_rotation",
            Language::JaJp => "unusual_rotation",
        },
        (_, TrendBreadthMode::StructuralDefense, _, _) => match language {
            Language::ZhCn => "defensive_rotation",
            Language::EnUs => "defensive_rotation",
            Language::JaJp => "defensive_rotation",
        },
        (_, _, MarketCyclePosition::DistributionWarning, _) => match language {
            Language::ZhCn => "distribution_warning",
            Language::EnUs => "distribution_warning",
            Language::JaJp => "distribution_warning",
        },
        (_, TrendBreadthMode::FragileRotation, _, x) if x.abs() >= 0.10 => match language {
            Language::ZhCn => "abnormal_flow",
            Language::EnUs => "abnormal_flow",
            Language::JaJp => "abnormal_flow",
        },
        _ => match language {
            Language::ZhCn => "trend_continuation",
            Language::EnUs => "trend_continuation",
            Language::JaJp => "trend_continuation",
        },
    }
}

fn market_interpretation_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Boundary: market interpretation is observation only. It does not enter Gate, Execution, Trader, Action Matrix, Position Sizing, risk sizing, or any decision threshold.",
        Language::EnUs => "Boundary: market interpretation is observation only. It does not enter Gate, Execution, Trader, Action Matrix, Position Sizing, risk sizing, or any decision threshold.",
        Language::JaJp => "境界: market interpretation は観測専用であり、Gate、Execution、Trader、Action Matrix、Position Sizing、risk sizing、いかなる decision threshold にも入らない。",
    }
}

fn concentration_label_broad(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "broad_participation",
        Language::EnUs => "broad_participation",
        Language::JaJp => "broad_participation",
    }
}

fn concentration_label_narrow(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "narrow",
        Language::EnUs => "narrow",
        Language::JaJp => "narrow",
    }
}

fn concentration_label_very_narrow(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "very_narrow",
        Language::EnUs => "very_narrow",
        Language::JaJp => "very_narrow",
    }
}

fn concentration_label_rotation(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "rotation",
        Language::EnUs => "rotation",
        Language::JaJp => "rotation",
    }
}

fn concentration_label_defensive(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "defensive",
        Language::EnUs => "defensive",
        Language::JaJp => "defensive",
    }
}

fn leadership_breadth_broad(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "broad",
        Language::EnUs => "broad",
        Language::JaJp => "broad",
    }
}

fn leadership_breadth_narrow(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "narrow",
        Language::EnUs => "narrow",
        Language::JaJp => "narrow",
    }
}

fn leadership_breadth_rotation(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "rotation",
        Language::EnUs => "rotation",
        Language::JaJp => "rotation",
    }
}

fn leadership_breadth_defensive(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "defensive",
        Language::EnUs => "defensive",
        Language::JaJp => "defensive",
    }
}

fn exceptional_factor_macro_surprise(language: Language) -> String {
    match language {
        Language::ZhCn => "macro surprise".to_string(),
        Language::EnUs => "macro surprise".to_string(),
        Language::JaJp => "macro surprise".to_string(),
    }
}

fn exceptional_factor_index_reconstitution(language: Language) -> String {
    match language {
        Language::ZhCn => "index reconstitution".to_string(),
        Language::EnUs => "index reconstitution".to_string(),
        Language::JaJp => "index reconstitution".to_string(),
    }
}

fn exceptional_factor_etf_rebalance(language: Language) -> String {
    match language {
        Language::ZhCn => "ETF rebalance".to_string(),
        Language::EnUs => "ETF rebalance".to_string(),
        Language::JaJp => "ETF rebalance".to_string(),
    }
}

fn exceptional_factor_major_earnings(language: Language) -> String {
    match language {
        Language::ZhCn => "major earnings surprise".to_string(),
        Language::EnUs => "major earnings surprise".to_string(),
        Language::JaJp => "major earnings surprise".to_string(),
    }
}

fn exceptional_factor_abnormal_flow(language: Language) -> String {
    match language {
        Language::ZhCn => "abnormal volume / flow".to_string(),
        Language::EnUs => "abnormal volume / flow".to_string(),
        Language::JaJp => "abnormal volume / flow".to_string(),
    }
}

fn exceptional_factor_unusual_rotation(language: Language) -> String {
    match language {
        Language::ZhCn => "unusual rotation".to_string(),
        Language::EnUs => "unusual rotation".to_string(),
        Language::JaJp => "unusual rotation".to_string(),
    }
}

fn exceptional_factor_structural_defense(language: Language) -> String {
    match language {
        Language::ZhCn => "structural defense".to_string(),
        Language::EnUs => "structural defense".to_string(),
        Language::JaJp => "structural defense".to_string(),
    }
}

fn exceptional_factor_distribution(language: Language) -> String {
    match language {
        Language::ZhCn => "distribution warning".to_string(),
        Language::EnUs => "distribution warning".to_string(),
        Language::JaJp => "distribution warning".to_string(),
    }
}

fn rotation_type_none(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "none",
        Language::EnUs => "none",
        Language::JaJp => "none",
    }
}

fn rotation_type_index(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "index_rotation",
        Language::EnUs => "index_rotation",
        Language::JaJp => "index_rotation",
    }
}

fn rotation_type_macro(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "macro_repricing",
        Language::EnUs => "macro_repricing",
        Language::JaJp => "macro_repricing",
    }
}

fn rotation_type_mega_cap(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "mega_cap_internal_rotation",
        Language::EnUs => "mega_cap_internal_rotation",
        Language::JaJp => "mega_cap_internal_rotation",
    }
}

fn rotation_type_defensive(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "defensive_rotation",
        Language::EnUs => "defensive_rotation",
        Language::JaJp => "defensive_rotation",
    }
}

fn rotation_type_broad(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "broad_participation",
        Language::EnUs => "broad_participation",
        Language::JaJp => "broad_participation",
    }
}

fn rotation_type_sector(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "sector_or_index_rotation",
        Language::EnUs => "sector_or_index_rotation",
        Language::JaJp => "sector_or_index_rotation",
    }
}

fn rotation_interpretation_none(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资金没有明显撤退，仍视为普通延续观察。",
        Language::EnUs => "No clear withdrawal; observation remains on ordinary continuation.",
        Language::JaJp => "資金は明確に撤退しておらず、通常の継続観測とみなす。",
    }
}

fn rotation_interpretation_broad(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "上涨主要来自广泛参与，而不是少数核心资产。",
        Language::EnUs => {
            "The upside is driven by broad participation rather than a small set of leaders."
        }
        Language::JaJp => "上昇は少数の主役ではなく、広い参加によって支えられている。",
    }
}

fn rotation_interpretation_mega_cap(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资金不是撤退，而是在主导大盘的核心资产之间轮动。",
        Language::EnUs => "Capital is not withdrawing; it is rotating within the mega-cap leaders.",
        Language::JaJp => "資金は撤退ではなく、メガキャップ主導銘柄内でローテーションしている。",
    }
}

fn rotation_interpretation_defensive(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资金更偏向防御与低风险承接，整体解释应视为防御轮动。",
        Language::EnUs => "Capital is tilting toward defense and lower-risk absorption; treat this as defensive rotation.",
        Language::JaJp => "資金は防御と低リスク吸収に寄り、全体は防御ローテーションとみなす。",
    }
}

fn rotation_interpretation_sector(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "不是全面撤退，而是行业 / 资产组内部的轮动。",
        Language::EnUs => {
            "This is not broad withdrawal; it is rotation within sectors or asset groups."
        }
        Language::JaJp => "全面的な撤退ではなく、セクター / 資産 समूहの内部ローテーション。",
    }
}

fn rotation_interpretation_macro(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "宏观信息触发重新定价，属于事件驱动的解释层。",
        Language::EnUs => {
            "Macro information triggered repricing; this is an event-driven explanatory layer."
        }
        Language::JaJp => "マクロ情報が再価格付けを引き起こした。イベント駆動の解釈層。",
    }
}

fn rotation_interpretation_index(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "指数调仓 / 重构解释优先，不应误判为结构趋势转折。",
        Language::EnUs => "Index rebalancing / reconstitution explains the move and should not be mistaken for a structural trend turn.",
        Language::JaJp => "指数リバランス / 再構成が主因であり、構造的なトレンド転換と誤読しない。",
    }
}

fn rotation_interpretation_etf(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "ETF 相关调仓更像技术性轮动，不应直接解读为资金撤退。",
        Language::EnUs => "ETF-related rebalancing looks technical and should not be read as outright capital withdrawal.",
        Language::JaJp => "ETF 関連の調整はテクニカルなローテーションであり、資金撤退と直読しない。",
    }
}

fn rotation_interpretation_withdrawal(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "flow 显示撤退迹象，但仍需结合其他层确认是否只是轮动。",
        Language::EnUs => "Flow shows withdrawal signs, but other layers are still needed to confirm whether this is only rotation.",
        Language::JaJp => "flow は撤退の兆候を示すが、単なるローテーションかどうかは他層の確認が必要。",
    }
}

use crate::features::radar::domain::rules::{
    CreditStress, GrowthValuationImpact, LiquidityCondition, MacroGravitySnapshot, MacroPressure,
    YieldCurveState,
};
use crate::features::radar::interface::presentation::{MarketCyclePosition, TrendBreadthMode};
use crate::features::radar::interface::risk_taxonomy_read_model;
use crate::features::shared::interface::i18n::DisplayDictionary;

pub(crate) fn build_strategic_context(
    substantive_signals: &[String],
    conviction_score: Option<f64>,
    gate_passed: bool,
    breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    macro_gravity: Option<&MacroGravitySnapshot>,
    dict: &DisplayDictionary,
) -> Vec<String> {
    if substantive_signals.is_empty() && macro_gravity.is_none() {
        return Vec::new();
    }

    let tr = &dict.trend_recognition;
    let strengthening = substantive_signals.len() >= 3 || conviction_score.unwrap_or(0.0) >= 3.0;
    let direction = if strengthening {
        &tr.strategic_direction_strengthening
    } else {
        &tr.strategic_direction_observed
    };
    let continuity = if strengthening || substantive_signals.len() >= 2 {
        &tr.strategic_evidence_accumulating
    } else {
        &tr.strategic_evidence_initial
    };
    let tactical_status = if gate_passed {
        &tr.strategic_tactical_ready
    } else {
        &tr.strategic_tactical_waiting
    };
    let breadth_mode_label = match breadth_mode {
        TrendBreadthMode::BroadExpansion => &tr.trend_breadth_broad_expansion,
        TrendBreadthMode::NarrowLeadership => &tr.trend_breadth_narrow_leadership,
        TrendBreadthMode::FragileRotation => &tr.trend_breadth_fragile_rotation,
        TrendBreadthMode::StructuralDefense => &tr.trend_breadth_structural_defense,
    };

    let mut context = vec![
        format!(
            "{}: {}",
            tr.strategic_market_structure_mode, breadth_mode_label
        ),
        format!("{}: {}", tr.strategic_direction, direction),
        format!(
            "{}: {}",
            tr.strategic_cycle_position,
            risk_taxonomy_read_model::map_market_cycle_position(market_cycle_position, dict)
        ),
        format!(
            "{}: {}",
            tr.strategic_cycle_features,
            risk_taxonomy_read_model::map_market_cycle_features(market_cycle_position, dict)
        ),
        format!(
            "{}: {}",
            tr.strategic_crowding_risk,
            risk_taxonomy_read_model::map_crowding_risk(market_cycle_position, dict)
        ),
    ];

    if let Some(macro_gravity) = macro_gravity.filter(|macro_gravity| macro_gravity.enabled) {
        context.extend(format_macro_gravity_lines(macro_gravity, dict));
    }

    if !substantive_signals.is_empty() {
        context.push(format!(
            "{}: {}",
            tr.strategic_evidence_continuity, continuity
        ));
        context.push(format!(
            "{}: {}",
            tr.strategic_evidence_coverage,
            substantive_signals.join(" / ")
        ));
    }
    context.push(format!(
        "{}: {}",
        tr.strategic_tactical_status, tactical_status
    ));
    context
}

pub(crate) fn format_macro_gravity_lines(
    macro_gravity: &MacroGravitySnapshot,
    dict: &DisplayDictionary,
) -> Vec<String> {
    let tr = &dict.trend_recognition;
    let parts = [
        format!(
            "{} {}",
            tr.macro_rate_pressure,
            map_macro_pressure(macro_gravity.rate_pressure, dict)
        ),
        format!(
            "{} {}",
            tr.macro_real_yield_pressure,
            map_macro_pressure(macro_gravity.real_yield_pressure, dict)
        ),
        format!(
            "{} {}",
            tr.macro_credit_stress,
            map_credit_stress(macro_gravity.credit_stress, dict)
        ),
        format!(
            "{} {}",
            tr.macro_growth_valuation_impact,
            map_growth_valuation_impact(macro_gravity.growth_valuation_impact, dict)
        ),
        format!(
            "{} {}",
            tr.macro_liquidity,
            map_liquidity_condition(macro_gravity.liquidity, dict)
        ),
        format!(
            "{} {}",
            tr.macro_yield_curve,
            map_yield_curve_state(macro_gravity.yield_curve, dict)
        ),
    ];

    let mut lines = vec![format!(
        "{}: {}",
        tr.strategic_macro_gravity,
        parts.join(" / ")
    )];
    lines.push(format!(
        "{}: {}",
        tr.strategic_macro_gravity, tr.macro_boundary
    ));
    lines
}

pub(crate) fn map_macro_pressure(
    pressure: MacroPressure,
    _dict: &DisplayDictionary,
) -> &'static str {
    match pressure {
        MacroPressure::Falling => "FALLING",
        MacroPressure::Neutral => "NEUTRAL",
        MacroPressure::Rising => "RISING",
        MacroPressure::Tight => "TIGHT",
    }
}

pub(crate) fn map_yield_curve_state(
    state: YieldCurveState,
    _dict: &DisplayDictionary,
) -> &'static str {
    match state {
        YieldCurveState::Normal => "NORMAL",
        YieldCurveState::Flat => "FLAT",
        YieldCurveState::Inverted => "INVERTED",
        YieldCurveState::Steepening => "STEEPENING",
    }
}

pub(crate) fn map_credit_stress(stress: CreditStress, _dict: &DisplayDictionary) -> &'static str {
    match stress {
        CreditStress::Normal => "NORMAL",
        CreditStress::Watch => "WATCH",
        CreditStress::Stress => "STRESS",
    }
}

pub(crate) fn map_liquidity_condition(
    condition: LiquidityCondition,
    _dict: &DisplayDictionary,
) -> &'static str {
    match condition {
        LiquidityCondition::Loose => "LOOSE",
        LiquidityCondition::Neutral => "NEUTRAL",
        LiquidityCondition::Tight => "TIGHT",
    }
}

pub(crate) fn map_growth_valuation_impact(
    impact: GrowthValuationImpact,
    _dict: &DisplayDictionary,
) -> &'static str {
    match impact {
        GrowthValuationImpact::Supportive => "SUPPORTIVE",
        GrowthValuationImpact::Neutral => "NEUTRAL",
        GrowthValuationImpact::Compressing => "COMPRESSING",
    }
}

use crate::features::radar::domain::asset_state::AssetState;
use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::exit::AssetExitState;
use crate::features::radar::domain::market_regime::RiskOverlay;
use crate::features::radar::domain::trend_cohesion::EvidenceType;
use crate::features::radar::interface::presentation::{
    HoldingEfficiency, MarketCyclePosition, TrendBreadthMode,
};
use crate::features::shared::interface::i18n::DisplayDictionary;

pub(crate) fn classify_trend_breadth_mode(packet: &DecisionPacket) -> TrendBreadthMode {
    if packet
        .market_regime
        .transition_audit
        .as_ref()
        .is_some_and(|audit| audit.core_breakdown)
        || packet.market_regime.risk_overlay == RiskOverlay::BROKEN
    {
        return TrendBreadthMode::StructuralDefense;
    }

    let index_trend_positive = packet.assets.iter().any(|asset| {
        asset.symbol == "SPY"
            && matches!(
                asset.asset_state.state,
                AssetState::OPTIMAL
                    | AssetState::CRUISE
                    | AssetState::PULLBACK
                    | AssetState::OVERHEAT
            )
    });
    let leadership_symbols = ["MSFT", "GOOG", "NVDA"];
    let leadership_count = packet
        .assets
        .iter()
        .filter(|asset| {
            leadership_symbols.contains(&asset.symbol.as_str())
                && matches!(
                    asset.asset_state.state,
                    AssetState::OPTIMAL
                        | AssetState::CRUISE
                        | AssetState::PULLBACK
                        | AssetState::OVERHEAT
                )
        })
        .count();
    let up_ratio = if packet.market_features.total_count > 0 {
        packet.market_features.up_count as f64 / packet.market_features.total_count as f64
    } else {
        0.0
    };
    let substantive_signal_count = packet
        .trend_recognition
        .as_ref()
        .and_then(|tr| tr.substantive.as_ref())
        .map(substantive_signal_count)
        .unwrap_or(0);
    let conviction_score = packet
        .trend_recognition
        .as_ref()
        .map(|tr| tr.conviction_score)
        .unwrap_or(0.0);

    if up_ratio >= 0.60 && packet.market_features.up_count >= 4 {
        return TrendBreadthMode::BroadExpansion;
    }

    if index_trend_positive
        && leadership_count >= 2
        && substantive_signal_count >= 2
        && conviction_score >= 3.0
    {
        return TrendBreadthMode::NarrowLeadership;
    }

    TrendBreadthMode::FragileRotation
}

pub(crate) fn has_persistent_main_theme(packet: &DecisionPacket) -> bool {
    let Some(trend_recognition) = packet.trend_recognition.as_ref() else {
        return false;
    };
    let Some(substantive) = trend_recognition.substantive.as_ref() else {
        return false;
    };

    let trend_breadth_mode = classify_trend_breadth_mode(packet);
    matches!(
        trend_breadth_mode,
        TrendBreadthMode::BroadExpansion | TrendBreadthMode::NarrowLeadership
    ) && substantive_signal_count(substantive) >= 3
        && trend_recognition.conviction_score >= 3.0
}

pub(crate) fn substantive_signal_count(
    sub: &crate::features::radar::domain::trend_cohesion::SubstantiveEvidence,
) -> usize {
    let has_capex_payoff = sub.capex_payoff_signal
        || sub
            .records
            .iter()
            .any(|record| record.evidence_type == EvidenceType::CapexPayoff);
    let has_earnings_validation = sub.earnings_validation
        || sub
            .records
            .iter()
            .any(|record| record.evidence_type == EvidenceType::EarningsValidation);
    let has_order_visibility = sub.order_visibility
        || sub
            .records
            .iter()
            .any(|record| record.evidence_type == EvidenceType::OrderVisibility);

    [
        has_capex_payoff,
        has_earnings_validation,
        has_order_visibility,
    ]
    .into_iter()
    .filter(|has_signal| *has_signal)
    .count()
}

pub(crate) fn classify_market_cycle_position(
    packet: &DecisionPacket,
    breadth_mode: TrendBreadthMode,
    substantive_signal_count: usize,
    conviction_score: Option<f64>,
) -> MarketCyclePosition {
    if breadth_mode == TrendBreadthMode::StructuralDefense {
        return MarketCyclePosition::DistributionWarning;
    }

    let conviction_score = conviction_score.unwrap_or(0.0);
    let overheat_count = packet
        .assets
        .iter()
        .filter(|asset| {
            asset.asset_state.state == AssetState::OVERHEAT
                || asset.exit_decision.asset_exit_state == AssetExitState::OverheatProfitTake
        })
        .count();
    let leadership_count = core_leadership_count(packet);
    let narrow_or_fragile = matches!(
        breadth_mode,
        TrendBreadthMode::NarrowLeadership | TrendBreadthMode::FragileRotation
    );

    if narrow_or_fragile
        && substantive_signal_count >= 3
        && conviction_score >= 3.0
        && (overheat_count > 0 || leadership_count >= 2)
    {
        return MarketCyclePosition::CrowdedExpectation;
    }

    if substantive_signal_count >= 3 || conviction_score >= 3.0 {
        return MarketCyclePosition::LateAcceptance;
    }

    if substantive_signal_count >= 2 || breadth_mode == TrendBreadthMode::BroadExpansion {
        return MarketCyclePosition::MidConfirmation;
    }

    if substantive_signal_count >= 1 {
        return MarketCyclePosition::EarlyFormation;
    }

    MarketCyclePosition::Unknown
}

pub(crate) fn core_leadership_count(packet: &DecisionPacket) -> usize {
    let leadership_symbols = ["MSFT", "GOOG", "NVDA"];
    packet
        .assets
        .iter()
        .filter(|asset| {
            leadership_symbols.contains(&asset.symbol.as_str())
                && matches!(
                    asset.asset_state.state,
                    AssetState::OPTIMAL
                        | AssetState::CRUISE
                        | AssetState::PULLBACK
                        | AssetState::OVERHEAT
                )
        })
        .count()
}

pub(crate) fn classify_holding_efficiency(
    packet: &DecisionPacket,
    market_cycle_position: MarketCyclePosition,
    substantive_signal_count: usize,
) -> HoldingEfficiency {
    if market_cycle_position == MarketCyclePosition::DistributionWarning {
        return HoldingEfficiency::Neutral;
    }

    let has_overheat = packet.assets.iter().any(|asset| {
        asset.asset_state.state == AssetState::OVERHEAT
            || asset.exit_decision.asset_exit_state == AssetExitState::OverheatProfitTake
    });

    if has_overheat
        && matches!(
            market_cycle_position,
            MarketCyclePosition::LateAcceptance | MarketCyclePosition::CrowdedExpectation
        )
    {
        return HoldingEfficiency::TimeCostRising;
    }

    if substantive_signal_count >= 2
        && matches!(
            market_cycle_position,
            MarketCyclePosition::EarlyFormation | MarketCyclePosition::MidConfirmation
        )
    {
        return HoldingEfficiency::Efficient;
    }

    HoldingEfficiency::Neutral
}

pub(crate) fn build_risk_taxonomy(
    packet: &DecisionPacket,
    log: &crate::features::radar::domain::transition_log::StateTransitionLog,
    market_cycle_position: MarketCyclePosition,
    holding_efficiency: HoldingEfficiency,
    dict: &DisplayDictionary,
) -> Vec<String> {
    let te = &dict.transition_evidence;
    let structure_risk = if packet
        .market_regime
        .transition_audit
        .as_ref()
        .is_some_and(|audit| audit.core_breakdown)
        || packet.market_regime.risk_overlay == RiskOverlay::BROKEN
    {
        &te.risk_collapse
    } else if matches!(
        packet.market_regime.risk_overlay,
        RiskOverlay::DECELERATING | RiskOverlay::DEFENSIVE
    ) {
        &te.risk_fragile
    } else {
        &te.risk_normal
    };

    let initiation_volatility = if !log.trend_cohesion_gate.to && log.breakout_active_count > 0 {
        &te.volatility_active
    } else {
        &te.volatility_inactive
    };

    let position_risk = if packet.assets.iter().any(|asset| {
        asset.asset_state.state == AssetState::OVERHEAT
            || asset.exit_decision.asset_exit_state == AssetExitState::OverheatProfitTake
    }) {
        &te.position_risk_overheated
    } else {
        &te.position_risk_normal
    };

    let crowding_risk = map_crowding_risk(market_cycle_position, dict);
    let holding_efficiency = map_holding_efficiency(holding_efficiency, dict);

    vec![
        format!("{}: {}", te.market_structure_risk, structure_risk),
        format!("{}: {}", te.initiation_volatility, initiation_volatility),
        format!("{}: {}", te.position_risk, position_risk),
        format!("{}: {}", te.crowding_risk, crowding_risk),
        format!("{}: {}", te.holding_efficiency, holding_efficiency),
    ]
}

pub(crate) fn build_structural_strength(
    substantive_signal_count: usize,
    price_confirmation_record_count: usize,
    conviction_score: Option<f64>,
    dict: &DisplayDictionary,
) -> Option<String> {
    if substantive_signal_count == 0 && price_confirmation_record_count == 0 {
        return None;
    }

    let tr = &dict.trend_recognition;
    let label = if substantive_signal_count >= 3
        || price_confirmation_record_count >= 2
        || conviction_score.unwrap_or(0.0) >= 3.0
    {
        &tr.structural_strength_strengthening
    } else {
        &tr.structural_strength_observed
    };

    let mut parts = Vec::new();
    if substantive_signal_count > 0 {
        parts.push(format!(
            "{} {}",
            substantive_signal_count,
            count_unit(
                substantive_signal_count,
                &tr.structural_strength_type_unit_singular,
                &tr.structural_strength_type_unit,
            )
        ));
    }
    if price_confirmation_record_count > 0 {
        parts.push(format!(
            "{} {}",
            price_confirmation_record_count,
            count_unit(
                price_confirmation_record_count,
                &tr.structural_strength_price_confirmation_unit_singular,
                &tr.structural_strength_price_confirmation_unit,
            )
        ));
    }

    Some(format!("{} ({})", label, parts.join(" / ")))
}

pub(crate) fn map_market_cycle_position(
    position: MarketCyclePosition,
    dict: &DisplayDictionary,
) -> &str {
    let tr = &dict.trend_recognition;
    match position {
        MarketCyclePosition::EarlyFormation => &tr.cycle_position_early,
        MarketCyclePosition::MidConfirmation => &tr.cycle_position_mid,
        MarketCyclePosition::LateAcceptance => &tr.cycle_position_late,
        MarketCyclePosition::CrowdedExpectation => &tr.cycle_position_crowded,
        MarketCyclePosition::DistributionWarning => &tr.cycle_position_distribution,
        MarketCyclePosition::Unknown => &tr.cycle_position_unknown,
    }
}

pub(crate) fn map_market_cycle_features(
    position: MarketCyclePosition,
    dict: &DisplayDictionary,
) -> &str {
    let tr = &dict.trend_recognition;
    match position {
        MarketCyclePosition::EarlyFormation => &tr.cycle_features_early,
        MarketCyclePosition::MidConfirmation => &tr.cycle_features_mid,
        MarketCyclePosition::LateAcceptance => &tr.cycle_features_late,
        MarketCyclePosition::CrowdedExpectation => &tr.cycle_features_crowded,
        MarketCyclePosition::DistributionWarning => &tr.cycle_features_distribution,
        MarketCyclePosition::Unknown => &tr.cycle_features_unknown,
    }
}

pub(crate) fn map_crowding_risk(position: MarketCyclePosition, dict: &DisplayDictionary) -> &str {
    let te = &dict.transition_evidence;
    match position {
        MarketCyclePosition::CrowdedExpectation | MarketCyclePosition::DistributionWarning => {
            &te.crowding_risk_active
        }
        MarketCyclePosition::LateAcceptance => &te.crowding_risk_watch,
        MarketCyclePosition::EarlyFormation
        | MarketCyclePosition::MidConfirmation
        | MarketCyclePosition::Unknown => &te.crowding_risk_normal,
    }
}

pub(crate) fn map_holding_efficiency(
    efficiency: HoldingEfficiency,
    dict: &DisplayDictionary,
) -> &str {
    let te = &dict.transition_evidence;
    match efficiency {
        HoldingEfficiency::Efficient => &te.holding_efficiency_efficient,
        HoldingEfficiency::Neutral => &te.holding_efficiency_neutral,
        HoldingEfficiency::TimeCostRising => &te.holding_efficiency_time_cost_rising,
        HoldingEfficiency::Overdiscounted => &te.holding_efficiency_overdiscounted,
    }
}

pub(crate) fn count_unit<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 {
        singular
    } else {
        plural
    }
}

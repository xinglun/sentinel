use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::market_regime::{MarketState, RiskOverlay};
use crate::features::radar::domain::rules::ParsedRules;
use crate::features::radar::interface::presentation::{
    StateTransitionViewModel, UnmetDiffViewModel,
};
use crate::features::radar::interface::risk_taxonomy_read_model;
use crate::features::radar::interface::strategic_context_read_model;
use crate::features::radar::interface::trend_recognition_read_model;
use crate::features::shared::interface::i18n::DisplayDictionary;
use std::collections::HashSet;

pub(crate) fn build_transition_evidence(
    packet: &DecisionPacket,
    rules: &ParsedRules,
    dict: &DisplayDictionary,
) -> Option<StateTransitionViewModel> {
    let log = packet.transition_log.as_ref()?;

    let mut breakout_changes = Vec::new();
    let mut structural_breakout_symbols = HashSet::<String>::new();
    for b in &log.breakout_changes {
        if !b.status_changed {
            continue;
        }
        structural_breakout_symbols.insert(b.symbol.clone());
        breakout_changes.push(format_structural_breakout_change(b, dict));
    }

    if !breakout_changes.is_empty() && packet.assets.len() > structural_breakout_symbols.len() {
        breakout_changes.push(
            dict.transition_evidence
                .breakout_others_no_structural_change
                .clone(),
        );
    }
    let has_structural_breakout_change = !breakout_changes.is_empty();
    let scout_continuity = if !log.trend_cohesion_gate.to {
        Some(if log.breakout_active_count >= 2 {
            dict.transition_evidence
                .scout_continuity_multi_point
                .clone()
        } else {
            format!(
                "{}/{}",
                log.scout_days_without_expansion,
                log.scout_abort_days.max(1)
            )
        })
    } else {
        None
    };
    let scout_expansion = if !log.trend_cohesion_gate.to {
        Some(
            match log.breakout_active_count {
                0 => dict.transition_evidence.scout_expansion_none.as_str(),
                1 => dict.transition_evidence.scout_expansion_single.as_str(),
                _ => dict.transition_evidence.scout_expansion_multi.as_str(),
            }
            .to_string(),
        )
    } else {
        None
    };
    let scout_reset = if !log.trend_cohesion_gate.to {
        Some(
            if log.scout_reset_triggered {
                dict.transition_evidence.yes.as_str()
            } else {
                dict.transition_evidence.no.as_str()
            }
            .to_string(),
        )
    } else {
        None
    };

    let trend_recognition =
        trend_recognition_read_model::build_trend_recognition_read_model(log, dict);
    let trend_breadth_mode = risk_taxonomy_read_model::classify_trend_breadth_mode(packet);
    let market_cycle_position = risk_taxonomy_read_model::classify_market_cycle_position(
        packet,
        trend_breadth_mode,
        trend_recognition.substantive_signals.len(),
        trend_recognition.conviction_score,
    );
    let holding_efficiency = risk_taxonomy_read_model::classify_holding_efficiency(
        packet,
        market_cycle_position,
        trend_recognition.substantive_signals.len(),
    );
    let risk_taxonomy = risk_taxonomy_read_model::build_risk_taxonomy(
        packet,
        log,
        market_cycle_position,
        holding_efficiency,
        dict,
    );
    let structural_strength = risk_taxonomy_read_model::build_structural_strength(
        trend_recognition.substantive_signals.len(),
        trend_recognition.price_confirmation_record_count,
        trend_recognition.conviction_score,
        dict,
    );
    let strategic_context = strategic_context_read_model::build_strategic_context(
        &trend_recognition.substantive_signals,
        trend_recognition.conviction_score,
        log.trend_cohesion_gate.to,
        trend_breadth_mode,
        market_cycle_position,
        rules.macro_gravity.as_ref(),
        dict,
    );
    Some(StateTransitionViewModel {
        has_significant_change: log.market_state.changed
            || log.risk_overlay.changed
            || log.trend_cohesion_gate.from != log.trend_cohesion_gate.to
            || log.trend_cohesion_gate.unmet_conditions_changed
            || log.trend_cohesion_status.changed
            || log.trend_cohesion_topology.changed
            || has_structural_breakout_change
            || !trend_recognition.substantive_signals.is_empty()
            || structural_strength.is_some()
            || !strategic_context.is_empty(),
        no_trade_persists: log.no_trade_persists,
        market_state_change: if log.market_state.changed {
            Some(format!(
                "{} -> {}",
                map_market_state(log.market_state.from, dict),
                map_market_state(log.market_state.to, dict)
            ))
        } else {
            None
        },
        risk_overlay_change: if log.risk_overlay.changed {
            Some(format!(
                "{} -> {}",
                map_risk_overlay(log.risk_overlay.from, dict),
                map_risk_overlay(log.risk_overlay.to, dict)
            ))
        } else {
            None
        },
        trend_cohesion_gate_change: if log.trend_cohesion_gate.from != log.trend_cohesion_gate.to {
            Some(format!(
                "{} -> {}",
                map_gate(log.trend_cohesion_gate.from, dict),
                map_gate(log.trend_cohesion_gate.to, dict)
            ))
        } else {
            None
        },
        trend_cohesion_gate_passed: log.trend_cohesion_gate.to,
        trend_cohesion_status_change: if log.trend_cohesion_status.changed {
            Some(format!(
                "{} -> {}",
                map_trend_status(log.trend_cohesion_status.from, dict),
                map_trend_status(log.trend_cohesion_status.to, dict)
            ))
        } else {
            None
        },
        trend_cohesion_topology_change: if log.trend_cohesion_topology.changed {
            Some(format!(
                "{} -> {}",
                map_topology(log.trend_cohesion_topology.from, dict),
                map_topology(log.trend_cohesion_topology.to, dict)
            ))
        } else {
            None
        },
        trend_unmet_diff: map_unmet_diff(&log.trend_cohesion_gate, packet, dict),
        breakout_changes,
        risk_taxonomy,
        scout_continuity,
        scout_expansion,
        scout_reset,
        trend_recognition_state: trend_recognition.state,
        trend_recognition_diffusion_score: trend_recognition.diffusion_score,
        trend_recognition_conviction_score: trend_recognition.conviction_score,
        trend_recognition_lag_state: trend_recognition.lag_state,
        trend_recognition_single_asset_decay: trend_recognition.single_asset_decay,
        structural_strength,
        evidence_quality_summary: trend_recognition.evidence_quality_summary,
        substantive_signals: trend_recognition.substantive_signals,
        substantive_details: trend_recognition.substantive_details,
        strategic_context,
        trend_breadth_mode,
        market_cycle_position,
        holding_efficiency,
    })
}

fn format_structural_breakout_change(
    b: &crate::features::radar::domain::transition_log::BreakoutTransition,
    dict: &DisplayDictionary,
) -> String {
    use crate::features::radar::domain::breakout_detection::BreakoutStatus;

    let format_template = |template: &str, symbol: &str, status: &str| {
        template
            .replace("{symbol}", symbol)
            .replace("{status}", status)
    };

    if b.from_status == BreakoutStatus::NoBreakout && b.to_status != BreakoutStatus::NoBreakout {
        return format_template(
            &dict.transition_evidence.breakout_added,
            &b.symbol,
            &map_breakout_status(b.to_status, dict),
        );
    }

    if b.from_status != BreakoutStatus::NoBreakout && b.to_status == BreakoutStatus::NoBreakout {
        return format_template(
            &dict.transition_evidence.breakout_removed,
            &b.symbol,
            &map_breakout_status(b.from_status, dict),
        );
    }

    format!(
        "{}: {} -> {}",
        b.symbol,
        map_breakout_status(b.from_status, dict),
        map_breakout_status(b.to_status, dict)
    )
}

fn map_unmet_diff(
    diff: &crate::features::radar::domain::transition_log::GateTransition,
    packet: &DecisionPacket,
    dict: &DisplayDictionary,
) -> Option<UnmetDiffViewModel> {
    if diff.added.is_empty() && diff.removed.is_empty() && diff.persisting.is_empty() {
        return None;
    }

    let map_fn = |s: &str| match s {
        "StabilityThreshold" => dict.trend_cohesion.unmet.stability_threshold.replace(
            "{}",
            &format!("{:.1}", packet.trend_cohesion.stability_score),
        ),
        "ContinuityThreshold" => dict
            .trend_cohesion
            .unmet
            .continuity_threshold
            .replace("{}", &packet.trend_cohesion.continuity_streak.to_string()),
        "DirectionalCohesion" => dict.trend_cohesion.unmet.directional_cohesion.clone(),
        "HighCandidateDispersion" => dict
            .trend_cohesion
            .unmet
            .high_candidate_dispersion
            .replace("{}", &packet.trend_cohesion.candidate_count.to_string()),
        "UnstableRotation" => dict.trend_cohesion.unmet.unstable_rotation.replace(
            "{}",
            &format!("{:.0}", packet.trend_cohesion.rotation_quality_score),
        ),
        "WeakLeadership" => dict
            .trend_cohesion
            .unmet
            .weak_leadership
            .replace("{}", &packet.trend_cohesion.leader_count.to_string()),
        _ => s.to_string(),
    };

    Some(UnmetDiffViewModel {
        added: diff.added.iter().map(|s| map_fn(s)).collect(),
        removed: diff.removed.iter().map(|s| map_fn(s)).collect(),
        persisting: diff.persisting.iter().map(|s| map_fn(s)).collect(),
    })
}

fn map_market_state(state: MarketState, dict: &DisplayDictionary) -> String {
    match state {
        MarketState::ESTABLISHED | MarketState::CONFIRMED => dict.market_stages.established.clone(),
        MarketState::DEFENSIVE => dict.market_stages.defensive.clone(),
        MarketState::IGNITION | MarketState::NEWBORN => dict.market_stages.ignition.clone(),
        _ => dict.market_stages.neutral.clone(),
    }
}

fn map_risk_overlay(risk: RiskOverlay, dict: &DisplayDictionary) -> String {
    match risk {
        RiskOverlay::NORMAL => dict.risks.normal.clone(),
        RiskOverlay::DECELERATING => dict.risks.mixed.clone(),
        _ => dict.risks.defensive.clone(),
    }
}

fn map_breakout_status(
    status: crate::features::radar::domain::breakout_detection::BreakoutStatus,
    dict: &DisplayDictionary,
) -> String {
    use crate::features::radar::domain::breakout_detection::BreakoutStatus;
    match status {
        BreakoutStatus::NoBreakout => dict.breakout.no_breakout.clone(),
        BreakoutStatus::EmergingBreakout => dict.breakout.emerging_breakout.clone(),
        BreakoutStatus::ConfirmedBreakout => dict.breakout.confirmed_breakout.clone(),
    }
}

fn map_trend_status(
    status: crate::features::radar::domain::trend_cohesion::TrendCohesionStatus,
    dict: &DisplayDictionary,
) -> String {
    use crate::features::radar::domain::trend_cohesion::TrendCohesionStatus;
    match status {
        TrendCohesionStatus::Formed => dict.trend_cohesion.formed.clone(),
        TrendCohesionStatus::Forming => dict.trend_cohesion.forming.clone(),
        TrendCohesionStatus::Dispersed => dict.trend_cohesion.dispersed.clone(),
    }
}

fn map_topology(
    topology: crate::features::radar::domain::trend_cohesion::TrendCohesionTopology,
    dict: &DisplayDictionary,
) -> String {
    use crate::features::radar::domain::trend_cohesion::TrendCohesionTopology;
    match topology {
        TrendCohesionTopology::NoLeader => dict.trend_cohesion.topology_no_leader.clone(),
        TrendCohesionTopology::SingleLeader => dict.trend_cohesion.topology_single_leader.clone(),
        TrendCohesionTopology::FragmentedLeaders => {
            dict.trend_cohesion.topology_fragmented_leaders.clone()
        }
    }
}

fn map_gate(passed: bool, dict: &DisplayDictionary) -> String {
    if passed {
        dict.transition_evidence.gate_pass.clone()
    } else {
        dict.transition_evidence.gate_fail.clone()
    }
}

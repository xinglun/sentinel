use anyhow::Result;
use chrono::NaiveDate;
use serde_json::json;
use std::collections::BTreeMap;

use crate::features::shared::interface::i18n::Language;

#[derive(Clone)]
pub(crate) struct WeeklyReportContext {
    pub macro_gravity: Option<WeeklyMacroGravityContext>,
    pub research_attention_entries: usize,
    pub asset_thesis_entries: usize,
    pub capital_absorption_ipo_queue: serde_json::Value,
    pub capital_dynamics_flow_layer: serde_json::Value,
    pub expectation_layer: serde_json::Value,
}

#[derive(Clone)]
pub(crate) struct WeeklyMacroGravityContext {
    pub rate_pressure: String,
    pub real_yield_pressure: String,
    pub yield_curve: String,
    pub credit_stress: String,
    pub liquidity: String,
    pub growth_valuation_impact: String,
}

pub(crate) fn persist_weekly_state_outputs(
    save_dir: &std::path::Path,
    history: &[crate::features::radar::domain::decision::DecisionPacket],
    current_packet: &crate::features::radar::domain::decision::DecisionPacket,
    include_current_packet: bool,
    pres_packet: &crate::features::radar::interface::presentation::PresentationPacket,
    context: &WeeklyReportContext,
    current_state_machine: Option<
        &crate::features::shared::application::run_status::StateMachineSummary,
    >,
) -> Result<()> {
    let mut recent_packets: Vec<&crate::features::radar::domain::decision::DecisionPacket> =
        history.iter().rev().take(7).collect();
    recent_packets.reverse();
    if include_current_packet {
        recent_packets.push(current_packet);
    }
    if recent_packets.len() > 7 {
        recent_packets = recent_packets[recent_packets.len() - 7..].to_vec();
    }

    let mut market_state_counts = BTreeMap::<String, usize>::new();
    let mut risk_overlay_counts = BTreeMap::<String, usize>::new();
    let mut total_confidence = 0.0;
    let mut total_stability = 0.0;
    let mut trend_cohesion_ready_days = 0usize;

    for packet in &recent_packets {
        *market_state_counts
            .entry(format!("{:?}", packet.market_regime.market_state))
            .or_insert(0) += 1;
        *risk_overlay_counts
            .entry(format!("{:?}", packet.market_regime.risk_overlay))
            .or_insert(0) += 1;
        total_confidence += packet.market_features.system_confidence;
        total_stability += packet.market_features.stability_score;
        if packet.trend_cohesion.gate_passed {
            trend_cohesion_ready_days += 1;
        }
    }

    let day_count = recent_packets.len();
    let avg_confidence = if day_count > 0 {
        total_confidence / day_count as f64
    } else {
        0.0
    };
    let avg_stability = if day_count > 0 {
        total_stability / day_count as f64
    } else {
        0.0
    };
    let latest_context = build_weekly_latest_context(
        pres_packet,
        context,
        &context.capital_absorption_ipo_queue,
        &context.capital_dynamics_flow_layer,
        &context.expectation_layer,
    );
    let state_machine_summaries =
        load_weekly_state_machine_summaries(save_dir, current_packet.date, current_state_machine);
    let weekly_totals = build_weekly_totals(&state_machine_summaries);
    let daily_summaries = build_daily_summaries(&state_machine_summaries);

    let metrics = json!({
        "generated_at": chrono::Local::now().to_rfc3339(),
        "as_of_date": pres_packet.date_str,
        "days_analyzed": day_count,
        "include_current_packet": include_current_packet,
        "data_status": if include_current_packet { "OK" } else { "DATA_UNAVAILABLE" },
        "latest_market_state": format!("{:?}", current_packet.market_regime.market_state),
        "latest_risk_overlay": format!("{:?}", current_packet.market_regime.risk_overlay),
        "avg_confidence": avg_confidence,
        "avg_stability": avg_stability,
        "trend_cohesion_ready_days": trend_cohesion_ready_days,
        // 互換キーとして `participation_ready_days` を残すが、意味は `trend_cohesion_ready_days` です。
        // downstream script は従来の participation semantics ではなく、cohesion gate semantics を受け取ります。
        "participation_ready_days": trend_cohesion_ready_days,
        "market_state_counts": market_state_counts,
        "risk_overlay_counts": risk_overlay_counts,
        "weekly_totals": weekly_totals,
        "daily_summaries": daily_summaries,
        "latest_context": latest_context,
    });

    std::fs::write(
        save_dir.join("weekly_state_metrics.json"),
        serde_json::to_string_pretty(&metrics)?,
    )?;

    let text = weekly_text(pres_packet.language);
    let mut review = String::new();
    review.push_str(text.title);
    review.push_str("\n\n");
    review.push_str(&format!("- {}: {}\n", text.as_of, pres_packet.date_str));
    review.push_str(&format!(
        "- {}: {}\n",
        text.status,
        if include_current_packet {
            text.status_using_current
        } else {
            text.status_data_unavailable
        }
    ));
    review.push_str(&format!(
        "- {}: {} | {}\n",
        text.latest_headline,
        pres_packet.macro_display.headline,
        pres_packet.macro_display.bias_label
    ));
    review.push_str(&format!("- {}: {}\n", text.days_analyzed, day_count));
    review.push_str(&format!(
        "- {}: {:.1}\n",
        text.avg_confidence, avg_confidence
    ));
    review.push_str(&format!("- {}: {:.1}\n", text.avg_stability, avg_stability));
    review.push_str(&format!(
        "- {}: {}\n\n",
        text.trend_cohesion_ready_days, trend_cohesion_ready_days
    ));
    review.push_str(text.market_state_counts);
    review.push('\n');
    for (state, count) in metrics["market_state_counts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        review.push_str(&format!("- {}: {}\n", state, count));
    }
    review.push('\n');
    review.push_str(text.risk_overlay_counts);
    review.push('\n');
    for (state, count) in metrics["risk_overlay_counts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        review.push_str(&format!("- {}: {}\n", state, count));
    }
    push_weekly_state_machine_totals(
        &mut review,
        &weekly_totals,
        &metrics["daily_summaries"],
        text,
    );
    push_weekly_strategic_context_snapshot(&mut review, pres_packet, text);
    push_weekly_signal_context_snapshot(
        &mut review,
        pres_packet.interpretation_layer.as_ref(),
        text,
    );
    push_weekly_market_interpretation_snapshot(
        &mut review,
        pres_packet.market_interpretation.as_ref(),
        pres_packet.leader_persistence.as_ref(),
        text,
    );
    push_weekly_macro_gravity_snapshot(&mut review, context, text);
    push_weekly_capital_dynamics_snapshot(
        &mut review,
        &context.capital_absorption_ipo_queue,
        &context.capital_dynamics_flow_layer,
        text,
    );
    push_weekly_cognitive_calibration_snapshot(&mut review, context, text);
    push_weekly_expectation_snapshot(&mut review, &context.expectation_layer, text);

    std::fs::write(save_dir.join("weekly_state_review_auto.md"), review)?;
    Ok(())
}

#[derive(Clone)]
struct WeeklyStateMachineEntry {
    date: NaiveDate,
    summary: crate::features::shared::application::run_status::StateMachineSummary,
}

fn load_weekly_state_machine_summaries(
    save_dir: &std::path::Path,
    current_date: NaiveDate,
    current_state_machine: Option<
        &crate::features::shared::application::run_status::StateMachineSummary,
    >,
) -> Vec<WeeklyStateMachineEntry> {
    let mut entries = std::fs::read_dir(save_dir)
        .ok()
        .into_iter()
        .flat_map(|read_dir| read_dir.filter_map(std::result::Result::ok))
        .filter_map(|entry| load_state_machine_summary_from_run_status(&entry.path()))
        .filter(|(date, _)| *date <= current_date)
        .collect::<BTreeMap<_, _>>();

    if let Some(summary) = current_state_machine {
        entries.insert(current_date, summary.clone());
    }

    let mut recent = entries
        .into_iter()
        .map(|(date, summary)| WeeklyStateMachineEntry { date, summary })
        .collect::<Vec<_>>();
    if recent.len() > 7 {
        recent = recent.split_off(recent.len() - 7);
    }
    recent
}

fn load_state_machine_summary_from_run_status(
    path: &std::path::Path,
) -> Option<(
    NaiveDate,
    crate::features::shared::application::run_status::StateMachineSummary,
)> {
    let file_name = path.file_name()?.to_str()?;
    let raw_date = file_name
        .strip_prefix("run_status_")?
        .strip_suffix(".json")?;
    let date = NaiveDate::parse_from_str(raw_date, "%Y-%m-%d").ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    let summary = value.get("state_machine")?;
    serde_json::from_value(summary.clone())
        .ok()
        .map(|summary| (date, summary))
}

fn build_weekly_totals(entries: &[WeeklyStateMachineEntry]) -> serde_json::Value {
    let mut reset_confirmed_total = 0usize;
    let mut reset_blocked_total = 0usize;
    let mut soft_reset_total = 0usize;
    let mut duration_lock_total = 0usize;
    let mut defensive_override_total = 0usize;
    let mut core_breakdown_total = 0usize;
    let mut reconciliation_mismatch_total = 0usize;
    let mut preflight_failed_total = 0usize;

    for entry in entries {
        let summary = &entry.summary;
        reset_confirmed_total += usize::from(summary.reset_confirmed);
        reset_blocked_total += usize::from(summary.reset_blocked);
        soft_reset_total += usize::from(summary.soft_reset_applied);
        duration_lock_total += usize::from(summary.duration_locked);
        defensive_override_total += usize::from(summary.defensive_override);
        core_breakdown_total += usize::from(summary.core_breakdown);
        reconciliation_mismatch_total += summary.reconciliation_mismatch_count;
        preflight_failed_total += usize::from(summary.preflight_failed);
    }

    json!({
        "days": entries.len(),
        "reset_confirmed_total": reset_confirmed_total,
        "reset_blocked_total": reset_blocked_total,
        "soft_reset_total": soft_reset_total,
        "duration_lock_total": duration_lock_total,
        "defensive_override_total": defensive_override_total,
        "core_breakdown_total": core_breakdown_total,
        "reconciliation_mismatch_total": reconciliation_mismatch_total,
        "preflight_failed_total": preflight_failed_total
    })
}

fn build_daily_summaries(entries: &[WeeklyStateMachineEntry]) -> serde_json::Value {
    json!(entries
        .iter()
        .map(|entry| {
            let summary = &entry.summary;
            json!({
                "date": entry.date.to_string(),
                "from_state": &summary.from_state,
                "to_state": &summary.to_state,
                "reset_confirmed": summary.reset_confirmed,
                "reset_blocked": summary.reset_blocked,
                "soft_reset_applied": summary.soft_reset_applied,
                "duration_locked": summary.duration_locked,
                "defensive_override": summary.defensive_override,
                "core_breakdown": summary.core_breakdown,
                "reconciliation_mismatch_count": summary.reconciliation_mismatch_count,
                "preflight_failed": summary.preflight_failed
            })
        })
        .collect::<Vec<_>>())
}

fn build_weekly_latest_context(
    pres_packet: &crate::features::radar::interface::presentation::PresentationPacket,
    context: &WeeklyReportContext,
    capital_absorption_ipo_queue: &serde_json::Value,
    capital_dynamics_flow_layer: &serde_json::Value,
    expectation_layer: &serde_json::Value,
) -> serde_json::Value {
    let trend_breadth_mode = pres_packet
        .transition_evidence
        .as_ref()
        .map(|evidence| format!("{:?}", evidence.trend_breadth_mode));
    let market_cycle_position = pres_packet
        .transition_evidence
        .as_ref()
        .map(|evidence| format!("{:?}", evidence.market_cycle_position));
    let holding_efficiency = pres_packet
        .transition_evidence
        .as_ref()
        .map(|evidence| format!("{:?}", evidence.holding_efficiency));
    let strategic_context = pres_packet
        .transition_evidence
        .as_ref()
        .map(|evidence| evidence.strategic_context.clone())
        .unwrap_or_default();
    let signal_context = build_weekly_signal_context(pres_packet.interpretation_layer.as_ref());
    let market_interpretation =
        build_weekly_market_interpretation_context(pres_packet.market_interpretation.as_ref());
    let leader_persistence =
        build_weekly_leader_persistence_context(pres_packet.leader_persistence.as_ref());

    json!({
        "trend_breadth_mode": trend_breadth_mode,
        "market_cycle_position": market_cycle_position,
        "holding_efficiency": holding_efficiency,
        "strategic_context": strategic_context,
        "signal_context": signal_context,
        "market_interpretation": market_interpretation,
        "leader_persistence": leader_persistence,
        "macro_gravity": build_weekly_macro_gravity_context(context),
        "capital_absorption_ipo_queue": capital_absorption_ipo_queue,
        "capital_dynamics": {
            "supply_layer": capital_absorption_ipo_queue,
            "flow_layer": capital_dynamics_flow_layer
        },
        "expectation_layer": expectation_layer,
        "cognitive_calibration": {
            "research_attention_entries": context.research_attention_entries,
            "asset_thesis_entries": context.asset_thesis_entries
        }
    })
}

fn build_weekly_macro_gravity_context(context: &WeeklyReportContext) -> serde_json::Value {
    let Some(macro_gravity) = context.macro_gravity.as_ref() else {
        return json!({
            "configured": false
        });
    };

    json!({
        "configured": true,
        "rate_pressure": macro_gravity.rate_pressure,
        "real_yield_pressure": macro_gravity.real_yield_pressure,
        "yield_curve": macro_gravity.yield_curve,
        "credit_stress": macro_gravity.credit_stress,
        "liquidity": macro_gravity.liquidity,
        "growth_valuation_impact": macro_gravity.growth_valuation_impact
    })
}

fn build_weekly_signal_context(
    layer: Option<&crate::features::radar::interface::presentation::InterpretationLayerViewModel>,
) -> serde_json::Value {
    let Some(layer) = layer else {
        return json!({
            "configured": false
        });
    };

    json!({
        "configured": true,
        "information_content": layer.signal_context_information_content_value,
        "primary_context": layer.signal_context_primary_context_value,
        "context_quality": layer.signal_context_quality_value,
        "event_fact": layer.signal_context_event_fact_value,
        "source_diagnostics": layer.signal_context_source_diagnostics_value,
        "interpretation": layer.signal_context_interpretation_value
    })
}

fn build_weekly_market_interpretation_context(
    layer: Option<&crate::features::radar::interface::presentation::MarketInterpretationViewModel>,
) -> serde_json::Value {
    let Some(layer) = layer else {
        return json!({
            "configured": false
        });
    };

    json!({
        "configured": true,
        "decision_weight": layer.current_decision_weight_value,
        "day_type": layer.day_type_value,
        "reason": layer.day_type_reason_value,
        "exceptional_factors": layer.exceptional_factors_values,
        "leadership": {
            "primary": layer.primary_values,
            "supporting": layer.supporting_values,
            "weakening": layer.weakening_values,
            "breadth": layer.leadership_breadth_value
        },
        "rotation": {
            "type": layer.rotation_type_value,
            "from": layer.rotation_from_values,
            "to": layer.rotation_to_values,
            "interpretation": layer.rotation_interpretation_value,
            "observation_only": layer.observation_only_value
        },
        "concentration": {
            "breadth_score": layer.breadth_score_value,
            "concentration_score": layer.concentration_score_value,
            "rotation_score": layer.rotation_score_value,
            "label": layer.concentration_label
        },
        "confidence": {
            "trend": layer.trend_confidence_value,
            "macro": layer.macro_confidence_value,
            "supply": layer.supply_confidence_value,
            "expectation": layer.expectation_confidence_value,
            "gravity": layer.gravity_confidence_value,
            "flow": layer.flow_confidence_value,
            "overall": layer.overall_confidence_value
        },
        "priority": layer.interpretation_priority_values,
        "boundary": layer.boundary
    })
}

fn build_weekly_leader_persistence_context(
    layer: Option<&crate::features::radar::interface::presentation::LeaderPersistenceViewModel>,
) -> serde_json::Value {
    let Some(layer) = layer else {
        return json!({
            "configured": false
        });
    };

    json!({
        "configured": true,
        "primary_leader": layer.primary_leader_value,
        "persistence_days": layer.persistence_days,
        "leadership_score": layer.leadership_score,
        "state": layer.leader_state_value,
        "change_from_yesterday": {
            "persistence_days": layer.persistence_change_days,
            "score": layer.score_change,
            "display": layer.change_from_yesterday_value
        },
        "switch_history": layer.switch_history_values,
        "boundary": layer.boundary
    })
}

fn push_weekly_signal_context_snapshot(
    review: &mut String,
    layer: Option<&crate::features::radar::interface::presentation::InterpretationLayerViewModel>,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.signal_context_snapshot);
    review.push('\n');
    let Some(layer) = layer else {
        review.push_str(&format!("- {}\n", text.signal_context_not_configured));
        review.push_str("- ");
        review.push_str(text.signal_context_boundary);
        review.push('\n');
        return;
    };

    review.push_str(&format!(
        "- {}: {}\n",
        text.signal_context_information_content, layer.signal_context_information_content_value
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.signal_context_primary_context, layer.signal_context_primary_context_value
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.signal_context_quality, layer.signal_context_quality_value
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.signal_context_event_fact,
        if layer.signal_context_event_fact_value.is_empty() {
            "N/A"
        } else {
            &layer.signal_context_event_fact_value
        }
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.signal_context_source_diagnostics,
        if layer.signal_context_source_diagnostics_value.is_empty() {
            "N/A"
        } else {
            &layer.signal_context_source_diagnostics_value
        }
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.signal_context_interpretation,
        if layer.signal_context_interpretation_value.is_empty() {
            "N/A"
        } else {
            &layer.signal_context_interpretation_value
        }
    ));
    review.push_str("- ");
    review.push_str(text.signal_context_boundary);
    review.push('\n');
}

fn push_weekly_market_interpretation_snapshot(
    review: &mut String,
    layer: Option<&crate::features::radar::interface::presentation::MarketInterpretationViewModel>,
    leader_persistence: Option<
        &crate::features::radar::interface::presentation::LeaderPersistenceViewModel,
    >,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.market_interpretation_snapshot);
    review.push('\n');
    let Some(layer) = layer else {
        review.push_str(&format!(
            "- {}\n",
            text.market_interpretation_not_configured
        ));
        review.push_str("- ");
        review.push_str(text.market_interpretation_boundary);
        review.push('\n');
        return;
    };

    review.push_str(&format!(
        "- decision_weight: {}\n",
        layer.current_decision_weight_value
    ));
    review.push_str(&format!("- dayType: {}\n", layer.day_type_value));
    review.push_str(&format!("- reason: {}\n", layer.day_type_reason_value));
    review.push_str(&format!(
        "- exceptionalFactors: {}\n",
        render_json_string_array(&layer.exceptional_factors_values)
    ));
    if !layer.narrative_values.is_empty() {
        review.push_str(&format!("- {}:\n", layer.narrative_label));
        for value in &layer.narrative_values {
            review.push_str(&format!("  - {}\n", value));
        }
    }
    review.push_str(&format!(
        "- {}: {}\n",
        layer.leadership_classification_label, layer.leadership_classification_value
    ));
    review.push_str(&format!("- {}:\n", layer.leadership_metrics_label));
    review.push_str(&format!(
        "  - {}: {}\n",
        layer.primary_label,
        render_json_string_array(&layer.primary_values)
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        layer.supporting_label,
        render_json_string_array(&layer.supporting_values)
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        layer.weakening_label,
        render_json_string_array(&layer.weakening_values)
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        layer.leadership_breadth_label, layer.leadership_breadth_value
    ));
    review.push_str(&format!("- {}:\n", layer.concentration_label));
    review.push_str(&format!(
        "  - {}: {}\n",
        layer.breadth_score_label, layer.breadth_score_value
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        layer.concentration_score_label, layer.concentration_score_value
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        layer.rotation_score_label, layer.rotation_score_value
    ));
    review.push_str(&format!("- {}:\n", layer.rotation_label));
    review.push_str(&format!(
        "  - rotationType: {}\n",
        layer.rotation_type_value
    ));
    review.push_str(&format!(
        "  - from: {}\n",
        render_json_string_array(&layer.rotation_from_values)
    ));
    review.push_str(&format!(
        "  - to: {}\n",
        render_json_string_array(&layer.rotation_to_values)
    ));
    review.push_str(&format!(
        "  - interpretation: {}\n",
        layer.rotation_interpretation_value
    ));
    review.push_str(&format!(
        "  - observationOnly: {}\n",
        layer.observation_only_value
    ));
    review.push_str("- Observation Confidence:\n");
    review.push_str(&format!("  - trend: {}\n", layer.trend_confidence_value));
    review.push_str(&format!("  - macro: {}\n", layer.macro_confidence_value));
    review.push_str(&format!("  - supply: {}\n", layer.supply_confidence_value));
    review.push_str(&format!(
        "  - expectation: {}\n",
        layer.expectation_confidence_value
    ));
    review.push_str(&format!(
        "  - gravity: {}\n",
        layer.gravity_confidence_value
    ));
    review.push_str(&format!("  - flow: {}\n", layer.flow_confidence_value));
    review.push_str(&format!(
        "  - overall: {}\n",
        layer.overall_confidence_value
    ));
    review.push_str("- Interpretation Priority:\n");
    for item in &layer.interpretation_priority_values {
        review.push_str(&format!("  - {}\n", item));
    }
    if let Some(leader_persistence) = leader_persistence {
        review.push_str("- Leader Persistence:\n");
        review.push_str(&format!(
            "  - {}: {}\n",
            leader_persistence.primary_leader_label, leader_persistence.primary_leader_value
        ));
        review.push_str(&format!(
            "  - {}: {}\n",
            leader_persistence.persistence_label, leader_persistence.persistence_value
        ));
        review.push_str(&format!(
            "  - {}: {}\n",
            leader_persistence.leadership_score_label, leader_persistence.leadership_score_value
        ));
        review.push_str(&format!(
            "  - {}: {}\n",
            leader_persistence.leader_state_label, leader_persistence.leader_state_value
        ));
        review.push_str(&format!(
            "  - {}: {}\n",
            leader_persistence.change_from_yesterday_label,
            leader_persistence.change_from_yesterday_value
        ));
        if !leader_persistence.switch_history_values.is_empty() {
            review.push_str(&format!(
                "  - {}:\n",
                leader_persistence.switch_history_label
            ));
            for item in &leader_persistence.switch_history_values {
                review.push_str(&format!("    - {}\n", item));
            }
        }
        review.push_str(&format!("  - {}\n", leader_persistence.boundary));
    }
    review.push_str("- ");
    review.push_str(text.market_interpretation_boundary);
    review.push('\n');
}

fn render_json_string_array(values: &[String]) -> String {
    if values.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", values.join(", "))
    }
}

struct WeeklyText {
    title: &'static str,
    as_of: &'static str,
    status: &'static str,
    status_using_current: &'static str,
    status_data_unavailable: &'static str,
    latest_headline: &'static str,
    days_analyzed: &'static str,
    avg_confidence: &'static str,
    avg_stability: &'static str,
    trend_cohesion_ready_days: &'static str,
    market_state_counts: &'static str,
    risk_overlay_counts: &'static str,
    state_machine_totals: &'static str,
    state_summary_days: &'static str,
    reset_confirmed_blocked: &'static str,
    soft_reset_duration_lock_defensive_override: &'static str,
    core_breakdown_reconciliation_mismatch: &'static str,
    daily_state_timeline: &'static str,
    no_state_machine_summaries: &'static str,
    strategic_context_snapshot: &'static str,
    trend_breadth_mode: &'static str,
    market_cycle_position: &'static str,
    holding_efficiency: &'static str,
    strategic_context_lines: &'static str,
    strategic_context_none: &'static str,
    signal_context_snapshot: &'static str,
    signal_context_not_configured: &'static str,
    signal_context_information_content: &'static str,
    signal_context_primary_context: &'static str,
    signal_context_quality: &'static str,
    signal_context_event_fact: &'static str,
    signal_context_source_diagnostics: &'static str,
    signal_context_interpretation: &'static str,
    signal_context_boundary: &'static str,
    market_interpretation_snapshot: &'static str,
    market_interpretation_not_configured: &'static str,
    market_interpretation_boundary: &'static str,
    macro_gravity_snapshot: &'static str,
    macro_gravity_not_configured: &'static str,
    rate_pressure: &'static str,
    real_yield: &'static str,
    yield_curve: &'static str,
    credit_stress: &'static str,
    liquidity: &'static str,
    growth_valuation: &'static str,
    capital_dynamics_snapshot: &'static str,
    boundary_capital_dynamics: &'static str,
    capital_absorption_ipo_queue_snapshot: &'static str,
    capital_absorption_ipo_queue_not_configured: &'static str,
    capital_absorption_latest_date: &'static str,
    capital_absorption_near_term_latest: &'static str,
    capital_absorption_queue_latest: &'static str,
    capital_absorption_queue_min_max_7d: &'static str,
    capital_absorption_reported_confirmed: &'static str,
    capital_absorption_pressure: &'static str,
    boundary_capital_absorption: &'static str,
    flow_layer_snapshot: &'static str,
    flow_layer_not_configured: &'static str,
    flow_layer_latest_date: &'static str,
    flow_layer_observation_divergence: &'static str,
    flow_layer_positive_negative_divergence: &'static str,
    flow_layer_breadth: &'static str,
    flow_layer_market_breadth: &'static str,
    flow_layer_sector_breadth: &'static str,
    flow_layer_watchlist_breadth: &'static str,
    flow_layer_core_holding_breadth: &'static str,
    boundary_flow_layer: &'static str,
    cognitive_calibration_snapshot: &'static str,
    research_attention_entries: &'static str,
    asset_thesis_entries: &'static str,
    boundary_snapshot_only: &'static str,
    boundary_audit_facts: &'static str,
    boundary_macro: &'static str,
    boundary_macro_not_configured: &'static str,
    boundary_cognitive: &'static str,
    expectation_layer_snapshot: &'static str,
    expectation_layer_as_of: &'static str,
    expectation_layer_decision_weight: &'static str,
    expectation_layer_trade_signal: &'static str,
    expectation_layer_observation_count: &'static str,
    expectation_layer_subjects: &'static str,
    expectation_layer_boundary: &'static str,
}

fn weekly_text(language: Language) -> &'static WeeklyText {
    match language {
        Language::ZhCn => &WEEKLY_TEXT_ZH,
        Language::EnUs => &WEEKLY_TEXT_EN,
        Language::JaJp => &WEEKLY_TEXT_JA,
    }
}

static WEEKLY_TEXT_ZH: WeeklyText = WeeklyText {
    title: "# 周度状态复盘（自动草稿）",
    as_of: "截至",
    status: "状态",
    status_using_current: "使用当前市场判断",
    status_data_unavailable: "数据不可用；仅基于已保存历史",
    latest_headline: "最新摘要",
    days_analyzed: "分析天数",
    avg_confidence: "平均置信度",
    avg_stability: "平均稳定度",
    trend_cohesion_ready_days: "趋势凝聚 ready 天数",
    market_state_counts: "## 市场状态计数",
    risk_overlay_counts: "## 风险覆盖计数",
    state_machine_totals: "## 状态机周度汇总",
    state_summary_days: "有状态摘要的天数",
    reset_confirmed_blocked: "重置确认 / 阻止",
    soft_reset_duration_lock_defensive_override: "软重置 / duration lock / 防御覆盖",
    core_breakdown_reconciliation_mismatch: "核心破坏 / 对账不一致",
    daily_state_timeline: "## 日度状态机时间线",
    no_state_machine_summaries: "没有可用的状态机摘要。",
    strategic_context_snapshot: "## 战略上下文快照",
    trend_breadth_mode: "趋势广度模式",
    market_cycle_position: "市场周期位置",
    holding_efficiency: "持仓效率",
    strategic_context_lines: "战略上下文行",
    strategic_context_none: "无",
    signal_context_snapshot: "## Signal Context（信息质量上下文）",
    signal_context_not_configured: "Signal Context 未配置",
    signal_context_information_content: "Information Content",
    signal_context_primary_context: "Primary Context",
    signal_context_quality: "Context Quality",
    signal_context_event_fact: "Event Fact",
    signal_context_source_diagnostics: "Source Diagnostics",
    signal_context_interpretation: "Interpretation",
    signal_context_boundary:
        "边界: Signal Context 仅作周度追溯沉淀；不接入 Gate、Execution、Trader、READY / EXECUTE 或 Position Sizing。",
    market_interpretation_snapshot: "## Market Interpretation Snapshot",
    market_interpretation_not_configured: "Market interpretation not configured",
    market_interpretation_boundary:
        "Boundary: market interpretation is observation only. Decision weight stays at 0% and it does not enter Gate, Execution, Trader, Action Matrix, Position Sizing, or any decision threshold.",
    macro_gravity_snapshot: "## 宏观引力快照",
    macro_gravity_not_configured: "宏观引力未配置",
    rate_pressure: "利率压力",
    real_yield: "实际收益率",
    yield_curve: "收益率曲线",
    credit_stress: "信用压力",
    liquidity: "流动性",
    growth_valuation: "成长估值",
    capital_dynamics_snapshot: "## Capital Dynamics（供需观察）",
    boundary_capital_dynamics:
        "边界: Capital Dynamics 仅作 Observation shell，Current decision weight 为 0%，不接入 Gate、Execution、Trader、Action Matrix 或 Position Sizing。",
    capital_absorption_ipo_queue_snapshot: "### 6.1 Supply Layer（Capital Absorption）",
    capital_absorption_ipo_queue_not_configured: "资金吸收 IPO 队列未保存",
    capital_absorption_latest_date: "最新观测日",
    capital_absorption_near_term_latest: "最新 Near-Term Supply 数量",
    capital_absorption_queue_latest: "最新 Future Queue 数量",
    capital_absorption_queue_min_max_7d: "7 日 Future Queue 最小值 / 最大值",
    capital_absorption_reported_confirmed: "已报道 / 已确认",
    capital_absorption_pressure: "潜在供给压力",
    boundary_capital_absorption: "边界: 仅为潜在未来供给观察；不生成市场结论、风险升级或交易信号。",
    flow_layer_snapshot: "### 6.2 Demand Layer（Flow Layer）",
    flow_layer_not_configured: "Flow Layer 未配置",
    flow_layer_latest_date: "最新 Flow 观察日",
    flow_layer_observation_divergence: "Observation / Divergence",
    flow_layer_positive_negative_divergence: "正向 / 负向背离",
    flow_layer_breadth: "Flow Breadth",
    flow_layer_market_breadth: "Market Breadth",
    flow_layer_sector_breadth: "Sector Breadth",
    flow_layer_watchlist_breadth: "Watchlist Breadth",
    flow_layer_core_holding_breadth: "Core Holding Breadth",
    boundary_flow_layer: "边界: Flow Layer 仅作 Observation Only 观察，decision weight 固定为 0%，不覆盖 Trend Layer，也不生成交易信号。",
    cognitive_calibration_snapshot: "## 认知校准快照",
    research_attention_entries: "研究关注条目",
    asset_thesis_entries: "资产命题条目",
    boundary_snapshot_only: "边界: 仅为快照；不生成评分、建议或交易判断。",
    boundary_audit_facts: "边界: 仅为审计事实；不生成评分、建议或交易判断。",
    boundary_macro: "边界: 仅说明贴现率与流动性上下文；不作为 Gate 输入或交易指令。",
    boundary_macro_not_configured: "边界: 宏观引力仅解释贴现率与流动性上下文。",
    boundary_cognitive: "边界: 认知校准只管理注意力和命题复核；不生成交易信号。",
    expectation_layer_snapshot: "## Expectation Layer（市场预期观测）",
    expectation_layer_as_of: "观测日",
    expectation_layer_decision_weight: "decision_weight",
    expectation_layer_trade_signal: "trade_signal",
    expectation_layer_observation_count: "observation_count",
    expectation_layer_subjects: "subjects",
    expectation_layer_boundary:
        "边界: Expectation Layer 仅用于观测市场预期，不进入 Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing，也不生成交易信号。",
};

static WEEKLY_TEXT_EN: WeeklyText = WeeklyText {
    title: "# Weekly State Review (Auto)",
    as_of: "As of",
    status: "Status",
    status_using_current: "using current market decision",
    status_data_unavailable: "data unavailable; based on prior persisted history only",
    latest_headline: "Latest headline",
    days_analyzed: "Days analyzed",
    avg_confidence: "Avg confidence",
    avg_stability: "Avg stability",
    trend_cohesion_ready_days: "Trend cohesion ready days",
    market_state_counts: "## Market State Counts",
    risk_overlay_counts: "## Risk Overlay Counts",
    state_machine_totals: "## State Machine Weekly Totals",
    state_summary_days: "Days with state summary",
    reset_confirmed_blocked: "Reset confirmed / blocked",
    soft_reset_duration_lock_defensive_override: "Soft reset / duration lock / defensive override",
    core_breakdown_reconciliation_mismatch: "Core breakdown / reconciliation mismatch",
    daily_state_timeline: "## Daily State Machine Timeline",
    no_state_machine_summaries: "No state machine summaries available.",
    strategic_context_snapshot: "## Strategic Context Snapshot",
    trend_breadth_mode: "Trend breadth mode",
    market_cycle_position: "Market cycle position",
    holding_efficiency: "Holding efficiency",
    strategic_context_lines: "Strategic context lines",
    strategic_context_none: "none",
    signal_context_snapshot: "## Signal Context (Information Quality Context)",
    signal_context_not_configured: "Signal Context not configured",
    signal_context_information_content: "Information Content",
    signal_context_primary_context: "Primary Context",
    signal_context_quality: "Context Quality",
    signal_context_event_fact: "Event Fact",
    signal_context_source_diagnostics: "Source Diagnostics",
    signal_context_interpretation: "Interpretation",
    signal_context_boundary:
        "Boundary: Signal Context is kept only for weekly traceability. It does not connect to Gate, Execution, Trader, READY / EXECUTE, or Position Sizing.",
    market_interpretation_snapshot: "## Market Interpretation Snapshot",
    market_interpretation_not_configured: "Market interpretation not configured",
    market_interpretation_boundary:
        "Boundary: market interpretation is observation only. Decision weight stays at 0% and it does not enter Gate, Execution, Trader, Action Matrix, Position Sizing, or any decision threshold.",
    macro_gravity_snapshot: "## Macro Gravity Snapshot",
    macro_gravity_not_configured: "Macro gravity: not configured",
    rate_pressure: "Rate pressure",
    real_yield: "Real yield",
    yield_curve: "Yield curve",
    credit_stress: "Credit stress",
    liquidity: "Liquidity",
    growth_valuation: "Growth valuation",
    capital_dynamics_snapshot: "## Capital Dynamics (Supply / Demand Observation)",
    boundary_capital_dynamics:
        "Boundary: Capital Dynamics is an observation shell only. Current decision weight remains 0%, and it does not connect to Gate, Execution, Trader, Action Matrix, or Position Sizing.",
    capital_absorption_ipo_queue_snapshot: "### 6.1 Supply Layer (Capital Absorption)",
    capital_absorption_ipo_queue_not_configured: "Capital absorption IPO queue: not persisted",
    capital_absorption_latest_date: "Latest observation date",
    capital_absorption_near_term_latest: "Latest Near-Term Supply Count",
    capital_absorption_queue_latest: "Latest Future Queue Count",
    capital_absorption_queue_min_max_7d: "7D Future Queue min / max",
    capital_absorption_reported_confirmed: "Reported / Confirmed",
    capital_absorption_pressure: "Potential supply pressure",
    boundary_capital_absorption:
        "Boundary: potential future supply observation only; no market conclusion, risk upgrade, or trade signal.",
    flow_layer_snapshot: "### 6.2 Demand Layer (Flow Layer)",
    flow_layer_not_configured: "Flow Layer: not configured",
    flow_layer_latest_date: "Latest Flow observation date",
    flow_layer_observation_divergence: "Observations / Divergences",
    flow_layer_positive_negative_divergence: "Positive / Negative divergence",
    flow_layer_breadth: "Flow Breadth",
    flow_layer_market_breadth: "Market Breadth",
    flow_layer_sector_breadth: "Sector Breadth",
    flow_layer_watchlist_breadth: "Watchlist Breadth",
    flow_layer_core_holding_breadth: "Core Holding Breadth",
    boundary_flow_layer:
        "Boundary: Flow Layer is Observation Only. Decision weight remains 0%, it does not override Trend Layer, and it does not generate trade signals.",
    cognitive_calibration_snapshot: "## Cognitive Calibration Snapshot",
    research_attention_entries: "Research attention entries",
    asset_thesis_entries: "Asset thesis entries",
    boundary_snapshot_only: "Boundary: snapshot only; no score, advice, or trade decision.",
    boundary_audit_facts: "Boundary: audit facts only; no score, advice, or trade decision.",
    boundary_macro: "Boundary: context only; no Gate input or trade instruction.",
    boundary_macro_not_configured: "Boundary: macro gravity explains discount-rate and liquidity context only.",
    boundary_cognitive: "Boundary: cognitive calibration manages attention and thesis review only; it does not generate trade signals.",
    expectation_layer_snapshot: "## Expectation Layer (Market Expectation Observation)",
    expectation_layer_as_of: "As of",
    expectation_layer_decision_weight: "decision_weight",
    expectation_layer_trade_signal: "trade_signal",
    expectation_layer_observation_count: "observation_count",
    expectation_layer_subjects: "subjects",
    expectation_layer_boundary:
        "Boundary: Expectation Layer is for observing market expectations only. It does not enter Gate, Execution, Trader, Action Matrix, READY / EXECUTE, or Position Sizing, and it does not generate trade signals.",
};

static WEEKLY_TEXT_JA: WeeklyText = WeeklyText {
    title: "# 週次状態レビュー（自動下書き）",
    as_of: "基準日",
    status: "状態",
    status_using_current: "現在の市場判断を使用",
    status_data_unavailable: "データ利用不可。保存済み履歴のみを使用",
    latest_headline: "最新ヘッドライン",
    days_analyzed: "分析日数",
    avg_confidence: "平均確信度",
    avg_stability: "平均安定度",
    trend_cohesion_ready_days: "トレンド凝集 ready 日数",
    market_state_counts: "## 市場状態カウント",
    risk_overlay_counts: "## リスクオーバーレイカウント",
    state_machine_totals: "## 状態機械の週次集計",
    state_summary_days: "状態サマリーがある日数",
    reset_confirmed_blocked: "リセット確認 / ブロック",
    soft_reset_duration_lock_defensive_override: "ソフトリセット / duration lock / 防御 override",
    core_breakdown_reconciliation_mismatch: "core breakdown / reconciliation mismatch",
    daily_state_timeline: "## 日次状態機械タイムライン",
    no_state_machine_summaries: "利用可能な状態機械サマリーはありません。",
    strategic_context_snapshot: "## 戦略コンテキストスナップショット",
    trend_breadth_mode: "トレンド幅モード",
    market_cycle_position: "市場サイクル位置",
    holding_efficiency: "保有効率",
    strategic_context_lines: "戦略コンテキスト行",
    strategic_context_none: "なし",
    signal_context_snapshot: "## Signal Context（情報品質コンテキスト）",
    signal_context_not_configured: "Signal Context は未設定",
    signal_context_information_content: "Information Content",
    signal_context_primary_context: "Primary Context",
    signal_context_quality: "Context Quality",
    signal_context_event_fact: "Event Fact",
    signal_context_source_diagnostics: "Source Diagnostics",
    signal_context_interpretation: "Interpretation",
    signal_context_boundary:
        "境界: Signal Context は週次の追跡可能な蓄積のみを担当し、Gate、Execution、Trader、READY / EXECUTE、Position Sizing へ接続しない。",
    market_interpretation_snapshot: "## Market Interpretation Snapshot",
    market_interpretation_not_configured: "Market interpretation not configured",
    market_interpretation_boundary:
        "境界: market interpretation は観測専用であり、Decision weight は 0% に固定され、Gate、Execution、Trader、Action Matrix、Position Sizing、いかなる decision threshold にも入らない。",
    macro_gravity_snapshot: "## マクログラビティスナップショット",
    macro_gravity_not_configured: "マクログラビティ未設定",
    rate_pressure: "金利圧力",
    real_yield: "実質利回り",
    yield_curve: "イールドカーブ",
    credit_stress: "信用ストレス",
    liquidity: "流動性",
    growth_valuation: "成長評価",
    capital_dynamics_snapshot: "## Capital Dynamics（需給観測）",
    boundary_capital_dynamics:
        "境界: Capital Dynamics は Observation shell のみであり、Current decision weight は 0% に固定され、Gate、Execution、Trader、Action Matrix、Position Sizing へ接続しない。",
    capital_absorption_ipo_queue_snapshot: "### 6.1 Supply Layer（Capital Absorption）",
    capital_absorption_ipo_queue_not_configured: "資金吸収 IPO キューは未保存",
    capital_absorption_latest_date: "最新観測日",
    capital_absorption_near_term_latest: "最新 Near-Term Supply 数",
    capital_absorption_queue_latest: "最新 Future Queue 数",
    capital_absorption_queue_min_max_7d: "7 日 Future Queue 最小 / 最大",
    capital_absorption_reported_confirmed: "報道済み / 確認済み",
    capital_absorption_pressure: "潜在供給圧力",
    boundary_capital_absorption:
        "境界: 潜在的な将来供給の観測のみ。市場結論、リスク格上げ、取引信号は生成しない。",
    flow_layer_snapshot: "### 6.2 Demand Layer（Flow Layer）",
    flow_layer_not_configured: "Flow Layer は未設定",
    flow_layer_latest_date: "最新 Flow 観測日",
    flow_layer_observation_divergence: "Observation / Divergence",
    flow_layer_positive_negative_divergence: "正 / 負 divergence",
    flow_layer_breadth: "Flow Breadth",
    flow_layer_market_breadth: "Market Breadth",
    flow_layer_sector_breadth: "Sector Breadth",
    flow_layer_watchlist_breadth: "Watchlist Breadth",
    flow_layer_core_holding_breadth: "Core Holding Breadth",
    boundary_flow_layer:
        "境界: Flow Layer は Observation Only の観測であり、decision weight は 0% に固定され、Trend Layer を override せず、取引信号を生成しない。",
    cognitive_calibration_snapshot: "## 認知校正スナップショット",
    research_attention_entries: "Research attention 件数",
    asset_thesis_entries: "Asset thesis 件数",
    boundary_snapshot_only: "境界: スナップショットのみ。スコア、助言、取引判断は生成しない。",
    boundary_audit_facts: "境界: 監査事実のみ。スコア、助言、取引判断は生成しない。",
    boundary_macro: "境界: コンテキストのみ。Gate 入力や取引指示ではない。",
    boundary_macro_not_configured:
        "境界: マクログラビティは割引率と流動性コンテキストだけを説明する。",
    boundary_cognitive: "境界: 認知校正は注意力と命題レビューだけを扱い、取引信号を生成しない。",
    expectation_layer_snapshot: "## Expectation Layer（市場期待観測）",
    expectation_layer_as_of: "観測日",
    expectation_layer_decision_weight: "decision_weight",
    expectation_layer_trade_signal: "trade_signal",
    expectation_layer_observation_count: "observation_count",
    expectation_layer_subjects: "subjects",
    expectation_layer_boundary:
        "境界: Expectation Layer は市場期待の観測専用であり、Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing に入らず、売買シグナルも生成しない。",
};

fn push_weekly_strategic_context_snapshot(
    review: &mut String,
    pres_packet: &crate::features::radar::interface::presentation::PresentationPacket,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.strategic_context_snapshot);
    review.push('\n');
    if let Some(evidence) = pres_packet.transition_evidence.as_ref() {
        review.push_str(&format!(
            "- {}: {:?}\n",
            text.trend_breadth_mode, evidence.trend_breadth_mode
        ));
        review.push_str(&format!(
            "- {}: {:?}\n",
            text.market_cycle_position, evidence.market_cycle_position
        ));
        review.push_str(&format!(
            "- {}: {:?}\n",
            text.holding_efficiency, evidence.holding_efficiency
        ));
        if evidence.strategic_context.is_empty() {
            review.push_str(&format!(
                "- {}: {}\n",
                text.strategic_context_lines, text.strategic_context_none
            ));
        } else {
            review.push_str(&format!("- {}:\n", text.strategic_context_lines));
            for line in &evidence.strategic_context {
                review.push_str(&format!("  - {}\n", line));
            }
        }
    } else {
        review.push_str(&format!("- {}: N/A\n", text.trend_breadth_mode));
        review.push_str(&format!("- {}: N/A\n", text.market_cycle_position));
        review.push_str(&format!("- {}: N/A\n", text.holding_efficiency));
        review.push_str(&format!(
            "- {}: {}\n",
            text.strategic_context_lines, text.strategic_context_none
        ));
    }
    review.push_str("- ");
    review.push_str(text.boundary_snapshot_only);
    review.push('\n');
}

fn push_weekly_macro_gravity_snapshot(
    review: &mut String,
    context: &WeeklyReportContext,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.macro_gravity_snapshot);
    review.push('\n');
    let Some(macro_gravity) = context.macro_gravity.as_ref() else {
        review.push_str(&format!("- {}\n", text.macro_gravity_not_configured));
        review.push_str("- ");
        review.push_str(text.boundary_macro_not_configured);
        review.push('\n');
        return;
    };

    review.push_str(&format!(
        "- {}: {}\n",
        text.rate_pressure, macro_gravity.rate_pressure
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.real_yield, macro_gravity.real_yield_pressure
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.yield_curve, macro_gravity.yield_curve
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.credit_stress, macro_gravity.credit_stress
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.liquidity, macro_gravity.liquidity
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.growth_valuation, macro_gravity.growth_valuation_impact
    ));
    review.push_str("- ");
    review.push_str(text.boundary_macro);
    review.push('\n');
}

fn push_weekly_capital_absorption_ipo_queue_snapshot(
    review: &mut String,
    summary: &serde_json::Value,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.capital_absorption_ipo_queue_snapshot);
    review.push('\n');
    if !summary["configured"].as_bool().unwrap_or(false) {
        review.push_str(&format!(
            "- {}\n",
            text.capital_absorption_ipo_queue_not_configured
        ));
        review.push_str("- ");
        review.push_str(text.boundary_capital_absorption);
        review.push('\n');
        return;
    }
    review.push_str(&format!(
        "- {}: {}\n",
        text.capital_absorption_latest_date,
        summary["latest_date"].as_str().unwrap_or("unknown")
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.capital_absorption_near_term_latest,
        summary["near_term_supply_count_latest"]
            .as_u64()
            .unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.capital_absorption_queue_latest,
        summary["future_queue_count_latest"]
            .as_u64()
            .unwrap_or_else(|| summary["queue_count_latest"].as_u64().unwrap_or(0))
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.capital_absorption_queue_min_max_7d,
        summary["queue_count_min_7d"].as_u64().unwrap_or(0),
        summary["queue_count_max_7d"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.capital_absorption_reported_confirmed,
        summary["reported_count_latest"].as_u64().unwrap_or(0),
        summary["confirmed_count_latest"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.capital_absorption_pressure,
        summary["pressure_latest"].as_str().unwrap_or("unknown")
    ));
    review.push_str("- ");
    review.push_str(text.boundary_capital_absorption);
    review.push('\n');
}

fn push_weekly_capital_dynamics_snapshot(
    review: &mut String,
    supply_summary: &serde_json::Value,
    flow_summary: &serde_json::Value,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.capital_dynamics_snapshot);
    review.push('\n');
    review.push_str("- ");
    review.push_str(text.boundary_capital_dynamics);
    review.push('\n');
    push_weekly_capital_absorption_ipo_queue_snapshot(review, supply_summary, text);
    push_weekly_flow_layer_snapshot(review, flow_summary, text);
}

fn push_weekly_flow_layer_snapshot(
    review: &mut String,
    summary: &serde_json::Value,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.flow_layer_snapshot);
    review.push('\n');
    if !summary["configured"].as_bool().unwrap_or(false) {
        review.push_str(&format!("- {}\n", text.flow_layer_not_configured));
        review.push_str("- ");
        review.push_str(text.boundary_flow_layer);
        review.push('\n');
        return;
    }
    review.push_str(&format!(
        "- {}: {}\n",
        text.flow_layer_latest_date,
        summary["as_of_date"].as_str().unwrap_or("unknown")
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.flow_layer_observation_divergence,
        summary["observation_count"].as_u64().unwrap_or(0),
        summary["divergence_count"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.flow_layer_positive_negative_divergence,
        summary["positive_divergence_count"].as_u64().unwrap_or(0),
        summary["negative_divergence_count"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!("- {}:\n", text.flow_layer_breadth));
    review.push_str(&format!(
        "  - {}: {}\n",
        text.flow_layer_market_breadth,
        summary["breadth"]["market_breadth"]
            .as_str()
            .unwrap_or("UNKNOWN")
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        text.flow_layer_sector_breadth,
        summary["breadth"]["sector_breadth"]
            .as_str()
            .unwrap_or("UNKNOWN")
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        text.flow_layer_watchlist_breadth,
        summary["breadth"]["watchlist_breadth"]
            .as_str()
            .unwrap_or("UNKNOWN")
    ));
    review.push_str(&format!(
        "  - {}: {}\n",
        text.flow_layer_core_holding_breadth,
        summary["breadth"]["core_holding_breadth"]
            .as_str()
            .unwrap_or("UNKNOWN")
    ));
    review.push_str("- ");
    review.push_str(text.boundary_flow_layer);
    review.push('\n');
}

fn push_weekly_state_machine_totals(
    review: &mut String,
    totals: &serde_json::Value,
    daily_summaries: &serde_json::Value,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.state_machine_totals);
    review.push('\n');
    review.push_str(&format!(
        "- {}: {}\n",
        text.state_summary_days,
        totals["days"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.reset_confirmed_blocked,
        totals["reset_confirmed_total"].as_u64().unwrap_or(0),
        totals["reset_blocked_total"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {} / {} / {}\n",
        text.soft_reset_duration_lock_defensive_override,
        totals["soft_reset_total"].as_u64().unwrap_or(0),
        totals["duration_lock_total"].as_u64().unwrap_or(0),
        totals["defensive_override_total"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {} / {}\n",
        text.core_breakdown_reconciliation_mismatch,
        totals["core_breakdown_total"].as_u64().unwrap_or(0),
        totals["reconciliation_mismatch_total"]
            .as_u64()
            .unwrap_or(0)
    ));

    review.push('\n');
    review.push_str(text.daily_state_timeline);
    review.push('\n');
    if let Some(items) = daily_summaries.as_array() {
        if items.is_empty() {
            review.push_str(&format!("- {}\n", text.no_state_machine_summaries));
        }
        for item in items {
            review.push_str(&format!(
                "- {}: {} -> {} | reset C/B {} / {} | soft_reset {} | duration_lock {} | defensive_override {} | mismatch {}\n",
                item["date"].as_str().unwrap_or("unknown"),
                item["from_state"].as_str().unwrap_or("unknown"),
                item["to_state"].as_str().unwrap_or("unknown"),
                item["reset_confirmed"].as_bool().unwrap_or(false),
                item["reset_blocked"].as_bool().unwrap_or(false),
                item["soft_reset_applied"].as_bool().unwrap_or(false),
                item["duration_locked"].as_bool().unwrap_or(false),
                item["defensive_override"].as_bool().unwrap_or(false),
                item["reconciliation_mismatch_count"].as_u64().unwrap_or(0)
            ));
        }
    }
    review.push_str("- ");
    review.push_str(text.boundary_audit_facts);
    review.push('\n');
}

fn push_weekly_cognitive_calibration_snapshot(
    review: &mut String,
    context: &WeeklyReportContext,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.cognitive_calibration_snapshot);
    review.push('\n');
    review.push_str(&format!(
        "- {}: {}\n",
        text.research_attention_entries, context.research_attention_entries
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.asset_thesis_entries, context.asset_thesis_entries
    ));
    review.push_str("- ");
    review.push_str(text.boundary_cognitive);
    review.push('\n');
}

fn push_weekly_expectation_snapshot(
    review: &mut String,
    summary: &serde_json::Value,
    text: &WeeklyText,
) {
    review.push('\n');
    review.push_str(text.expectation_layer_snapshot);
    review.push('\n');
    if !summary["configured"].as_bool().unwrap_or(false) {
        review.push_str("- expectation layer not configured\n");
        review.push_str("- ");
        review.push_str(text.expectation_layer_boundary);
        review.push('\n');
        return;
    }

    review.push_str(&format!(
        "- {}: {}\n",
        text.expectation_layer_as_of,
        summary["as_of_date"].as_str().unwrap_or("unknown")
    ));
    review.push_str(&format!(
        "- {}: {}%\n",
        text.expectation_layer_decision_weight,
        summary["decision_weight_percent"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.expectation_layer_trade_signal,
        summary["trade_signal"].as_bool().unwrap_or(false)
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.expectation_layer_observation_count,
        summary["observation_count"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- {}: {}\n",
        text.expectation_layer_subjects,
        summary["subjects"]
            .as_array()
            .map(|subjects| {
                subjects
                    .iter()
                    .filter_map(|value| value.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "unknown".to_string())
    ));
    review.push_str("- ");
    review.push_str(text.expectation_layer_boundary);
    review.push('\n');
}

#[cfg(test)]
mod tests {
    use super::{
        build_weekly_latest_context, load_weekly_state_machine_summaries,
        persist_weekly_state_outputs, push_weekly_capital_absorption_ipo_queue_snapshot,
        push_weekly_capital_dynamics_snapshot, push_weekly_expectation_snapshot,
        push_weekly_flow_layer_snapshot, push_weekly_market_interpretation_snapshot,
        push_weekly_signal_context_snapshot, weekly_text, WeeklyReportContext,
    };
    use crate::features::radar::interface::display::{
        RiskOpportunityViewModel, TopActionViewModel,
    };
    use crate::features::radar::interface::presentation::{
        ExitDecisionItemViewModel, ExitDecisionSummaryViewModel, ExitDisplayIntent,
        InterpretationLayerViewModel, PresentationPacket,
    };
    use crate::features::shared::application::run_status::StateMachineSummary;
    use crate::features::shared::interface::i18n::Language;
    use chrono::NaiveDate;
    use tempfile::tempdir;

    fn write_run_status(
        save_dir: &std::path::Path,
        date: &str,
        state: &str,
        reset_confirmed: bool,
    ) {
        let value = serde_json::json!({
            "state_machine": {
                "from_state": "PREVIOUS",
                "to_state": state,
                "reset_confirmed": reset_confirmed,
                "reset_blocked": false,
                "soft_reset_applied": false,
                "duration_locked": false,
                "defensive_override": false,
                "core_breakdown": false,
                "reconciliation_mismatch_count": 0,
                "preflight_failed": false
            }
        });
        std::fs::write(
            save_dir.join(format!("run_status_{date}.json")),
            serde_json::to_string(&value).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn weekly_state_machine_summaries_ignore_future_run_status_files() {
        let tmp = tempdir().unwrap();
        write_run_status(tmp.path(), "2026-06-07", "VALID_HISTORY", true);
        write_run_status(tmp.path(), "2026-06-10", "FUTURE_SHOULD_NOT_APPEAR", true);
        let current = StateMachineSummary {
            from_state: "VALID_HISTORY".to_string(),
            to_state: "CURRENT".to_string(),
            ..Default::default()
        };

        let entries = load_weekly_state_machine_summaries(
            tmp.path(),
            NaiveDate::from_ymd_opt(2026, 6, 9).unwrap(),
            Some(&current),
        );

        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|entry| entry.summary.to_state == "CURRENT"));
        assert!(entries
            .iter()
            .any(|entry| entry.summary.to_state == "VALID_HISTORY"));
        assert!(!entries
            .iter()
            .any(|entry| entry.summary.to_state == "FUTURE_SHOULD_NOT_APPEAR"));
    }

    #[test]
    fn weekly_review_text_uses_configured_language_labels() {
        assert_eq!(
            weekly_text(Language::ZhCn).title,
            "# 周度状态复盘（自动草稿）"
        );
        assert_eq!(
            weekly_text(Language::EnUs).title,
            "# Weekly State Review (Auto)"
        );
        assert_eq!(
            weekly_text(Language::JaJp).title,
            "# 週次状態レビュー（自動下書き）"
        );
        assert!(weekly_text(Language::ZhCn)
            .boundary_cognitive
            .contains("不生成交易信号"));
        assert!(weekly_text(Language::JaJp)
            .boundary_cognitive
            .contains("取引信号を生成しない"));
        assert!(weekly_text(Language::ZhCn)
            .expectation_layer_boundary
            .contains("不进入 Gate"));
        assert!(weekly_text(Language::JaJp)
            .expectation_layer_boundary
            .contains("売買シグナルも生成しない"));
        assert!(!weekly_text(Language::ZhCn)
            .capital_absorption_ipo_queue_snapshot
            .contains("Capital Absorption IPO Queue Snapshot"));
        assert!(!weekly_text(Language::ZhCn)
            .capital_absorption_queue_min_max_7d
            .contains("min / max"));
        assert!(!weekly_text(Language::ZhCn)
            .capital_absorption_reported_confirmed
            .contains("Reported / Confirmed"));
        assert!(!weekly_text(Language::JaJp)
            .capital_absorption_ipo_queue_snapshot
            .contains("Capital Absorption IPO Queue Snapshot"));
        assert!(!weekly_text(Language::JaJp)
            .capital_absorption_queue_min_max_7d
            .contains("min / max"));
        assert!(!weekly_text(Language::JaJp)
            .capital_absorption_reported_confirmed
            .contains("Reported / Confirmed"));
    }

    #[test]
    fn weekly_capital_absorption_labels_do_not_leak_english_in_zh_or_ja() {
        let blocked = [
            "Capital Absorption IPO Queue",
            "min / max",
            "Reported / Confirmed",
        ];
        let localized_labels = [
            weekly_text(Language::ZhCn).capital_absorption_ipo_queue_snapshot,
            weekly_text(Language::ZhCn).capital_absorption_queue_min_max_7d,
            weekly_text(Language::ZhCn).capital_absorption_reported_confirmed,
            weekly_text(Language::JaJp).capital_absorption_ipo_queue_snapshot,
            weekly_text(Language::JaJp).capital_absorption_queue_min_max_7d,
            weekly_text(Language::JaJp).capital_absorption_reported_confirmed,
        ];

        for label in localized_labels {
            for blocked_label in blocked {
                assert!(!label.contains(blocked_label));
            }
        }
    }

    #[test]
    fn weekly_capital_absorption_review_section_keeps_observation_boundary() {
        let summary = serde_json::json!({
            "configured": true,
            "latest_date": "2026-06-08",
            "near_term_supply_count_latest": 1,
            "future_queue_count_latest": 3,
            "queue_count_latest": 3,
            "queue_count_min_7d": 1,
            "queue_count_max_7d": 3,
            "reported_count_latest": 2,
            "confirmed_count_latest": 1,
            "pressure_latest": "ELEVATED"
        });
        let mut review = String::new();

        push_weekly_capital_absorption_ipo_queue_snapshot(
            &mut review,
            &summary,
            weekly_text(Language::ZhCn),
        );

        assert!(review.contains("### 6.1 Supply Layer（Capital Absorption）"));
        assert!(review.contains("最新 Near-Term Supply 数量: 1"));
        assert!(review.contains("最新 Future Queue 数量: 3"));
        assert!(review.contains("7 日 Future Queue 最小值 / 最大值: 1 / 3"));
        assert!(review.contains("已报道 / 已确认: 2 / 1"));
        assert!(review.contains("潜在供给压力: ELEVATED"));
        assert!(review.contains("不生成市场结论、风险升级或交易信号"));
        assert!(!review.contains("Capital Absorption IPO Queue"));
        assert!(!review.contains("min / max"));
        assert!(!review.contains("Reported / Confirmed"));
        assert!(!review.contains("READY"));
        assert!(!review.contains("EXECUTE"));
    }

    #[test]
    fn weekly_flow_layer_review_section_keeps_observation_only_boundary() {
        let summary = serde_json::json!({
            "configured": true,
            "as_of_date": "2026-06-08",
            "observation_count": 2,
            "divergence_count": 1,
            "positive_divergence_count": 0,
            "negative_divergence_count": 1,
            "breadth": {
                "market_breadth": "UNAVAILABLE",
                "sector_breadth": "DIVERGENT",
                "watchlist_breadth": "SUPPORTIVE",
                "core_holding_breadth": "NEUTRAL"
            }
        });
        let mut review = String::new();

        push_weekly_flow_layer_snapshot(&mut review, &summary, weekly_text(Language::ZhCn));

        assert!(review.contains("### 6.2 Demand Layer（Flow Layer）"));
        assert!(review.contains("最新 Flow 观察日: 2026-06-08"));
        assert!(review.contains("Observation / Divergence: 2 / 1"));
        assert!(review.contains("正向 / 负向背离: 0 / 1"));
        assert!(review.contains("Market Breadth: UNAVAILABLE"));
        assert!(review.contains("Watchlist Breadth: SUPPORTIVE"));
        assert!(review.contains("decision weight 固定为 0%"));
        assert!(!review.contains("READY"));
        assert!(!review.contains("EXECUTE"));
    }

    #[test]
    fn weekly_capital_dynamics_review_shell_wraps_supply_and_flow() {
        let supply = serde_json::json!({
            "configured": true,
            "latest_date": "2026-06-08",
            "near_term_supply_count_latest": 1,
            "future_queue_count_latest": 3,
            "queue_count_latest": 3,
            "queue_count_min_7d": 1,
            "queue_count_max_7d": 3,
            "reported_count_latest": 2,
            "confirmed_count_latest": 1,
            "pressure_latest": "ELEVATED"
        });
        let flow = serde_json::json!({
            "configured": true,
            "as_of_date": "2026-06-08",
            "observation_count": 2,
            "divergence_count": 1,
            "positive_divergence_count": 0,
            "negative_divergence_count": 1,
            "breadth": {
                "market_breadth": "UNAVAILABLE",
                "sector_breadth": "DIVERGENT",
                "watchlist_breadth": "SUPPORTIVE",
                "core_holding_breadth": "NEUTRAL"
            }
        });
        let mut review = String::new();

        push_weekly_capital_dynamics_snapshot(
            &mut review,
            &supply,
            &flow,
            weekly_text(Language::ZhCn),
        );

        assert!(review.contains("## Capital Dynamics（供需观察）"));
        assert!(review.contains("Current decision weight 为 0%"));
        assert!(review.contains("### 6.1 Supply Layer（Capital Absorption）"));
        assert!(review.contains("### 6.2 Demand Layer（Flow Layer）"));
        assert!(review.contains("不接入 Gate、Execution、Trader、Action Matrix 或 Position Sizing"));
    }

    #[test]
    fn weekly_expectation_review_section_keeps_read_only_boundary() {
        let summary = serde_json::json!({
            "configured": true,
            "as_of_date": "2026-06-18",
            "decision_weight_percent": 0,
            "trade_signal": false,
            "gate_effect": "none",
            "execution_effect": "none",
            "position_sizing_effect": "none",
            "observation_count": 1,
            "subjects": ["TSLA"]
        });
        let mut review = String::new();

        push_weekly_expectation_snapshot(&mut review, &summary, weekly_text(Language::ZhCn));

        assert!(review.contains("## Expectation Layer（市场预期观测）"));
        assert!(review.contains("decision_weight: 0%"));
        assert!(review.contains("trade_signal: false"));
        assert!(review.contains("observation_count: 1"));
        assert!(review.contains("subjects: TSLA"));
        assert!(review.contains("不进入 Gate、Execution、Trader、Action Matrix"));
        assert!(!review.contains("BUY"));
        assert!(!review.contains("SELL"));
    }

    #[test]
    fn weekly_latest_context_keeps_supply_layer_and_legacy_alias_in_sync() {
        let supply = serde_json::json!({
            "configured": true,
            "latest_date": "2026-06-08",
            "near_term_supply_count_latest": 1
        });
        let flow = serde_json::json!({
            "configured": true,
            "as_of_date": "2026-06-08",
            "observation_count": 2
        });
        let expectation = serde_json::json!({
            "configured": true,
            "as_of_date": "2026-06-18",
            "decision_weight_percent": 0,
            "trade_signal": false,
            "gate_effect": "none",
            "execution_effect": "none",
            "position_sizing_effect": "none",
            "observation_count": 1,
            "subjects": ["TSLA"]
        });
        let interpretation_layer = InterpretationLayerViewModel {
            signal_context_information_content_value: "HIGH".to_string(),
            signal_context_primary_context_value: "Macro Event".to_string(),
            signal_context_quality_value: "HIGH".to_string(),
            signal_context_event_fact_value: "CPI at 08:30 ET".to_string(),
            signal_context_source_diagnostics_value:
                "Official Calendar coverage 1/1; unavailable 0; health SUCCEEDED.".to_string(),
            signal_context_interpretation_value: "Market is repricing new macro information."
                .to_string(),
            ..Default::default()
        };
        let packet = PresentationPacket {
            top_actions: vec![
                TopActionViewModel {
                    symbol: "SPY".to_string(),
                    ..Default::default()
                },
                TopActionViewModel {
                    symbol: "GOOG".to_string(),
                    ..Default::default()
                },
                TopActionViewModel {
                    symbol: "U".to_string(),
                    ..Default::default()
                },
            ],
            exit_summary: ExitDecisionSummaryViewModel {
                items: vec![ExitDecisionItemViewModel {
                    symbol: "NVDA".to_string(),
                    intent: ExitDisplayIntent::Trim,
                    ..Default::default()
                }],
                ..Default::default()
            },
            risk_opportunities: vec![RiskOpportunityViewModel {
                kind: "RISK".to_string(),
                symbol: "PLTR".to_string(),
                reason: "rotation".to_string(),
            }],
            interpretation_layer: Some(interpretation_layer.clone()),
            leader_persistence: Some(
                crate::features::radar::interface::presentation::LeaderPersistenceViewModel {
                    title: "Leader Persistence".to_string(),
                    primary_leader_label: "Primary Leader".to_string(),
                    primary_leader_value: "SPY".to_string(),
                    persistence_label: "Leader Persistence".to_string(),
                    persistence_value: "3 days".to_string(),
                    persistence_days: 3,
                    observed_days_label: "Observed Leadership Days in Lookback".to_string(),
                    observed_days_value: "3 days".to_string(),
                    breakout_continuity_label: "Breakout Continuity".to_string(),
                    breakout_continuity_value: "3 days".to_string(),
                    history_coverage_label: "History Coverage".to_string(),
                    history_coverage_value: "PARTIAL".to_string(),
                    history_note: Some("Leadership history unavailable before feature activation.".to_string()),
                    leadership_score_label: "Leadership Score".to_string(),
                    leadership_score_value: "64.2".to_string(),
                    leadership_score: 64.2,
                    leader_state_label: "Leader State".to_string(),
                    leader_state_value: "EARLY".to_string(),
                    change_from_yesterday_label: "Change from Yesterday".to_string(),
                    change_from_yesterday_value: "+1 day, score stable".to_string(),
                    persistence_change_days: 1,
                    score_change: 0.0,
                    switch_history_label: "Switch History".to_string(),
                    switch_history_values: vec!["2026-06-17: QQQ -> SPY".to_string()],
                    boundary: "Boundary: observation only; this block does not change Decision, Gate, Execution, Trader, or Position Sizing.".to_string(),
                },
            ),
            market_interpretation: {
                let weekly_market_packet = PresentationPacket {
                    top_actions: vec![
                        TopActionViewModel {
                            symbol: "SPY".to_string(),
                            ..Default::default()
                        },
                        TopActionViewModel {
                            symbol: "GOOG".to_string(),
                            ..Default::default()
                        },
                        TopActionViewModel {
                            symbol: "U".to_string(),
                            ..Default::default()
                        },
                    ],
                    exit_summary: ExitDecisionSummaryViewModel {
                        items: vec![ExitDecisionItemViewModel {
                            symbol: "NVDA".to_string(),
                            intent: ExitDisplayIntent::Trim,
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    risk_opportunities: vec![RiskOpportunityViewModel {
                        kind: "RISK".to_string(),
                        symbol: "PLTR".to_string(),
                        reason: "rotation".to_string(),
                    }],
                    interpretation_layer: Some(interpretation_layer.clone()),
                    ..Default::default()
                };
                let weekly_leadership_snapshot = crate::features::radar::interface::market_interpretation_read_model::build_leadership_snapshot_view_model(
                    &weekly_market_packet,
                    Language::ZhCn,
                );
                crate::features::radar::interface::market_interpretation_read_model::build_market_interpretation_view_model(
                    &crate::features::radar::domain::decision::DecisionPacket::default(),
                    &weekly_market_packet,
                    &weekly_leadership_snapshot,
                    Language::ZhCn,
                )
            },
            ..Default::default()
        };
        let latest = build_weekly_latest_context(
            &packet,
            &WeeklyReportContext {
                macro_gravity: None,
                research_attention_entries: 0,
                asset_thesis_entries: 0,
                capital_absorption_ipo_queue: supply.clone(),
                capital_dynamics_flow_layer: flow.clone(),
                expectation_layer: expectation.clone(),
            },
            &supply,
            &flow,
            &expectation,
        );

        assert_eq!(latest["capital_dynamics"]["supply_layer"], supply);
        assert_eq!(latest["capital_absorption_ipo_queue"], supply);
        assert_eq!(
            latest["capital_dynamics"]["supply_layer"],
            latest["capital_absorption_ipo_queue"]
        );
        assert_eq!(latest["capital_dynamics"]["flow_layer"], flow);
        assert_eq!(latest["expectation_layer"], expectation);
        assert_eq!(latest["signal_context"]["configured"], true);
        assert_eq!(latest["signal_context"]["information_content"], "HIGH");
        assert_eq!(latest["signal_context"]["primary_context"], "Macro Event");
        assert_eq!(latest["signal_context"]["context_quality"], "HIGH");
        assert_eq!(latest["signal_context"]["event_fact"], "CPI at 08:30 ET");
        assert_eq!(
            latest["signal_context"]["source_diagnostics"],
            "Official Calendar coverage 1/1; unavailable 0; health SUCCEEDED."
        );
        assert_eq!(
            latest["signal_context"]["interpretation"],
            "Market is repricing new macro information."
        );
        assert_eq!(latest["market_interpretation"]["configured"], true);
        assert_eq!(latest["market_interpretation"]["day_type"], "exceptional");
        assert_eq!(
            latest["market_interpretation"]["rotation"]["type"],
            "macro_repricing"
        );
        assert_eq!(
            latest["market_interpretation"]["leadership"]["primary"],
            serde_json::json!(["SPY"])
        );
        assert_eq!(latest["leader_persistence"]["configured"], true);
        assert_eq!(latest["leader_persistence"]["primary_leader"], "SPY");
        assert_eq!(latest["leader_persistence"]["persistence_days"], 3);
        assert_eq!(latest["leader_persistence"]["leadership_score"], 64.2);
    }

    #[test]
    fn weekly_signal_context_review_section_keeps_traceable_boundary() {
        let interpretation_layer = InterpretationLayerViewModel {
            signal_context_information_content_value: "LOW".to_string(),
            signal_context_primary_context_value: "Pre-Earnings Waiting".to_string(),
            signal_context_quality_value: "MEDIUM".to_string(),
            signal_context_event_fact_value: "TSLA / 2026-06-27 / EarningsConsensus pending"
                .to_string(),
            signal_context_source_diagnostics_value:
                "Expectation lifecycle source health: Succeeded".to_string(),
            signal_context_interpretation_value:
                "Market is waiting for new fundamental information.".to_string(),
            ..Default::default()
        };
        let mut review = String::new();

        push_weekly_signal_context_snapshot(
            &mut review,
            Some(&interpretation_layer),
            weekly_text(Language::ZhCn),
        );

        assert!(review.contains("## Signal Context（信息质量上下文）"));
        assert!(review.contains("Information Content: LOW"));
        assert!(review.contains("Primary Context: Pre-Earnings Waiting"));
        assert!(review.contains("Context Quality: MEDIUM"));
        assert!(review.contains("Event Fact: TSLA / 2026-06-27 / EarningsConsensus pending"));
        assert!(
            review.contains("Source Diagnostics: Expectation lifecycle source health: Succeeded")
        );
        assert!(
            review.contains("Interpretation: Market is waiting for new fundamental information.")
        );
        assert!(review.contains("仅作周度追溯沉淀"));
        assert!(!review.contains("BUY"));
        assert!(!review.contains("SELL"));
    }

    #[test]
    fn weekly_market_interpretation_review_section_keeps_observation_boundary() {
        let packet = crate::features::radar::domain::decision::DecisionPacket::default();
        let interpretation_layer = InterpretationLayerViewModel {
            signal_context_information_content_value: "LOW".to_string(),
            signal_context_primary_context_value: "Index Reconstitution".to_string(),
            signal_context_quality_value: "MEDIUM".to_string(),
            signal_context_event_fact_value: "reconstitution".to_string(),
            signal_context_source_diagnostics_value: "calendar".to_string(),
            signal_context_interpretation_value: "index driven".to_string(),
            trend_confidence_value: "HIGH".to_string(),
            supply_confidence_value: "MEDIUM".to_string(),
            expectation_confidence_value: "NONE".to_string(),
            gravity_confidence_value: "NONE".to_string(),
            flow_confidence_value: "LOW".to_string(),
            interpretation_quality_value: "MEDIUM".to_string(),
            ..Default::default()
        };
        let weekly_market_packet = PresentationPacket {
            top_actions: vec![
                TopActionViewModel {
                    symbol: "SPY".to_string(),
                    ..Default::default()
                },
                TopActionViewModel {
                    symbol: "QQQ".to_string(),
                    ..Default::default()
                },
            ],
            exit_summary: ExitDecisionSummaryViewModel::default(),
            risk_opportunities: vec![],
            interpretation_layer: Some(interpretation_layer),
            ..Default::default()
        };
        let weekly_leadership_snapshot = crate::features::radar::interface::market_interpretation_read_model::build_leadership_snapshot_view_model(
            &weekly_market_packet,
            Language::EnUs,
        );
        let market_interpretation = crate::features::radar::interface::market_interpretation_read_model::build_market_interpretation_view_model(
            &packet,
            &weekly_market_packet,
            &weekly_leadership_snapshot,
            Language::EnUs,
        )
        .unwrap();
        let mut review = String::new();

        push_weekly_market_interpretation_snapshot(
            &mut review,
            Some(&market_interpretation),
            None,
            weekly_text(Language::EnUs),
        );

        assert!(review.contains("## Market Interpretation Snapshot"));
        assert!(review.contains("decision_weight: 0%"));
        assert!(review.contains("dayType: exceptional"));
        assert!(review.contains("rotationType: index_rotation"));
        assert!(review.contains("observationOnly: true"));
        assert!(review.contains("Boundary: market interpretation is observation only"));
        assert!(!review.contains("BUY"));
        assert!(!review.contains("SELL"));
    }

    #[test]
    fn weekly_state_metrics_keep_trend_cohesion_alias_in_sync() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().to_path_buf();
        let packet_date = NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
        let market_features = crate::features::radar::domain::features::MarketFeatures {
            system_confidence: 0.8,
            stability_score: 0.7,
            ..Default::default()
        };
        let market_regime = crate::features::radar::domain::market_regime::MarketRegimeSnapshot {
            market_state: crate::features::radar::domain::market_regime::MarketState::NEWBORN,
            lifecycle_state: crate::features::radar::domain::market_regime::LifecycleState::NEWBORN,
            risk_overlay: crate::features::radar::domain::market_regime::RiskOverlay::NORMAL,
            reasons: vec![],
            low_stability_streak: 0,
            duration_in_state: 1,
            transition_audit: None,
        };
        let packet = crate::features::radar::domain::decision::DecisionPacket::new(
            packet_date,
            market_features,
            market_regime,
            None,
            crate::features::radar::domain::portfolio_policy::PortfolioPolicy::default(),
            vec![],
            Vec::new(),
            false,
            crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                gate_passed: true,
                continuity_streak: 3,
                ..Default::default()
            },
            None,
            None,
        );
        let pres_packet = crate::features::radar::interface::presentation::PresentationPacket {
            date_str: "2026-06-18".to_string(),
            language: Language::ZhCn,
            ..Default::default()
        };
        let context = WeeklyReportContext {
            macro_gravity: None,
            research_attention_entries: 0,
            asset_thesis_entries: 0,
            capital_absorption_ipo_queue: serde_json::json!({
                "configured": false
            }),
            capital_dynamics_flow_layer: serde_json::json!({
                "configured": false
            }),
            expectation_layer: serde_json::json!({
                "configured": false
            }),
        };

        persist_weekly_state_outputs(&save_dir, &[], &packet, true, &pres_packet, &context, None)
            .unwrap();

        let metrics: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(save_dir.join("weekly_state_metrics.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(metrics["trend_cohesion_ready_days"], 1);
        assert_eq!(metrics["participation_ready_days"], 1);
        assert_eq!(
            metrics["trend_cohesion_ready_days"],
            metrics["participation_ready_days"]
        );
    }
}

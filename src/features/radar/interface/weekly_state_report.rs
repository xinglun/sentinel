use anyhow::Result;
use chrono::NaiveDate;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Clone)]
pub(crate) struct WeeklyReportContext {
    pub macro_gravity: Option<WeeklyMacroGravityContext>,
    pub research_attention_entries: usize,
    pub asset_thesis_entries: usize,
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
    let latest_context = build_weekly_latest_context(pres_packet, context);
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
        // semantic shift warning: 'participation_ready_days' は現在 'trend_cohesion_ready_days' を出力する。
        // この key を読む downstream script は、従来の participation semantics ではなく cohesion gate semantics を受け取る。
        // script failure を避けるため、後方互換性のためだけにこの key を維持する。
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

    let mut review = String::new();
    review.push_str("# Weekly State Review (Auto)\n\n");
    review.push_str(&format!("- As of: {}\n", pres_packet.date_str));
    review.push_str(&format!(
        "- Status: {}\n",
        if include_current_packet {
            "using current market decision"
        } else {
            "data unavailable; based on prior persisted history only"
        }
    ));
    review.push_str(&format!(
        "- Latest headline: {} | {}\n",
        pres_packet.macro_display.headline, pres_packet.macro_display.bias_label
    ));
    review.push_str(&format!("- Days analyzed: {}\n", day_count));
    review.push_str(&format!("- Avg confidence: {:.1}\n", avg_confidence));
    review.push_str(&format!("- Avg stability: {:.1}\n", avg_stability));
    review.push_str(&format!(
        "- Trend cohesion ready days: {}\n\n",
        trend_cohesion_ready_days
    ));
    review.push_str("## Market State Counts\n");
    for (state, count) in metrics["market_state_counts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        review.push_str(&format!("- {}: {}\n", state, count));
    }
    review.push_str("\n## Risk Overlay Counts\n");
    for (state, count) in metrics["risk_overlay_counts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        review.push_str(&format!("- {}: {}\n", state, count));
    }
    push_weekly_state_machine_totals(&mut review, &weekly_totals, &metrics["daily_summaries"]);
    push_weekly_strategic_context_snapshot(&mut review, pres_packet);
    push_weekly_macro_gravity_snapshot(&mut review, context);
    push_weekly_cognitive_calibration_snapshot(&mut review, context);

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

    json!({
        "trend_breadth_mode": trend_breadth_mode,
        "market_cycle_position": market_cycle_position,
        "holding_efficiency": holding_efficiency,
        "strategic_context": strategic_context,
        "macro_gravity": build_weekly_macro_gravity_context(context),
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

fn push_weekly_strategic_context_snapshot(
    review: &mut String,
    pres_packet: &crate::features::radar::interface::presentation::PresentationPacket,
) {
    review.push_str("\n## Strategic Context Snapshot\n");
    if let Some(evidence) = pres_packet.transition_evidence.as_ref() {
        review.push_str(&format!(
            "- Trend breadth mode: {:?}\n",
            evidence.trend_breadth_mode
        ));
        review.push_str(&format!(
            "- Market cycle position: {:?}\n",
            evidence.market_cycle_position
        ));
        review.push_str(&format!(
            "- Holding efficiency: {:?}\n",
            evidence.holding_efficiency
        ));
        if evidence.strategic_context.is_empty() {
            review.push_str("- Strategic context lines: none\n");
        } else {
            review.push_str("- Strategic context lines:\n");
            for line in &evidence.strategic_context {
                review.push_str(&format!("  - {}\n", line));
            }
        }
    } else {
        review.push_str("- Trend breadth mode: N/A\n");
        review.push_str("- Market cycle position: N/A\n");
        review.push_str("- Holding efficiency: N/A\n");
        review.push_str("- Strategic context lines: none\n");
    }
    review.push_str("- Boundary: snapshot only; no score, advice, or trade decision.\n");
}

fn push_weekly_macro_gravity_snapshot(review: &mut String, context: &WeeklyReportContext) {
    review.push_str("\n## Macro Gravity Snapshot\n");
    let Some(macro_gravity) = context.macro_gravity.as_ref() else {
        review.push_str("- Macro gravity: not configured\n");
        review.push_str(
            "- Boundary: macro gravity explains discount-rate and liquidity context only.\n",
        );
        return;
    };

    review.push_str(&format!(
        "- Rate pressure: {}\n",
        macro_gravity.rate_pressure
    ));
    review.push_str(&format!(
        "- Real yield: {}\n",
        macro_gravity.real_yield_pressure
    ));
    review.push_str(&format!("- Yield curve: {}\n", macro_gravity.yield_curve));
    review.push_str(&format!(
        "- Credit stress: {}\n",
        macro_gravity.credit_stress
    ));
    review.push_str(&format!("- Liquidity: {}\n", macro_gravity.liquidity));
    review.push_str(&format!(
        "- Growth valuation: {}\n",
        macro_gravity.growth_valuation_impact
    ));
    review.push_str("- Boundary: context only; no Gate input or trade instruction.\n");
}

fn push_weekly_state_machine_totals(
    review: &mut String,
    totals: &serde_json::Value,
    daily_summaries: &serde_json::Value,
) {
    review.push_str("\n## State Machine Weekly Totals\n");
    review.push_str(&format!(
        "- Days with state summary: {}\n",
        totals["days"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- Reset confirmed / blocked: {} / {}\n",
        totals["reset_confirmed_total"].as_u64().unwrap_or(0),
        totals["reset_blocked_total"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- Soft reset / duration lock / defensive override: {} / {} / {}\n",
        totals["soft_reset_total"].as_u64().unwrap_or(0),
        totals["duration_lock_total"].as_u64().unwrap_or(0),
        totals["defensive_override_total"].as_u64().unwrap_or(0)
    ));
    review.push_str(&format!(
        "- Core breakdown / reconciliation mismatch: {} / {}\n",
        totals["core_breakdown_total"].as_u64().unwrap_or(0),
        totals["reconciliation_mismatch_total"]
            .as_u64()
            .unwrap_or(0)
    ));

    review.push_str("\n## Daily State Machine Timeline\n");
    if let Some(items) = daily_summaries.as_array() {
        if items.is_empty() {
            review.push_str("- No state machine summaries available.\n");
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
    review.push_str("- Boundary: audit facts only; no score, advice, or trade decision.\n");
}

fn push_weekly_cognitive_calibration_snapshot(review: &mut String, context: &WeeklyReportContext) {
    review.push_str("\n## Cognitive Calibration Snapshot\n");
    review.push_str(&format!(
        "- Research attention entries: {}\n",
        context.research_attention_entries
    ));
    review.push_str(&format!(
        "- Asset thesis entries: {}\n",
        context.asset_thesis_entries
    ));
    review.push_str(
        "- Boundary: cognitive calibration manages attention and thesis review only; it does not generate trade signals.\n",
    );
}

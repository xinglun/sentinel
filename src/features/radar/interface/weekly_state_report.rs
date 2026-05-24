use anyhow::Result;
use serde_json::json;

use crate::config;
use crate::features::research::interface::cognitive_reports::{
    credit_stress_label, enabled_asset_thesis_count, enabled_research_attention_count,
    growth_valuation_impact_label, liquidity_condition_label, macro_pressure_label,
    yield_curve_label,
};

pub(crate) fn persist_weekly_state_outputs(
    save_dir: &std::path::Path,
    history: &[crate::features::radar::application::policy::decision::DecisionPacket],
    current_packet: &crate::features::radar::application::policy::decision::DecisionPacket,
    include_current_packet: bool,
    pres_packet: &crate::features::radar::interface::presentation::PresentationPacket,
    app_config: &config::AppConfig,
) -> Result<()> {
    let mut recent_packets: Vec<
        &crate::features::radar::application::policy::decision::DecisionPacket,
    > = history.iter().rev().take(7).collect();
    recent_packets.reverse();
    if include_current_packet {
        recent_packets.push(current_packet);
    }
    if recent_packets.len() > 7 {
        recent_packets = recent_packets[recent_packets.len() - 7..].to_vec();
    }

    let mut market_state_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut risk_overlay_counts = std::collections::BTreeMap::<String, usize>::new();
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
    let latest_context = build_weekly_latest_context(pres_packet, app_config);

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
    push_weekly_strategic_context_snapshot(&mut review, pres_packet);
    push_weekly_macro_gravity_snapshot(&mut review, app_config);
    push_weekly_cognitive_calibration_snapshot(&mut review, app_config);

    std::fs::write(save_dir.join("weekly_state_review_auto.md"), review)?;
    Ok(())
}

fn build_weekly_latest_context(
    pres_packet: &crate::features::radar::interface::presentation::PresentationPacket,
    app_config: &config::AppConfig,
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
        "macro_gravity": build_weekly_macro_gravity_context(app_config),
        "cognitive_calibration": {
            "research_attention_entries": enabled_research_attention_count(app_config),
            "asset_thesis_entries": enabled_asset_thesis_count(app_config)
        }
    })
}

fn build_weekly_macro_gravity_context(app_config: &config::AppConfig) -> serde_json::Value {
    let Some(macro_gravity) = app_config
        .macro_gravity
        .as_ref()
        .filter(|macro_gravity| macro_gravity.enable.unwrap_or(true))
    else {
        return json!({
            "configured": false
        });
    };

    json!({
        "configured": true,
        "rate_pressure": macro_pressure_label(macro_gravity.rate_pressure),
        "real_yield_pressure": macro_pressure_label(macro_gravity.real_yield_pressure),
        "yield_curve": yield_curve_label(macro_gravity.yield_curve),
        "credit_stress": credit_stress_label(macro_gravity.credit_stress),
        "liquidity": liquidity_condition_label(macro_gravity.liquidity),
        "growth_valuation_impact": growth_valuation_impact_label(macro_gravity.growth_valuation_impact)
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

fn push_weekly_macro_gravity_snapshot(review: &mut String, app_config: &config::AppConfig) {
    review.push_str("\n## Macro Gravity Snapshot\n");
    let Some(macro_gravity) = app_config
        .macro_gravity
        .as_ref()
        .filter(|macro_gravity| macro_gravity.enable.unwrap_or(true))
    else {
        review.push_str("- Macro gravity: not configured\n");
        review.push_str(
            "- Boundary: macro gravity explains discount-rate and liquidity context only.\n",
        );
        return;
    };

    review.push_str(&format!(
        "- Rate pressure: {}\n",
        macro_pressure_label(macro_gravity.rate_pressure)
    ));
    review.push_str(&format!(
        "- Real yield: {}\n",
        macro_pressure_label(macro_gravity.real_yield_pressure)
    ));
    review.push_str(&format!(
        "- Yield curve: {}\n",
        yield_curve_label(macro_gravity.yield_curve)
    ));
    review.push_str(&format!(
        "- Credit stress: {}\n",
        credit_stress_label(macro_gravity.credit_stress)
    ));
    review.push_str(&format!(
        "- Liquidity: {}\n",
        liquidity_condition_label(macro_gravity.liquidity)
    ));
    review.push_str(&format!(
        "- Growth valuation: {}\n",
        growth_valuation_impact_label(macro_gravity.growth_valuation_impact)
    ));
    review.push_str("- Boundary: context only; no Gate input or trade instruction.\n");
}

fn push_weekly_cognitive_calibration_snapshot(review: &mut String, app_config: &config::AppConfig) {
    review.push_str("\n## Cognitive Calibration Snapshot\n");
    review.push_str(&format!(
        "- Research attention entries: {}\n",
        enabled_research_attention_count(app_config)
    ));
    review.push_str(&format!(
        "- Asset thesis entries: {}\n",
        enabled_asset_thesis_count(app_config)
    ));
    review.push_str(
        "- Boundary: cognitive calibration manages attention and thesis review only; it does not generate trade signals.\n",
    );
}

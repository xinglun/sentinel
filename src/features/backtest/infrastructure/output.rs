use crate::features::backtest::application::model::{
    BacktestDecisionClass, BacktestSimulationReport,
};
use crate::features::backtest::domain::metrics::StateMachineMetrics;
use anyhow::Result;
use std::fs;

pub fn write_run_artifacts(report: &BacktestSimulationReport) -> Result<()> {
    let summary_markdown = render_summary_markdown(report);
    let state_machine_metrics_markdown = render_state_machine_metrics_markdown(&report.metrics);
    let state_machine_metrics_json = serde_json::to_string_pretty(&report.metrics)?;
    let validation_json = serde_json::to_string_pretty(&serde_json::json!({
        "decision_window": {
            "start": report.window.decision_start,
            "end": report.window.decision_end,
        },
        "outcome_window": {
            "start": report.window.decision_start,
            "end": report.window.outcome_end,
        },
        "validation": report.validation,
    }))?;

    let dir_name = report.name.as_str();
    let base_dir = format!("backtest/{}", dir_name);
    fs::create_dir_all(&base_dir)?;
    fs::write(format!("{}/summary.md", base_dir), summary_markdown)?;
    fs::write(
        format!("{}/state_machine_metrics.md", base_dir),
        state_machine_metrics_markdown,
    )?;
    fs::write(
        format!("{}/state_machine_metrics.json", base_dir),
        state_machine_metrics_json,
    )?;
    fs::write(format!("{}/validation.json", base_dir), validation_json)?;
    Ok(())
}

fn render_summary_markdown(report: &BacktestSimulationReport) -> String {
    let metrics = &report.metrics;
    let days = metrics.total_days as f64;
    let breakout_status_total = metrics.evaluated_asset_days.max(1) as f64;
    let breakout_eligible_total = metrics.breakout_eligible_asset_days.max(1) as f64;
    let breakout_failed_rate =
        (metrics.breakout_failed_risk_count as f64 / breakout_eligible_total) * 100.0;

    let mut summary = String::new();
    summary.push_str(&format!("# 🔭 Backtest Summary ({})\n\n", report.name));
    summary.push_str(&format!(
        "## Validation Window\n\n- Decision window: {} → {}\n- Outcome window: {} → {}\n\n",
        format_window_date(report.window.decision_start),
        format_window_date(report.window.decision_end),
        format_window_date(report.window.decision_start),
        format_window_date(report.window.outcome_end),
    ));
    summary.push_str("## 1. Reliability Calibration\n| Bucket | Total | Correct | Win Rate |\n|---|---|---|---|\n");
    for bucket in &report.reliability {
        summary.push_str(&format!(
            "| {} | {} | {} | {:.1}% |\n",
            bucket.bucket,
            bucket.total,
            bucket.correct,
            (bucket.correct as f64 / bucket.total as f64) * 100.0
        ));
    }
    summary.push_str("\n## 2. Regime Performance Audit\n| State | Signals | Hit Rate | Avg 20d | Max DD |\n|---|---|---|---|---|\n");
    for regime in &report.regime_audit {
        summary.push_str(&format!(
            "| {} | {} | {:.1}% | {:+.2}% | {:+.2}% |\n",
            regime.state,
            regime.total_signals,
            (regime.correct_signals as f64 / regime.total_signals as f64) * 100.0,
            regime.average_20d_return * 100.0,
            regime.max_drawdown_20d * 100.0
        ));
    }

    summary.push_str("\n## 3. Gate / Topology / Breakout Distribution\n");
    summary.push_str(&format!(
        "- **Trend Gate Blocked Days**: {} / {} ({:.1}%)\n",
        metrics.trend_gate_blocked_days,
        metrics.total_days,
        (metrics.trend_gate_blocked_days as f64 / days.max(1.0)) * 100.0
    ));
    summary.push_str(&format!(
        "- **Trend Status**: Dispersed={} | Forming={} | Formed={}\n",
        metrics.trend_status_dispersed_days,
        metrics.trend_status_forming_days,
        metrics.trend_status_formed_days
    ));
    summary.push_str(&format!(
        "- **Topology**: NoLeader={} | SingleLeader={} | FragmentedLeaders={}\n",
        metrics.topology_no_leader_days,
        metrics.topology_single_leader_days,
        metrics.topology_fragmented_leaders_days
    ));
    summary.push_str(&format!(
        "- **Evaluated Asset-Days**: {} | Breakout-Eligible Asset-Days={} ({:.1}% of evaluated)\n",
        metrics.evaluated_asset_days,
        metrics.breakout_eligible_asset_days,
        (metrics.breakout_eligible_asset_days as f64 / breakout_status_total) * 100.0
    ));
    summary.push_str(&format!(
        "- **Breakout Status Counts**: NoBreakout={} | Emerging={} | Confirmed={}\n",
        metrics.breakout_no_breakout_count,
        metrics.breakout_emerging_count,
        metrics.breakout_confirmed_count
    ));
    summary.push_str(&format!(
        "- **Failed Breakout Risk Flags**: {} ({:.1}% of breakout-eligible asset-days)\n",
        metrics.breakout_failed_risk_count, breakout_failed_rate
    ));
    render_validation_summary(&mut summary, report);
    summary
}

fn format_window_date(date: Option<chrono::NaiveDate>) -> String {
    date.map(|value| value.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn render_validation_summary(summary: &mut String, report: &BacktestSimulationReport) {
    summary.push_str("\n## 4. Decision Validation\n\n");
    if report.validation.invalid_context_record_count > 0 {
        summary.push_str(&format!(
            "- **Invalid decision context records excluded**: {}\n",
            report.validation.invalid_context_record_count
        ));
    }
    if report.validation.cohorts.is_empty() {
        render_validation_cohort(
            summary,
            &crate::features::backtest::application::model::ValidationCohortReport {
                decision_snapshot_version: "LEGACY_OR_UNCOHORTED".to_string(),
                universe_id: "UNAVAILABLE".to_string(),
                outcomes: report.validation.outcomes.clone(),
                baseline: report.validation.baseline.clone(),
                sample_maturity: report.validation.sample_maturity.clone(),
                ..Default::default()
            },
        );
    } else {
        for cohort in &report.validation.cohorts {
            render_validation_cohort(summary, cohort);
        }
    }
    summary.push_str("\n### Net Decision Value (摘要)\n\nProtection Benefit と Opportunity Cost の構成値を cohort ごとに分離表示し、単一スコアによる有効性断定は行わない。\n");
}

fn render_validation_cohort(
    summary: &mut String,
    cohort: &crate::features::backtest::application::model::ValidationCohortReport,
) {
    summary.push_str(&format!(
        "### Cohort: {} / {}\n\n- **Sample Maturity**: {}\n- **Protection Maturity**: {}\n- **Confirmation Maturity**: {}\n",
        cohort.decision_snapshot_version,
        cohort.universe_id,
        cohort.sample_maturity,
        cohort.protection_sample_maturity,
        cohort.confirmation_sample_maturity
    ));
    let population = &cohort.population;
    summary.push_str(&format!(
        "\n#### Protection Population Audit\n\n- Classified records: {}\n- Gate blocked records: {}\n- Raw Top-3 candidate records: {}\n- Raw Top-3 × Gate blocked records: {}\n- Raw Top-3 × Gate blocked × NO_TRADE records: {}\n- Gate blocked non-candidate records: {}\n- Gate blocked non-candidate reasons (records may have multiple reasons): ",
        population.classified_record_count,
        population.gate_blocked_record_count,
        population.raw_candidate_record_count,
        population.raw_candidate_gate_blocked_record_count,
        population.raw_candidate_gate_blocked_no_trade_record_count,
        population.gate_blocked_non_candidate_record_count
    ));
    if population.gate_blocked_non_candidate_reasons.is_empty() {
        summary.push_str("N/A");
    } else {
        for (index, reason) in population
            .gate_blocked_non_candidate_reasons
            .iter()
            .enumerate()
        {
            if index > 0 {
                summary.push_str(", ");
            }
            summary.push_str(&format!("{}={}", reason.reason, reason.count));
        }
    }
    summary.push('\n');
    summary.push_str(
        "\n#### Coverage\n\n| Decision | Samples | T+5 | T+10 | T+20 |\n|---|---:|---:|---:|---:|\n",
    );
    for outcome in &cohort.outcomes {
        let name = outcome
            .decision_class
            .map(decision_class_code)
            .unwrap_or("UNKNOWN")
            .to_string();
        summary.push_str(&format!(
            "| {} | {} | {:.1}% | {:.1}% | {:.1}% |\n",
            name,
            outcome.sample_count,
            coverage(outcome.complete_5d, outcome.sample_count),
            coverage(outcome.complete_10d, outcome.sample_count),
            coverage(outcome.complete_20d, outcome.sample_count)
        ));
    }

    summary.push_str("\n#### Outcome Facts\n\n| Decision | Avg 5d | Avg 10d | Avg 20d | Avg MFE 20d | Avg MAE 20d | Median MAE 20d | P95 MAE 20d | Positive 20d |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for outcome in &cohort.outcomes {
        let name = outcome
            .decision_class
            .map(decision_class_code)
            .unwrap_or("UNKNOWN")
            .to_string();
        summary.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            name,
            percent(outcome.average_5d_return),
            percent(outcome.average_10d_return),
            percent(outcome.average_20d_return),
            percent(outcome.average_mfe_20d),
            percent(outcome.average_mae_20d),
            percent(outcome.median_mae_20d),
            percent(outcome.p95_mae_20d),
            outcome.positive_20d_count
        ));
    }

    let utility = &cohort.utility;
    summary.push_str(&format!(
        "\n#### Trend Gate Protection / Opportunity Cost\n\n- Raw Top-3 NO_TRADE Trend Gate blocked candidates: {} (T+20 complete: {})\n- Avg / Median / P90 / P95 MAE 20d: {} / {} / {} / {}\n- Blocked downside samples: {}\n- Missed upside samples: {}\n- Avg MFE 20d: {}\n- Avg positive 20d return: {}\n- Top-decile missed upside: {}\n- Horizon utility T+5/T+10/T+20 complete: {}/{}/{}\n- Horizon utility T+5/T+10/T+20 downside: {}/{}/{}\n- Horizon utility Avg MAE T+5/T+10/T+20: {}/{}/{}\n- Horizon utility Avg MFE T+5/T+10/T+20: {}/{}/{}\n",
        utility.blocked_candidate_count,
        utility.complete_20d_count,
        percent(utility.average_mae_20d),
        percent(utility.median_mae_20d),
        percent(utility.p90_mae_20d),
        percent(utility.p95_mae_20d),
        utility.downside_20d_count,
        utility.missed_upside_count,
        percent(utility.average_mfe_20d),
        percent(utility.average_positive_20d_return),
        percent(utility.top_decile_missed_upside),
        utility.horizon_5d.complete_sample_count,
        utility.horizon_10d.complete_sample_count,
        utility.horizon_20d.complete_sample_count,
        utility.horizon_5d.downside_count,
        utility.horizon_10d.downside_count,
        utility.horizon_20d.downside_count,
        percent(utility.horizon_5d.average_mae),
        percent(utility.horizon_10d.average_mae),
        percent(utility.horizon_20d.average_mae),
        percent(utility.horizon_5d.average_mfe),
        percent(utility.horizon_10d.average_mfe),
        percent(utility.horizon_20d.average_mfe)
    ));

    summary.push_str(
        "- Reason breakdown (complete/downside by horizon; reasons may overlap; counts are not additive): ",
    );
    for (index, reason) in utility.reason_breakdown.iter().enumerate() {
        if index > 0 {
            summary.push_str(", ");
        }
        summary.push_str(&format!(
            "{} T+5 complete/downside={}/{}; T+10 complete/downside={}/{}; T+20 complete/downside={}/{}",
            reason.reason,
            reason.horizon_5d.complete_sample_count,
            reason.horizon_5d.downside_count,
            reason.horizon_10d.complete_sample_count,
            reason.horizon_10d.downside_count,
            reason.horizon_20d.complete_sample_count,
            reason.horizon_20d.downside_count
        ));
    }
    summary.push('\n');

    let cost = &cohort.confirmation_cost;
    summary.push_str(&format!(
        "\n#### Confirmation Cost\n\n- Episodes: {} (lifecycle complete: {})\n- Raw Top-3 strength proxy → Breakout: {} sessions\n- Breakout → Ready: {} sessions\n- Raw Top-3 strength proxy → Ready: {} sessions\n- Raw Top-3 strength proxy → Ready signed return (raw fact): {}\n- Return lost before Ready (positive waiting-upside only): {}\n- Breakout → Ready return: {}\n- Raw Top-3 strength proxy → Ready maximum move: {}\n",
        cost.episode_sample_count,
        cost.lifecycle_complete_episode_count,
        scalar(cost.average_strength_to_breakout_sessions),
        scalar(cost.average_breakout_to_ready_sessions),
        scalar(cost.average_strength_to_ready_sessions),
        percent(cost.average_return_strength_to_ready),
        percent(cost.average_return_lost_before_ready),
        percent(cost.average_return_breakout_to_ready),
        percent(cost.average_max_move_strength_to_ready)
    ));

    let net = &cohort.net_decision_value;
    summary.push_str(&format!(
        "\n#### Net Decision Value (Trend Gate-only, same-episode paired facts)\n\n- Eligible episodes: {}\n- T+5: benefit {} / cost {} / adverse waiting return {} (adverse waiting samples: {}) / net {} (paired: {}, unpaired: {})\n- T+10: benefit {} / cost {} / adverse waiting return {} (adverse waiting samples: {}) / net {} (paired: {}, unpaired: {})\n- T+20: benefit {} / cost {} / adverse waiting return {} (adverse waiting samples: {}) / net {} (paired: {}, unpaired: {})\n",
        net.eligible_episode_count,
        percent(net.horizon_5d.protection_benefit),
        percent(net.horizon_5d.confirmation_cost),
        percent(net.horizon_5d.adverse_waiting_return),
        net.horizon_5d.adverse_waiting_sample_count,
        percent(net.horizon_5d.net_value),
        net.horizon_5d.paired_episode_count,
        net.horizon_5d.unpaired_episode_count,
        percent(net.horizon_10d.protection_benefit),
        percent(net.horizon_10d.confirmation_cost),
        percent(net.horizon_10d.adverse_waiting_return),
        net.horizon_10d.adverse_waiting_sample_count,
        percent(net.horizon_10d.net_value),
        net.horizon_10d.paired_episode_count,
        net.horizon_10d.unpaired_episode_count,
        percent(net.horizon_20d.protection_benefit),
        percent(net.horizon_20d.confirmation_cost),
        percent(net.horizon_20d.adverse_waiting_return),
        net.horizon_20d.adverse_waiting_sample_count,
        percent(net.horizon_20d.net_value),
        net.horizon_20d.paired_episode_count,
        net.horizon_20d.unpaired_episode_count
    ));

    let baseline = &cohort.baseline;
    summary.push_str(&format!(
        "\n#### Counterfactual Baseline\n\n| Cohort | Samples | Avg 20d Return | Avg MFE 20d | Avg MAE 20d |\n|---|---:|---:|---:|---:|\n| Raw Top-3 without Gate | {} | {} | {} | {} |\n| Sentinel READY subset | {} | {} | {} | {} |\n\n- Return sacrifice / improvement (READY - Raw): {}\n- Downside improvement (READY - Raw MAE): {}\n- MFE difference (READY - Raw): {}\n",
        baseline.raw_top3_sample_count,
        percent(baseline.raw_top3_average_20d_return),
        percent(baseline.raw_top3_average_20d_mfe),
        percent(baseline.raw_top3_average_mae_20d),
        baseline.ready_sample_count,
        percent(baseline.ready_average_20d_return),
        percent(baseline.ready_average_20d_mfe),
        percent(baseline.ready_average_mae_20d),
        percent(baseline.return_difference),
        percent(baseline.mae_difference),
        percent(baseline.mfe_difference)
    ));
}

fn coverage(complete: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        complete as f64 / total as f64 * 100.0
    }
}

fn percent(value: Option<f64>) -> String {
    value
        .map(|value| format!("{:+.2}%", value * 100.0))
        .unwrap_or_else(|| "N/A".to_string())
}

fn scalar(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn decision_class_code(class: BacktestDecisionClass) -> &'static str {
    match class {
        BacktestDecisionClass::NoTrade => "NO_TRADE",
        BacktestDecisionClass::Probe => "PROBE",
        BacktestDecisionClass::Ready => "READY",
    }
}

fn render_state_machine_metrics_markdown(metrics: &StateMachineMetrics) -> String {
    let days = metrics.total_days as f64;
    let breakout_status_total = metrics.evaluated_asset_days.max(1) as f64;
    let breakout_eligible_total = metrics.breakout_eligible_asset_days.max(1) as f64;
    let breakout_failed_rate =
        (metrics.breakout_failed_risk_count as f64 / breakout_eligible_total) * 100.0;

    let mut sm_md = String::new();
    sm_md.push_str(
        "# 🧭 State Machine Quality Metrics\n\n| Metric | Value | Rate |\n|---|---|---|\n",
    );
    sm_md.push_str(&format!(
        "| Reset | {} | {:.1}% |\n",
        metrics.reset_count,
        (metrics.reset_count as f64 / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Blocked Reset | {} | {:.1}% |\n",
        metrics.blocked_reset_count,
        (metrics.blocked_reset_count as f64 / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Duration Locked | {} | {:.1}% |\n",
        metrics.duration_lock_count,
        (metrics.duration_lock_count as f64 / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Soft Reset | {} | {:.1}% |\n",
        metrics.soft_reset_count,
        (metrics.soft_reset_count as f64 / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Flips (5d) | {} | {:.1}% |\n",
        metrics.state_flip_count_5d,
        (metrics.state_flip_count_5d as f64 / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Top Actions Turnover | - | {:.1}% |\n",
        (metrics.top_actions_turnover_sum / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Core Asset Protection Hits | {} | - |\n",
        metrics.core_asset_protection_hits
    ));
    sm_md.push_str(&format!(
        "| Weak Asset Promotion Cap Hits | {} | - |\n",
        metrics.weak_asset_promotion_cap_hits
    ));
    sm_md.push_str(&format!(
        "| Raw-vs-Actual Optimal Divergence (Days) | {} | - |\n",
        metrics.total_raw_vs_actual_divergence_days
    ));
    sm_md.push_str(&format!(
        "| Raw Optimal Suppression (Days) | {} | - |\n",
        metrics.total_raw_optimal_suppression_days
    ));
    sm_md.push_str(&format!(
        "| Initial Top Actions Latency (Days) | {} | - |\n",
        metrics.total_initial_top_actions_latency_days
    ));
    sm_md.push_str("\n## 2. Gate / Topology / Breakout Distribution\n\n| Metric | Value | Rate |\n|---|---|---|\n");
    sm_md.push_str(&format!(
        "| Trend Gate Blocked Days | {} | {:.1}% |\n",
        metrics.trend_gate_blocked_days,
        (metrics.trend_gate_blocked_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Trend Status: Dispersed | {} | {:.1}% |\n",
        metrics.trend_status_dispersed_days,
        (metrics.trend_status_dispersed_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Trend Status: Forming | {} | {:.1}% |\n",
        metrics.trend_status_forming_days,
        (metrics.trend_status_forming_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Trend Status: Formed | {} | {:.1}% |\n",
        metrics.trend_status_formed_days,
        (metrics.trend_status_formed_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Topology: NoLeader | {} | {:.1}% |\n",
        metrics.topology_no_leader_days,
        (metrics.topology_no_leader_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Topology: SingleLeader | {} | {:.1}% |\n",
        metrics.topology_single_leader_days,
        (metrics.topology_single_leader_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Topology: FragmentedLeaders | {} | {:.1}% |\n",
        metrics.topology_fragmented_leaders_days,
        (metrics.topology_fragmented_leaders_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Evaluated Asset-Days | {} | - |\n",
        metrics.evaluated_asset_days
    ));
    sm_md.push_str(&format!(
        "| Breakout-Eligible Asset-Days | {} | {:.1}% |\n",
        metrics.breakout_eligible_asset_days,
        (metrics.breakout_eligible_asset_days as f64 / breakout_status_total) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Breakout: NoBreakout | {} | {:.1}% |\n",
        metrics.breakout_no_breakout_count,
        (metrics.breakout_no_breakout_count as f64 / breakout_status_total) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Breakout: Emerging | {} | {:.1}% |\n",
        metrics.breakout_emerging_count,
        (metrics.breakout_emerging_count as f64 / breakout_status_total) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Breakout: Confirmed | {} | {:.1}% |\n",
        metrics.breakout_confirmed_count,
        (metrics.breakout_confirmed_count as f64 / breakout_status_total) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Breakout: Failed Risk Flags | {} | {:.1}% |\n",
        metrics.breakout_failed_risk_count, breakout_failed_rate
    ));
    sm_md
}

pub fn generate_comparison_report(
    baseline: &StateMachineMetrics,
    enhanced: &StateMachineMetrics,
) -> Result<()> {
    let mut report = String::new();
    report.push_str("# ⚖️ State Machine Comparison: Baseline vs Enhanced (V1.4)\n\n");
    report.push_str("| Metric | Baseline | Enhanced | Change |\n|---|---|---|---|\n");

    let metrics = [
        (
            "Resets",
            baseline.reset_count as f64,
            enhanced.reset_count as f64,
            false,
        ),
        (
            "Blocked Resets",
            baseline.blocked_reset_count as f64,
            enhanced.blocked_reset_count as f64,
            true,
        ),
        (
            "Duration Locks",
            baseline.duration_lock_count as f64,
            enhanced.duration_lock_count as f64,
            true,
        ),
        (
            "Soft Resets",
            baseline.soft_reset_count as f64,
            enhanced.soft_reset_count as f64,
            true,
        ),
        (
            "State Flips (5d)",
            baseline.state_flip_count_5d as f64,
            enhanced.state_flip_count_5d as f64,
            false,
        ),
        (
            "Top Actions Turnover (%)",
            (baseline.top_actions_turnover_sum / baseline.total_days as f64) * 100.0,
            (enhanced.top_actions_turnover_sum / enhanced.total_days as f64) * 100.0,
            false,
        ),
        (
            "Raw-vs-Actual Optimal Divergence",
            baseline.total_raw_vs_actual_divergence_days as f64,
            enhanced.total_raw_vs_actual_divergence_days as f64,
            false,
        ),
        (
            "Raw Optimal Suppression",
            baseline.total_raw_optimal_suppression_days as f64,
            enhanced.total_raw_optimal_suppression_days as f64,
            false,
        ),
        (
            "Initial Top Actions Latency",
            baseline.total_initial_top_actions_latency_days as f64,
            enhanced.total_initial_top_actions_latency_days as f64,
            false,
        ),
    ];

    for (name, b, e, higher_is_better) in metrics {
        let diff = e - b;
        let pct = if b != 0.0 { (diff / b) * 100.0 } else { 0.0 };
        let trend = if diff == 0.0 {
            "➡️"
        } else if (diff > 0.0) == higher_is_better {
            "✅"
        } else {
            "⚠️"
        };
        report.push_str(&format!(
            "| {} | {:.1} | {:.1} | {} {:+.1}% |\n",
            name, b, e, trend, pct
        ));
    }

    report.push_str("\n## 🔍 Asset Layer Specifics\n");
    report.push_str(&format!(
        "- **Core Asset Protection Hits**: {} (Enhanced only)\n",
        enhanced.core_asset_protection_hits
    ));
    report.push_str(&format!(
        "- **Weak Asset Promotion Cap Hits**: {} (Enhanced only)\n",
        enhanced.weak_asset_promotion_cap_hits
    ));

    fs::write("backtest/state_machine_comparison.md", report)?;
    Ok(())
}

pub fn publish_primary_backtest_outputs() -> Result<()> {
    fs::create_dir_all("backtest")?;
    fs::copy("backtest/enhanced/summary.md", "backtest/summary.md")?;
    fs::copy(
        "backtest/enhanced/state_machine_metrics.md",
        "backtest/state_machine_metrics.md",
    )?;
    fs::copy(
        "backtest/enhanced/state_machine_metrics.json",
        "backtest/state_machine_metrics.json",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::backtest::application::model::{
        BacktestSimulationReport, ConfirmationCostSummary, ValidationBaselineComparison,
        ValidationClassOutcome, ValidationCohortReport, ValidationReasonUtility, ValidationReport,
        ValidationUtility,
    };

    #[test]
    fn summary_contains_validation_sections_and_uppercase_decision_classes() {
        let report = BacktestSimulationReport {
            name: "test".to_string(),
            metrics: StateMachineMetrics::default(),
            reliability: Vec::new(),
            regime_audit: Vec::new(),
            validation: ValidationReport {
                cohorts: vec![ValidationCohortReport {
                    decision_snapshot_version: "radar-v1.0.0".to_string(),
                    universe_id: "watchlist:AAPL".to_string(),
                    outcomes: vec![ValidationClassOutcome {
                        decision_class: Some(BacktestDecisionClass::NoTrade),
                        sample_count: 1,
                        ..Default::default()
                    }],
                    baseline: ValidationBaselineComparison {
                        return_difference: Some(-0.01),
                        mae_difference: Some(0.05),
                        mfe_difference: Some(-0.02),
                        ..Default::default()
                    },
                    utility: ValidationUtility {
                        p95_mae_20d: Some(-0.20),
                        top_decile_missed_upside: Some(0.25),
                        reason_breakdown: vec![ValidationReasonUtility {
                            reason: "TREND_GATE_BLOCKED".to_string(),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    population: Default::default(),
                    confirmation_cost: ConfirmationCostSummary {
                        average_breakout_to_ready_sessions: Some(2.0),
                        average_return_breakout_to_ready: Some(0.03),
                        ..Default::default()
                    },
                    net_decision_value: Default::default(),
                    sample_maturity: "INSUFFICIENT".to_string(),
                    protection_sample_maturity: "INSUFFICIENT".to_string(),
                    confirmation_sample_maturity: "INSUFFICIENT".to_string(),
                }],
                ..Default::default()
            },
            window: Default::default(),
        };
        let summary = render_summary_markdown(&report);
        assert!(summary.contains("## 4. Decision Validation"));
        assert!(summary.contains("## Validation Window"));
        assert!(summary.contains("Decision window: N/A → N/A"));
        assert!(summary.contains("Outcome window: N/A → N/A"));
        assert!(summary.contains("#### Protection Population Audit"));
        assert!(summary.contains("Raw Top-3 × Gate blocked records"));
        assert!(summary.contains("Gate blocked non-candidate reasons"));
        assert!(summary.contains("| NO_TRADE |"));
        assert!(summary.contains("### Counterfactual Baseline"));
        assert!(summary.contains("watchlist:AAPL"));
        assert!(summary.contains("P95 MAE"));
        assert!(summary.contains("Top-decile missed upside"));
        assert!(summary.contains("Breakout → Ready"));
        assert!(summary.contains("Raw Top-3 strength proxy → Breakout"));
        assert!(summary.contains("Return lost before Ready (positive waiting-upside only)"));
        assert!(summary.contains("Raw Top-3 NO_TRADE Trend Gate blocked candidates"));
        assert!(summary.contains("Episodes:"));
        assert!(summary.contains("#### Net Decision Value (Trend Gate-only"));
        assert!(summary.contains("reasons may overlap"));
        assert!(summary.contains("T+5 complete/downside"));
        assert!(summary.contains("adverse waiting samples"));
    }
}

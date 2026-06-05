use crate::features::backtest::application::model::BacktestSimulationReport;
use crate::features::backtest::domain::metrics::StateMachineMetrics;
use anyhow::Result;
use std::fs;

pub fn write_run_artifacts(report: &BacktestSimulationReport) -> Result<()> {
    let summary_markdown = render_summary_markdown(report);
    let state_machine_metrics_markdown = render_state_machine_metrics_markdown(&report.metrics);
    let state_machine_metrics_json = serde_json::to_string_pretty(&report.metrics)?;

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
    summary
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

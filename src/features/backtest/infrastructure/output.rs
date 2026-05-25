use crate::features::backtest::domain::metrics::{BacktestRunArtifacts, StateMachineMetrics};
use anyhow::Result;
use std::fs;

pub fn write_run_artifacts(dir_name: &str, artifacts: &BacktestRunArtifacts) -> Result<()> {
    let base_dir = format!("backtest/{}", dir_name);
    fs::create_dir_all(&base_dir)?;
    fs::write(
        format!("{}/summary.md", base_dir),
        &artifacts.summary_markdown,
    )?;
    fs::write(
        format!("{}/state_machine_metrics.md", base_dir),
        &artifacts.state_machine_metrics_markdown,
    )?;
    fs::write(
        format!("{}/state_machine_metrics.json", base_dir),
        &artifacts.state_machine_metrics_json,
    )?;
    Ok(())
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

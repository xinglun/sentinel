use crate::config::{AppConfig, ParsedRules};
use crate::features::radar::application::engine::Engine;
use crate::features::radar::application::provider::{MarketDataProvider, TickerHistory};
use crate::features::radar::domain::action_matrix::AssetAction;
use crate::features::radar::domain::decision::DecisionPacket;
use anyhow::Result;
use chrono::NaiveDate;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use time::OffsetDateTime;

#[derive(Default, Debug)]
struct RegimeStats {
    total_signals: usize,
    correct_signals: usize,
    sum_20d_return: f64,
    sum_max_drawdown_20d: f64,
    count_drawdowns: usize,
}

#[derive(Default, Debug, serde::Serialize, Clone)]
pub struct StateMachineMetrics {
    pub reset_count: usize,
    pub blocked_reset_count: usize,
    pub multi_step_downgrade_attempt_count: usize,
    pub duration_lock_count: usize,
    pub soft_reset_count: usize,
    pub defensive_override_count: usize,
    pub state_flip_count_5d: usize,
    // Asset-level stability metrics (V1.4)
    pub top_actions_turnover_sum: f64,
    pub core_asset_protection_hits: usize,
    pub weak_asset_promotion_cap_hits: usize,
    // Behavior Calibration Proxy Metrics (V1.4+)
    pub total_raw_vs_actual_divergence_days: usize,
    pub total_raw_optimal_suppression_days: usize,
    pub total_initial_top_actions_latency_days: usize,
    pub total_overstay_events: usize,
    pub total_recovery_events: usize,
    pub total_days: usize,
    pub evaluated_asset_days: usize,
    pub breakout_eligible_asset_days: usize,
    pub trend_gate_blocked_days: usize,
    pub trend_status_dispersed_days: usize,
    pub trend_status_forming_days: usize,
    pub trend_status_formed_days: usize,
    pub topology_no_leader_days: usize,
    pub topology_single_leader_days: usize,
    pub topology_fragmented_leaders_days: usize,
    pub breakout_no_breakout_count: usize,
    pub breakout_emerging_count: usize,
    pub breakout_confirmed_count: usize,
    pub breakout_failed_risk_count: usize,
}

pub async fn run_backtest(
    config: &AppConfig,
    provider: &(dyn MarketDataProvider + Send + Sync),
    from_date_str: &str,
    to_date_str: &str,
) -> Result<()> {
    let from_date = NaiveDate::parse_from_str(from_date_str, "%Y-%m-%d")?;
    let to_date = NaiveDate::parse_from_str(to_date_str, "%Y-%m-%d")?;

    let from_dt = OffsetDateTime::from_unix_timestamp(
        from_date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp(),
    )
    .ok();
    let to_dt = OffsetDateTime::from_unix_timestamp(
        to_date
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc()
            .timestamp(),
    )
    .ok();

    println!(
        "📊 Fetching history for backtest from {} to {}...",
        from_date_str, to_date_str
    );

    let mut histories = HashMap::new();
    for entry in config.watchlist.iter().filter(|w| w.enable) {
        println!("   Fetching {}...", entry.symbol);
        if let Ok(hist) = provider.fetch_history(&entry.symbol, from_dt, to_dt).await {
            histories.insert(entry.symbol.clone(), hist);
        }
    }

    if histories.is_empty() {
        return Err(anyhow::anyhow!("No history fetched."));
    }

    let index_symbol = if histories.contains_key("SPY") {
        "SPY".to_string()
    } else {
        histories.keys().next().unwrap().clone()
    };
    let index_history = histories.get(&index_symbol).unwrap();
    let mut simulation_dates = Vec::new();
    for bar in index_history.bars.iter() {
        if bar.date >= from_date && bar.date <= to_date {
            simulation_dates.push(bar.date);
        }
    }
    simulation_dates.sort();

    println!(
        "🧪 Running Comparative Backtest (Baseline vs Enhanced) over {} days",
        simulation_dates.len()
    );

    let parsed_rules = config.get_parsed_rules();
    let watchlist = config.watchlist.clone();

    // baseline（memory / friction なし）を実行する。
    println!("   [1/2] Running Baseline...");
    let baseline_metrics = run_core_simulation(
        &histories,
        &watchlist,
        &simulation_dates,
        &parsed_rules,
        false,
        "baseline",
    )?;

    // enhanced（memory / friction あり）を実行する。
    println!("   [2/2] Running Enhanced (V1.4)...");
    let enhanced_metrics = run_core_simulation(
        &histories,
        &watchlist,
        &simulation_dates,
        &parsed_rules,
        true,
        "enhanced",
    )?;

    // 比較 report を生成する。
    generate_comparison_report(&baseline_metrics, &enhanced_metrics)?;
    publish_primary_backtest_outputs()?;

    Ok(())
}

fn run_core_simulation(
    histories: &HashMap<String, TickerHistory>,
    watchlist: &[crate::config::WatchlistEntry],
    simulation_dates: &[NaiveDate],
    parsed_rules: &ParsedRules,
    use_memory: bool,
    dir_name: &str,
) -> Result<StateMachineMetrics> {
    let mut transition_matrix: HashMap<(String, String), usize> = HashMap::new();
    let mut prev_packet: Option<DecisionPacket> = None;
    let mut history_window: Vec<DecisionPacket> = Vec::with_capacity(20);
    let mut reliability: HashMap<String, (usize, usize)> = HashMap::new();

    let mut regime_tracking: HashMap<String, RegimeStats> = HashMap::new();
    let mut potential_records: Vec<(NaiveDate, f64)> = Vec::new();
    let mut sm_metrics = StateMachineMetrics::default();
    let mut state_history: Vec<crate::features::radar::domain::market_regime::MarketState> =
        Vec::new();

    let mut asset_indices: HashMap<String, usize> = HashMap::new();
    let mut raw_top3_first_seen: HashMap<String, NaiveDate> = HashMap::new();
    let mut mem_top3_first_seen: HashMap<String, NaiveDate> = HashMap::new();

    let optimal_threshold = parsed_rules
        .sorted_bands
        .iter()
        .find(|(name, _)| name.to_lowercase().contains("optimal"))
        .map(|(_, t)| *t)
        .unwrap_or(f64::MAX);

    for sym in histories.keys() {
        asset_indices.insert(sym.clone(), 0);
    }

    for current_date in simulation_dates.iter() {
        let mut daily_histories = Vec::new();

        for entry in watchlist.iter().filter(|w| w.enable) {
            if let Some(hist) = histories.get(&entry.symbol) {
                let idx = asset_indices.get_mut(&entry.symbol).unwrap();
                while *idx < hist.bars.len() && hist.bars[*idx].date <= *current_date {
                    *idx += 1;
                }

                if *idx > 0 {
                    let segmented_history = TickerHistory {
                        symbol: entry.symbol.clone(),
                        bars: Cow::Borrowed(&hist.bars[0..*idx]),
                        total_trading_days: hist.total_trading_days,
                        latest_quote_timestamp: None,
                    };
                    daily_histories.push((segmented_history, entry));
                }
            }
        }

        // core decision pipeline。
        let effective_window: &[DecisionPacket] = if use_memory {
            history_window.as_slice()
        } else {
            &[]
        };
        let current_packet = Engine::run_daily_pipeline(
            &daily_histories,
            parsed_rules,
            effective_window,
            &[],
            &std::collections::HashMap::new(),
        )?;

        // history window を更新する。
        history_window.push(current_packet.clone());
        if history_window.len() > 20 {
            history_window.remove(0);
        }

        // metrics を集計する。
        sm_metrics.total_days += 1;
        state_history.push(current_packet.market_regime.market_state);
        if !current_packet.trend_cohesion.gate_passed {
            sm_metrics.trend_gate_blocked_days += 1;
        }
        match current_packet.trend_cohesion.status {
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed => {
                sm_metrics.trend_status_dispersed_days += 1;
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Forming => {
                sm_metrics.trend_status_forming_days += 1;
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Formed => {
                sm_metrics.trend_status_formed_days += 1;
            }
        }
        match current_packet.trend_cohesion.topology {
            crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::NoLeader => {
                sm_metrics.topology_no_leader_days += 1;
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::SingleLeader => {
                sm_metrics.topology_single_leader_days += 1;
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionTopology::FragmentedLeaders => {
                sm_metrics.topology_fragmented_leaders_days += 1;
            }
        }
        for asset in &current_packet.assets {
            sm_metrics.evaluated_asset_days += 1;
            if asset.breakout.breakout_eligible {
                sm_metrics.breakout_eligible_asset_days += 1;
            }
            match asset.breakout.status {
                crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout => {
                    sm_metrics.breakout_no_breakout_count += 1;
                }
                crate::features::radar::domain::breakout_detection::BreakoutStatus::EmergingBreakout => {
                    sm_metrics.breakout_emerging_count += 1;
                }
                crate::features::radar::domain::breakout_detection::BreakoutStatus::ConfirmedBreakout => {
                    sm_metrics.breakout_confirmed_count += 1;
                }
            }
            if asset
                .breakout
                .reasons
                .contains(&crate::features::radar::domain::breakout_detection::BreakoutReason::FailedBreakoutRisk)
            {
                sm_metrics.breakout_failed_risk_count += 1;
            }
        }

        if let Some(ref prev) = prev_packet {
            if prev.market_regime.market_state != current_packet.market_regime.market_state {
                let key = (
                    format!("{:?}", prev.market_regime.market_state),
                    format!("{:?}", current_packet.market_regime.market_state),
                );
                *transition_matrix.entry(key).or_insert(0) += 1;
            }
        }

        if state_history.len() >= 5 {
            let window = &state_history[state_history.len() - 5..];
            let mut flips = 0;
            for i in 0..4 {
                if window[i] != window[i + 1] {
                    flips += 1;
                }
            }
            if flips >= 2 {
                sm_metrics.state_flip_count_5d += 1;
            }
        }

        if let Some(audit) = &current_packet.market_regime.transition_audit {
            if audit.to == crate::features::radar::domain::market_regime::LifecycleState::IGNITION
                && audit.from
                    != crate::features::radar::domain::market_regime::LifecycleState::NEWBORN
            {
                sm_metrics.reset_count += 1;
            }
            if audit.is_reset_blocked {
                sm_metrics.blocked_reset_count += 1;
            }
            if audit.is_downgrade_clamped {
                sm_metrics.multi_step_downgrade_attempt_count += 1;
            }
            if audit.duration_locked {
                sm_metrics.duration_lock_count += 1;
            }
            if audit.soft_reset_applied {
                sm_metrics.soft_reset_count += 1;
            }
            if audit.defensive_override {
                sm_metrics.defensive_override_count += 1;
            }
        }

        potential_records.push((
            *current_date,
            current_packet.market_features.potential_energy,
        ));

        // Asset Level Stability & Behavior Calibration (V1.4+)
        let current_top_actions: Vec<String> = current_packet
            .assets
            .iter()
            .take(3)
            .map(|a| a.symbol.clone())
            .collect();

        let mut raw_sorted_assets = current_packet.assets.clone();
        raw_sorted_assets.sort_by(|a, b| {
            b.deviation
                .unwrap_or(0.0)
                .partial_cmp(&a.deviation.unwrap_or(0.0))
                .unwrap()
        });
        let raw_top3: Vec<String> = raw_sorted_assets
            .iter()
            .take(3)
            .map(|a| a.symbol.clone())
            .collect();

        for asset in &current_packet.assets {
            let raw_score = asset.deviation.unwrap_or(0.0);
            let is_raw_optimal = raw_score >= optimal_threshold;
            let is_actual_optimal = asset.asset_state.state
                == crate::features::radar::domain::asset_state::AssetState::OPTIMAL;

            if is_actual_optimal && !is_raw_optimal {
                sm_metrics.total_raw_vs_actual_divergence_days += 1;
            }
            if !is_actual_optimal && is_raw_optimal {
                sm_metrics.total_raw_optimal_suppression_days += 1;
            }
        }

        // top actions latency を追跡する。
        for sym in &raw_top3 {
            raw_top3_first_seen
                .entry(sym.to_string())
                .or_insert(*current_date);
            if current_top_actions.contains(sym) {
                if let Some(raw_date) = raw_top3_first_seen.get(sym) {
                    let delay = (*current_date - *raw_date).num_days() as usize;
                    if delay > 0 && !mem_top3_first_seen.contains_key(sym) {
                        sm_metrics.total_initial_top_actions_latency_days += delay;
                        mem_top3_first_seen.insert(sym.clone(), *current_date);
                    }
                }
            }
        }

        if let Some(ref prev) = prev_packet {
            let prev_top_actions: Vec<String> = prev
                .assets
                .iter()
                .take(3)
                .map(|a| a.symbol.clone())
                .collect();
            if !prev_top_actions.is_empty() {
                let mut matches = 0;
                for sym in &current_top_actions {
                    if prev_top_actions.contains(sym) {
                        matches += 1;
                    }
                }
                let turnover = 1.0 - (matches as f64 / 3.0);
                sm_metrics.top_actions_turnover_sum += turnover;
            }

            for asset in &current_packet.assets {
                let reasons_str = asset.reasons.join(" | ");
                if reasons_str.contains("Friction:Hold") || reasons_str.contains("Top Tier Lock") {
                    sm_metrics.core_asset_protection_hits += 1;
                }
                if reasons_str.contains("Friction:Block") || reasons_str.contains("Promotion Cap") {
                    sm_metrics.weak_asset_promotion_cap_hits += 1;
                }
            }
        }

        // reliability を確認する。
        for asset in &current_packet.assets {
            let state_str = format!("{:?}", asset.asset_state.state);
            let reg_entry = regime_tracking.entry(state_str.clone()).or_default();
            if let Some(full_hist) = histories.get(&asset.symbol) {
                let current_idx = asset_indices
                    .get(&asset.symbol)
                    .cloned()
                    .unwrap_or(0)
                    .saturating_sub(1);
                let future_idx = current_idx + 20;
                if future_idx < full_hist.bars.len() {
                    let curr = &full_hist.bars[current_idx];
                    let fut = &full_hist.bars[future_idx];
                    let fwd_return = (fut.close - curr.close) / curr.close;
                    reg_entry.total_signals += 1;
                    reg_entry.sum_20d_return += fwd_return;
                    let search_end = future_idx.min(full_hist.bars.len() - 1);
                    let mut min_price = curr.close;
                    for b in &full_hist.bars[current_idx..=search_end] {
                        if b.close < min_price {
                            min_price = b.close;
                        }
                    }
                    reg_entry.sum_max_drawdown_20d += (min_price - curr.close) / curr.close;
                    reg_entry.count_drawdowns += 1;
                    let is_bear = asset.action == AssetAction::REDUCE
                        || asset.action == AssetAction::FREEZE
                        || asset.action == AssetAction::AVOID;
                    let is_correct = if is_bear {
                        fwd_return < 0.0
                    } else {
                        fwd_return > 0.0
                    };
                    if is_correct {
                        reg_entry.correct_signals += 1;
                    }
                    let conf_bucket =
                        match current_packet.market_features.system_confidence as usize {
                            90..=100 => "90-100",
                            80..=89 => "80-90",
                            70..=79 => "70-80",
                            60..=69 => "60-70",
                            50..=59 => "50-60",
                            _ => "<50",
                        };
                    let rel_entry = reliability.entry(conf_bucket.to_string()).or_insert((0, 0));
                    rel_entry.0 += 1;
                    if is_correct {
                        rel_entry.1 += 1;
                    }
                }
            }
        }

        prev_packet = Some(current_packet);
    }

    // この run の output を書き出す。
    let base_dir = format!("backtest/{}", dir_name);
    fs::create_dir_all(&base_dir)?;

    let mut summary = String::new();
    summary.push_str(&format!("# 🔭 Backtest Summary ({})\n\n", dir_name));
    summary.push_str("## 1. Reliability Calibration\n| Bucket | Total | Correct | Win Rate |\n|---|---|---|---|\n");
    let mut rel_vec: Vec<_> = reliability.into_iter().collect();
    rel_vec.sort_by(|a, b| b.0.cmp(&a.0));
    for (b, (t, c)) in rel_vec {
        summary.push_str(&format!(
            "| {} | {} | {} | {:.1}% |\n",
            b,
            t,
            c,
            (c as f64 / t as f64) * 100.0
        ));
    }
    summary.push_str("\n## 2. Regime Performance Audit\n| State | Signals | Hit Rate | Avg 20d | Max DD |\n|---|---|---|---|---|\n");
    let mut reg_vec: Vec<_> = regime_tracking.into_iter().collect();
    reg_vec.sort_by_key(|b| std::cmp::Reverse(b.1.total_signals));
    for (state, s) in reg_vec {
        if s.total_signals > 0 {
            summary.push_str(&format!(
                "| {} | {} | {:.1}% | {:+.2}% | {:+.2}% |\n",
                state,
                s.total_signals,
                (s.correct_signals as f64 / s.total_signals as f64) * 100.0,
                (s.sum_20d_return / s.total_signals as f64) * 100.0,
                (s.sum_max_drawdown_20d / s.total_signals as f64) * 100.0
            ));
        }
    }
    let days = sm_metrics.total_days as f64;
    let breakout_status_total = sm_metrics.evaluated_asset_days.max(1) as f64;
    let breakout_eligible_total = sm_metrics.breakout_eligible_asset_days.max(1) as f64;
    let breakout_failed_rate =
        (sm_metrics.breakout_failed_risk_count as f64 / breakout_eligible_total) * 100.0;

    summary.push_str("\n## 3. Gate / Topology / Breakout Distribution\n");
    summary.push_str(&format!(
        "- **Trend Gate Blocked Days**: {} / {} ({:.1}%)\n",
        sm_metrics.trend_gate_blocked_days,
        sm_metrics.total_days,
        (sm_metrics.trend_gate_blocked_days as f64 / days.max(1.0)) * 100.0
    ));
    summary.push_str(&format!(
        "- **Trend Status**: Dispersed={} | Forming={} | Formed={}\n",
        sm_metrics.trend_status_dispersed_days,
        sm_metrics.trend_status_forming_days,
        sm_metrics.trend_status_formed_days
    ));
    summary.push_str(&format!(
        "- **Topology**: NoLeader={} | SingleLeader={} | FragmentedLeaders={}\n",
        sm_metrics.topology_no_leader_days,
        sm_metrics.topology_single_leader_days,
        sm_metrics.topology_fragmented_leaders_days
    ));
    summary.push_str(&format!(
        "- **Evaluated Asset-Days**: {} | Breakout-Eligible Asset-Days={} ({:.1}% of evaluated)\n",
        sm_metrics.evaluated_asset_days,
        sm_metrics.breakout_eligible_asset_days,
        (sm_metrics.breakout_eligible_asset_days as f64 / breakout_status_total) * 100.0
    ));
    summary.push_str(&format!(
        "- **Breakout Status Counts**: NoBreakout={} | Emerging={} | Confirmed={}\n",
        sm_metrics.breakout_no_breakout_count,
        sm_metrics.breakout_emerging_count,
        sm_metrics.breakout_confirmed_count
    ));
    summary.push_str(&format!(
        "- **Failed Breakout Risk Flags**: {} ({:.1}% of breakout-eligible asset-days)\n",
        sm_metrics.breakout_failed_risk_count, breakout_failed_rate
    ));
    fs::write(format!("{}/summary.md", base_dir), summary)?;

    let mut sm_md = String::new();
    sm_md.push_str(
        "# 🧭 State Machine Quality Metrics\n\n| Metric | Value | Rate |\n|---|---|---|\n",
    );
    sm_md.push_str(&format!(
        "| Reset | {} | {:.1}% |\n",
        sm_metrics.reset_count,
        (sm_metrics.reset_count as f64 / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Blocked Reset | {} | {:.1}% |\n",
        sm_metrics.blocked_reset_count,
        (sm_metrics.blocked_reset_count as f64 / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Duration Locked | {} | {:.1}% |\n",
        sm_metrics.duration_lock_count,
        (sm_metrics.duration_lock_count as f64 / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Soft Reset | {} | {:.1}% |\n",
        sm_metrics.soft_reset_count,
        (sm_metrics.soft_reset_count as f64 / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Flips (5d) | {} | {:.1}% |\n",
        sm_metrics.state_flip_count_5d,
        (sm_metrics.state_flip_count_5d as f64 / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Top Actions Turnover | - | {:.1}% |\n",
        (sm_metrics.top_actions_turnover_sum / days) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Core Asset Protection Hits | {} | - |\n",
        sm_metrics.core_asset_protection_hits
    ));
    sm_md.push_str(&format!(
        "| Weak Asset Promotion Cap Hits | {} | - |\n",
        sm_metrics.weak_asset_promotion_cap_hits
    ));
    sm_md.push_str(&format!(
        "| Raw-vs-Actual Optimal Divergence (Days) | {} | - |\n",
        sm_metrics.total_raw_vs_actual_divergence_days
    ));
    sm_md.push_str(&format!(
        "| Raw Optimal Suppression (Days) | {} | - |\n",
        sm_metrics.total_raw_optimal_suppression_days
    ));
    sm_md.push_str(&format!(
        "| Initial Top Actions Latency (Days) | {} | - |\n",
        sm_metrics.total_initial_top_actions_latency_days
    ));
    sm_md.push_str("\n## 2. Gate / Topology / Breakout Distribution\n\n| Metric | Value | Rate |\n|---|---|---|\n");
    sm_md.push_str(&format!(
        "| Trend Gate Blocked Days | {} | {:.1}% |\n",
        sm_metrics.trend_gate_blocked_days,
        (sm_metrics.trend_gate_blocked_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Trend Status: Dispersed | {} | {:.1}% |\n",
        sm_metrics.trend_status_dispersed_days,
        (sm_metrics.trend_status_dispersed_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Trend Status: Forming | {} | {:.1}% |\n",
        sm_metrics.trend_status_forming_days,
        (sm_metrics.trend_status_forming_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Trend Status: Formed | {} | {:.1}% |\n",
        sm_metrics.trend_status_formed_days,
        (sm_metrics.trend_status_formed_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Topology: NoLeader | {} | {:.1}% |\n",
        sm_metrics.topology_no_leader_days,
        (sm_metrics.topology_no_leader_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Topology: SingleLeader | {} | {:.1}% |\n",
        sm_metrics.topology_single_leader_days,
        (sm_metrics.topology_single_leader_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Topology: FragmentedLeaders | {} | {:.1}% |\n",
        sm_metrics.topology_fragmented_leaders_days,
        (sm_metrics.topology_fragmented_leaders_days as f64 / days.max(1.0)) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Evaluated Asset-Days | {} | - |\n",
        sm_metrics.evaluated_asset_days
    ));
    sm_md.push_str(&format!(
        "| Breakout-Eligible Asset-Days | {} | {:.1}% |\n",
        sm_metrics.breakout_eligible_asset_days,
        (sm_metrics.breakout_eligible_asset_days as f64 / breakout_status_total) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Breakout: NoBreakout | {} | {:.1}% |\n",
        sm_metrics.breakout_no_breakout_count,
        (sm_metrics.breakout_no_breakout_count as f64 / breakout_status_total) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Breakout: Emerging | {} | {:.1}% |\n",
        sm_metrics.breakout_emerging_count,
        (sm_metrics.breakout_emerging_count as f64 / breakout_status_total) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Breakout: Confirmed | {} | {:.1}% |\n",
        sm_metrics.breakout_confirmed_count,
        (sm_metrics.breakout_confirmed_count as f64 / breakout_status_total) * 100.0
    ));
    sm_md.push_str(&format!(
        "| Breakout: Failed Risk Flags | {} | {:.1}% |\n",
        sm_metrics.breakout_failed_risk_count, breakout_failed_rate
    ));
    fs::write(format!("{}/state_machine_metrics.md", base_dir), sm_md)?;
    fs::write(
        format!("{}/state_machine_metrics.json", base_dir),
        serde_json::to_string_pretty(&sm_metrics)?,
    )?;

    Ok(sm_metrics)
}

fn generate_comparison_report(
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

fn publish_primary_backtest_outputs() -> Result<()> {
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

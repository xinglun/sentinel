use crate::config::AppConfig;
use crate::core::engine::evaluate_snapshot;
use crate::data::yahoo_provider::{fetch_history, DailyBar, TickerHistory};
use anyhow::Result;
use chrono::NaiveDate;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use time::OffsetDateTime;

pub async fn run_backtest(
    config: &AppConfig,
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
        if let Ok(hist) = fetch_history(&entry.symbol, from_dt, to_dt).await {
            histories.insert(entry.symbol.clone(), hist);
        }
    }

    if histories.is_empty() {
        return Err(anyhow::anyhow!("No history fetched."));
    }

    // Determine the primary index symbol (default to SPY if exists, else the first)
    let index_symbol = if histories.contains_key("SPY") {
        "SPY".to_string()
    } else {
        histories.keys().next().unwrap().clone()
    };

    let index_history = histories.get(&index_symbol).unwrap();
    let mut index_prices = BTreeMap::new();
    // Build array of dates to simulate
    let mut simulation_dates = Vec::new();
    for bar in &index_history.bars {
        if bar.date >= from_date && bar.date <= to_date {
            simulation_dates.push(bar.date);
        }
        index_prices.insert(bar.date, bar.close);
    }
    simulation_dates.sort();

    println!("🧪 Simulating {} trading days...", simulation_dates.len());

    let parsed_rules = config.get_parsed_rules();

    // Tracking structures
    // Transition Matrix: (FromState, ToState) -> count
    let mut transition_matrix: HashMap<(String, String), usize> = HashMap::new();
    // Previous states: symbol -> Option<(Date, State)>
    let mut prev_states: HashMap<String, (NaiveDate, String)> = HashMap::new();

    // Reliability Curve: Confidence Bucket (e.g. "90-100") -> (Total, Correct)
    let mut reliability: HashMap<String, (usize, usize)> = HashMap::new();

    // Discrimination Score Tracking
    let mut high_conf_returns: Vec<f64> = Vec::new();
    let mut low_conf_returns: Vec<f64> = Vec::new();
    let mut high_conf_correct = 0;
    let mut low_conf_correct = 0;

    // --- Phase 13: Regime Alpha Separation Audit ---
    #[derive(Default, Debug)]
    struct RegimeStats {
        total_signals: usize,
        correct_signals: usize,
        sum_20d_return: f64,
        sum_max_drawdown_20d: f64,
        count_drawdowns: usize,
    }
    let mut regime_tracking: HashMap<String, RegimeStats> = HashMap::new();

    // Potential Energy -> Forward Returns
    let mut potential_records: Vec<(NaiveDate, f64)> = Vec::new();

    // Phase 15: ML Features CSV
    let mut ml_csv = "Date,Symbol,State_Code,Confidence_Score,Z_Score,Owner_MA_Slope,Curvature,Trend_Age,Global_Potential_Energy,Forward_20d_Return_Pct\n".to_string();

    struct TempMlRow {
        symbol: String,
        state_code: String,
        confidence: usize,
        z_score: Option<f64>,
        slope: Option<f64>,
        curvature: Option<f64>,
        trend_age: usize,
        fwd_return: f64,
    }

    for (i, current_date) in simulation_dates.iter().enumerate() {
        let mut daily_snapshots = Vec::new();
        let mut daily_ml_rows = Vec::new();

        for entry in config.watchlist.iter().filter(|w| w.enable) {
            if let Some(hist) = histories.get(&entry.symbol) {
                // Slice history up to current_date
                let sliced_bars: Vec<DailyBar> = hist
                    .bars
                    .iter()
                    .filter(|b| b.date <= *current_date)
                    .cloned()
                    .collect();
                if sliced_bars.is_empty() {
                    continue;
                }

                let sliced_history = TickerHistory {
                    symbol: entry.symbol.clone(),
                    bars: sliced_bars,
                    total_trading_days: hist.total_trading_days,
                    latest_quote_timestamp: None,
                };

                let snap = evaluate_snapshot(&sliced_history, entry, &parsed_rules);
                daily_snapshots.push(snap.clone());

                // Track transitions
                if let Some((_, prev_state)) = prev_states.get(&entry.symbol) {
                    if *prev_state != snap.state_code
                        && snap.state_code != "UNKNOWN"
                        && prev_state != "UNKNOWN"
                    {
                        let key = (prev_state.clone(), snap.state_code.clone());
                        *transition_matrix.entry(key).or_insert(0) += 1;
                    }
                }
                prev_states.insert(
                    entry.symbol.clone(),
                    (*current_date, snap.state_code.clone()),
                );

                // Record Reliability Signal
                if snap.state_code != "UNKNOWN" {
                    let bucket = if snap.confidence_score >= 90 {
                        "90-100"
                    } else if snap.confidence_score >= 80 {
                        "80-90"
                    } else if snap.confidence_score >= 70 {
                        "70-80"
                    } else if snap.confidence_score >= 60 {
                        "60-70"
                    } else if snap.confidence_score >= 50 {
                        "50-60"
                    } else {
                        "<50"
                    };

                    // We need forward returns. Let's look up index + 20 days.
                    // This is rough, a real backtest would store the signal and resolve later.
                    // We can resolve it by looking ahead in `simulation_dates`.
                    if i + 20 < simulation_dates.len() {
                        let future_date = simulation_dates[i + 20];
                        let full_hist = histories.get(&entry.symbol).unwrap();
                        let fut_bar = full_hist.bars.iter().find(|b| b.date >= future_date);
                        let curr_bar = full_hist.bars.iter().find(|b| b.date == *current_date);

                        if let (Some(curr), Some(fut)) = (curr_bar, fut_bar) {
                            let fwd_return = (fut.close - curr.close) / curr.close;

                            // Phase 15: Store for ML Export
                            daily_ml_rows.push(TempMlRow {
                                symbol: entry.symbol.clone(),
                                state_code: snap.state_code.clone(),
                                confidence: snap.confidence_score,
                                z_score: snap.dev_z_score,
                                slope: snap.owner_ma_slope_pct,
                                curvature: snap.curvature,
                                trend_age: snap.trend_age,
                                fwd_return: fwd_return * 100.0,
                            });

                            // Calculate Max Drawdown in those 20 days
                            let mut min_price_in_20d = curr.close;
                            for fd_bar in full_hist
                                .bars
                                .iter()
                                .filter(|b| b.date >= *current_date && b.date <= future_date)
                            {
                                if fd_bar.close < min_price_in_20d {
                                    min_price_in_20d = fd_bar.close;
                                }
                            }
                            let max_drawdown = (min_price_in_20d - curr.close) / curr.close;

                            // Tracking for Discrimination Score
                            let mut is_correct_for_fwd = false;
                            let is_bear_state =
                                snap.state_code == "DEFEND" || snap.state_code == "fear_downtrend";

                            // For regular trend following and safe fears (buying dip), we expect positive return.
                            // For structural breakdowns, we expect negative return.
                            if is_bear_state {
                                if fwd_return < 0.0 {
                                    is_correct_for_fwd = true;
                                }
                            } else if fwd_return > 0.0 {
                                is_correct_for_fwd = true;
                            }

                            if snap.confidence_score >= 80 {
                                high_conf_returns.push(fwd_return);
                                if is_correct_for_fwd {
                                    high_conf_correct += 1;
                                }
                            } else if snap.confidence_score <= 60 {
                                low_conf_returns.push(fwd_return);
                                if is_correct_for_fwd {
                                    low_conf_correct += 1;
                                }
                            }

                            let entry = reliability.entry(bucket.to_string()).or_insert((0, 0));
                            entry.0 += 1;
                            if is_correct_for_fwd {
                                entry.1 += 1;
                            }

                            // Phase 13: Store in regime
                            let reg_entry = regime_tracking
                                .entry(snap.state_code.clone())
                                .or_insert_with(RegimeStats::default);
                            reg_entry.total_signals += 1;
                            if is_correct_for_fwd {
                                reg_entry.correct_signals += 1;
                            }
                            reg_entry.sum_20d_return += fwd_return;
                            reg_entry.sum_max_drawdown_20d += max_drawdown;
                            reg_entry.count_drawdowns += 1;
                        }
                    }
                }
            }
        }

        // Calculate Potential Energy
        let mut total_z = 0.0;
        let mut weight_sum = 0.0;
        for s in &daily_snapshots {
            if let Some(z) = s.dev_z_score {
                total_z += z.abs() * s.weight;
                weight_sum += s.weight;
            }
        }
        let mut daily_pot = 0.0;
        if weight_sum > 0.0 {
            let potential = total_z / weight_sum;
            potential_records.push((*current_date, potential));
            daily_pot = potential;
        }

        // Phase 15: Write ML rows
        for row in daily_ml_rows {
            ml_csv.push_str(&format!(
                "{},{},{},{},{:.4},{:.4},{:.4},{},{:.4},{:.4}\n",
                current_date,
                row.symbol,
                row.state_code,
                row.confidence,
                row.z_score.unwrap_or(0.0),
                row.slope.unwrap_or(0.0),
                row.curvature.unwrap_or(0.0),
                row.trend_age,
                daily_pot,
                row.fwd_return
            ));
        }
    }

    // Write out metrics
    fs::create_dir_all("backtest")?;

    // 1. Transition Matrix
    let mut tm_csv = "FromState,ToState,Count\n".to_string();
    for ((from, to), count) in &transition_matrix {
        tm_csv.push_str(&format!("{},{},{}\n", from, to, count));
    }
    fs::write("backtest/transition_matrix.csv", tm_csv)?;

    // Phase 15: ML Data Write
    fs::write("backtest/ml_features.csv", ml_csv)?;

    // 2. Reliability Curve
    let mut rc_csv = "ConfidenceBucket,TotalSignals,CorrectSignals,WinRatePct\n".to_string();
    for (bucket, (total, correct)) in &reliability {
        let win_rate = (*correct as f64 / *total as f64) * 100.0;
        rc_csv.push_str(&format!(
            "{},{},{},{:.1}\n",
            bucket, total, correct, win_rate
        ));
    }
    fs::write("backtest/reliability_curve.csv", rc_csv)?;

    // 3. Potential Energy Forward Returns (CSV)
    let mut pe_csv = "Date,PotentialEnergy,Forward5dPct,Forward20dPct,Forward60dPct\n".to_string();

    // For medians
    let mut high_pot_20d = Vec::new();
    let mut high_pot_60d = Vec::new();
    let mut low_pot_20d = Vec::new();
    let mut low_pot_60d = Vec::new();

    for (date, energy) in &potential_records {
        if let Some(curr_idx) = simulation_dates.iter().position(|r| r == date) {
            let get_ret_val = |offset: usize| -> Option<f64> {
                if curr_idx + offset < simulation_dates.len() {
                    let f_date = simulation_dates[curr_idx + offset];
                    if let (Some(&c), Some(&f)) =
                        (index_prices.get(date), index_prices.get(&f_date))
                    {
                        Some((f - c) / c * 100.0)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            let r5 = get_ret_val(5);
            let r20 = get_ret_val(20);
            let r60 = get_ret_val(60);

            if *energy >= 2.0 {
                pe_csv.push_str(&format!(
                    "{},{:.2},{},{},{}\n",
                    date,
                    energy,
                    r5.map(|v| format!("{:.2}", v)).unwrap_or_default(),
                    r20.map(|v| format!("{:.2}", v)).unwrap_or_default(),
                    r60.map(|v| format!("{:.2}", v)).unwrap_or_default()
                ));

                if let Some(v) = r20 {
                    high_pot_20d.push(v);
                }
                if let Some(v) = r60 {
                    high_pot_60d.push(v);
                }
            } else if *energy <= 1.0 {
                if let Some(v) = r20 {
                    low_pot_20d.push(v);
                }
                if let Some(v) = r60 {
                    low_pot_60d.push(v);
                }
            }
        }
    }
    fs::write("backtest/potential_forward_returns.csv", pe_csv)?;

    // --- Generate summary.md ---
    let mut summary = String::new();
    summary.push_str("# 🔭 Backtest Calibration Summary\n\n");

    // Probability Calibration
    summary.push_str("## 1. Probability Calibration Curve\n");
    summary.push_str("| Confidence Bucket | Expected Hit Rate | Actual Hit Rate | Signals |\n");
    summary.push_str("|-------------------|-------------------|-----------------|---------|\n");
    let mut rel_vec: Vec<_> = reliability.iter().collect();
    rel_vec.sort_by(|a, b| b.0.cmp(a.0)); // sort bucket descending

    let mut total_calibration_error = 0.0;
    let mut total_signals_calib = 0;

    for (bucket, (total, correct)) in rel_vec {
        let win_rate = (*correct as f64 / *total as f64) * 100.0;

        let expected_conf = match bucket.as_str() {
            "90-100" => 95.0,
            "80-90" => 85.0,
            "70-80" => 75.0,
            "60-70" => 65.0,
            "50-60" => 55.0,
            "<50" => 45.0,
            _ => 50.0,
        };

        summary.push_str(&format!(
            "| {} | {:.0}% | {:.1}% | {} |\n",
            bucket, expected_conf, win_rate, total
        ));

        total_calibration_error += (expected_conf - win_rate).abs() * (*total as f64);
        total_signals_calib += *total;
    }

    let calibration_score = if total_signals_calib > 0 {
        total_calibration_error / (total_signals_calib as f64)
    } else {
        0.0
    };
    summary.push_str(&format!("\n**🌟 Calibration Error**: `{:.1}%` *(Measures probability scale accuracy, lower is better)*\n\n", calibration_score));

    // Regime Separation (Trend vs Reversion)
    let high_conf_len = high_conf_returns.len();
    let low_conf_len = low_conf_returns.len();

    let avg_high = if high_conf_len == 0 {
        0.0
    } else {
        high_conf_returns.iter().sum::<f64>() / high_conf_len as f64 * 100.0
    };
    let avg_low = if low_conf_len == 0 {
        0.0
    } else {
        low_conf_returns.iter().sum::<f64>() / low_conf_len as f64 * 100.0
    };

    let hit_rate_high = if high_conf_len == 0 {
        0.0
    } else {
        (high_conf_correct as f64 / high_conf_len as f64) * 100.0
    };
    let hit_rate_low = if low_conf_len == 0 {
        0.0
    } else {
        (low_conf_correct as f64 / low_conf_len as f64) * 100.0
    };

    summary.push_str("## 2. Regime Alpha Separation\n");
    summary.push_str("*This demonstrates the dual-alpha structure of the system. High Confidence represents stable trends, while Low Confidence represents highly-elastic mean reversion opportunities.*\n\n");

    summary.push_str("### 📈 Trend Stability Alpha (Confidence >= 80%)\n");
    summary.push_str(&format!("- **Hit Rate**: `{:.1}%`\n", hit_rate_high));
    summary.push_str(&format!(
        "- **Avg 20d Forward Return**: `{:+.2}%`\n",
        avg_high
    ));
    summary.push_str("- *Characteristics: High probability of success, lower elastic magnitude. Suitable for compounding over time.*\n\n");

    summary.push_str("### 🧲 Mean Reversion Alpha (Confidence <= 60%)\n");
    summary.push_str(&format!("- **Hit Rate**: `{:.1}%`\n", hit_rate_low));
    summary.push_str(&format!(
        "- **Avg 20d Forward Return**: `{:+.2}%`\n",
        avg_low
    ));
    summary.push_str("- *Characteristics: Lower probability of immediate success, but much higher elastic magnitude on resolution. Suitable for opportunistic accumulation.*\n\n");

    // Phase 13 Regime-Specific Audit
    summary.push_str("## 3. Regime-Specific Alpha Audit\n");
    summary
        .push_str("*Performance decoupled by generated Capital State (20d forward metrics).*\n\n");
    summary.push_str(
        "| Capital State | Signals | Hit Rate | Avg 20d Return | Avg 20d Max Drawdown |\n",
    );
    summary.push_str(
        "|---------------|---------|----------|----------------|----------------------|\n",
    );

    // We want a logical ordering for display if possible, or just default alphabetical string sorting
    let mut reg_vec: Vec<_> = regime_tracking.into_iter().collect();
    // Sort by total signals descending just to show the most active states first
    // You can also use a custom sort logic if you prefer optimal -> fear -> defend
    reg_vec.sort_by(|a, b| b.1.total_signals.cmp(&a.1.total_signals));

    for (state, stats) in reg_vec {
        if stats.total_signals == 0 {
            continue;
        }
        let hit_rate = (stats.correct_signals as f64 / stats.total_signals as f64) * 100.0;
        let avg_ret = (stats.sum_20d_return / stats.total_signals as f64) * 100.0;
        let avg_dd = if stats.count_drawdowns > 0 {
            (stats.sum_max_drawdown_20d / stats.count_drawdowns as f64) * 100.0
        } else {
            0.0
        };

        let mut state_name = state;
        if state_name == "fear_downtrend" || state_name == "DEFEND" {
            state_name = format!("🛑 {}", state_name);
        } else if state_name.starts_with("fear") {
            state_name = format!("🩸 {}", state_name);
        } else if state_name.starts_with("optimal") || state_name.starts_with("cruise") {
            state_name = format!("🟢 {}", state_name);
        } else if state_name.starts_with("overheat") || state_name.starts_with("pullback") {
            state_name = format!("🟡 {}", state_name);
        } else if state_name.starts_with("CAUTION") {
            state_name = format!("⚠️ {}", state_name);
        }

        summary.push_str(&format!(
            "| **{}** | `{}` | `{:.1}%` | `{:+.2}%` | `{:+.2}%` |\n",
            state_name, stats.total_signals, hit_rate, avg_ret, avg_dd
        ));
    }
    summary.push('\n');

    // Potential
    summary.push_str("## 4. Potential Energy Forward Returns (Median 20d/60d Index Returns)\n");
    let median = |mut arr: Vec<f64>| -> String {
        if arr.is_empty() {
            return "N/A".to_string();
        }
        arr.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = arr.len() / 2;
        format!("{:+.2}%", arr[mid])
    };
    summary.push_str(&format!(
        "- **High Tension (Potential >= 2.0)**: +20d = `{}`, +60d = `{}`\n",
        median(high_pot_20d),
        median(high_pot_60d)
    ));
    summary.push_str(&format!(
        "- **Low Tension (Potential <= 1.0)**: +20d = `{}`, +60d = `{}`\n",
        median(low_pot_20d),
        median(low_pot_60d)
    ));
    summary.push('\n');

    // Transitions
    summary.push_str("## 5. State Transition Flow\n");
    let mut from_groups: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    for ((from, to), count) in &transition_matrix {
        from_groups
            .entry(from.clone())
            .or_default()
            .push((to.clone(), *count));
    }
    for (from, targets) in &mut from_groups {
        let total: usize = targets.iter().map(|(_, c)| c).sum();
        targets.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending
        summary.push_str(&format!("**FROM {}** ({} transitions):\n", from, total));
        for (to, count) in targets.iter().take(3) {
            let pct = (*count as f64 / total as f64) * 100.0;
            summary.push_str(&format!("  - `→ {}`: {:.1}%\n", to, pct));
        }
    }
    fs::write("backtest/summary.md", summary)?;

    println!("✅ Backtest complete! Output written to ./backtest/");

    Ok(())
}

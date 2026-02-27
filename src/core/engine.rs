use crate::config::{DeviationBasis, ParsedRules, WatchlistEntry};
use crate::data::yahoo_provider::{DailyBar, TickerHistory};
use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum TrendStatus {
    Up,
    Down,
    Flat,
    Unknown,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum RegimeValidity {
    Valid,
    FormingEarly, // < 1.5x Owner MA
    FormingLate,  // 1.5x - 3x Owner MA or unstable slope
    Invalid,
}

#[derive(Debug, Serialize, Clone)]
pub struct TickerSnapshot {
    pub symbol: String,
    pub name: String,
    pub weight: f64,
    pub reason_code: Option<String>,
    pub current_date: NaiveDate,
    pub dog_price: f64,
    pub owner_ma: Option<f64>,
    pub leash_ma: Option<f64>,
    pub owner_ma_slope_pct: Option<f64>,
    pub dev_z_score: Option<f64>,
    pub curvature: Option<f64>,
    pub confidence_score: usize,
    pub trend_status: TrendStatus,
    pub deviation_pct: Option<f64>,
    pub deviation_basis_used: String,
    pub state_code: String,
    pub action_text: String,
    pub is_bear_mode_active: bool,
    pub is_caution_mode_active: bool,
    pub trend_age: usize,
    pub owner_deviation_pct: Option<f64>,
    pub deviation_percentile: Option<f64>,
    pub validity: RegimeValidity,
    pub history_days: usize,
}

pub fn calculate_ma(bars: &[DailyBar], days: usize, end_index: usize) -> Option<f64> {
    if days == 0 || end_index + 1 < days {
        return None;
    }
    let start_index = end_index + 1 - days;
    let slice = &bars[start_index..=end_index];
    let sum: f64 = slice.iter().map(|b| b.close).sum();
    Some(sum / days as f64)
}

pub fn calculate_std_dev(
    bars: &[DailyBar],
    days: usize,
    end_index: usize,
    mean: f64,
) -> Option<f64> {
    if days == 0 || end_index + 1 < days {
        return None;
    }
    let start_index = end_index + 1 - days;
    let slice = &bars[start_index..=end_index];
    let variance: f64 = slice
        .iter()
        .map(|b| {
            let diff = b.close - mean;
            diff * diff
        })
        .sum::<f64>()
        / days as f64;
    Some(variance.sqrt())
}

pub fn detect_trend(
    bars: &[DailyBar],
    ma_days: usize,
    lookback: usize,
    flat_threshold_pct: f64,
) -> TrendStatus {
    if bars.is_empty() {
        return TrendStatus::Unknown;
    }
    let current_idx = bars.len() - 1;
    let current_ma = calculate_ma(bars, ma_days, current_idx);

    if current_idx < lookback {
        return TrendStatus::Unknown;
    }
    let past_idx = current_idx - lookback;
    let past_ma = calculate_ma(bars, ma_days, past_idx);

    match (current_ma, past_ma) {
        (Some(curr), Some(past)) => {
            if past == 0.0 {
                return TrendStatus::Unknown; // Avoid division by zero
            }
            let change_pct = (curr - past) / past * 100.0;
            if change_pct > flat_threshold_pct {
                TrendStatus::Up
            } else if change_pct < -flat_threshold_pct {
                TrendStatus::Down
            } else {
                TrendStatus::Flat
            }
        }
        _ => TrendStatus::Unknown,
    }
}

pub fn evaluate_snapshot(
    history: &TickerHistory,
    entry: &WatchlistEntry,
    rules: &ParsedRules,
) -> TickerSnapshot {
    let name = entry.name.clone().unwrap_or_else(|| entry.symbol.clone());

    if history.bars.is_empty() {
        return TickerSnapshot {
            symbol: entry.symbol.clone(),
            name,
            weight: entry.weight.unwrap_or(1.0),
            reason_code: Some("[NO DATA]".to_string()),
            current_date: chrono::Local::now().date_naive(),
            dog_price: 0.0,
            owner_ma: None,
            leash_ma: None,
            owner_ma_slope_pct: None,
            dev_z_score: None,
            curvature: None,
            confidence_score: 0,
            trend_status: TrendStatus::Unknown,
            deviation_pct: None,
            deviation_basis_used: format!("{:?}", entry.deviation_basis).to_lowercase(),
            state_code: "ERROR".to_string(),
            action_text: "[NO DATA]".to_string(),
            is_bear_mode_active: false,
            is_caution_mode_active: false,
            trend_age: 0,
            owner_deviation_pct: None,
            deviation_percentile: None,
            validity: RegimeValidity::Invalid,
            history_days: 0,
        };
    }

    let last_idx = history.bars.len() - 1;
    let current_bar = &history.bars[last_idx];
    let dog_price = current_bar.close;

    let owner_ma = calculate_ma(&history.bars, entry.owner_ma_days, last_idx);
    let leash_ma = calculate_ma(&history.bars, entry.leash_ma_days, last_idx);

    let trend_status = detect_trend(
        &history.bars,
        entry.owner_ma_days,
        rules.trend.lookback_days,
        rules.trend.flat_threshold_pct,
    );

    let basis_val = match entry.deviation_basis {
        DeviationBasis::Owner => owner_ma,
        DeviationBasis::Leash => leash_ma,
    };

    let deviation_pct = if let Some(b) = basis_val {
        if b != 0.0 {
            Some((dog_price - b) / b * 100.0)
        } else {
            None
        }
    } else {
        None
    };
    let owner_deviation_pct = if let Some(om) = owner_ma {
        if om != 0.0 {
            Some((dog_price - om) / om * 100.0)
        } else {
            None
        }
    } else {
        None
    };

    let history_days = history.bars.len();

    // --- Phase 35: Behavioral Context (Historical Percentile) ---
    let mut deviation_percentile = None;
    if let Some(current_dev) = owner_deviation_pct {
        let lookback_bars = 1260; // Approx 5 years of trading days
        let start_sim = last_idx.saturating_sub(lookback_bars);
        let mut historical_devs = Vec::with_capacity(last_idx - start_sim + 1);

        for i in start_sim..=last_idx {
            if let Some(ma) = calculate_ma(&history.bars, entry.owner_ma_days, i) {
                if ma != 0.0 {
                    let dev = (history.bars[i].close - ma) / ma * 100.0;
                    historical_devs.push(dev);
                }
            }
        }

        // Phase 37: Institutional Audit - Min sample requirement for statistical validity
        if historical_devs.len() >= 500 {
            let count_lower = historical_devs.iter().filter(|&&d| d < current_dev).count();
            deviation_percentile =
                Some((count_lower as f64 / historical_devs.len() as f64) * 100.0);
        }
    }

    // --- Trend Age Calculation ---
    let mut trend_age = 1;
    if last_idx > 0 {
        let current_trend = trend_status.clone();
        for i in (0..last_idx).rev() {
            let past_trend = detect_trend(
                &history.bars[0..=i],
                entry.owner_ma_days,
                rules.trend.lookback_days,
                rules.trend.flat_threshold_pct,
            );
            if matches!(
                (&current_trend, &past_trend),
                (TrendStatus::Up, TrendStatus::Up)
                    | (TrendStatus::Down, TrendStatus::Down)
                    | (TrendStatus::Flat, TrendStatus::Flat)
                    | (TrendStatus::Unknown, TrendStatus::Unknown)
            ) {
                trend_age += 1;
            } else {
                break;
            }
        }
    }

    // --- Phase 9 Physics: Gravity Strength (Slope) ---
    let mut owner_ma_slope_pct = None;
    if last_idx >= rules.trend.lookback_days {
        let past_idx = last_idx - rules.trend.lookback_days;
        if let (Some(curr_ma), Some(past_ma)) = (
            owner_ma,
            calculate_ma(&history.bars, entry.owner_ma_days, past_idx),
        ) {
            if past_ma != 0.0 {
                owner_ma_slope_pct = Some((curr_ma - past_ma) / past_ma * 100.0);
            }
        }
    }

    // --- Phase 9 Physics: Deviation Z-Score ---
    let mut dev_z_score = None;
    if let Some(om) = owner_ma {
        if let Some(std_dev) = calculate_std_dev(&history.bars, entry.owner_ma_days, last_idx, om) {
            if std_dev != 0.0 {
                dev_z_score = Some((dog_price - om) / std_dev);
            }
        }
    }

    // --- Phase 9 Physics: Curvature (Acceleration/Inflection) ---
    let mut curvature = None;
    if last_idx >= 2 * rules.trend.lookback_days {
        let p1_idx = last_idx - rules.trend.lookback_days;
        let p2_idx = last_idx - 2 * rules.trend.lookback_days;
        if let (Some(om_curr), Some(om_p1), Some(om_p2)) = (
            owner_ma,
            calculate_ma(&history.bars, entry.owner_ma_days, p1_idx),
            calculate_ma(&history.bars, entry.owner_ma_days, p2_idx),
        ) {
            let slope_current = om_curr - om_p1;
            let slope_past = om_p1 - om_p2;
            curvature = Some(slope_current - slope_past);
        }
    }

    let mut state_code = "UNKNOWN".to_string();
    let mut action_text = "データ不足または計算異常".to_string();

    if let Some(dev) = deviation_pct {
        let mut found = false;
        for (band_name, threshold) in &rules.sorted_bands {
            if dev >= *threshold {
                state_code = band_name.clone();
                // Priority Check: Individual ticker overrides
                if let Some(ref overrides) = entry.action_overrides {
                    if let Some(act) = overrides.get(band_name) {
                        action_text = act.clone();
                    } else if let Some(act) = rules.actions.get(band_name) {
                        action_text = act.clone();
                    }
                } else if let Some(act) = rules.actions.get(band_name) {
                    action_text = act.clone();
                }
                found = true;
                break;
            }
        }

        if !found {
            if let Some((lowest_band, _)) = rules.sorted_bands.last() {
                state_code = lowest_band.clone();
                if let Some(ref overrides) = entry.action_overrides {
                    if let Some(act) = overrides.get(lowest_band) {
                        action_text = act.clone();
                    } else if let Some(act) = rules.actions.get(lowest_band) {
                        action_text = act.clone();
                    }
                } else if let Some(act) = rules.actions.get(lowest_band) {
                    action_text = act.clone();
                }
            }
        }
    }

    let mut reason_code: Option<String> = None;

    // Extreme Fear Exemption: 极度恐慌时直接豁免降级，尊重均值回归抄底
    let is_extreme_fear = state_code.starts_with("fear");

    if rules.bear_mode.enabled {
        if let TrendStatus::Down = trend_status {
            let caution_days = entry.caution_ma_days.unwrap_or(200);
            let caution_ma = calculate_ma(&history.bars, caution_days, last_idx);
            let caution_ma_trend = detect_trend(
                &history.bars,
                caution_days,
                rules.trend.lookback_days,
                rules.trend.flat_threshold_pct,
            );

            // Leash Cross rule: 用稳定的绳子(leash)代替发神经的狗(dog_price)
            if caution_ma.is_some() {
                // Apply Gravity Buffer Layer and N-day Anti-Jitter Verification
                let buffer_pct = rules.bear_mode.buffer_pct.unwrap_or(0.0);
                let confirm_days = rules.bear_mode.confirm_days.unwrap_or(1);
                let confirm_threshold = rules.bear_mode.confirm_threshold.unwrap_or(1);
                let recover_days = rules.bear_mode.recover_days.unwrap_or(confirm_days);
                let recover_threshold = rules
                    .bear_mode
                    .recover_threshold
                    .unwrap_or(confirm_threshold);

                // Stateless 60-day sliding window simulation to detect structural brokenness crossovers
                let mut is_structurally_broken = false;
                let sim_start = last_idx.saturating_sub(60);

                for step_idx in sim_start..=last_idx {
                    if !is_structurally_broken {
                        // Looking for breakdown
                        let lookback_start = if step_idx >= confirm_days {
                            step_idx - confirm_days + 1
                        } else {
                            0
                        };
                        let mut breakdown_count = 0;
                        for i in lookback_start..=step_idx {
                            let h_leash = calculate_ma(&history.bars, entry.leash_ma_days, i);
                            let h_ref = h_leash.unwrap_or(history.bars[i].close);
                            let h_cma = calculate_ma(&history.bars, caution_days, i);
                            if let Some(cma_val) = h_cma {
                                let h_buffered_cma = cma_val * (1.0 - (buffer_pct / 100.0));
                                if h_ref < h_buffered_cma {
                                    breakdown_count += 1;
                                }
                            }
                        }
                        if breakdown_count >= confirm_threshold {
                            is_structurally_broken = true;
                        }
                    } else {
                        // Looking for recovery
                        let lookback_start = if step_idx >= recover_days {
                            step_idx - recover_days + 1
                        } else {
                            0
                        };
                        let mut recover_count = 0;
                        for i in lookback_start..=step_idx {
                            let h_leash = calculate_ma(&history.bars, entry.leash_ma_days, i);
                            let h_ref = h_leash.unwrap_or(history.bars[i].close);
                            let h_cma = calculate_ma(&history.bars, caution_days, i);
                            if let Some(cma_val) = h_cma {
                                let h_buffered_cma_recover = cma_val * (1.0 + (buffer_pct / 100.0));
                                if h_ref > h_buffered_cma_recover {
                                    recover_count += 1;
                                }
                            }
                        }
                        if recover_count >= recover_threshold {
                            is_structurally_broken = false;
                        }
                    }
                }

                // If confirmed breakdown AND CAUTION_MA is pointing DOWN
                if is_structurally_broken && matches!(caution_ma_trend, TrendStatus::Down) {
                    reason_code = Some(format!(
                        "[B{} < 0.97 x{}/{}]",
                        caution_days, confirm_threshold, confirm_days
                    ));
                    if is_extreme_fear {
                        state_code = "fear_downtrend".to_string();
                        action_text =
                            "【防御 (DEFEND)】：长期趋势崩坏中的恐慌。严禁伸手接飞刀 (Cash 80%+)"
                                .to_string();
                    } else {
                        state_code = "DEFEND".to_string();
                        action_text = rules.bear_mode.fallback_action.clone();
                    }
                } else {
                    let is_structurally_safe =
                        !is_structurally_broken && !matches!(caution_ma_trend, TrendStatus::Down);

                    if is_extreme_fear {
                        if !is_structurally_safe {
                            state_code = "fear_downtrend".to_string();
                            reason_code = Some(format!("[B{}↓]", caution_days));
                            action_text = "【防御 (DEFEND)】：长期趋势下降或不稳定。严禁伸手接飞刀 (Cash 80%+)".to_string();
                        }
                        // else safe, keep fear_1, reason_code stays whatever dev triggered it
                    } else {
                        reason_code = Some(format!("[C{} SAFE]", caution_days));
                        action_text = rules.bear_mode.caution_action.clone().unwrap_or_else(|| {
                            "【警戒】：长期趋势维持。定投或小幅加仓".to_string()
                        });
                        state_code = "CAUTION".to_string();
                    }
                }
            } else {
                // Fallback to DEFEND if caution MA cannot be calculated
                reason_code = Some("[NO_CMA]".to_string());
                if is_extreme_fear {
                    state_code = "fear_downtrend".to_string();
                    action_text =
                        "【防御 (DEFEND)】：长期趋势不明中的恐慌。严禁伸手接飞刀 (Cash 80%+)"
                            .to_string();
                } else {
                    state_code = "DEFEND".to_string();
                    action_text = rules.bear_mode.fallback_action.clone();
                }
            }
        }
    }

    // --- Phase 10: Confidence Calibration (Baseline Adjustments) ---
    // A heuristic based on the convergence of the physics variables
    let mut confidence_score;

    match state_code.as_str() {
        "DEFEND" | "fear_downtrend" => {
            // High confidence if actively accelerating downward and heavily abnormal
            confidence_score = 70; // Baseline for clear structural breaks
            if let Some(z) = dev_z_score {
                if z < -2.0 {
                    confidence_score += 15;
                } else if z < -1.0 {
                    confidence_score += 10;
                }
            }
            if let Some(slope) = owner_ma_slope_pct {
                if slope < -0.5 {
                    confidence_score += 10;
                }
            }
            if let Some(curv) = curvature {
                if curv < 0.0 {
                    confidence_score += 10;
                }
            }
        }
        "CAUTION" => {
            // Medium baseline, relies on physical convergence to reach high confidence
            confidence_score = 60;
            if let Some(slope) = owner_ma_slope_pct {
                if slope > 0.0 {
                    confidence_score += 20;
                }
            }
            if let Some(curv) = curvature {
                if curv > 0.0 {
                    confidence_score += 10;
                }
            }
            if let Some(z) = dev_z_score {
                if z > -1.5 {
                    confidence_score += 10;
                }
            }
        }
        state
            if state.starts_with("optimal")
                || state.starts_with("cruise")
                || state.starts_with("pullback") =>
        {
            // These are clean "system is working" states. Default shouldn't be 50.
            confidence_score = 75; // Baseline for standard upward/neutral trends
            if let Some(slope) = owner_ma_slope_pct {
                if slope > 0.5 {
                    confidence_score += 10;
                }
            }
            if let Some(curv) = curvature {
                if curv > 0.0 {
                    confidence_score += 10;
                }
            }
            if let Some(z) = dev_z_score {
                if z > -0.5 && z < 1.0 {
                    confidence_score += 10;
                }
            }
        }
        state if state.starts_with("overheat") => {
            confidence_score = 60;
            // High confidence if actively accelerating upward and statistically absurd
            if let Some(z) = dev_z_score {
                if z > 2.0 {
                    confidence_score += 20;
                } else if z > 1.0 {
                    confidence_score += 10;
                }
            }
            if let Some(slope) = owner_ma_slope_pct {
                if slope > 1.0 {
                    confidence_score += 10;
                }
            }
            if let Some(curv) = curvature {
                if curv > 0.0 {
                    confidence_score += 10;
                }
            }
        }
        state if state.starts_with("fear") => {
            // Standard fear (safe fear, buying dip). High confidence if decelerating (curvature > 0)
            confidence_score = 60;
            if let Some(curv) = curvature {
                if curv > 0.0 {
                    confidence_score += 20;
                }
            }
            if let Some(z) = dev_z_score {
                if z < -2.0 {
                    confidence_score += 15;
                }
            }
        }
        _ => {
            confidence_score = 60; // base fallback
        }
    }

    // Cap at 99
    if confidence_score > 99 {
        confidence_score = 99;
    }

    // Phase 4.2 Institutional Audit: Granular Forming Stages
    // FORMING_EARLY: < 150 trading days history
    // FORMING_LATE: < 400 trading days history
    let mut validity = RegimeValidity::Valid;
    if history.total_trading_days < 150 {
        validity = RegimeValidity::FormingEarly;
    } else if history.total_trading_days < 400 {
        validity = RegimeValidity::FormingLate;
    }

    let is_any_forming =
        validity == RegimeValidity::FormingEarly || validity == RegimeValidity::FormingLate;

    let mut snapshot = TickerSnapshot {
        symbol: entry.symbol.clone(),
        name,
        weight: entry.weight.unwrap_or(1.0),
        reason_code,
        current_date: current_bar.date,
        dog_price,
        owner_ma,
        leash_ma,
        owner_ma_slope_pct,
        dev_z_score: if is_any_forming { None } else { dev_z_score },
        curvature: if is_any_forming { None } else { curvature },
        confidence_score: confidence_score as usize,
        trend_status,
        deviation_pct,
        deviation_basis_used: format!("{:?}", entry.deviation_basis).to_lowercase(),
        state_code,
        action_text,
        is_bear_mode_active: false, // Placeholder, will be computed in main
        is_caution_mode_active: false,
        trend_age,
        owner_deviation_pct: if is_any_forming {
            None
        } else {
            owner_deviation_pct
        },
        deviation_percentile: if is_any_forming {
            None
        } else {
            deviation_percentile
        },
        validity: validity.clone(),
        history_days,
    };

    if is_any_forming {
        snapshot.state_code = if validity == RegimeValidity::FormingEarly {
            "FORMING_EARLY".to_string()
        } else {
            "FORMING_LATE".to_string()
        };
        if let Some(act) = rules.actions.get("regime_forming") {
            snapshot.action_text = act.clone();
        } else {
            snapshot.action_text = "【观察期】：引力结构尚未形成，暂不参与".to_string();
        }

        // Phase 4.2 Institutional Audit: Semantic Honesty - Downgrade deviation_basis to leash
        if let Some(lm) = leash_ma {
            if lm != 0.0 {
                snapshot.deviation_pct = Some((dog_price - lm) / lm * 100.0);
                snapshot.deviation_basis_used = "leash (formation override)".to_string();
            }
        }
    }

    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BearModeConfig, RulesConfig, TrendConfig};
    use chrono::NaiveDate;
    use std::collections::{BTreeMap, HashMap};

    fn make_test_bars(prices: &[f64]) -> Vec<DailyBar> {
        let mut bars = Vec::new();
        let mut d = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        for &p in prices {
            bars.push(DailyBar {
                date: d,
                close: p,
                volume: Some(100.0),
            });
            d = d.succ_opt().unwrap();
        }
        bars
    }

    #[test]
    fn test_detect_trend() {
        // Continuous upward slope
        let prices: Vec<f64> = (1..=30).map(|x| x as f64).collect();
        let bars = make_test_bars(&prices);

        let status = detect_trend(&bars, 5, 10, 0.5);
        assert_eq!(status, TrendStatus::Up);

        // Continuous downward slope
        let prices_down: Vec<f64> = (1..=30).rev().map(|x| x as f64).collect();
        let bars_down = make_test_bars(&prices_down);
        let status_down = detect_trend(&bars_down, 5, 10, 0.5);
        assert_eq!(status_down, TrendStatus::Down);

        // Flat market
        let prices_flat: Vec<f64> = vec![100.0; 30];
        let bars_flat = make_test_bars(&prices_flat);
        let status_flat = detect_trend(&bars_flat, 5, 10, 0.5);
        assert_eq!(status_flat, TrendStatus::Flat);
    }

    #[test]
    fn test_regime_validity() {
        let rules = ParsedRules {
            trend: TrendConfig {
                lookback_days: 20,
                flat_threshold_pct: 0.5,
            },
            sorted_bands: vec![("optimal".to_string(), -5.0)],
            actions: HashMap::new(),
            sizing_multipliers: HashMap::new(),
            bear_mode: BearModeConfig {
                enabled: false,
                fallback_action: "DEFEND".to_string(),
                caution_action: None,
                buffer_pct: Some(3.0),
                confirm_days: Some(5),
                confirm_threshold: Some(3),
                recover_days: Some(5),
                recover_threshold: Some(3),
            },
        };

        let entry = WatchlistEntry {
            symbol: "TEST".to_string(),
            name: None,
            weight: None,
            market: "US".to_string(),
            owner_ma_days: 120,
            leash_ma_days: 20,
            caution_ma_days: None,
            deviation_basis: DeviationBasis::Owner,
            enable: true,
            action_overrides: None,
            trade_enabled: Some(true),
            trade_amount: Some(100.0),
        };

        // Early Forming (< 150 days)
        let prices = vec![100.0; 100];
        let hist = TickerHistory {
            symbol: "TEST".to_string(),
            bars: make_test_bars(&prices),
            total_trading_days: 100,
            latest_quote_timestamp: None,
        };
        let snap = evaluate_snapshot(&hist, &entry, &rules);
        assert_eq!(snap.validity, RegimeValidity::FormingEarly);
        assert_eq!(snap.state_code, "FORMING_EARLY");

        // Late Forming (< 400 days)
        let prices2 = vec![100.0; 250];
        let hist2 = TickerHistory {
            symbol: "TEST".to_string(),
            bars: make_test_bars(&prices2),
            total_trading_days: 250,
            latest_quote_timestamp: None,
        };
        let snap2 = evaluate_snapshot(&hist2, &entry, &rules);
        assert_eq!(snap2.validity, RegimeValidity::FormingLate);

        // Valid (> 400 days)
        let prices3 = vec![100.0; 450];
        let hist3 = TickerHistory {
            symbol: "TEST".to_string(),
            bars: make_test_bars(&prices3),
            total_trading_days: 450,
            latest_quote_timestamp: None,
        };
        let snap3 = evaluate_snapshot(&hist3, &entry, &rules);
        assert_eq!(snap3.validity, RegimeValidity::Valid);
    }
}

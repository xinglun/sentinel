use crate::config::{DeviationBasis, ParsedRules, WatchlistEntry};
use crate::data::yahoo_provider::{DailyBar, TickerHistory};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum TrendStatus {
    Up,
    Down,
    Flat,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssetFeatures {
    pub symbol: String,
    pub date: NaiveDate,
    pub close: f64,
    pub owner_ma: Option<f64>,
    pub leash_ma: Option<f64>,
    pub deviation: Option<f64>,
    pub z_score: Option<f64>,
    pub slope: Option<f64>,
    pub curvature: Option<f64>,
    pub trend_status: TrendStatus,
    pub trend_age: usize,
    pub deviation_percentile: Option<f64>,
    pub weight: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MarketFeatures {
    pub date: NaiveDate,
    pub up_count: usize,
    pub flat_count: usize,
    pub down_count: usize,
    pub total_count: usize,
    pub up_weight: f64,
    pub flat_weight: f64,
    pub down_weight: f64,
    pub total_weight: f64,
    pub gravity_strength: f64,
    pub potential_energy: f64,
    pub dominance_margin: f64,
    pub system_confidence: f64,
    pub stability_score: f64,
    pub stability_structural: f64,
    pub stability_temporal: f64,
    pub trend_maturity: f64,
    pub universe_integrity: f64,
    pub flow_acceleration: Option<f64>, // EMA of dominance margin delta
    pub regime_age: usize,
    pub any_pullback_occurred: bool, // Roadmap: Track if market survived a pullback
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
pub fn calculate_ma_many(
    bars: &[DailyBar],
    window: usize,
    indices: std::ops::RangeInclusive<usize>,
) -> Vec<Option<f64>> {
    if window == 0 || bars.is_empty() {
        return vec![None; indices.end().saturating_sub(*indices.start()) + 1];
    }

    let start_idx = *indices.start();
    let end_idx = *indices.end();
    let mut results = Vec::with_capacity(end_idx.saturating_sub(start_idx) + 1);

    let mut current_sum: f64 = 0.0;
    let mut initialized = false;

    for i in start_idx..=end_idx {
        if i < window - 1 {
            results.push(None);
            continue;
        }

        if !initialized {
            current_sum = bars[i + 1 - window..=i].iter().map(|b| b.close).sum();
            initialized = true;
        } else if i >= window {
            current_sum = current_sum - bars[i - window].close + bars[i].close;
        }
        results.push(Some(current_sum / window as f64));
    }

    results
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

    detect_trend_from_mas(current_ma, past_ma, flat_threshold_pct)
}

fn detect_trend_from_mas(
    current_ma: Option<f64>,
    past_ma: Option<f64>,
    flat_threshold_pct: f64,
) -> TrendStatus {
    match (current_ma, past_ma) {
        (Some(curr), Some(past)) => {
            if past == 0.0 {
                return TrendStatus::Unknown;
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

impl AssetFeatures {
    pub fn compute(
        history: &TickerHistory<'_>,
        entry: &WatchlistEntry,
        rules: &ParsedRules,
    ) -> Self {
        let last_idx = history.bars.len().saturating_sub(1);
        let current_bar = &history.bars[last_idx];
        let close = current_bar.close;

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

        let deviation = basis_val.and_then(|b| {
            if b != 0.0 {
                Some((close - b) / b * 100.0)
            } else {
                None
            }
        });

        let mut slope = None;
        if last_idx >= rules.trend.lookback_days {
            let past_idx = last_idx - rules.trend.lookback_days;
            if let (Some(curr_ma), Some(past_ma)) = (
                owner_ma,
                calculate_ma(&history.bars, entry.owner_ma_days, past_idx),
            ) {
                if past_ma != 0.0 {
                    slope = Some((curr_ma - past_ma) / past_ma * 100.0);
                }
            }
        }

        let mut z_score = None;
        if let Some(om) = owner_ma {
            if let Some(std_dev) =
                calculate_std_dev(&history.bars, entry.owner_ma_days, last_idx, om)
            {
                if std_dev != 0.0 {
                    z_score = Some((close - om) / std_dev);
                }
            }
        }

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

        let mut trend_age = 1;
        if last_idx > rules.trend.lookback_days {
            // OPTIMIZATION: Use pre-calculated MAs for the whole search range
            let search_start = last_idx.saturating_sub(500); // Caps search to 500 bars for performance
            let mas = calculate_ma_many(
                &history.bars,
                entry.owner_ma_days,
                search_start.saturating_sub(rules.trend.lookback_days)..=last_idx,
            );

            let current_ma_opt = mas.last().cloned().flatten();
            let lookback_idx = mas.len().saturating_sub(1 + rules.trend.lookback_days);
            let prev_ma_opt = mas.get(lookback_idx).cloned().flatten();
            let current_trend =
                detect_trend_from_mas(current_ma_opt, prev_ma_opt, rules.trend.flat_threshold_pct);

            for i in (1..mas.len()).rev() {
                let c_idx = i;
                if c_idx < rules.trend.lookback_days {
                    break;
                }
                let p_idx = c_idx - rules.trend.lookback_days;

                let past_trend =
                    detect_trend_from_mas(mas[c_idx], mas[p_idx], rules.trend.flat_threshold_pct);
                if past_trend == current_trend {
                    trend_age += 1;
                } else {
                    break;
                }
            }
        }

        let mut deviation_percentile = None;
        if let Some(curr_dev) = owner_ma.and_then(|om| {
            if om != 0.0 {
                Some((close - om) / om * 100.0)
            } else {
                None
            }
        }) {
            let lookback_bars = 1260; // 5 years
            let start_sim = last_idx.saturating_sub(lookback_bars);

            // OPTIMIZATION: Use bulk MA calculation O(N) instead of O(N*W)
            let mas = calculate_ma_many(&history.bars, entry.owner_ma_days, start_sim..=last_idx);
            let mut historical_devs = Vec::with_capacity(mas.len());

            for (offset, ma_opt) in mas.into_iter().enumerate() {
                if let Some(ma) = ma_opt {
                    if ma != 0.0 {
                        let bar_idx = start_sim + offset;
                        historical_devs.push((history.bars[bar_idx].close - ma) / ma * 100.0);
                    }
                }
            }

            if historical_devs.len() >= 500 {
                let count_lower = historical_devs.iter().filter(|&&d| d < curr_dev).count();
                deviation_percentile =
                    Some((count_lower as f64 / historical_devs.len() as f64) * 100.0);
            }
        }

        AssetFeatures {
            symbol: entry.symbol.clone(),
            date: current_bar.date,
            close,
            owner_ma,
            leash_ma,
            deviation,
            z_score,
            slope,
            curvature,
            trend_status,
            trend_age,
            deviation_percentile,
            weight: entry.weight.unwrap_or(1.0),
        }
    }
}

impl MarketFeatures {
    pub fn compute(
        assets: &[AssetFeatures],
        regime_age: usize,
        prev_packet: Option<&crate::core::decision::DecisionPacket>,
    ) -> Self {
        let date = assets
            .first()
            .map(|a| a.date)
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let mut up_count = 0;
        let mut flat_count = 0;
        let mut down_count = 0;
        let mut up_weight = 0.0;
        let mut flat_weight = 0.0;
        let mut down_weight = 0.0;

        let mut total_strength_sum = 0.0;
        let mut weight_for_strength = 0.0;
        let mut total_potential_sum = 0.0;
        let mut weight_for_potential = 0.0;
        let mut trend_alloc_weight = 0.0;
        let mut reversion_alloc_weight = 0.0;

        for s in assets {
            match s.trend_status {
                TrendStatus::Up => {
                    up_count += 1;
                    up_weight += s.weight;
                }
                TrendStatus::Flat => {
                    flat_count += 1;
                    flat_weight += s.weight;
                }
                TrendStatus::Down | TrendStatus::Unknown => {
                    down_count += 1;
                    down_weight += s.weight;
                }
            }

            if let Some(strength) = s.slope {
                total_strength_sum += strength * s.weight;
                weight_for_strength += s.weight;
            }
            if let Some(z) = s.z_score {
                total_potential_sum += z.abs() * s.weight;
                weight_for_potential += s.weight;

                if s.trend_status == TrendStatus::Up {
                    trend_alloc_weight += s.weight;
                } else if s.trend_status == TrendStatus::Flat || s.trend_status == TrendStatus::Down
                {
                    reversion_alloc_weight += s.weight * (z.abs() / 2.0).min(1.0);
                }
            }
        }

        let total_count = up_count + flat_count + down_count;
        let total_weight = up_weight + flat_weight + down_weight;
        let total_weight_safe = if total_weight <= 0.0 {
            1.0
        } else {
            total_weight
        };

        let gravity_strength = if weight_for_strength > 0.0 {
            total_strength_sum / weight_for_strength
        } else {
            0.0
        };
        let potential_energy = if weight_for_potential > 0.0 {
            total_potential_sum / weight_for_potential
        } else {
            0.0
        };

        let dominance_margin = (trend_alloc_weight - reversion_alloc_weight) / total_weight_safe;

        let conf_trend_alloc = (trend_alloc_weight / total_weight_safe * 50.0).clamp(0.0, 50.0);
        let conf_inverse_potential = (1.0 / (1.0 + potential_energy) * 50.0).clamp(0.0, 50.0);
        let system_confidence = (conf_trend_alloc + conf_inverse_potential).clamp(0.0, 100.0);

        let universe_integrity = if assets.is_empty() {
            0.0
        } else {
            total_count as f64 / assets.len() as f64
        };
        let trend_maturity = (regime_age as f64 / 40.0).min(1.0);
        let stability_structural = conf_inverse_potential;
        let stability_temporal = trend_maturity;
        let stability_score = (stability_structural / 50.0) * stability_temporal;

        let mut flow_acceleration = None;
        if let Some(prev) = prev_packet {
            let pm = prev.market_features.dominance_margin;
            let today_accel = dominance_margin - pm;
            let ema_accel = match prev.market_features.flow_acceleration {
                Some(prev_ema) => {
                    let alpha = 2.0 / (5.0 + 1.0);
                    alpha * today_accel + (1.0 - alpha) * prev_ema
                }
                None => today_accel,
            };
            flow_acceleration = Some(ema_accel);
        }

        let mut any_pullback_occurred = prev_packet
            .map(|p| p.market_features.any_pullback_occurred)
            .unwrap_or(false);
        if !any_pullback_occurred {
            // Heuristic: If confidence is high but many assets are in short-term dip, call it a pullback
            let pullback_proxy = assets
                .iter()
                .filter(|s| s.trend_status == TrendStatus::Up && s.z_score.unwrap_or(0.0) < -1.0)
                .count();
            if pullback_proxy >= up_count / 2 && up_count > 0 {
                any_pullback_occurred = true;
            }
        }

        MarketFeatures {
            date,
            up_count,
            flat_count,
            down_count,
            total_count,
            up_weight,
            flat_weight,
            down_weight,
            total_weight,
            gravity_strength,
            potential_energy,
            dominance_margin,
            system_confidence,
            stability_score,
            stability_structural,
            stability_temporal: stability_temporal * 100.0,
            trend_maturity,
            universe_integrity,
            flow_acceleration,
            regime_age,
            any_pullback_occurred,
        }
    }

    pub fn recalibrate(&mut self, new_age: usize) {
        self.regime_age = new_age;
        self.trend_maturity = (new_age as f64 / 40.0).min(1.0);
        self.stability_temporal = self.trend_maturity * 100.0;
        self.stability_score = (self.stability_structural / 50.0) * self.trend_maturity;
    }
}

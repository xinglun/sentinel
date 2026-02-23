use crate::config::{DeviationBasis, ParsedRules, WatchlistEntry};
use crate::fetcher::{DailyBar, TickerHistory};
use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub enum TrendStatus {
    Up,
    Down,
    Flat,
    Unknown,
}

#[derive(Debug, Serialize, Clone)]
pub struct TickerSnapshot {
    pub symbol: String,
    pub name: String,
    pub current_date: NaiveDate,
    pub dog_price: f64,
    pub owner_ma: Option<f64>,
    pub leash_ma: Option<f64>,
    pub trend_status: TrendStatus,
    pub deviation_pct: Option<f64>,
    pub deviation_basis_used: String,
    pub state_code: String,
    pub action_text: String,
    pub is_bear_mode_active: bool,
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

pub fn detect_trend(bars: &[DailyBar], ma_days: usize, lookback: usize, flat_threshold_pct: f64) -> TrendStatus {
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
                return TrendStatus::Unknown; // avoid division by zero
            }
            let change_pct = (curr - past) / past * 100.0;
            if change_pct > flat_threshold_pct {
                TrendStatus::Up
            } else if change_pct < -flat_threshold_pct {
                TrendStatus::Down
            } else {
                TrendStatus::Flat
            }
        },
        _ => TrendStatus::Unknown,
    }
}

pub fn evaluate_snapshot(history: &TickerHistory, entry: &WatchlistEntry, rules: &ParsedRules) -> TickerSnapshot {
    let name = entry.name.clone().unwrap_or_else(|| entry.symbol.clone());
    
    if history.bars.is_empty() {
        return TickerSnapshot {
            symbol: entry.symbol.clone(),
            name,
            current_date: chrono::Local::now().date_naive(),
            dog_price: 0.0,
            owner_ma: None,
            leash_ma: None,
            trend_status: TrendStatus::Unknown,
            deviation_pct: None,
            deviation_basis_used: format!("{:?}", entry.deviation_basis).to_lowercase(),
            state_code: "ERROR".to_string(),
            action_text: "No data found".to_string(),
            is_bear_mode_active: false,
        };
    }

    let last_idx = history.bars.len() - 1;
    let current_bar = &history.bars[last_idx];
    let dog_price = current_bar.close;
    
    let owner_ma = calculate_ma(&history.bars, entry.owner_ma_days, last_idx);
    let leash_ma = calculate_ma(&history.bars, entry.leash_ma_days, last_idx);
    
    let trend_status = detect_trend(&history.bars, entry.owner_ma_days, rules.trend.lookback_days, rules.trend.flat_threshold_pct);
    
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

    let mut state_code = "UNKNOWN".to_string();
    let mut action_text = "数据不足或计算异常".to_string();
    
    if let Some(dev) = deviation_pct {
        let mut found = false;
        for (band_name, threshold) in &rules.sorted_bands {
            if dev >= *threshold {
                state_code = band_name.clone();
                // Check for ticker-specific override first
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
    
    let mut is_bear_mode_active = false;
    if rules.bear_mode.enabled {
        if let TrendStatus::Down = trend_status {
            is_bear_mode_active = true;
            action_text = rules.bear_mode.fallback_action.clone();
            state_code = "DEFEND".to_string(); 
        }
    }

    TickerSnapshot {
        symbol: entry.symbol.clone(),
        name,
        current_date: current_bar.date,
        dog_price,
        owner_ma,
        leash_ma,
        trend_status,
        deviation_pct,
        deviation_basis_used: format!("{:?}", entry.deviation_basis).to_lowercase(),
        state_code,
        action_text,
        is_bear_mode_active,
    }
}

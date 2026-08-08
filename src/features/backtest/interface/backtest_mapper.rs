use crate::features::backtest::application::model::{
    BacktestRules, BacktestTickerHistory, BacktestWatchlistEntry,
};
use crate::features::radar::domain::rules::{ParsedRules, WatchlistEntry};
use crate::features::shared::domain::market_data::TickerHistory;
use std::collections::HashMap;

/// Radar / config 由来の入力を Backtest application DTO に変換する。
pub(crate) fn map_histories_to_backtest(
    histories: &HashMap<String, TickerHistory<'static>>,
) -> HashMap<String, BacktestTickerHistory<'static>> {
    histories
        .iter()
        .map(|(symbol, history)| {
            (
                symbol.clone(),
                BacktestTickerHistory {
                    symbol: history.symbol.clone(),
                    bars: history.bars.clone(),
                    total_trading_days: history.total_trading_days,
                },
            )
        })
        .collect()
}

/// Radar watchlist を Backtest application DTO に変換する。
pub(crate) fn map_watchlist_to_backtest(
    watchlist: &[WatchlistEntry],
) -> Vec<BacktestWatchlistEntry> {
    watchlist
        .iter()
        .map(|entry| BacktestWatchlistEntry {
            symbol: entry.symbol.clone(),
            enable: entry.enable,
        })
        .collect()
}

/// Backtest simulation が必要とする最小 rule DTO を作る。
pub(crate) fn map_rules_to_backtest(parsed_rules: &ParsedRules) -> BacktestRules {
    let optimal_threshold = parsed_rules
        .sorted_bands
        .iter()
        .find(|(name, _)| name.to_lowercase().contains("optimal"))
        .map(|(_, threshold)| *threshold)
        .unwrap_or(f64::MAX);
    BacktestRules { optimal_threshold }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::shared::domain::market_data::DailyBar;
    use chrono::NaiveDate;
    use std::borrow::Cow;

    #[test]
    fn maps_backtest_interface_histories_without_radar_leakage() {
        let mut histories = HashMap::new();
        histories.insert(
            "NVDA".to_string(),
            TickerHistory {
                symbol: "NVDA".to_string(),
                bars: Cow::Owned(vec![DailyBar {
                    date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
                    open: None,
                    high: None,
                    low: None,
                    close: 100.0,
                    volume: Some(10.0),
                }]),
                total_trading_days: 252,
                latest_quote_timestamp: Some(123),
            },
        );

        let mapped = map_histories_to_backtest(&histories);

        let history = mapped.get("NVDA").unwrap();
        assert_eq!(history.symbol, "NVDA");
        assert_eq!(history.total_trading_days, 252);
        assert_eq!(history.bars.len(), 1);
        assert_eq!(history.bars[0].close, 100.0);
    }

    #[test]
    fn maps_watchlist_and_rules_to_backtest_dtos() {
        let watchlist = vec![WatchlistEntry {
            symbol: "MSFT".to_string(),
            enable: true,
            ..Default::default()
        }];
        let rules = ParsedRules {
            sorted_bands: vec![("optimal".to_string(), 1.5), ("other".to_string(), 9.0)],
            ..Default::default()
        };

        let mapped_watchlist = map_watchlist_to_backtest(&watchlist);
        let mapped_rules = map_rules_to_backtest(&rules);

        assert_eq!(mapped_watchlist.len(), 1);
        assert_eq!(mapped_watchlist[0].symbol, "MSFT");
        assert!(mapped_watchlist[0].enable);
        assert_eq!(mapped_rules.optimal_threshold, 1.5);
    }
}

use chrono::NaiveDate;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct DailyBar {
    pub date: NaiveDate,
    pub close: f64,
    pub volume: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TickerHistory<'a> {
    pub symbol: String,
    pub bars: Cow<'a, [DailyBar]>,
    // IPO/初取引日以降の推定累計取引日数
    pub total_trading_days: usize,
    pub latest_quote_timestamp: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_history_fields_remain_constructible() {
        let bar = DailyBar {
            date: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            close: 241.5,
            volume: Some(12_345_678.0),
        };
        let history = TickerHistory {
            symbol: "TSLA".to_string(),
            bars: Cow::Owned(vec![bar.clone()]),
            total_trading_days: 1_200,
            latest_quote_timestamp: Some(1_725_000_000),
        };

        assert_eq!(history.symbol, "TSLA");
        assert_eq!(history.bars.len(), 1);
        assert_eq!(history.total_trading_days, 1_200);
        assert_eq!(history.latest_quote_timestamp, Some(1_725_000_000));
        assert_eq!(history.bars[0].date, bar.date);
    }
}

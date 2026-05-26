use chrono::NaiveDate;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct DailyBar {
    pub date: NaiveDate,
    pub close: f64,
    #[allow(dead_code)]
    pub volume: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TickerHistory<'a> {
    #[allow(dead_code)]
    pub symbol: String,
    pub bars: Cow<'a, [DailyBar]>,
    // IPO/初取引日以降の推定累計取引日数
    pub total_trading_days: usize,
    #[allow(dead_code)]
    pub latest_quote_timestamp: Option<i64>,
}

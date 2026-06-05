use std::borrow::Cow;

use stock_sentinel::config::AppConfig;
use stock_sentinel::features::backtest::interface::backtest::run_backtest;
use stock_sentinel::features::radar::application::provider::{
    DailyBar, MarketDataProvider, TickerHistory,
};
use time::OffsetDateTime;

struct OutOfRangeProvider;

#[async_trait::async_trait]
impl MarketDataProvider for OutOfRangeProvider {
    async fn fetch_history(
        &self,
        symbol: &str,
        _start_date: Option<OffsetDateTime>,
        _end_date: Option<OffsetDateTime>,
    ) -> anyhow::Result<TickerHistory<'static>> {
        Ok(TickerHistory {
            symbol: symbol.to_string(),
            bars: Cow::Owned(vec![DailyBar {
                date: chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                close: 100.0,
                volume: None,
            }]),
            total_trading_days: 1,
            latest_quote_timestamp: None,
        })
    }
}

#[tokio::test]
async fn backtest_rejects_empty_simulation_dates_without_rendering_nan_report() -> anyhow::Result<()>
{
    let config: AppConfig = toml::from_str(
        r#"
version = 1
provider = "yahoo"

[[watchlist]]
symbol = "SPY"
market = "US"
owner_ma_days = 120
leash_ma_days = 20
deviation_basis = "owner"
enable = true

[output]
save_to = "./target/test-output"
timezone = "Asia/Tokyo"
format = "markdown"

[rules.trend]
lookback_days = 20
flat_threshold_pct = 0.5

[rules.deviation_bands]
overheat_2 = 30.0
optimal = -5.0

[rules.actions]
overheat_2 = "stop"
optimal = "buy"
fear = "fear"

[rules.market_state_engine]
continuity_threshold = 2
stability_threshold = 5.5
"#,
    )?;

    let error = run_backtest(&config, &OutOfRangeProvider, "2024-01-01", "2024-01-31")
        .await
        .expect_err("empty simulation dates should be rejected");

    assert!(error.to_string().contains("No simulation dates found"));
    Ok(())
}

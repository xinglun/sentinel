use std::borrow::Cow;

use std::sync::{Arc, Mutex};
use stock_sentinel::config::AppConfig;
use stock_sentinel::features::backtest::interface::backtest::{
    run_backtest, run_backtest_with_outcome_to,
};
use stock_sentinel::features::radar::application::provider::{
    DailyBar, MarketDataProvider, TickerHistory,
};
use time::OffsetDateTime;

struct OutOfRangeProvider;

struct RecordingProvider {
    end_date: Arc<Mutex<Option<OffsetDateTime>>>,
}

#[async_trait::async_trait]
impl MarketDataProvider for RecordingProvider {
    async fn fetch_history(
        &self,
        symbol: &str,
        _start_date: Option<OffsetDateTime>,
        end_date: Option<OffsetDateTime>,
    ) -> anyhow::Result<TickerHistory<'static>> {
        *self.end_date.lock().unwrap() = end_date;
        Ok(TickerHistory {
            symbol: symbol.to_string(),
            bars: Cow::Owned(Vec::new()),
            total_trading_days: 0,
            latest_quote_timestamp: None,
        })
    }
}

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
                open: None,
                high: None,
                low: None,
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

#[tokio::test]
async fn backtest_fetches_through_outcome_window_without_extending_decision_dates(
) -> anyhow::Result<()> {
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
    let end_date = Arc::new(Mutex::new(None));
    let provider = RecordingProvider {
        end_date: Arc::clone(&end_date),
    };

    let error = run_backtest_with_outcome_to(
        &config,
        &provider,
        "2024-01-01",
        "2024-01-05",
        Some("2024-01-25"),
    )
    .await
    .expect_err("empty history should be rejected after the fetch boundary is checked");

    assert!(error.to_string().contains("No simulation dates found"));
    let fetched_end = end_date.lock().unwrap().expect("provider end date");
    assert_eq!(
        fetched_end.date(),
        time::Date::from_calendar_date(2024, time::Month::January, 25).unwrap()
    );
    Ok(())
}

#[tokio::test]
async fn backtest_rejects_outcome_window_before_decision_window() -> anyhow::Result<()> {
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

    let error = run_backtest_with_outcome_to(
        &config,
        &OutOfRangeProvider,
        "2024-01-01",
        "2024-01-20",
        Some("2024-01-19"),
    )
    .await
    .expect_err("invalid outcome window should fail closed");

    assert!(error.to_string().contains("cannot precede decision window"));
    Ok(())
}

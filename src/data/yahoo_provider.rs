use anyhow::{anyhow, Result};
use chrono::{NaiveDate, TimeZone, Utc};
use std::time::Duration;
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::time::sleep;
use yahoo_finance_api as yahoo;

#[derive(Debug, Clone)]
pub struct DailyBar {
    pub date: NaiveDate,
    pub close: f64,
    #[allow(dead_code)]
    pub volume: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TickerHistory {
    #[allow(dead_code)]
    pub symbol: String,
    pub bars: Vec<DailyBar>,
    // The estimated total trading days since IPO/First Trade Date
    pub total_trading_days: usize,
    pub latest_quote_timestamp: Option<i64>,
}

pub async fn fetch_history(
    symbol: &str,
    start_date: Option<OffsetDateTime>,
    end_date: Option<OffsetDateTime>,
) -> Result<TickerHistory> {
    let provider =
        yahoo::YahooConnector::new().map_err(|e| anyhow!("Yahoo connector failed: {}", e))?;

    let mut retries = 3;
    let mut delay = Duration::from_secs(2);

    loop {
        match fetch_once(&provider, symbol, start_date, end_date).await {
            Ok(history) => return Ok(history),
            Err(e) => {
                if retries == 0 {
                    return Err(anyhow!(
                        "Failed to fetch {} after multiple retries: {}",
                        symbol,
                        e
                    ));
                }
                println!(
                    "[WARNING] Failed to fetch {} ({}). Retrying in {} seconds...",
                    symbol,
                    e,
                    delay.as_secs()
                );
                sleep(delay).await;
                retries -= 1;
                delay *= 2;
            }
        }
    }
}

async fn fetch_once(
    provider: &yahoo::YahooConnector,
    symbol: &str,
    start_dt: Option<OffsetDateTime>,
    end_dt: Option<OffsetDateTime>,
) -> Result<TickerHistory> {
    let end = end_dt.unwrap_or_else(OffsetDateTime::now_utc);
    let start = start_dt.unwrap_or_else(|| end - TimeDuration::days(365 * 2));

    let response = match provider.get_quote_history(symbol, start, end).await {
        Ok(res) => res,
        Err(_) => {
            // Fallback for new listings or short-history symbols like "FIG"
            provider
                .get_quote_range(symbol, "1d", "max")
                .await
                .map_err(|e| anyhow!("Yahoo API error (fallback max): {}", e))?
        }
    };

    let quotes = response
        .quotes()
        .map_err(|e| anyhow!("Failed to parse quotes: {}", e))?;

    let latest_quote_timestamp = quotes.last().map(|q| q.timestamp as i64);
    let end_ts = end.unix_timestamp();
    let mut bars: Vec<DailyBar> = quotes
        .into_iter()
        .filter_map(|q| {
            let timestamp = q.timestamp as i64;

            // Cut data outside requested range or future dates
            if timestamp > end_ts {
                return None;
            }

            let dt = Utc.timestamp_opt(timestamp, 0).single()?;
            let d = dt.naive_utc().date();

            Some(DailyBar {
                date: d,
                close: q.close,
                volume: Some(q.volume as f64),
            })
        })
        .collect();

    bars.sort_by(|a, b| a.date.cmp(&b.date));

    // Calculate accurate total_trading_days using firstTradeDate from metadata
    let mut total_trading_days = bars.len();
    if let Ok(meta) = response.metadata() {
        if let Some(first_trade) = meta.first_trade_date {
            let now = OffsetDateTime::now_utc().unix_timestamp();
            let elapsed_seconds = now - first_trade as i64;
            if elapsed_seconds > 0 {
                let elapsed_days = elapsed_seconds / 86400; // calendar days
                                                            // Approximate trading days (minus weekends/holidays) = elapsed_days * 252 / 365
                let estimated_trading_days = (elapsed_days as f64 * (252.0 / 365.25)) as usize;
                // Use the highest of the two (in case the fetch downloaded more than estimated)
                total_trading_days = std::cmp::max(total_trading_days, estimated_trading_days);
            }
        }
    }

    Ok(TickerHistory {
        symbol: symbol.to_string(),
        bars,
        total_trading_days,
        latest_quote_timestamp,
    })
}

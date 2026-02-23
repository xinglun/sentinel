use anyhow::{anyhow, Result};
use chrono::{TimeZone, Utc, NaiveDate};
use time::{OffsetDateTime, Duration as TimeDuration};
use yahoo_finance_api as yahoo;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct DailyBar {
    pub date: NaiveDate,
    pub close: f64,
    pub volume: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TickerHistory {
    pub symbol: String,
    pub bars: Vec<DailyBar>,
}

pub async fn fetch_history(symbol: &str) -> Result<TickerHistory> {
    let provider = yahoo::YahooConnector::new()
        .map_err(|e| anyhow!("Yahoo connector failed: {}", e))?;
    
    let mut retries = 3;
    let mut delay = Duration::from_secs(2);
    
    loop {
        match fetch_once(&provider, symbol).await {
            Ok(history) => return Ok(history),
            Err(e) => {
                if retries == 0 {
                    return Err(anyhow!("Failed to fetch {} after retries: {}", symbol, e));
                }
                println!("[WARN] Failed to fetch {} ({}). Retrying in {}s...", symbol, e, delay.as_secs());
                sleep(delay).await;
                retries -= 1;
                delay *= 2; 
            }
        }
    }
}

async fn fetch_once(provider: &yahoo::YahooConnector, symbol: &str) -> Result<TickerHistory> {
    let end = OffsetDateTime::now_utc();
    let start = end - TimeDuration::days(365 * 2);

    let response = provider.get_quote_history(symbol, start, end).await
        .map_err(|e| anyhow!("Yahoo API error: {}", e))?;
        
    let quotes = response.quotes()
        .map_err(|e| anyhow!("Failed to parse quotes: {}", e))?;
        
    let mut bars: Vec<DailyBar> = quotes.into_iter().filter_map(|q| {
        let timestamp = q.timestamp as i64;
        let dt = Utc.timestamp_opt(timestamp, 0).single()?;
        Some(DailyBar {
            date: dt.naive_utc().date(),
            close: q.close,
            volume: Some(q.volume as f64),
        })
    }).collect();
    
    bars.sort_by(|a, b| a.date.cmp(&b.date));
    
    Ok(TickerHistory {
        symbol: symbol.to_string(),
        bars,
    })
}

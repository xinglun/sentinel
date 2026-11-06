use anyhow::{anyhow, Result};
use chrono::{TimeZone, Utc};
use std::borrow::Cow;
use std::time::Duration;
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::time::sleep;
use yahoo_finance_api as yahoo;

use crate::features::radar::application::provider::{DailyBar, MarketDataProvider, TickerHistory};

pub struct YahooProvider;

#[async_trait::async_trait]
impl MarketDataProvider for YahooProvider {
    async fn fetch_history(
        &self,
        symbol: &str,
        start_date: Option<OffsetDateTime>,
        end_date: Option<OffsetDateTime>,
    ) -> Result<TickerHistory<'static>> {
        fetch_history(symbol, start_date, end_date).await
    }
}

/// 単一 fetch 試行に適用する上限時間。
///
/// `yahoo_finance_api` の `YahooConnectorBuilder::timeout` は内部で新規の
/// `Client::builder()` を使い直すため実際には反映されない（crate 側の既知の
/// 挙動）。タイムアウトなしの reqwest 既定値に任せると、接続先からの応答が
/// 返らない障害（CI ランナーの共有 IP が暗黙にブロックされる等）でリトライへ
/// 到達できないまま無期限に待ち続けるため、自前で `tokio::time::timeout` を
/// かけて確実にリトライループへ戻す。
const PER_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(20);

pub async fn fetch_history(
    symbol: &str,
    start_date: Option<OffsetDateTime>,
    end_date: Option<OffsetDateTime>,
) -> Result<TickerHistory<'static>> {
    let provider =
        yahoo::YahooConnector::new().map_err(|e| anyhow!("Yahoo connector failed: {}", e))?;

    let mut retries = 3;
    let mut delay = Duration::from_secs(2);

    loop {
        let attempt_result = match tokio::time::timeout(
            PER_ATTEMPT_TIMEOUT,
            fetch_once(&provider, symbol, start_date, end_date),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(anyhow!(
                "Yahoo API request for {} timed out after {} seconds",
                symbol,
                PER_ATTEMPT_TIMEOUT.as_secs()
            )),
        };

        match attempt_result {
            Ok(history) => return Ok(history),
            Err(e) => {
                if retries == 0 {
                    eprintln!(
                        "[ERROR] Giving up on {} after exhausting retries: {}",
                        symbol, e
                    );
                    return Err(anyhow!(
                        "Failed to fetch {} after multiple retries: {}",
                        symbol,
                        e
                    ));
                }
                let sleep_duration = with_jitter(delay);
                eprintln!(
                    "[WARNING] Failed to fetch {} ({}). Retrying in {} seconds...",
                    symbol,
                    e,
                    sleep_duration.as_secs()
                );
                sleep(sleep_duration).await;
                retries -= 1;
                delay *= 2;
            }
        }
    }
}

/// 複数シンボルが同時に失敗した際のリトライ集中（thundering herd）を避けるため、
/// backoff 時間に ±30% のジッターを加える。暗号学的な乱数は不要なため、
/// 外部 crate を追加せずシステムクロックの端数から擬似乱数を導出する。
fn with_jitter(base: Duration) -> Duration {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let jitter_fraction = (nanos % 1000) as f64 / 1000.0; // 0.0..1.0
    base.mul_f64(0.85 + 0.3 * jitter_fraction) // 0.85x..1.15x
}

async fn fetch_once(
    provider: &yahoo::YahooConnector,
    symbol: &str,
    start_dt: Option<OffsetDateTime>,
    end_dt: Option<OffsetDateTime>,
) -> Result<TickerHistory<'static>> {
    let end = end_dt.unwrap_or_else(OffsetDateTime::now_utc);
    let start = start_dt.unwrap_or_else(|| end - TimeDuration::days(365 * 2));

    let response = match provider.get_quote_history(symbol, start, end).await {
        Ok(res) => res,
        Err(primary_err) => {
            // 新規上場銘柄や "FIG" のような履歴が短い銘柄のためのフォールバック。
            // 一次取得の失敗理由は診断上重要なため、フォールバックも失敗した場合は
            // 両方のエラーを保持して伝播する（一次エラーの握り潰しを防ぐ）。
            provider
                .get_quote_range(symbol, "1d", "max")
                .await
                .map_err(|fallback_err| {
                    anyhow!(
                        "Yahoo API error (primary: {}; fallback max: {})",
                        primary_err,
                        fallback_err
                    )
                })?
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

            // 要求された範囲外または未来の日付のデータをカットする
            if timestamp > end_ts {
                return None;
            }

            let dt = Utc.timestamp_opt(timestamp, 0).single()?;
            let d = dt.naive_utc().date();

            Some(DailyBar {
                date: d,
                open: Some(q.open),
                high: Some(q.high),
                low: Some(q.low),
                close: q.close,
                volume: Some(q.volume as f64),
            })
        })
        .collect();

    bars.sort_by_key(|a| a.date);

    // メタデータの firstTradeDate を使用して正確な total_trading_days を計算する
    let mut total_trading_days = bars.len();
    if let Ok(meta) = response.metadata() {
        if let Some(first_trade) = meta.first_trade_date {
            let now = OffsetDateTime::now_utc().unix_timestamp();
            let elapsed_seconds = now - first_trade as i64;
            if elapsed_seconds > 0 {
                let elapsed_days = elapsed_seconds / 86400; // カレンダー日数
                                                            // 推定取引日数（週末・祝日を除く） = elapsed_days * 252 / 365
                let estimated_trading_days = (elapsed_days as f64 * (252.0 / 365.25)) as usize;
                // 両者のうち大きい方を使用する（フェッチしたデータが推定より多い場合に備えて）
                total_trading_days = std::cmp::max(total_trading_days, estimated_trading_days);
            }
        }
    }

    Ok(TickerHistory {
        symbol: symbol.to_string(),
        bars: Cow::Owned(bars),
        total_trading_days,
        latest_quote_timestamp,
    })
}

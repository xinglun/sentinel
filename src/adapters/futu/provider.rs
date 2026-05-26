use anyhow::Result;
use async_trait::async_trait;
use chrono::DateTime;
use prost::Message;
use std::borrow::Cow;
use std::sync::Arc;
use time::OffsetDateTime;

use crate::adapters::futu::client::FutuClient;
use crate::adapters::futu::protocol::generated::qot_common::{
    KlType, QotMarket, RehabType, Security,
};
use crate::adapters::futu::protocol::generated::qot_get_history_kl::{C2s, Request, Response};
use crate::features::radar::application::provider::MarketDataProvider;
use crate::features::radar::application::provider::{DailyBar, TickerHistory};

pub struct FutuProvider {
    client: Arc<FutuClient>,
}

impl FutuProvider {
    pub fn new(client: Arc<FutuClient>) -> Self {
        Self { client }
    }

    fn parse_symbol(symbol: &str) -> Security {
        // 単純 mapping: HK. で始まる場合は HK Security、それ以外は US を default とする。
        if symbol.starts_with("HK.") || symbol.ends_with(".HK") {
            let code = symbol.replace("HK.", "").replace(".HK", "");
            Security {
                market: QotMarket::HkSecurity as i32,
                code,
            }
        } else if symbol.ends_with(".SS") {
            let code = symbol.replace(".SS", "");
            Security {
                market: QotMarket::CnshSecurity as i32,
                code,
            }
        } else if symbol.ends_with(".SZ") {
            let code = symbol.replace(".SZ", "");
            Security {
                market: QotMarket::CnszSecurity as i32,
                code,
            }
        } else {
            Security {
                market: QotMarket::UsSecurity as i32,
                code: symbol.to_string(),
            }
        }
    }
}

#[async_trait]
impl MarketDataProvider for FutuProvider {
    async fn fetch_history(
        &self,
        symbol: &str,
        start_date: Option<OffsetDateTime>,
        end_date: Option<OffsetDateTime>,
    ) -> Result<TickerHistory<'static>> {
        let security = Self::parse_symbol(symbol);

        let begin_time = match start_date {
            Some(dt) => format!(
                "{:04}-{:02}-{:02} 00:00:00",
                dt.year(),
                dt.month() as u8,
                dt.day()
            ),
            None => "1970-01-01 00:00:00".to_string(),
        };

        let end_time = match end_date {
            Some(dt) => format!(
                "{:04}-{:02}-{:02} 23:59:59",
                dt.year(),
                dt.month() as u8,
                dt.day()
            ),
            None => "2038-01-01 00:00:00".to_string(), // reasonably far future
        };

        let req = Request {
            c2s: C2s {
                rehab_type: RehabType::Forward as i32,
                kl_type: KlType::Day as i32,
                security: security.clone(),
                begin_time,
                end_time,
                max_ack_kl_num: Some(1000),
                need_kl_fields_flag: None, // return all fields
            },
        };

        let raw_res = self.client.send_request(3103, &req).await?;
        let res = Response::decode(&raw_res[..])?;

        if res.ret_type != 0 {
            anyhow::bail!("Futu HistoryKL failed: {:?}", res.ret_msg);
        }

        let s2c = res
            .s2c
            .ok_or_else(|| anyhow::anyhow!("Missing s2c payload"))?;

        // Futu API は古い順で返す。
        let mut bars = Vec::new();
        let mut latest_ts = None;
        for kline in s2c.kl_list {
            let dt = chrono::NaiveDateTime::parse_from_str(&kline.time, "%Y-%m-%d %H:%M:%S")
                .ok()
                .or_else(|| {
                    DateTime::from_timestamp(kline.timestamp.unwrap_or(0.0) as i64, 0)
                        .map(|dt| dt.naive_utc())
                })
                .unwrap_or_default();

            latest_ts = Some(dt.and_utc().timestamp());

            bars.push(DailyBar {
                date: dt.date(),
                close: kline.close_price.unwrap_or(0.0),
                volume: kline.volume.map(|v| v as f64),
            });
        }

        if bars.is_empty() {
            anyhow::bail!("Futu returned empty KLine list for {}", symbol);
        }

        let total_trading_days = bars.len();

        let bars_cow: Cow<'static, [DailyBar]> = Cow::Owned(bars);
        Ok(TickerHistory {
            symbol: symbol.to_string(),
            bars: bars_cow,
            total_trading_days,
            latest_quote_timestamp: latest_ts,
        })
    }
}

use anyhow::Result;
use time::OffsetDateTime;

use crate::data::yahoo_provider::TickerHistory;

#[async_trait::async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn fetch_history(
        &self,
        symbol: &str,
        start_date: Option<OffsetDateTime>,
        end_date: Option<OffsetDateTime>,
    ) -> Result<TickerHistory>;
}

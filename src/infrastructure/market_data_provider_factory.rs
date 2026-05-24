use crate::adapters::futu::client::FutuClient;
use crate::adapters::futu::provider::FutuProvider;
use crate::adapters::yahoo_provider::YahooProvider;
use crate::application::provider::MarketDataProvider;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarketDataProviderKind {
    Yahoo,
    Futu,
}

pub async fn build_market_data_provider(
    kind: MarketDataProviderKind,
    futu_addr: &str,
) -> Arc<dyn MarketDataProvider> {
    match kind {
        MarketDataProviderKind::Futu => match FutuClient::connect(futu_addr).await {
            Ok(client) => Arc::new(FutuProvider::new(Arc::new(client))),
            Err(_) => Arc::new(YahooProvider),
        },
        MarketDataProviderKind::Yahoo => Arc::new(YahooProvider),
    }
}

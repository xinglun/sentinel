use yahoo_finance_api as yahoo;

#[tokio::main]
async fn main() {
    let provider = yahoo::YahooConnector::new().unwrap();
    let response = provider.get_quote_range("SPY", "1d", "max").await.unwrap();
    
    // YResponse has metadata() method which returns YMetaData.
    if let Ok(meta) = response.metadata() {
        println!("firstTradeDate: {:?}", meta.first_trade_date);
    }
}

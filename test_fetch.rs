use yahoo_finance_api as yahoo;
#[tokio::main]
async fn main() {
    let provider = yahoo::YahooConnector::new().unwrap();
    let response = provider.get_quote_range("SPY", "1d", "max").await.unwrap();
    println!("meta: {:?}", response.metadata());
}

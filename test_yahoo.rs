use yahoo_finance_api as yahoo;
use time::{OffsetDateTime, Duration};

#[tokio::main]
async fn main() {
    let provider = yahoo::YahooConnector::new().unwrap();
    let end = OffsetDateTime::now_utc();
    let start = end - Duration::days(365 * 10);
    
    println!("Testing FIG history...");
    match provider.get_quote_history("FIG", start, end).await {
        Ok(res) => println!("History success: {} quotes", res.quotes().unwrap().len()),
        Err(e) => println!("History failed: {}", e),
    }

    println!("Testing FIG range max...");
    match provider.get_quote_range("FIG", "1d", "max").await {
        Ok(res) => println!("Range max success: {} quotes", res.quotes().unwrap().len()),
        Err(e) => println!("Range max failed: {}", e),
    }
}

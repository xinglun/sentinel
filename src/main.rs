use anyhow::Result;
use stock_sentinel::cli;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}

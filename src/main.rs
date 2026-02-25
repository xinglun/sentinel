mod adapters;
mod backtest;
mod cli;
mod config;
mod core;
mod data;
mod trade;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}

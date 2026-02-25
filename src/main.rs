mod config;
mod core;
mod data;
mod trade;
mod adapters;
mod backtest;
mod cli;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}

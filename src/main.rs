mod config;
mod engine;
mod fetcher;
mod report;
mod notify;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use crate::engine::TickerSnapshot;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🐕 Stock Sentinel initializing...");
    
    let app_config = config::AppConfig::load("config.toml")?;
    let parsed_rules = app_config.get_parsed_rules();
    
    let config_arc = Arc::new(app_config);
    let rules_arc = Arc::new(parsed_rules);
    
    let watchlist = &config_arc.watchlist;
    let enabled_count = watchlist.iter().filter(|w| w.enable).count();
    println!("📊 Fetching data for {} enabled tickers...", enabled_count);
    
    let mut snapshots = Vec::new();
    
    let fetches = stream::iter(watchlist.iter().filter(|w| w.enable))
        .map(|entry| {
            let rules_ref = Arc::clone(&rules_arc);
            async move {
                let symbol = &entry.symbol;
                match fetcher::fetch_history(symbol).await {
                    Ok(history) => {
                        let snapshot = engine::evaluate_snapshot(&history, entry, &rules_ref);
                        Some(snapshot)
                    },
                    Err(e) => {
                        println!("[ERROR] Could not process {}: {}", symbol, e);
                        let err_snap = TickerSnapshot {
                            symbol: symbol.clone(),
                            name: entry.name.clone().unwrap_or_else(|| symbol.clone()),
                            current_date: chrono::Local::now().date_naive(),
                            dog_price: 0.0,
                            owner_ma: None,
                            leash_ma: None,
                            trend_status: engine::TrendStatus::Unknown,
                            deviation_pct: None,
                            deviation_basis_used: format!("{:?}", entry.deviation_basis).to_lowercase(),
                            state_code: "ERROR".to_string(),
                            action_text: format!("Fetch failed: {}", e),
                            is_bear_mode_active: false,
                        };
                        Some(err_snap)
                    }
                }
            }
        })
        .buffer_unordered(10);
        
    let mut results_map = std::collections::HashMap::new();
    let results: Vec<Option<TickerSnapshot>> = fetches.collect().await;
    
    for res in results.into_iter().flatten() {
        results_map.insert(res.symbol.clone(), res);
    }
    
    // Preserve the original order defined in config.toml
    for entry in watchlist.iter().filter(|w| w.enable) {
        if let Some(snap) = results_map.remove(&entry.symbol) {
            snapshots.push(snap);
        }
    }
    
    if !snapshots.is_empty() {
        let report_result = report::generate_reports(&config_arc, &snapshots)?;
        println!("✅ Reports generated in: {}", config_arc.output.save_to);
        
        if let Some(ref tg_cfg) = config_arc.telegram {
            if tg_cfg.enabled {
                println!("📤 Pushing report to Telegram...");
                if let Err(e) = notify::send_telegram_message(tg_cfg, &report_result.telegram_html).await {
                    println!("❌ Failed to send Telegram message: {}", e);
                }
            }
        }
        
    } else {
        println!("⚠️ No valid data to report.");
    }
    
    Ok(())
}

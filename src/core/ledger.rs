use anyhow::Result;
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub date: NaiveDate,
    pub timestamp: String,
    pub symbol: String,
    pub side: String,
    pub qty: f64,
    pub price: f64,
    pub signal: String,
}

pub struct Ledger {
    file_path: PathBuf,
}

impl Ledger {
    pub fn new(save_dir: PathBuf) -> Self {
        let file_path = save_dir.join("ledger.csv");

        // Ensure header exists if file is new
        if !file_path.exists() {
            if let Some(parent) = file_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = std::fs::File::create(&file_path) {
                let _ = writeln!(file, "date,timestamp,symbol,side,qty,price,signal");
            }
        }

        Self { file_path }
    }

    pub fn record_trade(&self, record: TradeRecord) -> Result<()> {
        let mut file = OpenOptions::new().append(true).open(&self.file_path)?;

        writeln!(
            file,
            "{},{},{},{},{:.2},{:.2},{}",
            record.date,
            record.timestamp,
            record.symbol,
            record.side,
            record.qty,
            record.price,
            record.signal
        )?;
        Ok(())
    }

    pub fn has_acted_today(&self, symbol: &str, signal: &str) -> bool {
        let today = Local::now().date_naive();

        let file = match std::fs::File::open(&self.file_path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        let reader = BufReader::new(file);
        for line in reader.lines().skip(1).flatten() {
            // Skip header
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 7 {
                let date_str = parts[0];
                let sym_str = parts[2];
                let sig_str = parts[6];

                if date_str == today.to_string() && sym_str == symbol && sig_str == signal {
                    return true;
                }
            }
        }

        false
    }

    /// Get total traded value (buy + sell absolute) today to enforce budget limits.
    pub fn get_daily_traded_amount(&self) -> f64 {
        let today = Local::now().date_naive();
        let mut total = 0.0;

        let file = match std::fs::File::open(&self.file_path) {
            Ok(f) => f,
            Err(_) => return 0.0,
        };

        let reader = BufReader::new(file);
        for line in reader.lines().skip(1).flatten() {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 7 {
                let date_str = parts[0];
                if date_str == today.to_string() {
                    let qty = parts[4].parse::<f64>().unwrap_or(0.0);
                    let price = parts[5].parse::<f64>().unwrap_or(0.0);
                    total += qty * price;
                }
            }
        }
        total
    }

    /// Calculate realized P/L and current positions.
    /// Returns (Realized P/L, HashMap<Symbol, (Qty, AvgPrice)>)
    pub fn get_portfolio_stats(&self) -> (f64, std::collections::HashMap<String, (f64, f64)>) {
        let mut realized_pl = 0.0;
        let mut positions: std::collections::HashMap<String, (f64, f64)> =
            std::collections::HashMap::new();

        let file = match std::fs::File::open(&self.file_path) {
            Ok(f) => f,
            Err(_) => return (0.0, positions),
        };

        let reader = BufReader::new(file);
        for line in reader.lines().skip(1).flatten() {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 7 {
                let symbol = parts[2].to_string();
                let side = parts[3];
                let qty = parts[4].parse::<f64>().unwrap_or(0.0);
                let price = parts[5].parse::<f64>().unwrap_or(0.0);

                let entry = positions.entry(symbol).or_insert((0.0, 0.0));
                let (current_qty, current_avg) = *entry;

                if side == "BUY" {
                    let new_qty = current_qty + qty;
                    let new_avg = (current_qty * current_avg + qty * price) / new_qty;
                    *entry = (new_qty, new_avg);
                } else if side == "SELL" {
                    // Realized P/L calculation: (Sell Price - Avg Cost) * Sold Qty
                    realized_pl += (price - current_avg) * qty;
                    let new_qty = current_qty - qty;
                    if new_qty <= 0.0 {
                        *entry = (0.0, 0.0);
                    } else {
                        *entry = (new_qty, current_avg);
                    }
                }
            }
        }
        (realized_pl, positions)
    }
}

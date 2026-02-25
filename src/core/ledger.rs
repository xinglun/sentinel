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

    /// Check if we have already traded this symbol with this specific signal *today*.
    /// This prevents the bot from "spamming" orders every 60s while a signal persists.
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
                // parts[1] is timestamp
                let sym_str = parts[2];
                // parts[3] is side
                // parts[4] is qty
                // parts[5] is price
                let sig_str = parts[6];

                if date_str == today.to_string() && sym_str == symbol && sig_str == signal {
                    return true;
                }
            }
        }

        false
    }
}

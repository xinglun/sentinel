use anyhow::{anyhow, Result};
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

type PortfolioPositions = std::collections::HashMap<String, (f64, f64)>;
type PortfolioStats = (f64, PortfolioPositions);

impl Ledger {
    pub fn new(save_dir: PathBuf) -> Self {
        let file_path = save_dir.join("ledger.csv");

        // 新規 file の場合は header を作成する。
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
        if !record.qty.is_finite() || record.qty <= 0.0 {
            return Err(anyhow!("invalid ledger trade quantity: {}", record.qty));
        }
        if !record.price.is_finite() || record.price <= 0.0 {
            return Err(anyhow!("invalid ledger trade price: {}", record.price));
        }
        if record.side != "BUY" && record.side != "SELL" {
            return Err(anyhow!("invalid ledger trade side: {}", record.side));
        }

        let mut file = OpenOptions::new().append(true).open(&self.file_path)?;

        writeln!(
            file,
            "{},{},{},{},{:.8},{:.8},{}",
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
            // header を skip する。
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

    /// budget limit を適用するため、当日の約定金額合計（買い + 売りの絶対値）を返す。
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

    /// 当日取引金額を厳格に読み取り、破損した ledger 行をエラーとして返す。
    pub fn get_daily_traded_amount_checked(&self) -> Result<f64> {
        let today = Local::now().date_naive();
        let file = std::fs::File::open(&self.file_path)?;
        let reader = BufReader::new(file);
        let mut total = 0.0;

        for (line_number, line) in reader.lines().skip(1).enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 7 {
                return Err(anyhow!(
                    "invalid ledger row at line {}: expected 7 columns",
                    line_number + 2
                ));
            }
            if parts[2].trim().is_empty() || (parts[3] != "BUY" && parts[3] != "SELL") {
                return Err(anyhow!(
                    "invalid ledger identity at line {}",
                    line_number + 2
                ));
            }
            let qty = parts[4].parse::<f64>().map_err(|error| {
                anyhow!(
                    "invalid ledger quantity at line {}: {}",
                    line_number + 2,
                    error
                )
            })?;
            let price = parts[5].parse::<f64>().map_err(|error| {
                anyhow!(
                    "invalid ledger price at line {}: {}",
                    line_number + 2,
                    error
                )
            })?;
            if !qty.is_finite() || qty <= 0.0 || !price.is_finite() || price <= 0.0 {
                return Err(anyhow!(
                    "invalid ledger values at line {}: qty={}, price={}",
                    line_number + 2,
                    qty,
                    price
                ));
            }
            if parts[0] == today.to_string() {
                total += qty * price;
                if !total.is_finite() {
                    return Err(anyhow!(
                        "ledger daily amount overflow at line {}",
                        line_number + 2
                    ));
                }
            }
        }
        Ok(total)
    }

    /// 実現損益と現在 position を計算する。
    /// 戻り値は (実現損益, HashMap<Symbol, (Qty, AvgPrice)>)。
    pub fn get_portfolio_stats(&self) -> PortfolioStats {
        let mut realized_pl = 0.0;
        let mut positions: PortfolioPositions = std::collections::HashMap::new();

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
                    // 実現損益は (売却価格 - 平均原価) * 売却数量で計算する。
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

    /// 账本内容を厳格に検証しながら、実現損益と現在 position を計算する。
    pub fn get_portfolio_stats_checked(&self) -> Result<PortfolioStats> {
        let file = std::fs::File::open(&self.file_path)?;
        let reader = BufReader::new(file);
        let mut realized_pl = 0.0;
        let mut positions: PortfolioPositions = std::collections::HashMap::new();

        for (line_number, line) in reader.lines().skip(1).enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 7 {
                return Err(anyhow!(
                    "invalid ledger row at line {}: expected 7 columns",
                    line_number + 2
                ));
            }

            let symbol = parts[2].trim();
            if symbol.is_empty() {
                return Err(anyhow!("invalid ledger symbol at line {}", line_number + 2));
            }
            let side = parts[3].trim();
            if side != "BUY" && side != "SELL" {
                return Err(anyhow!(
                    "invalid ledger side at line {}: {}",
                    line_number + 2,
                    side
                ));
            }
            let qty = parts[4].parse::<f64>().map_err(|error| {
                anyhow!(
                    "invalid ledger quantity at line {}: {} ({})",
                    line_number + 2,
                    parts[4],
                    error
                )
            })?;
            let price = parts[5].parse::<f64>().map_err(|error| {
                anyhow!(
                    "invalid ledger price at line {}: {} ({})",
                    line_number + 2,
                    parts[5],
                    error
                )
            })?;
            if !qty.is_finite() || qty <= 0.0 || !price.is_finite() || price <= 0.0 {
                return Err(anyhow!(
                    "invalid ledger values at line {}: symbol={}, qty={}, price={}",
                    line_number + 2,
                    symbol,
                    qty,
                    price
                ));
            }

            let entry = positions.entry(symbol.to_string()).or_insert((0.0, 0.0));
            let (current_qty, current_avg) = *entry;
            if side == "BUY" {
                let new_qty = current_qty + qty;
                let new_avg = (current_qty * current_avg + qty * price) / new_qty;
                if !new_qty.is_finite() || !new_avg.is_finite() {
                    return Err(anyhow!(
                        "ledger position overflow at line {}",
                        line_number + 2
                    ));
                }
                *entry = (new_qty, new_avg);
            } else {
                realized_pl += (price - current_avg) * qty;
                if !realized_pl.is_finite() {
                    return Err(anyhow!(
                        "ledger realized P/L overflow at line {}",
                        line_number + 2
                    ));
                }
                let new_qty = current_qty - qty;
                if new_qty <= 0.0 {
                    *entry = (0.0, 0.0);
                } else {
                    *entry = (new_qty, current_avg);
                }
            }
        }

        Ok((realized_pl, positions))
    }
}

#[cfg(test)]
mod tests {
    use super::{Ledger, TradeRecord};
    use chrono::Local;
    use tempfile::tempdir;

    #[test]
    fn record_trade_preserves_fractional_quantity_and_price_precision() {
        let temp = tempdir().unwrap();
        let ledger = Ledger::new(temp.path().to_path_buf());
        let qty = 0.12345678;
        let price = 123.45678901;

        ledger
            .record_trade(TradeRecord {
                date: Local::now().date_naive(),
                timestamp: "10:00:00".to_string(),
                symbol: "FRACTIONAL".to_string(),
                side: "BUY".to_string(),
                qty,
                price,
                signal: "TEST".to_string(),
            })
            .unwrap();

        let (_, positions) = ledger.get_portfolio_stats();
        let (stored_qty, stored_price) = positions.get("FRACTIONAL").copied().unwrap();
        assert!((stored_qty - qty).abs() < 1e-8);
        assert!((stored_price - price).abs() < 1e-8);
        assert!((ledger.get_daily_traded_amount() - qty * price).abs() < 1e-6);
    }

    #[test]
    fn record_trade_rejects_non_finite_non_positive_values_and_unknown_side() {
        let temp = tempdir().unwrap();
        let ledger = Ledger::new(temp.path().to_path_buf());
        let base = |qty: f64, price: f64, side: &str| TradeRecord {
            date: Local::now().date_naive(),
            timestamp: "10:00:00".to_string(),
            symbol: "INVALID".to_string(),
            side: side.to_string(),
            qty,
            price,
            signal: "TEST".to_string(),
        };

        for record in [
            base(f64::NAN, 100.0, "BUY"),
            base(f64::INFINITY, 100.0, "BUY"),
            base(0.0, 100.0, "BUY"),
            base(1.0, f64::NAN, "BUY"),
            base(1.0, f64::INFINITY, "BUY"),
            base(1.0, 0.0, "BUY"),
            base(1.0, 100.0, "HOLD"),
        ] {
            assert!(ledger.record_trade(record).is_err());
        }
        assert_eq!(ledger.get_daily_traded_amount(), 0.0);
    }

    #[test]
    fn checked_portfolio_stats_rejects_corrupt_ledger_rows() {
        let temp = tempdir().unwrap();
        let ledger = Ledger::new(temp.path().to_path_buf());
        std::fs::OpenOptions::new()
            .append(true)
            .open(temp.path().join("ledger.csv"))
            .unwrap();
        std::fs::write(
            temp.path().join("ledger.csv"),
            "date,timestamp,symbol,side,qty,price,signal\n2026-08-13,10:00,BAD,BUY,not-a-number,100,TEST\n",
        )
        .unwrap();

        let error = ledger.get_portfolio_stats_checked().unwrap_err();
        assert!(error.to_string().contains("invalid ledger quantity"));
    }

    #[test]
    fn checked_daily_traded_amount_rejects_corrupt_ledger_rows() {
        let temp = tempdir().unwrap();
        let ledger = Ledger::new(temp.path().to_path_buf());
        std::fs::write(
            temp.path().join("ledger.csv"),
            "date,timestamp,symbol,side,qty,price,signal\n2026-08-13,10:00,BAD,BUY,not-a-number,100,TEST\n",
        )
        .unwrap();

        let error = ledger.get_daily_traded_amount_checked().unwrap_err();
        assert!(error.to_string().contains("invalid ledger quantity"));
    }
}

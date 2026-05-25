use crate::features::shared::infrastructure::ledger::{Ledger, TradeRecord};
use std::path::PathBuf;

pub type LedgerAdapter = Ledger;
pub type TradeRecordAdapter = TradeRecord;

pub fn build_ledger_adapter(save_dir: PathBuf) -> LedgerAdapter {
    Ledger::new(save_dir)
}

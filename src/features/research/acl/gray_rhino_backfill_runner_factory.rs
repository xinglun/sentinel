use crate::features::research::infrastructure::gray_rhino_backfill_runner::{
    self, GrayRhinoCategorySourceSummary,
};
use anyhow::Result;

pub(crate) fn append_gray_rhino_backfill_run(
    save_dir: &std::path::Path,
    value: &serde_json::Value,
) -> Result<()> {
    gray_rhino_backfill_runner::append_gray_rhino_backfill_run(save_dir, value)
}

pub(crate) fn collect_gray_rhino_category_source(
    save_to: &str,
    category: &str,
    symbol: Option<String>,
    source_file: Option<String>,
    dry_run_requested: bool,
    observed_date_arg: Option<&str>,
    metrics: &[&str],
) -> Result<GrayRhinoCategorySourceSummary> {
    gray_rhino_backfill_runner::collect_gray_rhino_category_source(
        save_to,
        category,
        symbol,
        source_file,
        dry_run_requested,
        observed_date_arg,
        metrics,
    )
}

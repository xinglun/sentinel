use anyhow::{anyhow, Context, Result};
use chrono::NaiveDate;

use crate::config;

/// Review command の report 選択、検証、出力境界を保持したまま実行する。
pub(crate) fn run_review_command(config: &config::AppConfig) -> Result<()> {
    println!("{}", load_latest_daily_report(config)?);
    Ok(())
}

pub(crate) fn load_latest_daily_report(config: &config::AppConfig) -> Result<String> {
    let save_dir = std::path::Path::new(&config.output.save_to);
    let latest_path = std::fs::read_dir(save_dir)
        .with_context(|| format!("Failed to read report directory: {}", save_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            let stem = path.file_stem().and_then(|stem| stem.to_str());
            path.extension().and_then(|extension| extension.to_str()) == Some("md")
                && stem
                    .and_then(|stem| NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok())
                    .is_some()
        })
        .max();
    let latest_path =
        latest_path.ok_or_else(|| anyhow!("No daily report found in {}", save_dir.display()))?;

    let report = std::fs::read_to_string(&latest_path).with_context(|| {
        format!(
            "Failed to read latest daily report: {}",
            latest_path.display()
        )
    })?;
    if report.contains("tests/fixtures/") || report.contains("file://") {
        return Err(anyhow!(
            "Latest daily report contains non-production evidence and cannot be reviewed as a valid report: {}",
            latest_path.display()
        ));
    }
    validate_latest_report_run_status(
        save_dir,
        latest_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
    )?;
    Ok(report)
}

fn validate_latest_report_run_status(save_dir: &std::path::Path, report_date: &str) -> Result<()> {
    let status_path = save_dir.join(format!("run_status_{report_date}.json"));
    let raw = std::fs::read_to_string(&status_path).with_context(|| {
        format!(
            "Latest daily report has no corresponding run status: {}",
            status_path.display()
        )
    })?;
    let outcome =
        serde_json::from_str::<crate::features::shared::application::run_status::RunOutcome>(&raw)
            .with_context(|| {
                format!(
                    "Failed to parse latest report run status: {}",
                    status_path.display()
                )
            })?;
    match outcome.decisioning {
        crate::features::shared::application::run_status::DeliveryStatus::Succeeded => Ok(()),
        crate::features::shared::application::run_status::DeliveryStatus::Failed { reason } => Err(
            anyhow!("Latest daily report run failed and cannot be reviewed: {reason}"),
        ),
        crate::features::shared::application::run_status::DeliveryStatus::Skipped => Err(anyhow!(
            "Latest daily report run was skipped and cannot be reviewed"
        )),
    }
}

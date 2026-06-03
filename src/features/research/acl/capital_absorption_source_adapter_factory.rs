use crate::config;
use crate::features::research::application::capital_absorption::CapitalAbsorptionAutoSnapshot;
use crate::features::research::infrastructure::capital_absorption_source_adapter::build_automatic_capital_absorption_snapshot;
use chrono::NaiveDate;

/// Capital Absorption source adapter の infrastructure 実装を interface から隠蔽する。
pub(crate) async fn build_capital_absorption_auto_snapshot(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
    lookback_days: usize,
) -> CapitalAbsorptionAutoSnapshot {
    build_automatic_capital_absorption_snapshot(app_config, as_of_date, lookback_days).await
}

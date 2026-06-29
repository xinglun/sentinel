use crate::config;
use crate::features::research::acl::capital_absorption_source_adapter_factory::{
    build_capital_absorption_auto_snapshot,
    build_capital_absorption_ipo_queue_weekly_summary as build_capital_absorption_ipo_queue_weekly_summary_from_acl,
};
use crate::features::research::domain::capital_absorption::CapitalAbsorptionAutoSnapshot;
use crate::features::research::interface::capital_absorption_report::build_capital_absorption_report_from_config;
use crate::features::shared::interface::i18n::Language;

/// Capital Absorption report の自動観測取得と表示合成を分離する。
pub(crate) async fn build_capital_absorption_report_with_auto(
    app_config: &config::AppConfig,
    as_of_date: chrono::NaiveDate,
    lookback_days: usize,
    language: Language,
) -> String {
    let auto_enabled = app_config
        .capital_absorption
        .as_ref()
        .and_then(|config| config.auto_enable)
        .unwrap_or(true);
    let snapshot = if auto_enabled {
        Some(build_capital_absorption_auto_snapshot(app_config, as_of_date, lookback_days).await)
    } else {
        None
    };
    build_capital_absorption_report_from_config(
        app_config.capital_absorption.as_ref(),
        snapshot.as_ref(),
        language,
    )
}

/// 自動観測が有効な場合だけ capital absorption snapshot を返す。
pub(crate) async fn build_capital_absorption_auto_snapshot_with_config(
    app_config: &config::AppConfig,
    as_of_date: chrono::NaiveDate,
    lookback_days: usize,
) -> Option<CapitalAbsorptionAutoSnapshot> {
    let auto_enabled = app_config
        .capital_absorption
        .as_ref()
        .and_then(|config| config.auto_enable)
        .unwrap_or(true);
    if auto_enabled {
        Some(build_capital_absorption_auto_snapshot(app_config, as_of_date, lookback_days).await)
    } else {
        None
    }
}

/// IPO Queue ledger から週次 review 用 summary を組み立てる。
pub(crate) fn build_capital_absorption_ipo_queue_weekly_summary(
    save_dir: &std::path::Path,
    as_of_date: chrono::NaiveDate,
) -> serde_json::Value {
    build_capital_absorption_ipo_queue_weekly_summary_from_acl(save_dir, as_of_date)
}

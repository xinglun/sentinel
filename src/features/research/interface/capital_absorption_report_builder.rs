use crate::config;
use crate::features::research::acl::capital_absorption_source_adapter_factory::build_capital_absorption_auto_snapshot;
use crate::features::research::interface::cognitive_reports::build_capital_absorption_report;
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
    build_capital_absorption_report(app_config, snapshot.as_ref(), language)
}

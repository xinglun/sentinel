use crate::config;
use crate::features::research::acl::valuation_gravity_source_adapter_factory::{
    build_valuation_gravity_observation, build_valuation_gravity_observation_for_market_date,
};
use crate::features::research::application::valuation_gravity::FutureValuationDateError;
use crate::features::research::interface::valuation_gravity_report::build_valuation_gravity_report;
use crate::features::shared::interface::i18n::Language;
use chrono::NaiveDate;

/// 外部 source の取得、snapshot 保存、read-only report 合成を調停する。
pub(crate) async fn build_valuation_gravity_report_with_auto(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
    language: Language,
) -> Result<String, FutureValuationDateError> {
    let observation = build_valuation_gravity_observation(app_config, as_of_date).await?;
    Ok(build_valuation_gravity_report(&observation, language))
}

pub(crate) async fn build_valuation_gravity_observation_for_market_date_with_auto(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
    current_date: NaiveDate,
) -> Result<
    crate::features::research::application::valuation_gravity::ValuationGravityObservation,
    FutureValuationDateError,
> {
    build_valuation_gravity_observation_for_market_date(app_config, as_of_date, current_date).await
}

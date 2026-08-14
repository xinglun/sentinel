use crate::config;
use crate::features::research::application::valuation_gravity::{
    FutureValuationDateError, ValuationGravityObservation, ValuationGravityUseCase,
};
use crate::features::research::infrastructure::valuation_gravity_snapshot_store::FileValuationGravitySnapshotRepository;
use crate::features::research::infrastructure::valuation_gravity_source_adapter::FinnhubValuationGravitySourceAdapter;
use chrono::NaiveDate;

/// Infrastructure 実装を組み立て、Application use case を公開する ACL facade。
pub(crate) async fn build_valuation_gravity_observation(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
) -> Result<ValuationGravityObservation, FutureValuationDateError> {
    build_valuation_gravity_observation_for_market_date(
        app_config,
        as_of_date,
        chrono::Local::now().date_naive(),
    )
    .await
}

/// Radar が取得した最新取引日を current_date として source/replay 分岐を決める。
pub(crate) async fn build_valuation_gravity_observation_for_market_date(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
    current_date: NaiveDate,
) -> Result<ValuationGravityObservation, FutureValuationDateError> {
    let source = FinnhubValuationGravitySourceAdapter::new(app_config);
    let repository = FileValuationGravitySnapshotRepository::new(app_config.output.save_to.clone());
    let use_case = ValuationGravityUseCase::new(&source, &repository);
    use_case
        .execute(&enabled_symbols(app_config), as_of_date, current_date)
        .await
}

fn enabled_symbols(app_config: &config::AppConfig) -> Vec<String> {
    let mut symbols = app_config
        .watchlist
        .iter()
        .filter(|entry| entry.enable)
        .map(|entry| entry.symbol.clone())
        .collect::<Vec<_>>();
    symbols.sort();
    symbols.dedup();
    symbols
}

#[cfg(test)]
mod tests {
    #[test]
    fn acl_facade_keeps_use_case_and_ports_in_application() {
        let source = include_str!("../application/valuation_gravity.rs");
        assert!(source.contains("trait ValuationGravitySourcePort"));
        assert!(source.contains("trait ValuationGravitySnapshotRepository"));
        assert!(source.contains("struct ValuationGravityUseCase"));
    }
}

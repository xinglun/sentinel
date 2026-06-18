pub(crate) use crate::features::research::domain::valuation_gravity::*;

use async_trait::async_trait;
use chrono::NaiveDate;
use futures::{stream, StreamExt};
use std::time::Duration;

const MAX_CONCURRENT_ASSETS: usize = 10;
const ASSET_COLLECTION_BUDGET: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FutureValuationDateError {
    pub as_of_date: NaiveDate,
    pub current_date: NaiveDate,
}

impl std::fmt::Display for FutureValuationDateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "valuation as-of date {} is later than current date {}",
            self.as_of_date, self.current_date
        )
    }
}

impl std::error::Error for FutureValuationDateError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValuationSourceErrorKind {
    MissingCredential,
    EntitlementDenied,
    ProviderFailure,
    InvalidResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValuationSourceError {
    pub kind: ValuationSourceErrorKind,
    pub detail: String,
}

impl ValuationSourceError {
    pub(crate) fn new(kind: ValuationSourceErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

#[async_trait]
pub(crate) trait ValuationGravitySourcePort: Send + Sync {
    async fn price_target(
        &self,
        symbol: &str,
        as_of_date: NaiveDate,
    ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError>;

    async fn market_multiple(
        &self,
        symbol: &str,
        as_of_date: NaiveDate,
    ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError>;

    async fn recommendation(
        &self,
        symbol: &str,
        as_of_date: NaiveDate,
    ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError>;
}

pub(crate) trait ValuationGravitySnapshotRepository: Send + Sync {
    fn load(&self, as_of_date: NaiveDate) -> Result<Option<ValuationGravitySnapshot>, String>;
    fn save(&self, snapshot: &ValuationGravitySnapshot) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValuationPersistenceHealth {
    Saved,
    Replayed,
    Missing,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValuationPersistenceReason {
    SnapshotSaved,
    HistoricalSnapshotReplayed,
    HistoricalSnapshotMissing,
    SnapshotReadFailed,
    SnapshotWriteFailed,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValuationGravityObservation {
    pub snapshot: ValuationGravitySnapshot,
    pub persistence_health: ValuationPersistenceHealth,
    pub persistence_reason: ValuationPersistenceReason,
    pub persistence_detail: String,
}

pub(crate) struct ValuationGravityUseCase<'a> {
    source: &'a dyn ValuationGravitySourcePort,
    repository: &'a dyn ValuationGravitySnapshotRepository,
}

impl<'a> ValuationGravityUseCase<'a> {
    pub(crate) fn new(
        source: &'a dyn ValuationGravitySourcePort,
        repository: &'a dyn ValuationGravitySnapshotRepository,
    ) -> Self {
        Self { source, repository }
    }

    pub(crate) async fn execute(
        &self,
        symbols: &[String],
        as_of_date: NaiveDate,
        current_date: NaiveDate,
    ) -> Result<ValuationGravityObservation, FutureValuationDateError> {
        if as_of_date > current_date {
            return Err(FutureValuationDateError {
                as_of_date,
                current_date,
            });
        }
        if as_of_date < current_date {
            return Ok(self.replay(symbols, as_of_date));
        }

        let assets = self
            .collect_assets_with_budget(symbols, as_of_date, ASSET_COLLECTION_BUDGET)
            .await;
        let snapshot = ValuationGravitySnapshot {
            as_of_date,
            assets,
            observation_only: true,
        };
        Ok(match self.repository.save(&snapshot) {
            Ok(()) => ValuationGravityObservation {
                snapshot,
                persistence_health: ValuationPersistenceHealth::Saved,
                persistence_reason: ValuationPersistenceReason::SnapshotSaved,
                persistence_detail: String::new(),
            },
            Err(detail) => ValuationGravityObservation {
                snapshot,
                persistence_health: ValuationPersistenceHealth::Failed,
                persistence_reason: ValuationPersistenceReason::SnapshotWriteFailed,
                persistence_detail: detail,
            },
        })
    }

    async fn collect_assets_with_budget(
        &self,
        symbols: &[String],
        as_of_date: NaiveDate,
        budget: Duration,
    ) -> Vec<ValuationGravityAssetSnapshot> {
        let mut pending = stream::iter(symbols.iter().enumerate().map(
            |(index, symbol)| async move { (index, self.collect_asset(symbol, as_of_date).await) },
        ))
        .buffer_unordered(MAX_CONCURRENT_ASSETS);
        let deadline = tokio::time::sleep(budget);
        tokio::pin!(deadline);
        let mut collected = vec![None; symbols.len()];

        loop {
            tokio::select! {
                result = pending.next() => match result {
                    Some((index, asset)) => collected[index] = Some(asset),
                    None => break,
                },
                _ = &mut deadline => break,
            }
        }

        collected
            .into_iter()
            .zip(symbols)
            .map(|(asset, symbol)| {
                asset.unwrap_or_else(|| {
                    unavailable_asset(
                        symbol,
                        as_of_date,
                        ValuationDataQualityReason::ProviderFailure,
                        unavailable_message(ValuationDataQualityReason::ProviderFailure)
                            .to_string(),
                    )
                })
            })
            .collect()
    }

    fn replay(&self, symbols: &[String], as_of_date: NaiveDate) -> ValuationGravityObservation {
        match self.repository.load(as_of_date) {
            Ok(Some(snapshot)) => ValuationGravityObservation {
                snapshot,
                persistence_health: ValuationPersistenceHealth::Replayed,
                persistence_reason: ValuationPersistenceReason::HistoricalSnapshotReplayed,
                persistence_detail: String::new(),
            },
            Ok(None) => ValuationGravityObservation {
                snapshot: unavailable_snapshot(
                    symbols,
                    as_of_date,
                    ValuationDataQualityReason::HistoricalSnapshotMissing,
                    "historical valuation snapshot is not available",
                ),
                persistence_health: ValuationPersistenceHealth::Missing,
                persistence_reason: ValuationPersistenceReason::HistoricalSnapshotMissing,
                persistence_detail: String::new(),
            },
            Err(detail) => ValuationGravityObservation {
                snapshot: unavailable_snapshot(
                    symbols,
                    as_of_date,
                    ValuationDataQualityReason::HistoricalSnapshotReadFailure,
                    "historical valuation snapshot could not be read",
                ),
                persistence_health: ValuationPersistenceHealth::Failed,
                persistence_reason: ValuationPersistenceReason::SnapshotReadFailed,
                persistence_detail: detail,
            },
        }
    }

    async fn collect_asset(
        &self,
        symbol: &str,
        as_of_date: NaiveDate,
    ) -> ValuationGravityAssetSnapshot {
        let mut errors = Vec::new();
        let mut had_empty_response = false;
        match self.source.price_target(symbol, as_of_date).await {
            Ok(Some(asset)) => return asset,
            Ok(None) => had_empty_response = true,
            Err(error) => errors.push(error),
        }
        match self.source.market_multiple(symbol, as_of_date).await {
            Ok(Some(mut asset)) => {
                asset.quality_reason = ValuationDataQualityReason::MarketMultipleFallback;
                return asset;
            }
            Ok(None) => had_empty_response = true,
            Err(error) => errors.push(error),
        }
        match self.source.recommendation(symbol, as_of_date).await {
            Ok(Some(mut asset)) => {
                asset.quality_reason = ValuationDataQualityReason::RecommendationFallback;
                return asset;
            }
            Ok(None) => had_empty_response = true,
            Err(error) => errors.push(error),
        }

        let reason = failure_reason(&errors, had_empty_response);
        unavailable_asset(
            symbol,
            as_of_date,
            reason,
            unavailable_message(reason).to_string(),
        )
    }
}

fn failure_reason(
    errors: &[ValuationSourceError],
    had_empty_response: bool,
) -> ValuationDataQualityReason {
    if errors
        .iter()
        .any(|error| error.kind == ValuationSourceErrorKind::MissingCredential)
    {
        ValuationDataQualityReason::MissingCredential
    } else if errors
        .iter()
        .any(|error| error.kind == ValuationSourceErrorKind::ProviderFailure)
    {
        ValuationDataQualityReason::ProviderFailure
    } else if errors
        .iter()
        .any(|error| error.kind == ValuationSourceErrorKind::InvalidResponse)
    {
        ValuationDataQualityReason::InvalidResponse
    } else if !had_empty_response
        && !errors.is_empty()
        && errors
            .iter()
            .all(|error| error.kind == ValuationSourceErrorKind::EntitlementDenied)
    {
        ValuationDataQualityReason::EntitlementDenied
    } else {
        ValuationDataQualityReason::InsufficientEvidence
    }
}

fn unavailable_message(reason: ValuationDataQualityReason) -> &'static str {
    match reason {
        ValuationDataQualityReason::MissingCredential => "missing credential",
        ValuationDataQualityReason::EntitlementDenied => "provider entitlement denied",
        ValuationDataQualityReason::ProviderFailure => "provider request failed",
        ValuationDataQualityReason::InvalidResponse => "invalid provider response",
        ValuationDataQualityReason::InsufficientEvidence => "insufficient valuation evidence",
        ValuationDataQualityReason::HistoricalSnapshotMissing => "historical snapshot missing",
        ValuationDataQualityReason::HistoricalSnapshotReadFailure => {
            "historical snapshot read failed"
        }
        ValuationDataQualityReason::PriceTargetConsensus
        | ValuationDataQualityReason::MarketMultipleFallback
        | ValuationDataQualityReason::RecommendationFallback => "valuation evidence available",
    }
}

fn unavailable_snapshot(
    symbols: &[String],
    as_of_date: NaiveDate,
    reason: ValuationDataQualityReason,
    message: &str,
) -> ValuationGravitySnapshot {
    ValuationGravitySnapshot {
        as_of_date,
        assets: symbols
            .iter()
            .map(|symbol| unavailable_asset(symbol, as_of_date, reason, message.to_string()))
            .collect(),
        observation_only: true,
    }
}

fn unavailable_asset(
    symbol: &str,
    as_of_date: NaiveDate,
    reason: ValuationDataQualityReason,
    message: String,
) -> ValuationGravityAssetSnapshot {
    ValuationGravityAssetSnapshot {
        symbol: symbol.to_string(),
        gravity: None,
        confidence: None,
        source: None,
        provider: "Finnhub".to_string(),
        as_of_date,
        source_health: ValuationSourceHealth::Unavailable,
        quality_reason: reason,
        evidence_count: 0,
        relative_ratio: None,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct StubSource {
        calls: Mutex<Vec<&'static str>>,
        target: Option<ValuationGravityAssetSnapshot>,
        multiple: Option<ValuationGravityAssetSnapshot>,
        recommendation: Option<ValuationGravityAssetSnapshot>,
    }

    #[async_trait]
    impl ValuationGravitySourcePort for StubSource {
        async fn price_target(
            &self,
            _symbol: &str,
            _as_of_date: NaiveDate,
        ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
            self.calls.lock().unwrap().push("target");
            Ok(self.target.clone())
        }

        async fn market_multiple(
            &self,
            _symbol: &str,
            _as_of_date: NaiveDate,
        ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
            self.calls.lock().unwrap().push("multiple");
            Ok(self.multiple.clone())
        }

        async fn recommendation(
            &self,
            _symbol: &str,
            _as_of_date: NaiveDate,
        ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
            self.calls.lock().unwrap().push("recommendation");
            Ok(self.recommendation.clone())
        }
    }

    struct StubRepository {
        load_result: Result<Option<ValuationGravitySnapshot>, String>,
        save_result: Result<(), String>,
    }

    struct SlowSource;

    #[async_trait]
    impl ValuationGravitySourcePort for SlowSource {
        async fn price_target(
            &self,
            _symbol: &str,
            _as_of_date: NaiveDate,
        ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(None)
        }

        async fn market_multiple(
            &self,
            _symbol: &str,
            _as_of_date: NaiveDate,
        ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
            unreachable!("asset budget must stop fallback")
        }

        async fn recommendation(
            &self,
            _symbol: &str,
            _as_of_date: NaiveDate,
        ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
            unreachable!("asset budget must stop fallback")
        }
    }

    struct SecretErrorSource;

    #[async_trait]
    impl ValuationGravitySourcePort for SecretErrorSource {
        async fn price_target(
            &self,
            _symbol: &str,
            _as_of_date: NaiveDate,
        ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
            Err(ValuationSourceError::new(
                ValuationSourceErrorKind::ProviderFailure,
                "https://provider.invalid/path?token=secret-token",
            ))
        }

        async fn market_multiple(
            &self,
            _symbol: &str,
            _as_of_date: NaiveDate,
        ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
            Ok(None)
        }

        async fn recommendation(
            &self,
            _symbol: &str,
            _as_of_date: NaiveDate,
        ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
            Ok(None)
        }
    }

    impl ValuationGravitySnapshotRepository for StubRepository {
        fn load(&self, _as_of_date: NaiveDate) -> Result<Option<ValuationGravitySnapshot>, String> {
            self.load_result.clone()
        }

        fn save(&self, _snapshot: &ValuationGravitySnapshot) -> Result<(), String> {
            self.save_result.clone()
        }
    }

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 18).unwrap()
    }

    fn fixture_asset() -> ValuationGravityAssetSnapshot {
        ValuationGravityAssetSnapshot {
            symbol: "MSFT".to_string(),
            gravity: Some(GravityStatus::Fair),
            confidence: Some(ValuationConfidence::Low),
            source: Some(ValuationSource::MarketMultiple),
            provider: "Finnhub".to_string(),
            as_of_date: date(),
            source_health: ValuationSourceHealth::Partial,
            quality_reason: ValuationDataQualityReason::MarketMultipleFallback,
            evidence_count: 5,
            relative_ratio: Some(1.0),
            message: "fixture".to_string(),
        }
    }

    #[tokio::test]
    async fn use_case_owns_fallback_order_and_stops_after_first_usable_source() {
        let source = StubSource {
            calls: Mutex::new(Vec::new()),
            target: None,
            multiple: Some(fixture_asset()),
            recommendation: Some(fixture_asset()),
        };
        let repository = StubRepository {
            load_result: Ok(None),
            save_result: Ok(()),
        };
        let use_case = ValuationGravityUseCase::new(&source, &repository);

        let output = use_case
            .execute(&["MSFT".to_string()], date(), date())
            .await
            .unwrap();

        assert_eq!(*source.calls.lock().unwrap(), vec!["target", "multiple"]);
        assert_eq!(output.persistence_health, ValuationPersistenceHealth::Saved);
        assert_eq!(
            output.snapshot.assets[0].quality_reason,
            ValuationDataQualityReason::MarketMultipleFallback
        );
    }

    #[tokio::test]
    async fn use_case_reaches_recommendation_only_after_numeric_sources_are_empty() {
        let source = StubSource {
            calls: Mutex::new(Vec::new()),
            target: None,
            multiple: None,
            recommendation: Some(fixture_asset()),
        };
        let repository = StubRepository {
            load_result: Ok(None),
            save_result: Ok(()),
        };

        let output = ValuationGravityUseCase::new(&source, &repository)
            .execute(&["MSFT".to_string()], date(), date())
            .await
            .unwrap();

        assert_eq!(
            *source.calls.lock().unwrap(),
            vec!["target", "multiple", "recommendation"]
        );
        assert_eq!(
            output.snapshot.assets[0].quality_reason,
            ValuationDataQualityReason::RecommendationFallback
        );
    }

    #[tokio::test]
    async fn asset_collection_runs_concurrently_with_one_overall_budget() {
        let repository = StubRepository {
            load_result: Ok(None),
            save_result: Ok(()),
        };
        let use_case = ValuationGravityUseCase::new(&SlowSource, &repository);
        let symbols = (0..(MAX_CONCURRENT_ASSETS * 2))
            .map(|index| format!("ASSET{index}"))
            .collect::<Vec<_>>();
        let started = std::time::Instant::now();

        let assets = use_case
            .collect_assets_with_budget(&symbols, date(), Duration::from_millis(20))
            .await;

        assert!(started.elapsed() < Duration::from_millis(100));
        assert!(assets.iter().all(|asset| {
            asset.quality_reason == ValuationDataQualityReason::ProviderFailure
                && asset.message == "provider request failed"
        }));
    }

    #[tokio::test]
    async fn provider_error_detail_is_not_persisted_in_asset_snapshot() {
        let repository = StubRepository {
            load_result: Ok(None),
            save_result: Ok(()),
        };

        let output = ValuationGravityUseCase::new(&SecretErrorSource, &repository)
            .execute(&["MSFT".to_string()], date(), date())
            .await
            .unwrap();
        let encoded = serde_json::to_string(&output.snapshot).unwrap();

        assert!(!encoded.contains("secret-token"));
        assert!(!encoded.contains("provider.invalid"));
        assert_eq!(output.snapshot.assets[0].message, "provider request failed");
    }

    #[tokio::test]
    async fn snapshot_write_failure_is_returned_as_auditable_health() {
        let source = StubSource {
            calls: Mutex::new(Vec::new()),
            target: Some(fixture_asset()),
            multiple: None,
            recommendation: None,
        };
        let repository = StubRepository {
            load_result: Ok(None),
            save_result: Err("permission denied".to_string()),
        };
        let output = ValuationGravityUseCase::new(&source, &repository)
            .execute(&["MSFT".to_string()], date(), date())
            .await
            .unwrap();

        assert_eq!(
            output.persistence_health,
            ValuationPersistenceHealth::Failed
        );
        assert_eq!(
            output.persistence_reason,
            ValuationPersistenceReason::SnapshotWriteFailed
        );
        assert_eq!(output.persistence_detail, "permission denied");
    }

    #[tokio::test]
    async fn snapshot_read_failure_does_not_fall_back_to_current_provider_data() {
        let source = StubSource {
            calls: Mutex::new(Vec::new()),
            target: Some(fixture_asset()),
            multiple: None,
            recommendation: None,
        };
        let repository = StubRepository {
            load_result: Err("corrupt snapshot".to_string()),
            save_result: Ok(()),
        };
        let output = ValuationGravityUseCase::new(&source, &repository)
            .execute(
                &["MSFT".to_string()],
                NaiveDate::from_ymd_opt(2026, 6, 17).unwrap(),
                date(),
            )
            .await
            .unwrap();

        assert!(source.calls.lock().unwrap().is_empty());
        assert_eq!(
            output.persistence_health,
            ValuationPersistenceHealth::Failed
        );
        assert_eq!(
            output.snapshot.assets[0].quality_reason,
            ValuationDataQualityReason::HistoricalSnapshotReadFailure
        );
    }

    #[test]
    fn entitlement_requires_all_attempts_to_be_denied() {
        let denied =
            ValuationSourceError::new(ValuationSourceErrorKind::EntitlementDenied, "forbidden");
        assert_eq!(
            failure_reason(&[denied.clone(), denied.clone(), denied], false),
            ValuationDataQualityReason::EntitlementDenied
        );
        assert_eq!(
            failure_reason(
                &[ValuationSourceError::new(
                    ValuationSourceErrorKind::EntitlementDenied,
                    "forbidden",
                )],
                true,
            ),
            ValuationDataQualityReason::InsufficientEvidence
        );
    }

    #[tokio::test]
    async fn future_as_of_date_is_rejected_before_source_or_repository_access() {
        let source = StubSource {
            calls: Mutex::new(Vec::new()),
            target: Some(fixture_asset()),
            multiple: None,
            recommendation: None,
        };
        let repository = StubRepository {
            load_result: Ok(None),
            save_result: Ok(()),
        };
        let future_date = NaiveDate::from_ymd_opt(2026, 6, 19).unwrap();

        let error = ValuationGravityUseCase::new(&source, &repository)
            .execute(&["MSFT".to_string()], future_date, date())
            .await
            .unwrap_err();

        assert_eq!(error.as_of_date, future_date);
        assert_eq!(error.current_date, date());
        assert!(source.calls.lock().unwrap().is_empty());
    }
}

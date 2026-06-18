use crate::config;
use crate::features::research::application::valuation_gravity::{
    classify_recommendation_score, classify_relative_ratio, market_multiple_confidence,
    price_target_confidence, recommendation_confidence, ValuationDataQualityReason,
    ValuationGravityAssetSnapshot, ValuationGravitySourcePort, ValuationSource,
    ValuationSourceError, ValuationSourceErrorKind, ValuationSourceHealth,
};
use async_trait::async_trait;
use chrono::{NaiveDate, NaiveDateTime};
use serde_json::Value;

const PROVIDER: &str = "Finnhub";

pub(crate) struct FinnhubValuationGravitySourceAdapter<'a> {
    app_config: &'a config::AppConfig,
    client: FinnhubClientState,
}

enum FinnhubClientState {
    Ready(reqwest::Client),
    Failed(String),
}

impl<'a> FinnhubValuationGravitySourceAdapter<'a> {
    pub(crate) fn new(app_config: &'a config::AppConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map(FinnhubClientState::Ready)
            .unwrap_or_else(|_| {
                FinnhubClientState::Failed("Finnhub client initialization failed".to_string())
            });
        Self { app_config, client }
    }

    fn token(&self) -> Result<&str, ValuationSourceError> {
        self.app_config
            .finnhub
            .as_ref()
            .map(|config| config.finnhub_api_key.trim())
            .filter(|key| !key.is_empty())
            .ok_or_else(|| {
                ValuationSourceError::new(
                    ValuationSourceErrorKind::MissingCredential,
                    "Finnhub API key is not configured",
                )
            })
    }

    fn client(&self) -> Result<&reqwest::Client, ValuationSourceError> {
        match &self.client {
            FinnhubClientState::Ready(client) => Ok(client),
            FinnhubClientState::Failed(detail) => Err(ValuationSourceError::new(
                ValuationSourceErrorKind::ProviderFailure,
                detail.clone(),
            )),
        }
    }
}

#[async_trait]
impl ValuationGravitySourcePort for FinnhubValuationGravitySourceAdapter<'_> {
    async fn price_target(
        &self,
        symbol: &str,
        as_of_date: NaiveDate,
    ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
        let token = self.token()?;
        let target = fetch_json(
            self.client()?,
            &format!("https://finnhub.io/api/v1/stock/price-target?symbol={symbol}&token={token}"),
        )
        .await?;
        let quote = fetch_json(
            self.client()?,
            &format!("https://finnhub.io/api/v1/quote?symbol={symbol}&token={token}"),
        )
        .await?;
        Ok(parse_price_target_snapshot(
            symbol, as_of_date, &target, &quote,
        ))
    }

    async fn market_multiple(
        &self,
        symbol: &str,
        as_of_date: NaiveDate,
    ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
        let token = self.token()?;
        let value = fetch_json(
            self.client()?,
            &format!(
                "https://finnhub.io/api/v1/stock/metric?symbol={symbol}&metric=all&token={token}"
            ),
        )
        .await?;
        Ok(parse_market_multiple_snapshot(symbol, as_of_date, &value))
    }

    async fn recommendation(
        &self,
        symbol: &str,
        as_of_date: NaiveDate,
    ) -> Result<Option<ValuationGravityAssetSnapshot>, ValuationSourceError> {
        let token = self.token()?;
        let value = fetch_json(
            self.client()?,
            &format!(
                "https://finnhub.io/api/v1/stock/recommendation?symbol={symbol}&token={token}"
            ),
        )
        .await?;
        Ok(parse_recommendation_snapshot(symbol, as_of_date, &value))
    }
}

async fn fetch_json(client: &reqwest::Client, url: &str) -> Result<Value, ValuationSourceError> {
    let response = client.get(url).send().await.map_err(|_| {
        ValuationSourceError::new(
            ValuationSourceErrorKind::ProviderFailure,
            "Finnhub request failed",
        )
    })?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(ValuationSourceError::new(
            http_error_kind(status),
            format!("Finnhub returned {status}"),
        ));
    }
    response.json().await.map_err(|_| {
        ValuationSourceError::new(
            ValuationSourceErrorKind::InvalidResponse,
            "Finnhub response was not valid JSON",
        )
    })
}

fn http_error_kind(status: reqwest::StatusCode) -> ValuationSourceErrorKind {
    if matches!(
        status,
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) {
        ValuationSourceErrorKind::EntitlementDenied
    } else {
        ValuationSourceErrorKind::ProviderFailure
    }
}

pub(crate) fn parse_price_target_snapshot(
    symbol: &str,
    as_of_date: NaiveDate,
    target: &Value,
    quote: &Value,
) -> Option<ValuationGravityAssetSnapshot> {
    let current = quote.get("c")?.as_f64()?;
    let median = target.get("targetMedian")?.as_f64()?;
    let high = target.get("targetHigh")?.as_f64()?;
    let low = target.get("targetLow")?.as_f64()?;
    let analyst_count = target.get("numberAnalysts")?.as_u64()? as usize;
    if current <= 0.0 || median <= 0.0 || analyst_count == 0 {
        return None;
    }
    let source_date = parse_finnhub_date(target.get("lastUpdated")?.as_str()?)?;
    if source_date > as_of_date {
        return None;
    }
    let age_days = (as_of_date - source_date).num_days();
    let dispersion = ((high - low).abs() / median).max(0.0);
    let ratio = current / median;
    Some(ValuationGravityAssetSnapshot {
        symbol: symbol.to_string(),
        gravity: classify_relative_ratio(ratio),
        confidence: Some(price_target_confidence(analyst_count, age_days, dispersion)),
        source: Some(ValuationSource::AnalystConsensus),
        provider: PROVIDER.to_string(),
        as_of_date: source_date,
        source_health: ValuationSourceHealth::Succeeded,
        quality_reason: ValuationDataQualityReason::PriceTargetConsensus,
        evidence_count: analyst_count,
        relative_ratio: Some(ratio),
        message: "price target consensus".to_string(),
    })
}

pub(crate) fn parse_market_multiple_snapshot(
    symbol: &str,
    as_of_date: NaiveDate,
    value: &Value,
) -> Option<ValuationGravityAssetSnapshot> {
    let current = value
        .pointer("/metric/peTTM")
        .and_then(Value::as_f64)
        .or_else(|| value.pointer("/metric/forwardPE").and_then(Value::as_f64))?;
    let mut history = value
        .pointer("/series/annual/pe")?
        .as_array()?
        .iter()
        .filter_map(|item| {
            let period =
                NaiveDate::parse_from_str(item.get("period")?.as_str()?, "%Y-%m-%d").ok()?;
            let multiple = item.get("v")?.as_f64()?;
            (period <= as_of_date && multiple.is_finite() && multiple > 0.0)
                .then_some((period, multiple))
        })
        .collect::<Vec<_>>();
    history.sort_by(|left, right| right.0.cmp(&left.0));
    let mut history = history
        .into_iter()
        .take(5)
        .map(|(_, multiple)| multiple)
        .collect::<Vec<_>>();
    if current <= 0.0 || history.len() < 3 {
        return None;
    }
    history.sort_by(|left, right| left.total_cmp(right));
    let midpoint = history.len() / 2;
    let median = if history.len() % 2 == 0 {
        (history[midpoint - 1] + history[midpoint]) / 2.0
    } else {
        history[midpoint]
    };
    let ratio = current / median;
    Some(ValuationGravityAssetSnapshot {
        symbol: symbol.to_string(),
        gravity: classify_relative_ratio(ratio),
        confidence: Some(market_multiple_confidence(history.len())),
        source: Some(ValuationSource::MarketMultiple),
        provider: PROVIDER.to_string(),
        as_of_date,
        source_health: ValuationSourceHealth::Partial,
        quality_reason: ValuationDataQualityReason::MarketMultipleFallback,
        evidence_count: history.len(),
        relative_ratio: Some(ratio),
        message: "current P/E relative to five-year historical median".to_string(),
    })
}

pub(crate) fn parse_recommendation_snapshot(
    symbol: &str,
    as_of_date: NaiveDate,
    value: &Value,
) -> Option<ValuationGravityAssetSnapshot> {
    let (source_date, latest) = value
        .as_array()?
        .iter()
        .filter_map(|item| {
            let period =
                NaiveDate::parse_from_str(item.get("period")?.as_str()?, "%Y-%m-%d").ok()?;
            (period <= as_of_date).then_some((period, item))
        })
        .max_by_key(|(period, _)| *period)?;
    let strong_buy = latest.get("strongBuy")?.as_u64()? as usize;
    let buy = latest.get("buy")?.as_u64()? as usize;
    let hold = latest.get("hold")?.as_u64()? as usize;
    let sell = latest.get("sell")?.as_u64()? as usize;
    let strong_sell = latest.get("strongSell")?.as_u64()? as usize;
    let count = strong_buy + buy + hold + sell + strong_sell;
    if count == 0 {
        return None;
    }
    let score = (2.0 * strong_buy as f64 + buy as f64 - sell as f64 - 2.0 * strong_sell as f64)
        / count as f64;
    let age_days = (as_of_date - source_date).num_days();
    Some(ValuationGravityAssetSnapshot {
        symbol: symbol.to_string(),
        gravity: classify_recommendation_score(score),
        confidence: Some(recommendation_confidence(count, age_days)),
        source: Some(ValuationSource::AnalystConsensus),
        provider: PROVIDER.to_string(),
        as_of_date: source_date,
        source_health: ValuationSourceHealth::Partial,
        quality_reason: ValuationDataQualityReason::RecommendationFallback,
        evidence_count: count,
        relative_ratio: None,
        message: "recommendation consensus fallback".to_string(),
    })
}

fn parse_finnhub_date(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok().or_else(|| {
        NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
            .ok()
            .map(|value| value.date())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::application::valuation_gravity::{
        GravityStatus, ValuationConfidence,
    };

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 18).unwrap()
    }

    #[test]
    fn parses_price_target_consensus_without_exposing_precise_anchor() {
        let target = serde_json::json!({
            "lastUpdated": "2026-06-10 00:00:00",
            "numberAnalysts": 35,
            "targetHigh": 150.0,
            "targetLow": 100.0,
            "targetMedian": 125.0
        });
        let quote = serde_json::json!({"c": 100.0});
        let snapshot = parse_price_target_snapshot("MSFT", date(), &target, &quote).unwrap();
        assert_eq!(snapshot.gravity, Some(GravityStatus::Undervalued));
        assert_eq!(snapshot.confidence, Some(ValuationConfidence::High));
        assert_eq!(snapshot.evidence_count, 35);
    }

    #[test]
    fn price_target_rejects_future_dated_consensus() {
        let target = serde_json::json!({
            "lastUpdated": "2026-06-19 00:00:00", "numberAnalysts": 20,
            "targetHigh": 150.0, "targetLow": 100.0, "targetMedian": 125.0
        });
        assert!(parse_price_target_snapshot(
            "MSFT",
            date(),
            &target,
            &serde_json::json!({"c": 100.0})
        )
        .is_none());
    }

    #[test]
    fn parses_historical_market_multiple_as_low_confidence_fallback() {
        let value = serde_json::json!({
            "metric": {"forwardPE": 45.0},
            "series": {"annual": {"pe": [
                {"period": "2025-12-31", "v": 30.0},
                {"period": "2024-12-31", "v": 31.0},
                {"period": "2023-12-31", "v": 29.0},
                {"period": "2022-12-31", "v": 28.0},
                {"period": "2021-12-31", "v": 32.0}
            ]}}}
        );
        let snapshot = parse_market_multiple_snapshot("NVDA", date(), &value).unwrap();
        assert_eq!(snapshot.gravity, Some(GravityStatus::VeryExpensive));
        assert_eq!(snapshot.confidence, Some(ValuationConfidence::Low));
    }

    #[test]
    fn market_multiple_sorts_periods_and_excludes_future_and_old_records() {
        let value = serde_json::json!({
            "metric": {"peTTM": 30.0},
            "series": {"annual": {"pe": [
                {"period": "2021-12-31", "v": 60.0},
                {"period": "2027-12-31", "v": 1.0},
                {"period": "2024-12-31", "v": 30.0},
                {"period": "2020-12-31", "v": 100.0},
                {"period": "2025-12-31", "v": 20.0},
                {"period": "2022-12-31", "v": 50.0},
                {"period": "2023-12-31", "v": 40.0}
            ]}}}
        );

        let snapshot = parse_market_multiple_snapshot("MSFT", date(), &value).unwrap();

        assert_eq!(snapshot.evidence_count, 5);
        assert_eq!(snapshot.gravity, Some(GravityStatus::Undervalued));
        assert_eq!(snapshot.relative_ratio, Some(0.75));
    }

    #[test]
    fn market_multiple_averages_middle_values_for_even_sample_count() {
        let value = serde_json::json!({
            "metric": {"peTTM": 25.0},
            "series": {"annual": {"pe": [
                {"period": "2025-12-31", "v": 10.0},
                {"period": "2024-12-31", "v": 20.0},
                {"period": "2023-12-31", "v": 30.0},
                {"period": "2022-12-31", "v": 40.0}
            ]}}}
        );

        let snapshot = parse_market_multiple_snapshot("MSFT", date(), &value).unwrap();

        assert_eq!(snapshot.relative_ratio, Some(1.0));
        assert_eq!(snapshot.gravity, Some(GravityStatus::Fair));
    }

    #[test]
    fn parses_recommendations_when_numeric_anchors_are_unavailable() {
        let value = serde_json::json!([{
            "period": "2026-06-01",
            "strongBuy": 1, "buy": 2, "hold": 2, "sell": 10, "strongSell": 5
        }]);
        let snapshot = parse_recommendation_snapshot("TSLA", date(), &value).unwrap();
        assert_eq!(snapshot.gravity, Some(GravityStatus::Expensive));
        assert_eq!(snapshot.confidence, Some(ValuationConfidence::Low));
    }

    #[test]
    fn recommendation_selects_latest_non_future_period_independent_of_input_order() {
        let value = serde_json::json!([
            {"period": "2027-01-01", "strongBuy": 20, "buy": 0, "hold": 0, "sell": 0, "strongSell": 0},
            {"period": "2026-05-01", "strongBuy": 0, "buy": 0, "hold": 0, "sell": 0, "strongSell": 20},
            {"period": "2026-06-01", "strongBuy": 20, "buy": 0, "hold": 0, "sell": 0, "strongSell": 0}
        ]);

        let snapshot = parse_recommendation_snapshot("MSFT", date(), &value).unwrap();

        assert_eq!(
            snapshot.as_of_date,
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()
        );
        assert_eq!(snapshot.gravity, Some(GravityStatus::DeepUndervalued));
    }

    #[test]
    fn malformed_source_yields_no_classification_instead_of_unknown() {
        assert!(parse_market_multiple_snapshot("FIG", date(), &serde_json::json!({})).is_none());
        assert!(parse_recommendation_snapshot("FIG", date(), &serde_json::json!([])).is_none());
    }

    #[test]
    fn http_status_distinguishes_entitlement_from_provider_failure() {
        assert_eq!(
            http_error_kind(reqwest::StatusCode::FORBIDDEN),
            ValuationSourceErrorKind::EntitlementDenied
        );
        assert_eq!(
            http_error_kind(reqwest::StatusCode::TOO_MANY_REQUESTS),
            ValuationSourceErrorKind::ProviderFailure
        );
    }

    async fn serve_once(response: &'static str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).await;
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{address}/fixture")
    }

    #[tokio::test]
    async fn fetch_json_classifies_http_and_invalid_json_failures() {
        let client = reqwest::Client::new();
        let forbidden_url = serve_once(
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
        )
        .await;
        let forbidden = fetch_json(&client, &forbidden_url).await.unwrap_err();
        assert_eq!(forbidden.kind, ValuationSourceErrorKind::EntitlementDenied);

        let invalid_url = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 8\r\nConnection: close\r\n\r\nnot-json",
        )
        .await;
        let invalid = fetch_json(&client, &invalid_url).await.unwrap_err();
        assert_eq!(invalid.kind, ValuationSourceErrorKind::InvalidResponse);
    }

    #[tokio::test]
    async fn fetch_json_does_not_expose_request_url_or_token_in_error_detail() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let url = format!("http://{address}/fixture?token=secret-token");

        let error = fetch_json(&reqwest::Client::new(), &url).await.unwrap_err();

        assert_eq!(error.kind, ValuationSourceErrorKind::ProviderFailure);
        assert_eq!(error.detail, "Finnhub request failed");
        assert!(!error.detail.contains("secret-token"));
        assert!(!error.detail.contains(&address.to_string()));
    }
}

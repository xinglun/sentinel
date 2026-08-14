use futures::stream::{self, StreamExt};
use serde::Serialize;
use std::sync::Arc;

/// Radar run の data acquisition 結果概要。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataAcquisitionSummary {
    pub successful_fetches: usize,
    pub failed_fetches: usize,
}

/// Radar run の data quality status。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataQualityStatus {
    Ok,
    Warning,
    Critical,
}

/// data acquisition 後に CLI が従う pipeline policy。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarPipelinePlan {
    pub data_acquisition: DataAcquisitionSummary,
    pub should_persist_history: bool,
    pub should_enter_pipeline_body: bool,
    pub data_quality_status: DataQualityStatus,
}

/// pipeline body に入る前の data acquisition 準備結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadarPreparedData<T> {
    pub successful_items: Vec<T>,
    pub failed_symbols: Vec<String>,
    pub rate_limited_symbols: Vec<String>,
    pub summary: DataAcquisitionSummary,
    pub plan: RadarPipelinePlan,
}

/// Radar pipeline orchestration の application use case。
#[derive(Debug, Default, Clone, Copy)]
pub struct RadarPipelineUseCase;

/// Radar run の runtime context。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadarRunContext {
    pub date: chrono::NaiveDate,
    pub timestamp: String,
}

impl RadarRunContext {
    pub fn new(now: chrono::DateTime<chrono::Local>) -> Self {
        Self {
            date: now.date_naive(),
            timestamp: now.to_rfc3339(),
        }
    }

    pub fn date_string(&self) -> String {
        self.date.to_string()
    }

    pub fn initial_run_outcome(
        &self,
        evidence_collection: crate::features::shared::application::run_status::DeliveryStatus,
    ) -> crate::features::shared::application::run_status::RunOutcome {
        crate::features::shared::application::run_status::RunOutcome {
            date: self.date_string(),
            timestamp: self.timestamp.clone(),
            evidence_collection,
            ..Default::default()
        }
    }

    pub fn initial_run_outcome_with_data_quality(
        &self,
        evidence_collection: crate::features::shared::application::run_status::DeliveryStatus,
        data_quality_status: DataQualityStatus,
    ) -> crate::features::shared::application::run_status::RunOutcome {
        let mut outcome = self.initial_run_outcome(evidence_collection);
        outcome.data_quality = data_quality_status.as_str().to_string();
        outcome
    }
}

impl DataQualityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warning => "WARNING",
            Self::Critical => "CRITICAL",
        }
    }
}

impl RadarPipelineUseCase {
    pub fn new() -> Self {
        Self
    }

    /// provider fetch の結果列を data acquisition result へ集約する。
    pub fn collect_data_acquisition<T, E, I>(self, results: I) -> DataAcquisitionResult<T>
    where
        I: IntoIterator<Item = (Result<T, E>, String)>,
    {
        let mut successful_items = Vec::new();
        let mut failed_symbols = Vec::new();

        for (result, symbol) in results {
            match result {
                Ok(item) => successful_items.push(item),
                Err(_) => failed_symbols.push(symbol),
            }
        }

        DataAcquisitionResult::new(successful_items, failed_symbols)
    }

    /// data acquisition result を pipeline 実行前の policy 付き payload へ変換する。
    pub fn prepare_data_acquisition<T>(
        self,
        data_acquisition: DataAcquisitionResult<T>,
    ) -> RadarPreparedData<T> {
        let summary = data_acquisition.summary();
        let plan = summary.pipeline_plan();
        let (successful_items, failed_symbols) = data_acquisition.into_parts();

        RadarPreparedData {
            successful_items,
            failed_symbols,
            rate_limited_symbols: Vec::new(),
            summary,
            plan,
        }
    }

    /// provider fetch の結果列から pipeline 実行前の payload まで一括で構築する。
    pub fn prepare_from_fetch_results<T, E, I>(self, results: I) -> RadarPreparedData<T>
    where
        I: IntoIterator<Item = (Result<T, E>, String)>,
    {
        let data_acquisition = self.collect_data_acquisition(results);
        self.prepare_data_acquisition(data_acquisition)
    }

    /// market data port を介して有効な監視対象を並列取得する。
    pub async fn acquire_market_data(
        self,
        provider: Arc<dyn crate::features::radar::application::provider::MarketDataProvider>,
        watchlist: &[crate::features::radar::domain::rules::WatchlistEntry],
    ) -> RadarPreparedData<(
        crate::features::shared::domain::market_data::TickerHistory<'static>,
        crate::features::radar::domain::rules::WatchlistEntry,
    )> {
        let fetches = stream::iter(watchlist.iter().filter(|entry| entry.enable).cloned())
            .map(|entry| {
                let provider_ref = Arc::clone(&provider);
                async move {
                    let symbol = entry.symbol.clone();
                    (
                        provider_ref
                            .fetch_history(&entry.symbol, None, None)
                            .await
                            .map(|history| (history, entry)),
                        symbol,
                    )
                }
            })
            .buffer_unordered(10);

        let results = fetches.collect::<Vec<_>>().await;
        let rate_limited_symbols = results
            .iter()
            .filter(|(result, _)| {
                result
                    .as_ref()
                    .err()
                    .is_some_and(|error| is_rate_limited_message(&error.to_string()))
            })
            .map(|(_, symbol)| symbol.clone())
            .collect::<Vec<_>>();
        let mut prepared = self.prepare_from_fetch_results(results);
        prepared.rate_limited_symbols = rate_limited_symbols;
        prepared
    }

    /// 取得済みの market data から日次判定 outcome を構築する。
    ///
    /// Interface は IO を調停するだけとし、完全取得失敗時の診断 packet を含む判定方針は
    /// application use case が保持する。
    pub fn decide_daily<'a>(
        ticker_histories: &[(
            crate::features::shared::domain::market_data::TickerHistory<'a>,
            &crate::features::radar::domain::rules::WatchlistEntry,
        )],
        failed_fetch_count: usize,
        rules: &crate::features::radar::domain::rules::ParsedRules,
        history: &[crate::features::radar::domain::decision::DecisionPacket],
        evidence_history: &[crate::features::radar::domain::trend_cohesion::AutomatedEvidenceRecord],
        positions: &std::collections::HashMap<String, (f64, f64)>,
        date: chrono::NaiveDate,
    ) -> anyhow::Result<RadarDecisionOutcome> {
        if ticker_histories.is_empty() {
            return Ok(RadarDecisionOutcome {
                packet: build_diagnostic_packet(date),
                decisioning: build_full_fetch_failure_status(failed_fetch_count),
            });
        }

        crate::features::radar::application::engine::Engine::run_daily_pipeline(
            ticker_histories,
            rules,
            history,
            evidence_history,
            positions,
        )
        .map(build_successful_decision_outcome)
    }
}

fn is_rate_limited_message(message: &str) -> bool {
    message.contains("429")
}

impl DataAcquisitionSummary {
    pub fn new(successful_fetches: usize, failed_fetches: usize) -> Self {
        Self {
            successful_fetches,
            failed_fetches,
        }
    }

    /// run history へ正式な decision packet を保存してよいかを判定する。
    pub fn should_persist_decision_history(self) -> bool {
        should_persist_decision_history(self.successful_fetches, self.failed_fetches)
    }

    /// すべての取得対象が失敗したかを返す。
    pub fn is_full_failure(self) -> bool {
        self.successful_fetches == 0 && self.failed_fetches > 0
    }

    /// data quality log 用の status を返す。
    pub fn data_quality_status(self) -> DataQualityStatus {
        if self.is_full_failure() {
            DataQualityStatus::Critical
        } else if self.failed_fetches > 0 {
            DataQualityStatus::Warning
        } else {
            DataQualityStatus::Ok
        }
    }

    /// run_pipeline の main body へ進む必要があるかを判定する。
    pub fn should_enter_pipeline_body(self) -> bool {
        self.successful_fetches > 0 || self.failed_fetches > 0
    }

    /// data acquisition summary から pipeline policy を構築する。
    pub fn pipeline_plan(self) -> RadarPipelinePlan {
        RadarPipelinePlan {
            data_acquisition: self,
            should_persist_history: self.should_persist_decision_history(),
            should_enter_pipeline_body: self.should_enter_pipeline_body(),
            data_quality_status: self.data_quality_status(),
        }
    }
}

/// Radar run の data acquisition 成功・失敗結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataAcquisitionResult<T> {
    pub successful_items: Vec<T>,
    pub failed_symbols: Vec<String>,
}

/// account snapshot 保存用の入力値。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccountSnapshotInput<'a> {
    pub date: &'a str,
    pub global_budget: f64,
    pub max_daily_budget: Option<f64>,
    pub daily_traded: f64,
    pub buying_power: f64,
    pub current_exposure: f64,
    pub realized_pl: f64,
    pub failed_fetch_count: usize,
}

/// portfolio snapshot の position 明細。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PortfolioPositionSnapshot {
    pub symbol: String,
    pub qty: f64,
    pub avg_price: f64,
    pub market_value_estimate: f64,
}

/// portfolio snapshot 保存用の typed payload。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PortfolioSnapshot {
    pub date: String,
    pub realized_pl: f64,
    pub current_exposure: f64,
    pub position_count: usize,
    pub positions: Vec<PortfolioPositionSnapshot>,
}

/// account snapshot 保存用の typed payload。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountSnapshot {
    pub date: String,
    pub global_budget: f64,
    pub max_daily_budget: Option<f64>,
    pub daily_traded: f64,
    pub buying_power_estimate: f64,
    pub current_exposure: f64,
    pub realized_pl: f64,
    pub failed_fetch_count: usize,
}

/// data quality log 保存用の typed payload。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataQualityLog {
    pub timestamp: String,
    pub date: String,
    pub successful_fetches: usize,
    pub failed_fetches: usize,
    pub failed_symbols: Vec<String>,
    pub status: &'static str,
}

/// Radar decisioning の application-level outcome。
#[derive(Debug, Clone)]
pub struct RadarDecisionOutcome {
    pub packet: crate::features::radar::domain::decision::DecisionPacket,
    pub decisioning: crate::features::shared::application::run_status::DeliveryStatus,
}

impl<T> DataAcquisitionResult<T> {
    pub fn new(successful_items: Vec<T>, failed_symbols: Vec<String>) -> Self {
        Self {
            successful_items,
            failed_symbols,
        }
    }

    /// 成功・失敗件数を application policy 用の summary へ変換する。
    pub fn summary(&self) -> DataAcquisitionSummary {
        DataAcquisitionSummary::new(self.successful_items.len(), self.failed_symbols.len())
    }

    /// run history へ正式な decision packet を保存してよいかを判定する。
    pub fn should_persist_decision_history(&self) -> bool {
        self.summary().should_persist_decision_history()
    }

    /// すべての取得対象が失敗したかを返す。
    pub fn is_full_failure(&self) -> bool {
        self.summary().is_full_failure()
    }

    /// data quality log 用の status を返す。
    pub fn data_quality_status(&self) -> DataQualityStatus {
        self.summary().data_quality_status()
    }

    /// run_pipeline の main body へ進む必要があるかを判定する。
    pub fn should_enter_pipeline_body(&self) -> bool {
        self.summary().should_enter_pipeline_body()
    }

    /// data acquisition result から pipeline policy を構築する。
    pub fn pipeline_plan(&self) -> RadarPipelinePlan {
        self.summary().pipeline_plan()
    }

    pub fn into_parts(self) -> (Vec<T>, Vec<String>) {
        (self.successful_items, self.failed_symbols)
    }
}

/// データ取得結果に基づく decision history persistence policy。
pub fn should_persist_decision_history(successful_fetches: usize, failed_fetches: usize) -> bool {
    successful_fetches > 0 || failed_fetches == 0
}

/// 正常に生成された decision packet を application outcome へ変換する。
pub fn build_successful_decision_outcome(
    packet: crate::features::radar::domain::decision::DecisionPacket,
) -> RadarDecisionOutcome {
    RadarDecisionOutcome {
        packet,
        decisioning: crate::features::shared::application::run_status::DeliveryStatus::Succeeded,
    }
}

/// decisioning failure 用の status を構築する。
pub fn build_decisioning_failure_status(
    reason: impl Into<String>,
) -> crate::features::shared::application::run_status::DeliveryStatus {
    crate::features::shared::application::run_status::DeliveryStatus::Failed {
        reason: reason.into(),
    }
}

/// 100% data acquisition failure 用の diagnostic packet を構築する。
pub fn build_diagnostic_packet(
    date: chrono::NaiveDate,
) -> crate::features::radar::domain::decision::DecisionPacket {
    crate::features::radar::domain::decision::DecisionPacket {
        date,
        ..Default::default()
    }
}

/// 100% data acquisition failure 用の decisioning status を構築する。
pub fn build_full_fetch_failure_status(
    failed_fetch_count: usize,
) -> crate::features::shared::application::run_status::DeliveryStatus {
    build_decisioning_failure_status(format!(
        "100% data acquisition failure: {} symbols failed",
        failed_fetch_count
    ))
}

/// portfolio snapshot 保存用 payload を構築する。
pub fn build_portfolio_snapshot(
    date: &str,
    realized_pl: f64,
    current_exposure: f64,
    positions: &std::collections::HashMap<String, (f64, f64)>,
) -> PortfolioSnapshot {
    let mut positions = positions
        .iter()
        .map(|(symbol, (qty, avg_price))| PortfolioPositionSnapshot {
            symbol: symbol.clone(),
            qty: *qty,
            avg_price: *avg_price,
            market_value_estimate: qty * avg_price,
        })
        .collect::<Vec<_>>();
    positions.sort_by(|left, right| left.symbol.cmp(&right.symbol));
    PortfolioSnapshot {
        date: date.to_string(),
        realized_pl,
        current_exposure,
        position_count: positions.len(),
        positions,
    }
}

/// account snapshot 保存用 payload を構築する。
pub fn build_account_snapshot(input: AccountSnapshotInput<'_>) -> AccountSnapshot {
    AccountSnapshot {
        date: input.date.to_string(),
        global_budget: input.global_budget,
        max_daily_budget: input.max_daily_budget,
        daily_traded: input.daily_traded,
        buying_power_estimate: input.buying_power,
        current_exposure: input.current_exposure,
        realized_pl: input.realized_pl,
        failed_fetch_count: input.failed_fetch_count,
    }
}

/// data quality log 保存用 payload を構築する。
pub fn build_data_quality_log(
    timestamp: &str,
    date: &str,
    summary: DataAcquisitionSummary,
    failed_symbols: &[String],
) -> DataQualityLog {
    DataQualityLog {
        timestamp: timestamp.to_string(),
        date: date.to_string(),
        successful_fetches: summary.successful_fetches,
        failed_fetches: summary.failed_fetches,
        failed_symbols: failed_symbols.to_vec(),
        status: summary.data_quality_status().as_str(),
    }
}

/// run status 用の state machine summary を構築する。
pub fn build_state_machine_summary(
    prev_market_state: Option<crate::features::shared::domain::market_regime::MarketState>,
    current_market_state: crate::features::shared::domain::market_regime::MarketState,
    transition_audit: Option<
        &crate::features::shared::domain::market_regime::MarketTransitionAudit,
    >,
    should_persist_history: bool,
) -> crate::features::shared::application::run_status::StateMachineSummary {
    let mut summary = crate::features::shared::application::run_status::StateMachineSummary {
        from_state: format!(
            "{:?}",
            prev_market_state
                .unwrap_or(crate::features::shared::domain::market_regime::MarketState::IGNITION)
        ),
        to_state: if should_persist_history {
            format!("{:?}", current_market_state)
        } else {
            "DATA_UNAVAILABLE".to_string()
        },
        ..Default::default()
    };

    if let Some(audit) = transition_audit {
        summary.reset_confirmed = audit.reset_gate_passed;
        summary.reset_blocked = audit.is_reset_blocked;
        summary.soft_reset_applied = audit.soft_reset_applied;
        summary.duration_locked = audit.duration_locked;
        summary.defensive_override = audit.defensive_override;
        summary.core_breakdown = audit.core_breakdown;
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::application::provider::{MarketDataProvider, TickerHistory};
    use crate::features::shared::domain::market_regime::{
        LifecycleState, MarketState, MarketTransitionAudit,
    };
    use std::borrow::Cow;
    use std::collections::HashMap;

    #[test]
    fn radar_application_boundary_persists_when_at_least_one_fetch_succeeds() {
        let summary = DataAcquisitionSummary::new(1, 8);
        assert!(summary.should_persist_decision_history());
        assert!(!summary.is_full_failure());
        assert!(summary.should_enter_pipeline_body());
    }

    #[test]
    fn radar_application_boundary_does_not_persist_full_fetch_failure() {
        let summary = DataAcquisitionSummary::new(0, 9);
        assert!(!summary.should_persist_decision_history());
        assert!(summary.is_full_failure());
        assert!(summary.should_enter_pipeline_body());
    }

    #[test]
    fn radar_application_boundary_empty_fetch_set_is_non_failure() {
        let summary = DataAcquisitionSummary::new(0, 0);
        assert!(summary.should_persist_decision_history());
        assert!(!summary.is_full_failure());
        assert_eq!(summary.data_quality_status(), DataQualityStatus::Ok);
        assert!(!summary.should_enter_pipeline_body());
    }

    #[test]
    fn radar_application_boundary_result_summarizes_fetch_outcome() {
        let result = DataAcquisitionResult::new(vec!["AAPL"], vec!["MSFT".to_string()]);
        let summary = result.summary();
        let plan = result.pipeline_plan();

        assert_eq!(summary.successful_fetches, 1);
        assert_eq!(summary.failed_fetches, 1);
        assert!(result.should_persist_decision_history());
        assert!(!result.is_full_failure());
        assert_eq!(result.data_quality_status(), DataQualityStatus::Warning);
        assert!(result.should_enter_pipeline_body());
        assert_eq!(plan.data_acquisition, summary);
        assert!(plan.should_persist_history);
        assert!(plan.should_enter_pipeline_body);
        assert_eq!(plan.data_quality_status, DataQualityStatus::Warning);
    }

    #[test]
    fn rate_limit_classification_requires_explicit_429_fact() {
        assert!(is_rate_limited_message("provider returned HTTP 429"));
        assert!(!is_rate_limited_message("provider returned HTTP 500"));
        assert!(!is_rate_limited_message("network timeout"));
    }

    #[test]
    fn radar_application_boundary_result_preserves_parts() {
        let result = DataAcquisitionResult::new(vec!["AAPL"], vec!["MSFT".to_string()]);
        let (successful_items, failed_symbols) = result.into_parts();

        assert_eq!(successful_items, vec!["AAPL"]);
        assert_eq!(failed_symbols, vec!["MSFT".to_string()]);
    }

    #[test]
    fn radar_application_boundary_status_matches_existing_policy() {
        assert_eq!(
            DataAcquisitionSummary::new(1, 0)
                .data_quality_status()
                .as_str(),
            "OK"
        );
        assert_eq!(
            DataAcquisitionSummary::new(1, 8)
                .data_quality_status()
                .as_str(),
            "WARNING"
        );
        assert_eq!(
            DataAcquisitionSummary::new(0, 9)
                .data_quality_status()
                .as_str(),
            "CRITICAL"
        );
    }

    #[test]
    fn radar_application_boundary_builds_pipeline_plan() {
        let plan = DataAcquisitionSummary::new(0, 9).pipeline_plan();

        assert_eq!(plan.data_acquisition, DataAcquisitionSummary::new(0, 9));
        assert!(!plan.should_persist_history);
        assert!(plan.should_enter_pipeline_body);
        assert_eq!(plan.data_quality_status, DataQualityStatus::Critical);
    }

    #[test]
    fn radar_application_boundary_builds_run_context() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-24T09:30:00+09:00")
            .unwrap()
            .with_timezone(&chrono::Local);
        let context = RadarRunContext::new(now);
        let outcome = context.initial_run_outcome(
            crate::features::shared::application::run_status::DeliveryStatus::Skipped,
        );

        let expected_date = now.date_naive().to_string();
        let parsed_timestamp = chrono::DateTime::parse_from_rfc3339(&context.timestamp).unwrap();

        assert_eq!(context.date_string(), expected_date);
        assert_eq!(parsed_timestamp.timestamp(), now.timestamp());
        assert_eq!(outcome.date, expected_date);
        assert_eq!(
            outcome.evidence_collection,
            crate::features::shared::application::run_status::DeliveryStatus::Skipped
        );
    }

    #[test]
    fn radar_application_boundary_projects_data_quality_into_run_status() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-24T09:30:00+09:00")
            .unwrap()
            .with_timezone(&chrono::Local);
        let context = RadarRunContext::new(now);
        for (status, expected) in [
            (DataQualityStatus::Ok, "OK"),
            (DataQualityStatus::Warning, "WARNING"),
            (DataQualityStatus::Critical, "CRITICAL"),
        ] {
            let outcome = context.initial_run_outcome_with_data_quality(
                crate::features::shared::application::run_status::DeliveryStatus::Skipped,
                status,
            );
            assert_eq!(outcome.data_quality, expected);
        }
    }

    #[test]
    fn radar_application_boundary_use_case_prepares_data_acquisition() {
        let result = DataAcquisitionResult::new(vec!["AAPL"], vec!["MSFT".to_string()]);
        let prepared = RadarPipelineUseCase::new().prepare_data_acquisition(result);

        assert_eq!(prepared.successful_items, vec!["AAPL"]);
        assert_eq!(prepared.failed_symbols, vec!["MSFT".to_string()]);
        assert_eq!(prepared.summary, DataAcquisitionSummary::new(1, 1));
        assert!(prepared.plan.should_persist_history);
        assert!(prepared.plan.should_enter_pipeline_body);
        assert_eq!(
            prepared.plan.data_quality_status,
            DataQualityStatus::Warning
        );
    }

    #[test]
    fn radar_application_boundary_use_case_collects_fetch_results() {
        let results: Vec<(Result<&str, &str>, String)> = vec![
            (Ok("AAPL-history"), "AAPL".to_string()),
            (Err("timeout"), "MSFT".to_string()),
        ];
        let data_acquisition = RadarPipelineUseCase::new().collect_data_acquisition(results);

        assert_eq!(data_acquisition.successful_items, vec!["AAPL-history"]);
        assert_eq!(data_acquisition.failed_symbols, vec!["MSFT".to_string()]);
    }

    #[test]
    fn radar_application_boundary_use_case_prepares_from_fetch_results() {
        let results: Vec<(Result<&str, &str>, String)> = vec![
            (Ok("AAPL-history"), "AAPL".to_string()),
            (Err("timeout"), "MSFT".to_string()),
        ];
        let prepared = RadarPipelineUseCase::new().prepare_from_fetch_results(results);

        assert_eq!(prepared.successful_items, vec!["AAPL-history"]);
        assert_eq!(prepared.failed_symbols, vec!["MSFT".to_string()]);
        assert_eq!(prepared.summary, DataAcquisitionSummary::new(1, 1));
        assert_eq!(
            prepared.plan.data_quality_status,
            DataQualityStatus::Warning
        );
    }

    struct FixtureMarketDataProvider;

    #[async_trait::async_trait]
    impl MarketDataProvider for FixtureMarketDataProvider {
        async fn fetch_history(
            &self,
            symbol: &str,
            _start_date: Option<time::OffsetDateTime>,
            _end_date: Option<time::OffsetDateTime>,
        ) -> anyhow::Result<TickerHistory<'static>> {
            if symbol == "NVDA" {
                Ok(TickerHistory {
                    symbol: symbol.to_string(),
                    bars: Cow::Owned(Vec::new()),
                    total_trading_days: 0,
                    latest_quote_timestamp: None,
                })
            } else {
                Err(anyhow::anyhow!("fixture fetch failure"))
            }
        }
    }

    #[tokio::test]
    async fn radar_application_use_case_acquires_market_data_through_port() {
        let watchlist = vec![
            crate::features::radar::domain::rules::WatchlistEntry {
                symbol: "NVDA".to_string(),
                enable: true,
                ..Default::default()
            },
            crate::features::radar::domain::rules::WatchlistEntry {
                symbol: "MSFT".to_string(),
                enable: true,
                ..Default::default()
            },
        ];
        let provider: Arc<dyn MarketDataProvider> = Arc::new(FixtureMarketDataProvider);

        let prepared = RadarPipelineUseCase::new()
            .acquire_market_data(provider, &watchlist)
            .await;

        assert_eq!(prepared.successful_items.len(), 1);
        assert_eq!(prepared.failed_symbols, vec!["MSFT".to_string()]);
        assert_eq!(
            prepared.plan.data_quality_status,
            DataQualityStatus::Warning
        );
    }

    #[test]
    fn radar_application_use_case_owns_full_fetch_failure_decision() {
        let outcome = RadarPipelineUseCase::decide_daily(
            &[],
            9,
            &crate::features::radar::domain::rules::ParsedRules::default(),
            &[],
            &[],
            &HashMap::new(),
            chrono::NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
        )
        .unwrap();

        assert_eq!(outcome.packet.date.to_string(), "2026-05-25");
        assert!(matches!(
            outcome.decisioning,
            crate::features::shared::application::run_status::DeliveryStatus::Failed { .. }
        ));
    }

    #[test]
    fn radar_application_boundary_builds_persistence_payloads() {
        let mut positions = HashMap::new();
        positions.insert("NVDA".to_string(), (2.0, 100.0));
        let portfolio = build_portfolio_snapshot("2026-05-24", 12.5, 200.0, &positions);
        let account = build_account_snapshot(AccountSnapshotInput {
            date: "2026-05-24",
            global_budget: 1000.0,
            max_daily_budget: Some(250.0),
            daily_traded: 50.0,
            buying_power: 800.0,
            current_exposure: 200.0,
            realized_pl: 12.5,
            failed_fetch_count: 1,
        });
        let failed_symbols = vec!["MSFT".to_string()];
        let data_quality = build_data_quality_log(
            "2026-05-24T00:00:00+09:00",
            "2026-05-24",
            DataAcquisitionSummary::new(1, 1),
            &failed_symbols,
        );

        assert_eq!(portfolio.date, "2026-05-24");
        assert_eq!(portfolio.position_count, 1);
        assert_eq!(portfolio.positions[0].market_value_estimate, 200.0);
        assert_eq!(account.failed_fetch_count, 1);
        assert_eq!(account.buying_power_estimate, 800.0);
        assert_eq!(data_quality.status, "WARNING");
        assert_eq!(data_quality.failed_symbols[0], "MSFT");
    }

    #[test]
    fn radar_application_boundary_builds_full_failure_diagnostic_output() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        let packet = build_diagnostic_packet(date);
        let status = build_full_fetch_failure_status(9);

        assert_eq!(packet.date, date);
        assert!(packet.assets.is_empty());
        assert_eq!(
            status,
            crate::features::shared::application::run_status::DeliveryStatus::Failed {
                reason: "100% data acquisition failure: 9 symbols failed".to_string()
            }
        );
    }

    #[test]
    fn radar_application_boundary_builds_decision_outcome_status() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        let packet = build_diagnostic_packet(date);
        let outcome = build_successful_decision_outcome(packet.clone());
        let failed = build_decisioning_failure_status("engine failed");

        assert_eq!(outcome.packet.date, packet.date);
        assert!(outcome.packet.assets.is_empty());
        assert_eq!(
            outcome.decisioning,
            crate::features::shared::application::run_status::DeliveryStatus::Succeeded
        );
        assert_eq!(
            failed,
            crate::features::shared::application::run_status::DeliveryStatus::Failed {
                reason: "engine failed".to_string()
            }
        );
    }

    #[test]
    fn radar_application_boundary_builds_state_machine_summary() {
        let audit = MarketTransitionAudit {
            from: LifecycleState::IGNITION,
            to: LifecycleState::NEWBORN,
            is_reset_blocked: true,
            is_downgrade_clamped: false,
            core_breakdown: true,
            duration_locked: true,
            trend_dominant: false,
            reset_gate_passed: true,
            indicator_cap: LifecycleState::NEWBORN,
            soft_reset_applied: true,
            defensive_override: true,
        };

        let summary = build_state_machine_summary(
            Some(MarketState::IGNITION),
            MarketState::DEFENSIVE,
            Some(&audit),
            true,
        );

        assert_eq!(summary.from_state, "IGNITION");
        assert_eq!(summary.to_state, "DEFENSIVE");
        assert!(summary.reset_confirmed);
        assert!(summary.reset_blocked);
        assert!(summary.soft_reset_applied);
        assert!(summary.duration_locked);
        assert!(summary.defensive_override);
        assert!(summary.core_breakdown);

        let unavailable = build_state_machine_summary(None, MarketState::ESTABLISHED, None, false);
        assert_eq!(unavailable.from_state, "IGNITION");
        assert_eq!(unavailable.to_state, "DATA_UNAVAILABLE");
    }
}

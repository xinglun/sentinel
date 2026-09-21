use crate::config;
use crate::features::research::domain::expectation::{
    derive_lifecycle_state, ExpectationEventType, ExpectationObservation, ExpectationPressure,
    RevisionDirection, SourceHealth, SurpriseState,
};
use crate::features::research::infrastructure::expectation_source_adapter::{
    parse_period_date, ConsensusFetchResult, FinnhubConsensusMetric,
    FinnhubExpectationSourceAdapter,
};
use chrono::{Datelike, NaiveDate};

use crate::features::research::interface::expectation_report_builder::{
    build_expectation_layer_fixture_snapshot, ExpectationLayerSnapshot,
};

const PROVIDER: &str = "Finnhub";

/// Radar が指定した取引日を Expectation の観測日として source/replay 分岐に渡す。
pub(crate) fn build_expectation_layer_snapshot_for_market_date(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
) -> ExpectationLayerSnapshot {
    if !FinnhubExpectationSourceAdapter::has_credential(app_config) {
        return unavailable_snapshot(as_of_date, "Finnhub credential is not configured");
    }

    let current_period = quarter_label_from_date(as_of_date);
    let Some(adapter) = FinnhubExpectationSourceAdapter::new(app_config) else {
        return unavailable_snapshot(as_of_date, "Finnhub provider initialization failed");
    };

    ExpectationLayerSnapshot {
        as_of_date,
        decision_weight_percent: 0,
        trade_signal: false,
        gate_effect: "none".to_string(),
        execution_effect: "none".to_string(),
        position_sizing_effect: "none".to_string(),
        observations: vec![
            unavailable_observation(
                "TSLA",
                &current_period,
                ExpectationEventType::DeliveryConsensus,
                "deliveries",
                "Tesla には直接の consensus endpoint がないため、source unavailable として扱う。",
                as_of_date,
            ),
            consensus_observation(
                &adapter,
                "NVDA",
                "earnings_usd_per_share",
                ExpectationEventType::EarningsConsensus,
                FinnhubConsensusMetric::Eps,
                "$ EPS",
                "利益期待は Finnhub EPS estimates から取得される。",
                &current_period,
                as_of_date,
            ),
            consensus_observation(
                &adapter,
                "NVDA",
                "revenue_usd_billion",
                ExpectationEventType::RevenueConsensus,
                FinnhubConsensusMetric::Revenue,
                "$B revenue",
                "売上期待は Finnhub revenue estimates から取得される。",
                &current_period,
                as_of_date,
            ),
            consensus_margin_observation(
                &adapter,
                "NVDA",
                ExpectationEventType::MarginConsensus,
                "gross_margin_pct",
                "粗利率期待は Finnhub gross income / revenue estimates から推定される。",
                &current_period,
                as_of_date,
            ),
            unavailable_observation(
                "GOOG",
                &current_period,
                ExpectationEventType::CloudGrowthConsensus,
                "cloud_growth_pct",
                "Cloud growth の直接 consensus endpoint がないため、source unavailable として扱う。",
                as_of_date,
            ),
            unavailable_observation(
                "GOOG",
                &current_period,
                ExpectationEventType::CapexConsensus,
                "capex_usd_billion",
                "CAPEX の直接 consensus endpoint がないため、source unavailable として扱う。",
                as_of_date,
            ),
            consensus_observation(
                &adapter,
                "GOOG",
                "earnings_usd_per_share",
                ExpectationEventType::EarningsConsensus,
                FinnhubConsensusMetric::Eps,
                "$ EPS",
                "利益期待は Finnhub EPS estimates から取得される。",
                &current_period,
                as_of_date,
            ),
            unavailable_observation(
                "MSFT",
                &current_period,
                ExpectationEventType::CloudGrowthConsensus,
                "cloud_growth_pct",
                "Cloud growth の直接 consensus endpoint がないため、source unavailable として扱う。",
                as_of_date,
            ),
            unavailable_observation(
                "MSFT",
                &current_period,
                ExpectationEventType::ProductEventExpectation,
                "product_event",
                "Product event expectation の consensus endpoint がないため、source unavailable として扱う。",
                as_of_date,
            ),
            unavailable_observation(
                "MSFT",
                &current_period,
                ExpectationEventType::CapexConsensus,
                "capex_usd_billion",
                "CAPEX の直接 consensus endpoint がないため、source unavailable として扱う。",
                as_of_date,
            ),
            consensus_observation(
                &adapter,
                "PLTR",
                "revenue_usd_billion",
                ExpectationEventType::RevenueConsensus,
                FinnhubConsensusMetric::Revenue,
                "$B revenue",
                "売上期待は Finnhub revenue estimates から取得される。",
                &current_period,
                as_of_date,
            ),
            unavailable_observation(
                "PLTR",
                &current_period,
                ExpectationEventType::UserGrowthConsensus,
                "user_growth_pct",
                "User growth の直接 consensus endpoint がないため、source unavailable として扱う。",
                as_of_date,
            ),
            consensus_margin_observation(
                &adapter,
                "PLTR",
                ExpectationEventType::MarginConsensus,
                "gross_margin_pct",
                "粗利率期待は Finnhub gross income / revenue estimates から推定される。",
                &current_period,
                as_of_date,
            ),
            unavailable_observation(
                "ISRG",
                &current_period,
                ExpectationEventType::ProcedureGrowthConsensus,
                "procedure_growth_pct",
                "Procedure growth の直接 consensus endpoint がないため、source unavailable として扱う。",
                as_of_date,
            ),
            consensus_observation(
                &adapter,
                "ISRG",
                "revenue_usd_billion",
                ExpectationEventType::RevenueConsensus,
                FinnhubConsensusMetric::Revenue,
                "$B revenue",
                "売上期待は Finnhub revenue estimates から取得される。",
                &current_period,
                as_of_date,
            ),
            consensus_margin_observation(
                &adapter,
                "ISRG",
                ExpectationEventType::MarginConsensus,
                "gross_margin_pct",
                "粗利率期待は Finnhub gross income / revenue estimates から推定される。",
                &current_period,
                as_of_date,
            ),
        ],
    }
}

fn unavailable_snapshot(as_of_date: NaiveDate, reason: &str) -> ExpectationLayerSnapshot {
    let fixture = build_expectation_layer_fixture_snapshot();
    let observations = fixture
        .observations
        .into_iter()
        .map(|observation| {
            unavailable_observation(
                &observation.subject,
                &observation.period,
                observation.event_type,
                &observation.unit,
                reason,
                as_of_date,
            )
        })
        .collect();
    ExpectationLayerSnapshot {
        as_of_date,
        decision_weight_percent: 0,
        trade_signal: false,
        gate_effect: "none".to_string(),
        execution_effect: "none".to_string(),
        position_sizing_effect: "none".to_string(),
        observations,
    }
}

#[allow(clippy::too_many_arguments)]
fn consensus_observation(
    adapter: &FinnhubExpectationSourceAdapter<'_>,
    subject: &str,
    unit: &str,
    event_type: ExpectationEventType,
    metric: FinnhubConsensusMetric,
    unit_suffix: &str,
    _not_applicable_reason: &str,
    period: &str,
    as_of_date: NaiveDate,
) -> ExpectationObservation {
    match adapter.fetch_consensus_series(subject, metric, as_of_date) {
        ConsensusFetchResult::Available(series) if has_consensus_center(&series) => {
            build_consensus_observation(subject, unit, event_type, series, unit_suffix, as_of_date)
        }
        ConsensusFetchResult::Available(_) | ConsensusFetchResult::NoConsensus => {
            unavailable_observation(
                subject,
                period,
                event_type,
                unit,
                &no_consensus_reason(event_type),
                as_of_date,
            )
        }
        ConsensusFetchResult::ProviderUnavailable => unavailable_observation(
            subject,
            period,
            event_type,
            unit,
            &provider_unavailable_reason(event_type),
            as_of_date,
        ),
    }
}

fn consensus_margin_observation(
    adapter: &FinnhubExpectationSourceAdapter<'_>,
    subject: &str,
    event_type: ExpectationEventType,
    unit: &str,
    _not_applicable_reason: &str,
    period: &str,
    as_of_date: NaiveDate,
) -> ExpectationObservation {
    let revenue =
        adapter.fetch_consensus_series(subject, FinnhubConsensusMetric::Revenue, as_of_date);
    let gross_income =
        adapter.fetch_consensus_series(subject, FinnhubConsensusMetric::GrossIncome, as_of_date);

    match (gross_income, revenue) {
        (
            ConsensusFetchResult::Available(gross_income),
            ConsensusFetchResult::Available(revenue),
        ) if has_consensus_center(&gross_income) && has_positive_consensus_center(&revenue) => {
            build_margin_observation(subject, event_type, gross_income, revenue, unit, as_of_date)
        }
        (gross_income, revenue)
            if matches!(gross_income, ConsensusFetchResult::ProviderUnavailable)
                || matches!(revenue, ConsensusFetchResult::ProviderUnavailable) =>
        {
            unavailable_observation(
                subject,
                period,
                event_type,
                unit,
                &provider_unavailable_reason(event_type),
                as_of_date,
            )
        }
        _ => unavailable_observation(
            subject,
            period,
            event_type,
            unit,
            &no_consensus_reason(event_type),
            as_of_date,
        ),
    }
}

fn provider_unavailable_reason(event_type: ExpectationEventType) -> String {
    format!(
        "Finnhub の {} 提供元を利用できず、期待データを取得できない。",
        metric_label(event_type)
    )
}

fn no_consensus_reason(event_type: ExpectationEventType) -> String {
    format!(
        "市場に利用可能な {} consensus がない。",
        metric_label(event_type)
    )
}

fn has_consensus_center(
    series: &crate::features::research::infrastructure::expectation_source_adapter::ConsensusSeries,
) -> bool {
    series.average.or(series.median).is_some()
}

fn has_positive_consensus_center(
    series: &crate::features::research::infrastructure::expectation_source_adapter::ConsensusSeries,
) -> bool {
    series
        .average
        .or(series.median)
        .is_some_and(|value| value > 0.0)
}

fn build_consensus_observation(
    subject: &str,
    unit: &str,
    event_type: ExpectationEventType,
    series: crate::features::research::infrastructure::expectation_source_adapter::ConsensusSeries,
    unit_suffix: &str,
    as_of_date: NaiveDate,
) -> ExpectationObservation {
    let expected = series.average.or(series.median).unwrap_or_default();
    let expected_value = format_consensus_value(expected, unit_suffix);
    let period = quarter_label_from_series(&series.period);
    let lifecycle_state = derive_lifecycle_state(&period, as_of_date, "未発表", None, None, None);
    let revision_direction = revision_direction(series.previous_average, series.average);
    let spread = spread_ratio(series.high, series.low, series.average);

    ExpectationObservation {
        subject: subject.to_string(),
        period,
        as_of_date,
        event_type,
        lifecycle_state,
        expected_value: format_consensus_value(expected, unit_suffix),
        actual_value: "未発表".to_string(),
        result: None,
        surprise_percent: None,
        market_reaction: None,
        released_at: None,
        archived_at: None,
        unit: unit.to_string(),
        consensus_source: format!("{PROVIDER} {} estimates", metric_label(event_type)),
        estimate_count: series.count,
        estimate_high: series
            .high
            .map(|value| format_consensus_value(value, unit_suffix)),
        estimate_low: series
            .low
            .map(|value| format_consensus_value(value, unit_suffix)),
        estimate_median: series
            .median
            .map(|value| format_consensus_value(value, unit_suffix)),
        estimate_average: series
            .average
            .map(|value| format_consensus_value(value, unit_suffix)),
        revision_direction,
        surprise_state: SurpriseState::NotReleased,
        expectation_pressure: expectation_pressure(series.count, spread),
        confidence: Some(confidence_score(series.count, spread)),
        source_health: SourceHealth::Succeeded,
        interpretation: format!(
            "{subject} の {} は {expected_value} 前後で観測されている。",
            metric_label(event_type)
        ),
        observed_at: as_of_date,
    }
}

fn build_margin_observation(
    subject: &str,
    event_type: ExpectationEventType,
    gross_income: crate::features::research::infrastructure::expectation_source_adapter::ConsensusSeries,
    revenue: crate::features::research::infrastructure::expectation_source_adapter::ConsensusSeries,
    unit: &str,
    as_of_date: NaiveDate,
) -> ExpectationObservation {
    let period = quarter_label_from_series(&gross_income.period);
    let lifecycle_state = derive_lifecycle_state(&period, as_of_date, "未発表", None, None, None);
    let gross_average = gross_income
        .average
        .or(gross_income.median)
        .unwrap_or_default();
    let revenue_average = revenue.average.or(revenue.median).unwrap_or_default();
    let gross_margin = if revenue_average > 0.0 {
        gross_average / revenue_average
    } else {
        0.0
    };
    let gross_high = ratio_value(gross_income.high, revenue.low);
    let gross_low = ratio_value(gross_income.low, revenue.high);
    let gross_median = ratio_value(gross_income.median, revenue.median);
    let gross_average_ratio = ratio_value(gross_income.average, revenue.average);
    let count = gross_income.count.min(revenue.count);
    let spread = spread_ratio(gross_high, gross_low, Some(gross_margin));

    ExpectationObservation {
        subject: subject.to_string(),
        period,
        as_of_date,
        event_type,
        lifecycle_state,
        expected_value: format_percent(gross_margin),
        actual_value: "未発表".to_string(),
        result: None,
        surprise_percent: None,
        market_reaction: None,
        released_at: None,
        archived_at: None,
        unit: unit.to_string(),
        consensus_source: format!("{PROVIDER} gross-income estimates / revenue estimates"),
        estimate_count: count,
        estimate_high: gross_high.map(format_percent),
        estimate_low: gross_low.map(format_percent),
        estimate_median: gross_median.map(format_percent),
        estimate_average: gross_average_ratio.map(format_percent),
        revision_direction: revision_direction(gross_income.previous_average, gross_income.average),
        surprise_state: SurpriseState::NotReleased,
        expectation_pressure: expectation_pressure(count, spread),
        confidence: Some(confidence_score(count, spread)),
        source_health: SourceHealth::Succeeded,
        interpretation: format!("{subject} の粗利率は売上と粗利総額の推定比率から観測される。"),
        observed_at: as_of_date,
    }
}

fn unavailable_observation(
    subject: &str,
    period: &str,
    event_type: ExpectationEventType,
    unit: &str,
    reason: &str,
    as_of_date: NaiveDate,
) -> ExpectationObservation {
    let lifecycle_state = derive_lifecycle_state(period, as_of_date, "未発表", None, None, None);
    ExpectationObservation {
        subject: subject.to_string(),
        period: period.to_string(),
        as_of_date,
        event_type,
        lifecycle_state,
        expected_value: "未対応".to_string(),
        actual_value: "未発表".to_string(),
        result: None,
        surprise_percent: None,
        market_reaction: None,
        released_at: None,
        archived_at: None,
        unit: unit.to_string(),
        consensus_source: format!("unavailable: {reason}"),
        estimate_count: 0,
        estimate_high: None,
        estimate_low: None,
        estimate_median: None,
        estimate_average: None,
        revision_direction: RevisionDirection::Unknown,
        surprise_state: SurpriseState::NotReleased,
        expectation_pressure: ExpectationPressure::Low,
        confidence: None,
        source_health: SourceHealth::Unavailable,
        interpretation: reason.to_string(),
        observed_at: as_of_date,
    }
}

fn metric_label(event_type: ExpectationEventType) -> &'static str {
    match event_type {
        ExpectationEventType::DeliveryConsensus => "delivery consensus",
        ExpectationEventType::EarningsConsensus => "earnings consensus",
        ExpectationEventType::RevenueConsensus => "revenue consensus",
        ExpectationEventType::MarginConsensus => "margin consensus",
        ExpectationEventType::CloudGrowthConsensus => "cloud growth consensus",
        ExpectationEventType::CapexConsensus => "capex consensus",
        ExpectationEventType::ProductEventExpectation => "product event expectation",
        ExpectationEventType::UserGrowthConsensus => "user growth consensus",
        ExpectationEventType::ProcedureGrowthConsensus => "procedure growth consensus",
    }
}

fn confidence_score(count: usize, spread: f64) -> f64 {
    let analyst_score = (count.min(20) as f64) / 20.0;
    let spread_score = (1.0 - spread.clamp(0.0, 0.5) / 0.5).clamp(0.0, 1.0);
    (0.35 + analyst_score * 0.4 + spread_score * 0.25).clamp(0.35, 0.95)
}

fn expectation_pressure(count: usize, spread: f64) -> ExpectationPressure {
    if count >= 20 || spread >= 0.18 {
        ExpectationPressure::Extreme
    } else if count >= 12 || spread >= 0.1 {
        ExpectationPressure::High
    } else if count >= 6 || spread >= 0.05 {
        ExpectationPressure::Normal
    } else {
        ExpectationPressure::Low
    }
}

fn revision_direction(
    previous_average: Option<f64>,
    current_average: Option<f64>,
) -> RevisionDirection {
    match (previous_average, current_average) {
        (Some(previous), Some(current)) if current > previous * 1.02 => RevisionDirection::Up,
        (Some(previous), Some(current)) if current < previous * 0.98 => RevisionDirection::Down,
        (Some(_), Some(_)) => RevisionDirection::Stable,
        _ => RevisionDirection::Unknown,
    }
}

fn spread_ratio(high: Option<f64>, low: Option<f64>, center: Option<f64>) -> f64 {
    match (high, low, center) {
        (Some(high), Some(low), Some(center)) if center > 0.0 => {
            ((high - low).abs() / center).clamp(0.0, 1.0)
        }
        _ => 0.0,
    }
}

fn ratio_value(numerator: Option<f64>, denominator: Option<f64>) -> Option<f64> {
    let numerator = numerator?;
    let denominator = denominator?;
    (denominator > 0.0).then_some(numerator / denominator)
}

fn format_consensus_value(value: f64, unit_suffix: &str) -> String {
    if unit_suffix == "$ EPS" {
        return format!("${value:.2} EPS");
    }
    if unit_suffix == "$B revenue" {
        return format_money(value, "revenue");
    }
    format!("{value:.1}{unit_suffix}")
}

fn format_percent(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

fn format_money(value: f64, label: &str) -> String {
    let absolute = value.abs();
    let rendered = if absolute >= 1_000_000_000_000.0 {
        format!("${:.1}T", value / 1_000_000_000_000.0)
    } else if absolute >= 1_000_000_000.0 {
        format!("${:.1}B", value / 1_000_000_000.0)
    } else if absolute >= 1_000_000.0 {
        format!("${:.1}M", value / 1_000_000.0)
    } else {
        format!("${value:.2}")
    };
    if label.is_empty() {
        rendered
    } else {
        format!("{rendered} {label}")
    }
}

fn quarter_label_from_series(period: &str) -> String {
    parse_period_date(period)
        .map(quarter_label_from_date)
        .unwrap_or_else(|| period.to_string())
}

fn quarter_label_from_date(date: NaiveDate) -> String {
    let quarter = match date.month() {
        1..=3 => 1,
        4..=6 => 2,
        7..=9 => 3,
        _ => 4,
    };
    format!("{}Q{}", date.year(), quarter)
}

#[cfg(test)]
mod tests {
    use super::{
        build_consensus_observation, build_expectation_layer_snapshot_for_market_date,
        build_margin_observation, confidence_score, consensus_margin_observation,
        consensus_observation, expectation_pressure, format_consensus_value, format_money,
        format_percent, has_consensus_center, has_positive_consensus_center, metric_label,
        no_consensus_reason, provider_unavailable_reason, quarter_label_from_date,
        quarter_label_from_series, ratio_value, revision_direction, spread_ratio,
        unavailable_observation, unavailable_snapshot, FinnhubConsensusMetric,
        FinnhubExpectationSourceAdapter,
    };
    use crate::config::AppConfig;
    use crate::features::research::domain::expectation::{
        ExpectationEventType, ExpectationPressure, RevisionDirection, SourceHealth,
    };
    use crate::features::research::infrastructure::expectation_source_adapter::ConsensusSeries;
    use chrono::NaiveDate;

    fn series(average: Option<f64>, median: Option<f64>) -> ConsensusSeries {
        ConsensusSeries {
            period: "2026-09-30".to_string(),
            count: 3,
            high: None,
            low: None,
            median,
            average,
            previous_average: None,
        }
    }

    #[test]
    fn missing_consensus_center_is_not_treated_as_zero() {
        let missing = series(None, None);
        assert!(!has_consensus_center(&missing));
        assert!(!has_positive_consensus_center(&missing));
    }

    #[test]
    fn margin_requires_positive_revenue_center() {
        assert!(!has_positive_consensus_center(&series(Some(0.0), None)));
        assert!(has_positive_consensus_center(&series(None, Some(1.0))));
    }

    #[test]
    fn unavailable_snapshot_never_exposes_fixture_consensus() {
        let market_date = NaiveDate::from_ymd_opt(2026, 8, 12).expect("valid market date");
        let snapshot = unavailable_snapshot(market_date, "test unavailable");

        assert_eq!(snapshot.as_of_date, market_date);
        assert!(snapshot
            .observations
            .iter()
            .all(|observation| observation.as_of_date == market_date
                && observation.source_health == SourceHealth::Unavailable
                && observation.estimate_count == 0
                && observation.consensus_source.starts_with("unavailable:")));
    }

    #[test]
    fn missing_credential_production_snapshot_is_unavailable() {
        let mut config: AppConfig = toml::from_str(include_str!("../../../../config.toml"))
            .expect("repository config should parse");
        config.finnhub = None;
        let snapshot = build_expectation_layer_snapshot_for_market_date(
            &config,
            NaiveDate::from_ymd_opt(2026, 8, 12).expect("valid market date"),
        );

        assert!(snapshot
            .observations
            .iter()
            .all(|observation| observation.source_health == SourceHealth::Unavailable));
        assert_eq!(snapshot.decision_weight_percent, 0);
        assert!(!snapshot.trade_signal);
        assert!(snapshot.observations.iter().all(|observation| observation
            .interpretation
            .contains("Finnhub credential is not configured")));
    }

    #[test]
    fn provider_failure_reason_does_not_claim_successful_consensus_fetch() {
        let reason = super::provider_unavailable_reason(
            crate::features::research::domain::expectation::ExpectationEventType::EarningsConsensus,
        );

        assert!(reason.contains("Finnhub"));
        assert!(reason.contains("提供元を利用できず"));
        assert!(!reason.contains("取得される"));
    }

    fn valid_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 21).expect("valid market date")
    }

    fn consensus_series() -> ConsensusSeries {
        ConsensusSeries {
            period: "2026-09-30".to_string(),
            count: 15,
            high: Some(3.0),
            low: Some(2.0),
            median: Some(2.4),
            average: Some(2.5),
            previous_average: Some(2.0),
        }
    }

    fn missing_credential_adapter() -> FinnhubExpectationSourceAdapter<'static> {
        let mut config: AppConfig = toml::from_str(include_str!("../../../../config.toml"))
            .expect("repository config should parse");
        config.finnhub = None;
        let config = Box::leak(Box::new(config));
        FinnhubExpectationSourceAdapter::new(config).expect("adapter should initialize")
    }

    #[test]
    fn consensus_observation_projects_available_series_without_network_side_effects() {
        let observation = build_consensus_observation(
            "NVDA",
            "earnings_usd_per_share",
            ExpectationEventType::EarningsConsensus,
            consensus_series(),
            "$ EPS",
            valid_date(),
        );

        assert_eq!(observation.period, "2026Q3");
        assert_eq!(observation.expected_value, "$2.50 EPS");
        assert_eq!(observation.estimate_high.as_deref(), Some("$3.00 EPS"));
        assert_eq!(observation.estimate_low.as_deref(), Some("$2.00 EPS"));
        assert_eq!(observation.revision_direction, RevisionDirection::Up);
        assert_eq!(
            observation.expectation_pressure,
            ExpectationPressure::Extreme
        );
        assert_eq!(observation.source_health, SourceHealth::Succeeded);
        assert_eq!(observation.observed_at, valid_date());
    }

    #[test]
    fn margin_observation_projects_ratio_and_preserves_source_health() {
        let gross_income = consensus_series();
        let revenue = ConsensusSeries {
            period: "2026-09-30".to_string(),
            count: 12,
            high: Some(12.0),
            low: Some(8.0),
            median: Some(9.0),
            average: Some(10.0),
            previous_average: Some(9.0),
        };

        let observation = build_margin_observation(
            "PLTR",
            ExpectationEventType::MarginConsensus,
            gross_income,
            revenue,
            "gross_margin_pct",
            valid_date(),
        );

        assert_eq!(observation.period, "2026Q3");
        assert_eq!(observation.expected_value, "25.0%");
        assert_eq!(observation.estimate_average.as_deref(), Some("25.0%"));
        assert_eq!(observation.estimate_high.as_deref(), Some("37.5%"));
        assert_eq!(observation.estimate_low.as_deref(), Some("16.7%"));
        assert_eq!(observation.estimate_count, 12);
        assert_eq!(observation.source_health, SourceHealth::Succeeded);
    }

    #[test]
    fn missing_credential_consensus_paths_fail_closed_as_unavailable() {
        let adapter = missing_credential_adapter();
        let observation = consensus_observation(
            &adapter,
            "NVDA",
            "earnings_usd_per_share",
            ExpectationEventType::EarningsConsensus,
            FinnhubConsensusMetric::Eps,
            "$ EPS",
            "not used",
            "2026Q3",
            valid_date(),
        );
        let margin = consensus_margin_observation(
            &adapter,
            "PLTR",
            ExpectationEventType::MarginConsensus,
            "gross_margin_pct",
            "not used",
            "2026Q3",
            valid_date(),
        );

        for observation in [observation, margin] {
            assert_eq!(observation.source_health, SourceHealth::Unavailable);
            assert!(observation
                .consensus_source
                .starts_with("unavailable: Finnhub の"));
            assert_eq!(observation.estimate_count, 0);
        }
    }

    #[test]
    fn unavailable_observation_preserves_fact_and_temporal_fields() {
        let observation = unavailable_observation(
            "TSLA",
            "2026Q3",
            ExpectationEventType::DeliveryConsensus,
            "deliveries",
            "source unavailable",
            valid_date(),
        );

        assert_eq!(observation.subject, "TSLA");
        assert_eq!(observation.period, "2026Q3");
        assert_eq!(observation.as_of_date, valid_date());
        assert_eq!(observation.observed_at, valid_date());
        assert_eq!(observation.expected_value, "未対応");
        assert_eq!(observation.source_health, SourceHealth::Unavailable);
        assert_eq!(
            observation.consensus_source,
            "unavailable: source unavailable"
        );
    }

    #[test]
    fn metric_labels_and_reasons_cover_every_expectation_event_type() {
        let cases = [
            (
                ExpectationEventType::DeliveryConsensus,
                "delivery consensus",
            ),
            (
                ExpectationEventType::EarningsConsensus,
                "earnings consensus",
            ),
            (ExpectationEventType::RevenueConsensus, "revenue consensus"),
            (ExpectationEventType::MarginConsensus, "margin consensus"),
            (
                ExpectationEventType::CloudGrowthConsensus,
                "cloud growth consensus",
            ),
            (ExpectationEventType::CapexConsensus, "capex consensus"),
            (
                ExpectationEventType::ProductEventExpectation,
                "product event expectation",
            ),
            (
                ExpectationEventType::UserGrowthConsensus,
                "user growth consensus",
            ),
            (
                ExpectationEventType::ProcedureGrowthConsensus,
                "procedure growth consensus",
            ),
        ];

        for (event_type, label) in cases {
            assert_eq!(metric_label(event_type), label);
            assert!(provider_unavailable_reason(event_type).contains(label));
            assert!(no_consensus_reason(event_type).contains(label));
        }
    }

    #[test]
    fn confidence_pressure_and_numeric_helpers_cover_boundaries() {
        assert_eq!(expectation_pressure(20, 0.0), ExpectationPressure::Extreme);
        assert_eq!(expectation_pressure(1, 0.18), ExpectationPressure::Extreme);
        assert_eq!(expectation_pressure(12, 0.0), ExpectationPressure::High);
        assert_eq!(expectation_pressure(1, 0.10), ExpectationPressure::High);
        assert_eq!(expectation_pressure(6, 0.0), ExpectationPressure::Normal);
        assert_eq!(expectation_pressure(1, 0.05), ExpectationPressure::Normal);
        assert_eq!(expectation_pressure(5, 0.0), ExpectationPressure::Low);

        assert!(confidence_score(20, 0.0) > confidence_score(1, 0.5));
        assert_eq!(spread_ratio(Some(3.0), Some(1.0), Some(2.0)), 1.0);
        assert_eq!(spread_ratio(Some(3.0), Some(1.0), Some(0.0)), 0.0);
        assert_eq!(spread_ratio(None, Some(1.0), Some(2.0)), 0.0);
        assert_eq!(ratio_value(Some(2.0), Some(4.0)), Some(0.5));
        assert_eq!(ratio_value(Some(2.0), Some(0.0)), None);
        assert_eq!(ratio_value(None, Some(4.0)), None);
    }

    #[test]
    fn revision_and_formatting_helpers_cover_all_output_shapes() {
        assert_eq!(
            revision_direction(Some(1.0), Some(1.1)),
            RevisionDirection::Up
        );
        assert_eq!(
            revision_direction(Some(1.0), Some(0.9)),
            RevisionDirection::Down
        );
        assert_eq!(
            revision_direction(Some(1.0), Some(1.01)),
            RevisionDirection::Stable
        );
        assert_eq!(
            revision_direction(None, Some(1.0)),
            RevisionDirection::Unknown
        );

        assert_eq!(format_consensus_value(2.5, "$ EPS"), "$2.50 EPS");
        assert_eq!(
            format_consensus_value(2_500_000_000.0, "$B revenue"),
            "$2.5B revenue"
        );
        assert_eq!(format_consensus_value(12.34, " units"), "12.3 units");
        assert_eq!(format_percent(0.257), "25.7%");
        assert_eq!(format_money(2_000_000_000_000.0, "value"), "$2.0T value");
        assert_eq!(format_money(2_000_000_000.0, "value"), "$2.0B value");
        assert_eq!(format_money(2_000_000.0, "value"), "$2.0M value");
        assert_eq!(format_money(-12.34, ""), "$-12.34");
    }

    #[test]
    fn quarter_helpers_cover_date_series_and_fallback_paths() {
        assert_eq!(
            quarter_label_from_date(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()),
            "2026Q1"
        );
        assert_eq!(
            quarter_label_from_date(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
            "2026Q2"
        );
        assert_eq!(
            quarter_label_from_date(NaiveDate::from_ymd_opt(2026, 8, 1).unwrap()),
            "2026Q3"
        );
        assert_eq!(
            quarter_label_from_date(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()),
            "2026Q4"
        );
        assert_eq!(quarter_label_from_series("2026-09-30"), "2026Q3");
        assert_eq!(quarter_label_from_series("2026Q4"), "2026Q4");
        assert_eq!(quarter_label_from_series("unknown"), "unknown");
    }
}

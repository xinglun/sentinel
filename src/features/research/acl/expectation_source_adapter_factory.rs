use crate::config;
use crate::features::research::domain::expectation::{
    derive_lifecycle_state, ExpectationEventType, ExpectationObservation, ExpectationPressure,
    RevisionDirection, SourceHealth, SurpriseState,
};
use crate::features::research::infrastructure::expectation_source_adapter::{
    parse_period_date, FinnhubConsensusMetric, FinnhubExpectationSourceAdapter,
};
use chrono::{Datelike, NaiveDate};

use crate::features::research::interface::expectation_report_builder::{
    build_expectation_layer_fixture_snapshot, ExpectationLayerSnapshot,
};

const PROVIDER: &str = "Finnhub";

/// Expectation Layer の live source snapshot を組み立てる。
pub(crate) fn build_expectation_layer_snapshot_from_config(
    app_config: &config::AppConfig,
) -> ExpectationLayerSnapshot {
    if !FinnhubExpectationSourceAdapter::has_credential(app_config) {
        return build_expectation_layer_fixture_snapshot();
    }

    let as_of_date = chrono::Local::now().date_naive();
    let current_period = quarter_label_from_date(as_of_date);
    let Some(adapter) = FinnhubExpectationSourceAdapter::new(app_config) else {
        return build_expectation_layer_fixture_snapshot();
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

#[allow(clippy::too_many_arguments)]
fn consensus_observation(
    adapter: &FinnhubExpectationSourceAdapter<'_>,
    subject: &str,
    unit: &str,
    event_type: ExpectationEventType,
    metric: FinnhubConsensusMetric,
    unit_suffix: &str,
    unavailable_reason: &str,
    period: &str,
    as_of_date: NaiveDate,
) -> ExpectationObservation {
    adapter
        .fetch_consensus_series(subject, metric, as_of_date)
        .map(|series| {
            build_consensus_observation(subject, unit, event_type, series, unit_suffix, as_of_date)
        })
        .unwrap_or_else(|| {
            unavailable_observation(
                subject,
                period,
                event_type,
                unit,
                unavailable_reason,
                as_of_date,
            )
        })
}

fn consensus_margin_observation(
    adapter: &FinnhubExpectationSourceAdapter<'_>,
    subject: &str,
    event_type: ExpectationEventType,
    unit: &str,
    unavailable_reason: &str,
    period: &str,
    as_of_date: NaiveDate,
) -> ExpectationObservation {
    let revenue =
        adapter.fetch_consensus_series(subject, FinnhubConsensusMetric::Revenue, as_of_date);
    let gross_income =
        adapter.fetch_consensus_series(subject, FinnhubConsensusMetric::GrossIncome, as_of_date);

    match (gross_income, revenue) {
        (Some(gross_income), Some(revenue)) => {
            build_margin_observation(subject, event_type, gross_income, revenue, unit, as_of_date)
        }
        _ => unavailable_observation(
            subject,
            period,
            event_type,
            unit,
            unavailable_reason,
            as_of_date,
        ),
    }
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

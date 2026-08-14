use crate::config;
use chrono::{Datelike, NaiveDate};
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;
use tokio::task::block_in_place;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinnhubConsensusMetric {
    Eps,
    Revenue,
    GrossIncome,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ConsensusFetchResult {
    Available(ConsensusSeries),
    NoConsensus,
    ProviderUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsensusPayloadStatus {
    Available,
    NoConsensus,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConsensusSeries {
    pub period: String,
    pub count: usize,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub median: Option<f64>,
    pub average: Option<f64>,
    pub previous_average: Option<f64>,
}

pub(crate) struct FinnhubExpectationSourceAdapter<'a> {
    app_config: &'a config::AppConfig,
}

impl<'a> FinnhubExpectationSourceAdapter<'a> {
    pub(crate) fn new(app_config: &'a config::AppConfig) -> Option<Self> {
        Some(Self { app_config })
    }

    pub(crate) fn has_credential(app_config: &config::AppConfig) -> bool {
        app_config
            .finnhub
            .as_ref()
            .map(|config| config.finnhub_api_key.trim())
            .is_some_and(|key| !key.is_empty())
    }

    pub(crate) fn fetch_consensus_series(
        &self,
        symbol: &str,
        metric: FinnhubConsensusMetric,
        as_of_date: NaiveDate,
    ) -> ConsensusFetchResult {
        let Some(token) = self.token() else {
            return ConsensusFetchResult::ProviderUnavailable;
        };
        let endpoint = match metric {
            FinnhubConsensusMetric::Eps => "eps-estimate",
            FinnhubConsensusMetric::Revenue => "revenue-estimate",
            FinnhubConsensusMetric::GrossIncome => "gross-income-estimate",
        };
        let metric_prefix = match metric {
            FinnhubConsensusMetric::Eps => "eps",
            FinnhubConsensusMetric::Revenue => "revenue",
            FinnhubConsensusMetric::GrossIncome => "grossIncome",
        };

        let url = format!(
            "https://finnhub.io/api/v1/stock/{endpoint}?symbol={symbol}&freq=quarterly&token={token}"
        );
        let Some(json) = fetch_json(&url) else {
            return ConsensusFetchResult::ProviderUnavailable;
        };
        if classify_consensus_payload(&json, metric_prefix) == ConsensusPayloadStatus::NoConsensus {
            return ConsensusFetchResult::NoConsensus;
        }
        let mut series = parse_consensus_series(&json, metric_prefix);

        series.sort_by_key(|item| parse_period_date(&item.period).unwrap_or(as_of_date));
        let current_quarter = current_quarter_bounds(as_of_date);
        let Some(target_index) = choose_target_index(&series, current_quarter) else {
            return ConsensusFetchResult::NoConsensus;
        };
        let Some(target) = series.get(target_index).cloned() else {
            return ConsensusFetchResult::NoConsensus;
        };
        let previous_average = series
            .get(target_index.saturating_sub(1))
            .and_then(|item| item.average);

        ConsensusFetchResult::Available(ConsensusSeries {
            period: target.period,
            count: target.count,
            high: target.high,
            low: target.low,
            median: target.median,
            average: target.average,
            previous_average,
        })
    }

    fn token(&self) -> Option<String> {
        self.app_config
            .finnhub
            .as_ref()
            .map(|config| config.finnhub_api_key.trim().to_string())
            .filter(|key| !key.is_empty())
    }
}

fn classify_consensus_payload(value: &Value, metric_prefix: &str) -> ConsensusPayloadStatus {
    if parse_consensus_series(value, metric_prefix).is_empty() {
        ConsensusPayloadStatus::NoConsensus
    } else {
        ConsensusPayloadStatus::Available
    }
}

fn fetch_json(url: &str) -> Option<Value> {
    block_in_place(|| {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .ok()?;
        let response = client.get(url).send().ok()?;
        if !response.status().is_success() {
            return None;
        }
        response.json().ok()
    })
}

#[derive(Debug, Clone)]
struct ConsensusSeriesEntry {
    period: String,
    count: usize,
    high: Option<f64>,
    low: Option<f64>,
    median: Option<f64>,
    average: Option<f64>,
}

fn parse_consensus_series(value: &Value, metric_prefix: &str) -> Vec<ConsensusSeriesEntry> {
    let records = match value {
        Value::Array(values) => values.clone(),
        Value::Object(map) => map
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    records
        .into_iter()
        .filter_map(|record| {
            let period = record.get("period")?.as_str()?.trim().to_string();
            let count = read_usize(
                &record,
                &["numberAnalysts", "numberAnalyst", "analystCount"],
            )
            .unwrap_or(0);
            let average_key = format!("{metric_prefix}Avg");
            let average_alt_key = format!("{metric_prefix}Average");
            let high_key = format!("{metric_prefix}High");
            let low_key = format!("{metric_prefix}Low");
            let median_key = format!("{metric_prefix}Median");
            let average = read_optional_f64(
                &record,
                &[average_key.as_str(), average_alt_key.as_str(), "average"],
            );
            let high = read_optional_f64(&record, &[high_key.as_str(), "high"]);
            let low = read_optional_f64(&record, &[low_key.as_str(), "low"]);
            let median = read_optional_f64(
                &record,
                &[median_key.as_str(), "median", average_key.as_str()],
            )
            .or(average);
            let average = average.or(median);

            Some(ConsensusSeriesEntry {
                period,
                count,
                high,
                low,
                median,
                average,
            })
        })
        .collect()
}

fn choose_target_index(
    series: &[ConsensusSeriesEntry],
    current_quarter: (NaiveDate, NaiveDate),
) -> Option<usize> {
    let (quarter_start, quarter_end) = current_quarter;
    if let Some(index) = series.iter().position(|item| {
        parse_period_date(&item.period)
            .map(|date| date >= quarter_start && date <= quarter_end)
            .unwrap_or(false)
    }) {
        return Some(index);
    }

    if let Some(index) = series
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            parse_period_date(&item.period)
                .filter(|date| *date > quarter_end)
                .map(|date| (index, date))
        })
        .min_by_key(|(_, date)| *date)
        .map(|(index, _)| index)
    {
        return Some(index);
    }

    series
        .iter()
        .enumerate()
        .filter_map(|(index, item)| parse_period_date(&item.period).map(|date| (index, date)))
        .max_by_key(|(_, date)| *date)
        .map(|(index, _)| index)
}

fn current_quarter_bounds(as_of_date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let year = as_of_date.year();
    match as_of_date.month() {
        1..=3 => (
            NaiveDate::from_ymd_opt(year, 1, 1).expect("valid quarter start"),
            NaiveDate::from_ymd_opt(year, 3, 31).expect("valid quarter end"),
        ),
        4..=6 => (
            NaiveDate::from_ymd_opt(year, 4, 1).expect("valid quarter start"),
            NaiveDate::from_ymd_opt(year, 6, 30).expect("valid quarter end"),
        ),
        7..=9 => (
            NaiveDate::from_ymd_opt(year, 7, 1).expect("valid quarter start"),
            NaiveDate::from_ymd_opt(year, 9, 30).expect("valid quarter end"),
        ),
        _ => (
            NaiveDate::from_ymd_opt(year, 10, 1).expect("valid quarter start"),
            NaiveDate::from_ymd_opt(year, 12, 31).expect("valid quarter end"),
        ),
    }
}

pub(crate) fn parse_period_date(value: &str) -> Option<NaiveDate> {
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Some(date);
    }

    let value = value.trim();
    if value.len() == 6 && value.as_bytes().get(4).copied() == Some(b'Q') {
        let year = value[0..4].parse::<i32>().ok()?;
        let quarter = value[5..6].parse::<u32>().ok()?;
        return quarter_end_date(year, quarter);
    }

    None
}

fn quarter_end_date(year: i32, quarter: u32) -> Option<NaiveDate> {
    match quarter {
        1 => NaiveDate::from_ymd_opt(year, 3, 31),
        2 => NaiveDate::from_ymd_opt(year, 6, 30),
        3 => NaiveDate::from_ymd_opt(year, 9, 30),
        4 => NaiveDate::from_ymd_opt(year, 12, 31),
        _ => None,
    }
}

fn read_usize(record: &Value, keys: &[&str]) -> Option<usize> {
    keys.iter().find_map(|key| {
        let value = record.get(*key)?;
        if let Some(number) = value.as_u64() {
            return Some(number as usize);
        }
        value.as_str()?.trim().parse::<usize>().ok()
    })
}

fn read_optional_f64(record: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        let value = record.get(*key)?;
        value
            .as_f64()
            .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
    })
}

#[cfg(test)]
mod tests {
    use super::{classify_consensus_payload, ConsensusPayloadStatus};

    #[test]
    fn empty_consensus_payload_is_classified_as_no_consensus() {
        let status = classify_consensus_payload(&serde_json::json!({"data": []}), "eps");

        assert_eq!(status, ConsensusPayloadStatus::NoConsensus);
    }

    use super::*;

    #[test]
    fn parses_eps_estimate_series_from_data_array() {
        let value: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/expectation_source/eps_estimate_sample.json"
        ))
        .expect("valid fixture json");
        let series = parse_consensus_series(&value, "eps");
        assert_eq!(series.len(), 3);
        assert_eq!(series[0].period, "2026-06-30");
        assert_eq!(series[0].count, 21);
        assert_eq!(series[0].average, Some(1.19));
        assert_eq!(series[0].high, Some(1.31));
        assert_eq!(series[0].low, Some(1.08));
    }

    #[test]
    fn chooses_current_quarter_before_future_records() {
        let series = vec![
            ConsensusSeriesEntry {
                period: "2026-03-31".to_string(),
                count: 10,
                high: Some(1.2),
                low: Some(1.0),
                median: Some(1.1),
                average: Some(1.1),
            },
            ConsensusSeriesEntry {
                period: "2026-06-30".to_string(),
                count: 11,
                high: Some(1.3),
                low: Some(1.1),
                median: Some(1.2),
                average: Some(1.2),
            },
            ConsensusSeriesEntry {
                period: "2026-09-30".to_string(),
                count: 12,
                high: Some(1.4),
                low: Some(1.2),
                median: Some(1.3),
                average: Some(1.3),
            },
        ];

        let (start, end) =
            current_quarter_bounds(NaiveDate::from_ymd_opt(2026, 6, 18).expect("valid date"));
        let index = choose_target_index(&series, (start, end)).expect("target index");
        assert_eq!(index, 1);
    }
}

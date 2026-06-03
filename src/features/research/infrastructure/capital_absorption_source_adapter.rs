use crate::config;
use crate::features::research::application::capital_absorption::{
    build_capital_absorption_snapshot_from_events, unavailable_capital_absorption_snapshot,
    CapitalAbsorptionAutoConfidence, CapitalAbsorptionAutoEvent,
    CapitalAbsorptionAutoEventCategory, CapitalAbsorptionAutoSnapshot,
    CapitalAbsorptionSourceHealth, CapitalAbsorptionSourceStatus,
};
use anyhow::{anyhow, Context, Result};
use chrono::{Duration, NaiveDate};

const MAX_NEWS_PER_SYMBOL: usize = 20;
const DEFAULT_MARKET_SYMBOLS: &[&str] = &[
    "AAPL", "MSFT", "GOOG", "GOOGL", "AMZN", "META", "NVDA", "TSLA", "AVGO", "ORCL", "AMD", "PLTR",
    "IBM", "INTC",
];
const AI_IPO_CANDIDATES: &[&str] = &[
    "Anthropic",
    "OpenAI",
    "SpaceX",
    "Databricks",
    "Stripe",
    "Figure",
];

pub(crate) async fn build_automatic_capital_absorption_snapshot(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
    lookback_days: usize,
) -> CapitalAbsorptionAutoSnapshot {
    match fetch_finnhub_capital_absorption_events(app_config, as_of_date, lookback_days).await {
        Ok(events) => build_capital_absorption_snapshot_from_events(
            events,
            CapitalAbsorptionSourceStatus {
                provider: "Finnhub company-news + market-news".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "market-wide automatic scan completed".to_string(),
            },
        ),
        Err(err) => unavailable_capital_absorption_snapshot(err.to_string()),
    }
}

async fn fetch_finnhub_capital_absorption_events(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
    lookback_days: usize,
) -> Result<Vec<CapitalAbsorptionAutoEvent>> {
    let token = app_config
        .finnhub
        .as_ref()
        .map(|config| config.finnhub_api_key.as_str())
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| anyhow!("Finnhub API key is not configured"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let from = as_of_date - Duration::days(lookback_days as i64);
    let symbols = capital_absorption_symbols(app_config);
    let mut events = Vec::new();
    for symbol in symbols {
        let url = format!(
            "https://finnhub.io/api/v1/company-news?symbol={symbol}&from={from}&to={as_of_date}&token={token}"
        );
        let response = client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Finnhub returned {} for {}",
                response.status(),
                symbol
            ));
        }
        let raw = response.text().await?;
        events.extend(extract_capital_absorption_events(
            &symbol, &raw, as_of_date,
        )?);
    }
    let market_news_url = format!("https://finnhub.io/api/v1/news?category=general&token={token}");
    let response = client.get(&market_news_url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Finnhub market news returned {}",
            response.status()
        ));
    }
    let raw = response.text().await?;
    events.extend(extract_capital_absorption_events(
        "Market", &raw, as_of_date,
    )?);
    events.sort_by(|a, b| {
        a.observed_at
            .cmp(&b.observed_at)
            .reverse()
            .then_with(|| a.subject.cmp(&b.subject))
            .then_with(|| a.description.cmp(&b.description))
    });
    Ok(events)
}

fn capital_absorption_symbols(_app_config: &config::AppConfig) -> Vec<String> {
    let mut symbols = DEFAULT_MARKET_SYMBOLS
        .iter()
        .map(|symbol| (*symbol).to_string())
        .collect::<Vec<_>>();
    symbols.sort();
    symbols.dedup();
    symbols
}

fn extract_capital_absorption_events(
    symbol: &str,
    raw_json: &str,
    fallback_date: NaiveDate,
) -> Result<Vec<CapitalAbsorptionAutoEvent>> {
    let items: Vec<serde_json::Value> =
        serde_json::from_str(raw_json).context("Failed to parse Finnhub news JSON")?;
    Ok(items
        .iter()
        .take(MAX_NEWS_PER_SYMBOL)
        .filter_map(|item| event_from_news_item(symbol, item, fallback_date))
        .collect())
}

fn event_from_news_item(
    symbol: &str,
    item: &serde_json::Value,
    fallback_date: NaiveDate,
) -> Option<CapitalAbsorptionAutoEvent> {
    let headline = item
        .get("headline")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let summary = item
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let text = format!("{headline} {summary}");
    let category = classify_capital_absorption_event(&text)?;
    let observed_at = item
        .get("datetime")
        .and_then(|value| value.as_i64())
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|datetime| datetime.date_naive())
        .unwrap_or(fallback_date);
    let source_url = item
        .get("url")
        .and_then(|value| value.as_str())
        .filter(|url| !url.trim().is_empty())
        .map(|url| url.to_string());
    Some(CapitalAbsorptionAutoEvent {
        category,
        subject: detect_event_subject(symbol, &text),
        description: headline.to_string(),
        amount_usd_b: extract_usd_billions(&text),
        ai_capex_related: is_ai_capex_related(&text),
        source_url,
        observed_at,
        source_count: 1,
        confidence: CapitalAbsorptionAutoConfidence::Low,
    })
}

fn detect_event_subject(symbol: &str, text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    for candidate in AI_IPO_CANDIDATES {
        if lower.contains(&candidate.to_ascii_lowercase()) {
            return (*candidate).to_string();
        }
    }
    if symbol == "Market" {
        extract_known_public_subject(&lower).unwrap_or_else(|| symbol.to_string())
    } else {
        symbol.to_string()
    }
}

fn extract_known_public_subject(lower_text: &str) -> Option<String> {
    [
        ("alphabet", "GOOG"),
        ("google", "GOOG"),
        ("microsoft", "MSFT"),
        ("amazon", "AMZN"),
        ("meta", "META"),
        ("nvidia", "NVDA"),
        ("tesla", "TSLA"),
        ("apple", "AAPL"),
        ("broadcom", "AVGO"),
        ("oracle", "ORCL"),
        ("amd", "AMD"),
    ]
    .iter()
    .find_map(|(name, symbol)| lower_text.contains(name).then(|| (*symbol).to_string()))
}

fn classify_capital_absorption_event(text: &str) -> Option<CapitalAbsorptionAutoEventCategory> {
    let lower = text.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "ipo",
            "initial public offering",
            "files to go public",
            "listing",
        ],
    ) {
        return Some(CapitalAbsorptionAutoEventCategory::IpoSupply);
    }
    if contains_any(
        &lower,
        &[
            "tender offer",
            "secondary sale",
            "share sale",
            "vc exit",
            "private equity exit",
        ],
    ) {
        return Some(CapitalAbsorptionAutoEventCategory::SecondaryLiquidity);
    }
    if contains_any(
        &lower,
        &[
            "secondary offering",
            "stock offering",
            "share offering",
            "at-the-market",
            "atm offering",
            "convertible",
            "debt offering",
            "bond offering",
            "raise capital",
            "raises capital",
            "financing",
        ],
    ) {
        return Some(CapitalAbsorptionAutoEventCategory::MegaCapFinancing);
    }
    None
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn is_ai_capex_related(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    contains_any(
        &lower,
        &[
            "ai",
            "artificial intelligence",
            "data center",
            "datacenter",
            "gpu",
            "capex",
            "cloud infrastructure",
            "compute",
        ],
    )
}

fn extract_usd_billions(text: &str) -> Option<f64> {
    let tokens = text
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '$'))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    for window in tokens.windows(2) {
        let number = window[0].trim_start_matches('$').parse::<f64>().ok();
        let unit = window[1].to_ascii_lowercase();
        if let Some(number) = number {
            if unit.starts_with("billion") || unit == "bn" {
                return Some(number);
            }
            if unit.starts_with("million") || unit == "mn" {
                return Some(number / 1000.0);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_offering_event_from_finnhub_news() {
        let raw = r#"[
          {
            "headline": "Alphabet announces $80 billion secondary offering for AI data center capex",
            "summary": "The company plans to raise capital for AI infrastructure.",
            "url": "https://example.com/alphabet-offering",
            "datetime": 1780444800
          }
        ]"#;

        let events = extract_capital_absorption_events(
            "GOOG",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].category,
            CapitalAbsorptionAutoEventCategory::MegaCapFinancing
        );
        assert_eq!(events[0].amount_usd_b, Some(80.0));
        assert!(events[0].ai_capex_related);
    }

    #[test]
    fn ignores_ordinary_news_without_capital_absorption_terms() {
        let raw = r#"[{"headline":"Alphabet launches a new product","summary":"Product availability expanded globally.","datetime":1780444800}]"#;

        let events = extract_capital_absorption_events(
            "GOOG",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert!(events.is_empty());
    }

    #[test]
    fn detects_ai_ipo_candidate_subject_from_market_news() {
        let raw = r#"[
          {
            "headline": "SpaceX IPO expected as investor discussion increases",
            "summary": "Several reports say the company is preparing for a possible listing.",
            "url": "https://example.com/spacex-ipo",
            "datetime": 1780444800
          }
        ]"#;

        let events = extract_capital_absorption_events(
            "Market",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].subject, "SpaceX");
        assert_eq!(
            events[0].category,
            CapitalAbsorptionAutoEventCategory::IpoSupply
        );
    }
}

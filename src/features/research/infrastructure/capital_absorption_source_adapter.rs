use crate::config;
use crate::features::research::application::capital_absorption::{
    build_capital_absorption_snapshot_from_events, unavailable_capital_absorption_snapshot,
    CapitalAbsorptionAutoConfidence, CapitalAbsorptionAutoEvent,
    CapitalAbsorptionAutoEventCategory, CapitalAbsorptionAutoSnapshot,
    CapitalAbsorptionObservationEventType, CapitalAbsorptionSourceHealth,
    CapitalAbsorptionSourceStatus, CapitalAbsorptionSupplyKind,
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
    if is_weak_related_news(&text) {
        return None;
    }
    let category = classify_capital_absorption_event(&text)?;
    let subject = detect_event_subject(symbol, &text);
    let event_type = observation_event_type_from_text(&text);
    let supply_kind = supply_kind_from_category_type_and_text(category, event_type, &text)?;
    if supply_kind == CapitalAbsorptionSupplyKind::Potential
        && !is_ai_ipo_candidate_subject(&subject)
    {
        return None;
    }
    let amount_usd_b = if supply_kind == CapitalAbsorptionSupplyKind::Actual
        && has_confirmed_financing_amount(category, &text)
    {
        extract_usd_billions(&text)
    } else {
        None
    };
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
        supply_kind,
        event_type,
        subject,
        description: headline.to_string(),
        amount_usd_b,
        ai_capex_related: is_ai_capex_related(&text),
        source_url,
        observed_at,
        source_count: 1,
        confidence: CapitalAbsorptionAutoConfidence::Low,
    })
}

fn supply_kind_from_category_type_and_text(
    category: CapitalAbsorptionAutoEventCategory,
    event_type: CapitalAbsorptionObservationEventType,
    text: &str,
) -> Option<CapitalAbsorptionSupplyKind> {
    match category {
        CapitalAbsorptionAutoEventCategory::MegaCapFinancing
        | CapitalAbsorptionAutoEventCategory::SecondaryLiquidity
            if is_confirmed_non_ipo_financing_event(text) =>
        {
            Some(CapitalAbsorptionSupplyKind::Actual)
        }
        CapitalAbsorptionAutoEventCategory::IpoSupply
            if event_type == CapitalAbsorptionObservationEventType::Confirmed
                && is_confirmed_ipo_financing_event(text) =>
        {
            Some(CapitalAbsorptionSupplyKind::Actual)
        }
        CapitalAbsorptionAutoEventCategory::IpoSupply => {
            Some(CapitalAbsorptionSupplyKind::Potential)
        }
        CapitalAbsorptionAutoEventCategory::MegaCapFinancing
        | CapitalAbsorptionAutoEventCategory::SecondaryLiquidity => None,
    }
}

fn observation_event_type_from_text(text: &str) -> CapitalAbsorptionObservationEventType {
    let lower = text.to_ascii_lowercase();
    if lower.contains("rumor")
        || lower.contains("rumour")
        || lower.contains("speculation")
        || lower.contains("reportedly considering")
        || lower.contains("considering an ipo")
        || lower.contains("pre-ipo discussion")
    {
        CapitalAbsorptionObservationEventType::Rumor
    } else if lower.contains("announces")
        || lower.contains("announced")
        || lower.contains("prices ipo")
        || lower.contains("priced ipo")
        || lower.contains("begins trading")
        || lower.contains("listed")
        || lower.contains("completed")
        || lower.contains(" filed ")
        || lower.contains("filed for")
        || lower.contains("s-1")
        || lower.contains("launches")
    {
        CapitalAbsorptionObservationEventType::Confirmed
    } else {
        CapitalAbsorptionObservationEventType::Reported
    }
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

fn is_confirmed_non_ipo_financing_event(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    contains_any(
        &lower,
        &[
            "announces",
            "announced",
            "prices",
            "priced",
            "launches",
            "launched",
            "closes",
            "closed",
            "completes",
            "completed",
            "files",
            "filed",
        ],
    ) && contains_any(
        &lower,
        &[
            "equity raise",
            "secondary offering",
            "follow-on offering",
            "stock offering",
            "share offering",
            "convertible debt",
            "convertible notes",
            "convertible senior notes",
            "at-the-market",
            "atm program",
            "atm offering",
        ],
    )
}

fn is_confirmed_ipo_financing_event(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    contains_any(
        &lower,
        &[
            "prices ipo",
            "priced ipo",
            "ipo priced",
            "completed ipo",
            "completes ipo",
            "begins trading",
            "filed for ipo",
            "files for ipo",
            "filed to raise",
            "files to raise",
            "s-1",
        ],
    ) && has_confirmed_financing_amount(CapitalAbsorptionAutoEventCategory::IpoSupply, text)
}

fn has_confirmed_financing_amount(
    category: CapitalAbsorptionAutoEventCategory,
    text: &str,
) -> bool {
    if extract_usd_billions(text).is_none() {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "valuation",
            "valued at",
            "could be worth",
            "expected value",
            "projected valuation",
            "target valuation",
            "market cap",
        ],
    ) {
        return false;
    }
    match category {
        CapitalAbsorptionAutoEventCategory::IpoSupply => contains_any(
            &lower,
            &[
                "raise",
                "raises",
                "priced",
                "prices",
                "offering size",
                "gross proceeds",
                "proceeds",
            ],
        ),
        CapitalAbsorptionAutoEventCategory::MegaCapFinancing
        | CapitalAbsorptionAutoEventCategory::SecondaryLiquidity => true,
    }
}

fn is_weak_related_news(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    contains_any(
        &lower,
        &[
            "wall street analyst",
            "analyst research",
            "analyst rating",
            "analyst says",
            "analyst sees",
            "stock recommendation",
            "stocks to buy",
            "stock to buy",
            "buy before",
            "before the ipo",
            "before its ipo",
            "ahead of the ipo",
            "ahead of its ipo",
            "consider ahead of ipo",
        ],
    ) || (lower.contains("ipo") && lower.contains("competitor"))
        || (lower.contains("ipo") && lower.contains("related ticker"))
}

fn is_ai_ipo_candidate_subject(subject: &str) -> bool {
    AI_IPO_CANDIDATES
        .iter()
        .any(|candidate| subject.eq_ignore_ascii_case(candidate))
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
        assert_eq!(events[0].supply_kind, CapitalAbsorptionSupplyKind::Actual);
        assert_eq!(
            events[0].event_type,
            CapitalAbsorptionObservationEventType::Confirmed
        );
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
        assert_eq!(
            events[0].supply_kind,
            CapitalAbsorptionSupplyKind::Potential
        );
        assert_eq!(events[0].amount_usd_b, None);
    }

    #[test]
    fn keeps_anthropic_ipo_discussion_in_potential_queue_without_amount() {
        let raw = r#"[
          {
            "headline": "Anthropic IPO discussion grows after private valuation reaches $60 billion",
            "summary": "Investors are considering the company ahead of a possible IPO.",
            "url": "https://example.com/anthropic-ipo-discussion",
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
        assert_eq!(events[0].subject, "Anthropic");
        assert_eq!(
            events[0].supply_kind,
            CapitalAbsorptionSupplyKind::Potential
        );
        assert_eq!(
            events[0].event_type,
            CapitalAbsorptionObservationEventType::Reported
        );
        assert_eq!(events[0].amount_usd_b, None);
    }

    #[test]
    fn ignores_weak_ipo_related_stock_recommendations() {
        let raw = r#"[
          {
            "headline": "3 stocks to buy before the Anthropic IPO",
            "summary": "A Wall Street analyst research call mentions related tickers.",
            "url": "https://example.com/stocks-before-ipo",
            "datetime": 1780444800
          }
        ]"#;

        let events = extract_capital_absorption_events(
            "Market",
            raw,
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        )
        .unwrap();

        assert!(events.is_empty());
    }

    #[test]
    fn confirmed_ipo_uses_only_confirmed_financing_amount() {
        let raw = r#"[
          {
            "headline": "Figure filed for IPO to raise $750 million",
            "summary": "The S-1 confirms expected gross proceeds from the offering.",
            "url": "https://example.com/figure-ipo-filed",
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
        assert_eq!(events[0].subject, "Figure");
        assert_eq!(events[0].supply_kind, CapitalAbsorptionSupplyKind::Actual);
        assert_eq!(events[0].amount_usd_b, Some(0.75));
    }

    #[test]
    fn ignores_projected_ipo_valuation_amount_for_actual_supply() {
        let raw = r#"[
          {
            "headline": "Stripe IPO expected at $90 billion valuation",
            "summary": "The company remains an IPO candidate, with no offering amount confirmed.",
            "url": "https://example.com/stripe-ipo-valuation",
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
        assert_eq!(events[0].subject, "Stripe");
        assert_eq!(
            events[0].supply_kind,
            CapitalAbsorptionSupplyKind::Potential
        );
        assert_eq!(events[0].amount_usd_b, None);
    }
}

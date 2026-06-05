use chrono::NaiveDate;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionAutoStatus {
    Normal,
    Watch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionAutoRatioState {
    Low,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionAutoTrend {
    Decreasing,
    Stable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionPotentialSupplyTrend {
    Falling,
    Stable,
    Rising,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionAutoEventCategory {
    MegaCapFinancing,
    IpoSupply,
    SecondaryLiquidity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionSupplyKind {
    Actual,
    Potential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CapitalAbsorptionObservationEventType {
    Rumor,
    Reported,
    Confirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionAutoConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CapitalAbsorptionIpoQueueStatus {
    Rumor,
    Expected,
    Filed,
    Scheduled,
    Completed,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionAutoEvent {
    pub category: CapitalAbsorptionAutoEventCategory,
    pub supply_kind: CapitalAbsorptionSupplyKind,
    pub event_type: CapitalAbsorptionObservationEventType,
    pub subject: String,
    pub description: String,
    pub amount_usd_b: Option<f64>,
    pub ai_capex_related: bool,
    pub source_url: Option<String>,
    pub observed_at: NaiveDate,
    pub source_count: usize,
    pub confidence: CapitalAbsorptionAutoConfidence,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionSupplyEventCounts {
    pub mega_cap_financing: usize,
    pub ai_ipo_candidate: usize,
    pub secondary_offering: usize,
    pub convertible_debt: usize,
    pub secondary_liquidity: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionIpoQueueItem {
    pub issuer: String,
    pub status: CapitalAbsorptionIpoQueueStatus,
    pub source_count: usize,
    pub event_type: CapitalAbsorptionObservationEventType,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionIpoQueueHistoryPoint {
    pub observed_at: NaiveDate,
    pub queue_size: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionAutoSnapshot {
    pub source_status: CapitalAbsorptionSourceStatus,
    pub status: CapitalAbsorptionAutoStatus,
    pub observed_events: Vec<CapitalAbsorptionAutoEvent>,
    pub supply_event_counts: CapitalAbsorptionSupplyEventCounts,
    pub ai_ipo_queue: Vec<CapitalAbsorptionIpoQueueItem>,
    pub ipo_queue_history: Vec<CapitalAbsorptionIpoQueueHistoryPoint>,
    pub potential_supply_trend: CapitalAbsorptionPotentialSupplyTrend,
    pub capital_demand: CapitalDemandAutoSnapshot,
    pub capital_supply: CapitalSupplyAutoSnapshot,
    pub absorption_ratio: CapitalAbsorptionAutoRatio,
    pub structural_impact: String,
    pub upgrade_to_active: Vec<String>,
    pub upgrade_to_stressed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionSourceStatus {
    pub provider: String,
    pub status: CapitalAbsorptionSourceHealth,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionSourceHealth {
    Succeeded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalDemandAutoSnapshot {
    pub rolling_12m_usd_b: Option<f64>,
    pub score: Option<f64>,
    pub trend: CapitalAbsorptionAutoTrend,
    pub ipo_financing_usd_b: Option<f64>,
    pub secondary_offering_usd_b: Option<f64>,
    pub convertible_debt_usd_b: Option<f64>,
    pub ai_related_financing_usd_b: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalSupplyAutoSnapshot {
    pub rolling_12m_usd_b: Option<f64>,
    pub score: Option<f64>,
    pub trend: CapitalAbsorptionAutoTrend,
    pub etf_net_inflow_usd_b: Option<f64>,
    pub mutual_fund_net_inflow_usd_b: Option<f64>,
    pub pension_allocation_flow_usd_b: Option<f64>,
    pub foreign_capital_inflow_usd_b: Option<f64>,
    pub corporate_buyback_usd_b: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionAutoRatio {
    pub value: Option<f64>,
    pub state: CapitalAbsorptionAutoRatioState,
}

pub(crate) fn build_capital_absorption_snapshot_from_events(
    events: Vec<CapitalAbsorptionAutoEvent>,
    source_status: CapitalAbsorptionSourceStatus,
) -> CapitalAbsorptionAutoSnapshot {
    let events = deduplicate_events(events);
    let actual_events = events
        .iter()
        .filter(|event| event.supply_kind == CapitalAbsorptionSupplyKind::Actual)
        .collect::<Vec<_>>();
    let potential_events = events
        .iter()
        .filter(|event| event.supply_kind == CapitalAbsorptionSupplyKind::Potential)
        .collect::<Vec<_>>();
    let demand_total = sum_amounts(actual_events.iter().copied());
    let ai_related = sum_amounts(
        actual_events
            .iter()
            .copied()
            .filter(|event| event.ai_capex_related),
    );
    let ipo_total = sum_amounts(
        actual_events
            .iter()
            .copied()
            .filter(|event| event.category == CapitalAbsorptionAutoEventCategory::IpoSupply),
    );
    let secondary_total =
        sum_amounts(actual_events.iter().copied().filter(|event| {
            event.category == CapitalAbsorptionAutoEventCategory::MegaCapFinancing
        }));
    let convertible_total = sum_amounts(actual_events.iter().copied().filter(|event| {
        event
            .description
            .to_ascii_lowercase()
            .contains("convertible")
    }));
    let unique_subjects = unique_subject_count(&events);
    let status = classify_status(&events, demand_total, unique_subjects);
    let supply_event_counts = build_supply_event_counts(&actual_events);
    let auto_source_available = source_status.status != CapitalAbsorptionSourceHealth::Unavailable;
    let ai_ipo_queue = if auto_source_available {
        build_ai_ipo_queue(&events)
    } else {
        Vec::new()
    };
    let ipo_queue_history = if auto_source_available {
        build_ipo_queue_history(&potential_events)
    } else {
        Vec::new()
    };
    let potential_supply_trend = classify_potential_supply_trend(&ipo_queue_history);
    let supply_trend = if actual_events
        .iter()
        .any(|event| event.category == CapitalAbsorptionAutoEventCategory::SecondaryLiquidity)
    {
        CapitalAbsorptionAutoTrend::Decreasing
    } else {
        CapitalAbsorptionAutoTrend::Stable
    };
    let ratio_state = match status {
        CapitalAbsorptionAutoStatus::Normal => CapitalAbsorptionAutoRatioState::Low,
        CapitalAbsorptionAutoStatus::Watch => CapitalAbsorptionAutoRatioState::Neutral,
    };
    CapitalAbsorptionAutoSnapshot {
        source_status,
        status,
        observed_events: events,
        supply_event_counts,
        ai_ipo_queue,
        ipo_queue_history,
        potential_supply_trend,
        capital_demand: CapitalDemandAutoSnapshot {
            rolling_12m_usd_b: demand_total,
            score: demand_total.map(|value| (value / 100.0).min(1.0)),
            trend: CapitalAbsorptionAutoTrend::Stable,
            ipo_financing_usd_b: ipo_total,
            secondary_offering_usd_b: secondary_total,
            convertible_debt_usd_b: convertible_total,
            ai_related_financing_usd_b: ai_related,
        },
        capital_supply: CapitalSupplyAutoSnapshot {
            rolling_12m_usd_b: None,
            score: None,
            trend: supply_trend,
            etf_net_inflow_usd_b: None,
            mutual_fund_net_inflow_usd_b: None,
            pension_allocation_flow_usd_b: None,
            foreign_capital_inflow_usd_b: None,
            corporate_buyback_usd_b: None,
        },
        absorption_ratio: CapitalAbsorptionAutoRatio {
            value: None,
            state: ratio_state,
        },
        structural_impact: "Observation Only".to_string(),
        upgrade_to_active: vec![
            "Second mega cap financing".to_string(),
            "Large AI IPO starts".to_string(),
            "Repeated large equity financing events".to_string(),
        ],
        upgrade_to_stressed: vec![
            "Capital Demand > Capital Supply".to_string(),
            "ETF net inflow remains weaker than financing scale".to_string(),
            "Liquidity absorption pressure appears in market structure".to_string(),
        ],
    }
}

pub(crate) fn unavailable_capital_absorption_snapshot(
    message: String,
) -> CapitalAbsorptionAutoSnapshot {
    build_capital_absorption_snapshot_from_events(
        Vec::new(),
        CapitalAbsorptionSourceStatus {
            provider: "Finnhub company-news".to_string(),
            status: CapitalAbsorptionSourceHealth::Unavailable,
            message,
        },
    )
}

fn classify_status(
    events: &[CapitalAbsorptionAutoEvent],
    _demand_total: Option<f64>,
    _unique_subjects: usize,
) -> CapitalAbsorptionAutoStatus {
    if !events.is_empty() {
        CapitalAbsorptionAutoStatus::Watch
    } else {
        CapitalAbsorptionAutoStatus::Normal
    }
}

fn deduplicate_events(events: Vec<CapitalAbsorptionAutoEvent>) -> Vec<CapitalAbsorptionAutoEvent> {
    let mut grouped: HashMap<String, CapitalAbsorptionAutoEvent> = HashMap::new();
    for mut event in events {
        let key = event_key(&event);
        event.source_count = event.source_count.max(1);
        event.confidence = confidence_from_source_count(event.source_count);
        grouped
            .entry(key)
            .and_modify(|existing| merge_event(existing, &event))
            .or_insert(event);
    }
    let mut events = grouped.into_values().collect::<Vec<_>>();
    events.sort_by(|a, b| {
        a.observed_at
            .cmp(&b.observed_at)
            .reverse()
            .then_with(|| a.subject.cmp(&b.subject))
            .then_with(|| a.description.cmp(&b.description))
    });
    events
}

fn merge_event(existing: &mut CapitalAbsorptionAutoEvent, incoming: &CapitalAbsorptionAutoEvent) {
    existing.source_count += incoming.source_count.max(1);
    existing.confidence = confidence_from_source_count(existing.source_count);
    existing.ai_capex_related |= incoming.ai_capex_related;
    existing.event_type = existing.event_type.max(incoming.event_type);
    if incoming.supply_kind == CapitalAbsorptionSupplyKind::Actual {
        existing.supply_kind = CapitalAbsorptionSupplyKind::Actual;
    }
    existing.amount_usd_b = match (existing.amount_usd_b, incoming.amount_usd_b) {
        (Some(current), Some(next)) => Some(current.max(next)),
        (Some(current), None) => Some(current),
        (None, Some(next)) => Some(next),
        (None, None) => None,
    };
    if incoming.observed_at > existing.observed_at {
        existing.observed_at = incoming.observed_at;
        existing.description = incoming.description.clone();
        existing.source_url = incoming.source_url.clone();
    }
}

fn event_key(event: &CapitalAbsorptionAutoEvent) -> String {
    format!(
        "{}:{}:{}",
        event_category_key(event.category),
        normalize_subject(&event.subject),
        event_family(&event.description)
    )
}

fn event_category_key(category: CapitalAbsorptionAutoEventCategory) -> &'static str {
    match category {
        CapitalAbsorptionAutoEventCategory::MegaCapFinancing => "mega_cap_financing",
        CapitalAbsorptionAutoEventCategory::IpoSupply => "ipo_supply",
        CapitalAbsorptionAutoEventCategory::SecondaryLiquidity => "secondary_liquidity",
    }
}

fn normalize_subject(subject: &str) -> String {
    subject
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

fn event_family(description: &str) -> &'static str {
    let lower = description.to_ascii_lowercase();
    if lower.contains("convertible") {
        "convertible_debt"
    } else if lower.contains("secondary offering")
        || lower.contains("stock offering")
        || lower.contains("share offering")
        || lower.contains("at-the-market")
        || lower.contains("atm offering")
    {
        "secondary_offering"
    } else if lower.contains("ipo")
        || lower.contains("initial public offering")
        || lower.contains("go public")
        || lower.contains("listing")
    {
        "ipo"
    } else if lower.contains("tender") || lower.contains("secondary sale") {
        "secondary_liquidity"
    } else {
        "financing"
    }
}

fn confidence_from_source_count(source_count: usize) -> CapitalAbsorptionAutoConfidence {
    match source_count {
        0 | 1 => CapitalAbsorptionAutoConfidence::Low,
        2 => CapitalAbsorptionAutoConfidence::Medium,
        _ => CapitalAbsorptionAutoConfidence::High,
    }
}

fn build_supply_event_counts(
    events: &[&CapitalAbsorptionAutoEvent],
) -> CapitalAbsorptionSupplyEventCounts {
    CapitalAbsorptionSupplyEventCounts {
        mega_cap_financing: events
            .iter()
            .filter(|event| event.category == CapitalAbsorptionAutoEventCategory::MegaCapFinancing)
            .count(),
        ai_ipo_candidate: events
            .iter()
            .filter(|event| {
                event.category == CapitalAbsorptionAutoEventCategory::IpoSupply
                    && is_ai_ipo_candidate(&event.subject)
            })
            .count(),
        secondary_offering: events
            .iter()
            .filter(|event| event_family(&event.description) == "secondary_offering")
            .count(),
        convertible_debt: events
            .iter()
            .filter(|event| event_family(&event.description) == "convertible_debt")
            .count(),
        secondary_liquidity: events
            .iter()
            .filter(|event| {
                event.category == CapitalAbsorptionAutoEventCategory::SecondaryLiquidity
            })
            .count(),
    }
}

fn build_ai_ipo_queue(events: &[CapitalAbsorptionAutoEvent]) -> Vec<CapitalAbsorptionIpoQueueItem> {
    [
        "Anthropic",
        "OpenAI",
        "SpaceX",
        "Databricks",
        "Stripe",
        "Figure",
    ]
    .iter()
    .map(|issuer| {
        let mut status = CapitalAbsorptionIpoQueueStatus::Rumor;
        let mut source_count = 0;
        let mut event_type = CapitalAbsorptionObservationEventType::Rumor;
        for event in events
            .iter()
            .filter(|event| same_issuer(&event.subject, issuer))
        {
            status = status.max(queue_status_from_text(&event.description));
            source_count += event.source_count.max(1);
            event_type = event_type.max(event.event_type);
        }
        CapitalAbsorptionIpoQueueItem {
            issuer: (*issuer).to_string(),
            status,
            source_count,
            event_type,
        }
    })
    .collect()
}

fn build_ipo_queue_history(
    events: &[&CapitalAbsorptionAutoEvent],
) -> Vec<CapitalAbsorptionIpoQueueHistoryPoint> {
    let mut dates = events
        .iter()
        .map(|event| event.observed_at)
        .collect::<Vec<_>>();
    dates.sort_unstable();
    dates.dedup();
    dates
        .into_iter()
        .map(|observed_at| {
            let mut issuers = events
                .iter()
                .filter(|event| event.observed_at <= observed_at)
                .filter(|event| is_ai_ipo_candidate(&event.subject))
                .map(|event| normalize_subject(&event.subject))
                .collect::<Vec<_>>();
            issuers.sort_unstable();
            issuers.dedup();
            CapitalAbsorptionIpoQueueHistoryPoint {
                observed_at,
                queue_size: issuers.len(),
            }
        })
        .collect()
}

fn classify_potential_supply_trend(
    history: &[CapitalAbsorptionIpoQueueHistoryPoint],
) -> CapitalAbsorptionPotentialSupplyTrend {
    match (history.first(), history.last()) {
        (Some(_), Some(last)) if history.len() == 1 && last.queue_size > 0 => {
            CapitalAbsorptionPotentialSupplyTrend::Rising
        }
        (Some(first), Some(last)) if last.queue_size > first.queue_size => {
            CapitalAbsorptionPotentialSupplyTrend::Rising
        }
        (Some(first), Some(last)) if last.queue_size < first.queue_size => {
            CapitalAbsorptionPotentialSupplyTrend::Falling
        }
        (None, _) => CapitalAbsorptionPotentialSupplyTrend::Stable,
        _ => CapitalAbsorptionPotentialSupplyTrend::Stable,
    }
}

fn same_issuer(subject: &str, issuer: &str) -> bool {
    normalize_subject(subject) == normalize_subject(issuer)
}

fn is_ai_ipo_candidate(subject: &str) -> bool {
    [
        "Anthropic",
        "OpenAI",
        "SpaceX",
        "Databricks",
        "Stripe",
        "Figure",
    ]
    .iter()
    .any(|issuer| same_issuer(subject, issuer))
}

fn queue_status_from_text(text: &str) -> CapitalAbsorptionIpoQueueStatus {
    let lower = text.to_ascii_lowercase();
    if lower.contains("completed")
        || lower.contains("debut")
        || lower.contains("begins trading")
        || lower.contains("listed")
    {
        CapitalAbsorptionIpoQueueStatus::Completed
    } else if lower.contains("scheduled")
        || lower.contains("prices ipo")
        || lower.contains("expected to price")
    {
        CapitalAbsorptionIpoQueueStatus::Scheduled
    } else if lower.contains("filed")
        || lower.contains("files to go public")
        || lower.contains("s-1")
    {
        CapitalAbsorptionIpoQueueStatus::Filed
    } else if lower.contains("expected")
        || lower.contains("plans")
        || lower.contains("prepares")
        || lower.contains("considering")
    {
        CapitalAbsorptionIpoQueueStatus::Expected
    } else {
        CapitalAbsorptionIpoQueueStatus::Rumor
    }
}

fn sum_amounts<'a>(
    events: impl IntoIterator<Item = &'a CapitalAbsorptionAutoEvent>,
) -> Option<f64> {
    let mut total = 0.0;
    let mut found = false;
    for event in events {
        if let Some(amount) = event.amount_usd_b {
            total += amount;
            found = true;
        }
    }
    found.then_some(total)
}

fn unique_subject_count(events: &[CapitalAbsorptionAutoEvent]) -> usize {
    let mut subjects = events
        .iter()
        .map(|event| event.subject.as_str())
        .collect::<Vec<_>>();
    subjects.sort_unstable();
    subjects.dedup();
    subjects.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_automatic_observation_status_at_watch_without_trade_signal() {
        let events = vec![event("GOOG", 80.0), event("MSFT", 40.0)];

        let snapshot = build_capital_absorption_snapshot_from_events(
            events,
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "fixture".to_string(),
            },
        );

        assert_eq!(snapshot.status, CapitalAbsorptionAutoStatus::Watch);
        assert_eq!(snapshot.capital_demand.rolling_12m_usd_b, Some(120.0));
        assert_eq!(
            snapshot.potential_supply_trend,
            CapitalAbsorptionPotentialSupplyTrend::Stable
        );
        assert_eq!(
            snapshot.absorption_ratio.state,
            CapitalAbsorptionAutoRatioState::Neutral
        );
        assert_eq!(snapshot.structural_impact, "Observation Only");
    }

    #[test]
    fn deduplicates_repeated_news_and_keeps_sources_count() {
        let mut first = event("SpaceX", 0.0);
        first.category = CapitalAbsorptionAutoEventCategory::IpoSupply;
        first.supply_kind = CapitalAbsorptionSupplyKind::Potential;
        first.event_type = CapitalAbsorptionObservationEventType::Rumor;
        first.description =
            "SpaceX IPO expected as public listing discussion increases".to_string();
        first.amount_usd_b = None;
        let mut second = first.clone();
        second.description = "SpaceX IPO expected amid investor discussion".to_string();

        let snapshot = build_capital_absorption_snapshot_from_events(
            vec![first, second],
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "fixture".to_string(),
            },
        );

        assert_eq!(snapshot.observed_events.len(), 1);
        assert_eq!(snapshot.observed_events[0].source_count, 2);
        assert_eq!(
            snapshot.observed_events[0].confidence,
            CapitalAbsorptionAutoConfidence::Medium
        );
        assert_eq!(snapshot.capital_demand.rolling_12m_usd_b, None);
        assert_eq!(snapshot.supply_event_counts.ai_ipo_candidate, 0);
        assert_eq!(
            snapshot.potential_supply_trend,
            CapitalAbsorptionPotentialSupplyTrend::Rising
        );
        assert_eq!(snapshot.ipo_queue_history.len(), 1);
        assert_eq!(snapshot.ipo_queue_history[0].queue_size, 1);
        assert_eq!(
            snapshot
                .ai_ipo_queue
                .iter()
                .find(|item| item.issuer == "SpaceX")
                .map(|item| item.status),
            Some(CapitalAbsorptionIpoQueueStatus::Expected)
        );
    }

    #[test]
    fn unavailable_source_does_not_emit_default_ipo_queue() {
        let snapshot = unavailable_capital_absorption_snapshot("429 Too Many Requests".to_string());

        assert_eq!(
            snapshot.source_status.status,
            CapitalAbsorptionSourceHealth::Unavailable
        );
        assert!(snapshot.observed_events.is_empty());
        assert!(snapshot.ai_ipo_queue.is_empty());
        assert!(snapshot.ipo_queue_history.is_empty());
        assert_eq!(snapshot.capital_demand.rolling_12m_usd_b, None);
    }

    fn event(subject: &str, amount_usd_b: f64) -> CapitalAbsorptionAutoEvent {
        CapitalAbsorptionAutoEvent {
            category: CapitalAbsorptionAutoEventCategory::MegaCapFinancing,
            supply_kind: CapitalAbsorptionSupplyKind::Actual,
            event_type: CapitalAbsorptionObservationEventType::Confirmed,
            subject: subject.to_string(),
            description: "secondary offering for AI capex".to_string(),
            amount_usd_b: Some(amount_usd_b),
            ai_capex_related: true,
            source_url: None,
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
            source_count: 1,
            confidence: CapitalAbsorptionAutoConfidence::Low,
        }
    }
}

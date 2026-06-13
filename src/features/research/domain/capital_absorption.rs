use chrono::{Duration, NaiveDate};
use std::collections::HashMap;

const AI_IPO_CANDIDATES: &[&str] = &[
    "Anthropic",
    "OpenAI",
    "SpaceX",
    "Databricks",
    "Stripe",
    "Figure",
];

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
pub(crate) enum CapitalAbsorptionPotentialSupplyPressureLevel {
    Low,
    Normal,
    Elevated,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CapitalAbsorptionIpoLifecycleStatus {
    Rumor,
    Reported,
    Confirmed,
    Listed,
    Observed,
    Graduated,
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
    Reported,
    Preparation,
    PreIpo,
    NearTerm,
    Filed,
    Ipo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionSupplyTimelineBucket {
    Next30Days,
    Next12Months,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionNearTermSupplyWeight {
    High,
    Medium,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionPressureDriverStrength {
    High,
    Medium,
    Low,
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
    pub lifecycle_status: CapitalAbsorptionIpoLifecycleStatus,
    pub observed_at: Option<NaiveDate>,
    pub observation_day: Option<i64>,
    pub near_term_weight: Option<CapitalAbsorptionNearTermSupplyWeight>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionSupplyTimelineItem {
    pub issuer: String,
    pub bucket: CapitalAbsorptionSupplyTimelineBucket,
    pub lifecycle_status: CapitalAbsorptionIpoLifecycleStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionObservationWatchlistItem {
    pub issuer: String,
    pub lifecycle_status: CapitalAbsorptionIpoLifecycleStatus,
    pub observation_day: Option<i64>,
    pub review_window_days: Option<i64>,
    pub review_candidate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionIpoQueueHistoryPoint {
    pub observed_at: NaiveDate,
    pub queue_size: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionPotentialSupplyPressureDriver {
    pub label: String,
    pub strength: CapitalAbsorptionPressureDriverStrength,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionPotentialSupplyPressure {
    pub level: CapitalAbsorptionPotentialSupplyPressureLevel,
    pub near_term_supply_count: usize,
    pub future_queue_count: usize,
    pub queue_count: usize,
    pub reported_count: usize,
    pub confirmed_count: usize,
    pub drivers: Vec<CapitalAbsorptionPotentialSupplyPressureDriver>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionAutoSnapshot {
    pub source_status: CapitalAbsorptionSourceStatus,
    pub status: CapitalAbsorptionAutoStatus,
    pub observed_events: Vec<CapitalAbsorptionAutoEvent>,
    pub supply_event_counts: CapitalAbsorptionSupplyEventCounts,
    pub near_term_supply: Vec<CapitalAbsorptionIpoQueueItem>,
    pub ai_ipo_queue: Vec<CapitalAbsorptionIpoQueueItem>,
    pub upcoming_supply_timeline: Vec<CapitalAbsorptionSupplyTimelineItem>,
    pub observation_watchlist: Vec<CapitalAbsorptionObservationWatchlistItem>,
    pub ipo_queue_history: Vec<CapitalAbsorptionIpoQueueHistoryPoint>,
    pub potential_supply_trend: CapitalAbsorptionPotentialSupplyTrend,
    pub potential_supply_pressure: CapitalAbsorptionPotentialSupplyPressure,
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
    let (near_term_supply, ai_ipo_queue, observation_watchlist) = if auto_source_available {
        split_supply_queue(build_ai_ipo_queue(&events))
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    let upcoming_supply_timeline = build_upcoming_supply_timeline(&near_term_supply, &ai_ipo_queue);
    let ipo_queue_history = if auto_source_available {
        build_ipo_queue_history(&ai_ipo_queue, &potential_events)
    } else {
        Vec::new()
    };
    let potential_supply_trend = classify_potential_supply_trend(&ipo_queue_history);
    let potential_supply_pressure =
        classify_potential_supply_pressure(&near_term_supply, &ai_ipo_queue);
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
        near_term_supply,
        ai_ipo_queue,
        upcoming_supply_timeline,
        observation_watchlist,
        ipo_queue_history,
        potential_supply_trend,
        potential_supply_pressure,
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

pub(crate) fn classify_capital_absorption_news_observation(
    symbol: &str,
    headline: &str,
    summary: &str,
    observed_at: NaiveDate,
    source_url: Option<String>,
) -> Option<CapitalAbsorptionAutoEvent> {
    let text = format!("{headline} {summary}");
    if is_weak_related_news(&text) {
        return None;
    }
    let category = classify_capital_absorption_event(&text)?;
    let subject = detect_event_subject(symbol, &text);
    let event_type = observation_event_type_from_text(&text);
    let supply_kind = supply_kind_from_category_type_and_text(category, event_type, &text)?;
    if supply_kind == CapitalAbsorptionSupplyKind::Potential && !is_ai_ipo_candidate(&subject) {
        return None;
    }
    let amount_usd_b = if supply_kind == CapitalAbsorptionSupplyKind::Actual {
        extract_confirmed_financing_amount(category, &text)
    } else {
        None
    };
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

pub(crate) fn event_family(description: &str) -> &'static str {
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
    let latest_observed_at = events.iter().map(|event| event.observed_at).max();
    AI_IPO_CANDIDATES
        .iter()
        .map(|issuer| {
            let mut status = CapitalAbsorptionIpoQueueStatus::Rumor;
            let mut source_count = 0;
            let mut event_type = CapitalAbsorptionObservationEventType::Rumor;
            let mut observed_at = None;
            for event in events
                .iter()
                .filter(|event| same_issuer(&event.subject, issuer))
            {
                source_count += event.source_count.max(1);
                event_type = event_type.max(event.event_type);
                status = status.max(queue_status_from_event(event));
                observed_at = Some(observed_at.map_or(event.observed_at, |current: NaiveDate| {
                    current.max(event.observed_at)
                }));
            }
            let lifecycle_status = lifecycle_status_from_queue_status(
                status,
                event_type,
                observed_at,
                latest_observed_at,
            );
            CapitalAbsorptionIpoQueueItem {
                issuer: (*issuer).to_string(),
                status,
                source_count,
                event_type,
                lifecycle_status,
                observed_at,
                observation_day: observation_day(observed_at, latest_observed_at),
                near_term_weight: near_term_weight(status, observed_at, latest_observed_at),
            }
        })
        .collect()
}

fn split_supply_queue(
    queue: Vec<CapitalAbsorptionIpoQueueItem>,
) -> (
    Vec<CapitalAbsorptionIpoQueueItem>,
    Vec<CapitalAbsorptionIpoQueueItem>,
    Vec<CapitalAbsorptionObservationWatchlistItem>,
) {
    let mut near_term_supply = Vec::new();
    let mut future_queue = Vec::new();
    let mut observation_watchlist = Vec::new();
    for item in queue {
        if item.source_count > 0 {
            observation_watchlist.push(observation_watchlist_item(&item));
        }
        if !is_observation_asset(&item) {
            if is_near_term_supply(&item) {
                near_term_supply.push(item);
            } else {
                future_queue.push(item);
            }
        }
    }
    near_term_supply.sort_by(|a, b| a.issuer.cmp(&b.issuer));
    future_queue.sort_by(|a, b| a.issuer.cmp(&b.issuer));
    observation_watchlist.sort_by(|a, b| a.issuer.cmp(&b.issuer));
    (near_term_supply, future_queue, observation_watchlist)
}

fn is_near_term_supply(item: &CapitalAbsorptionIpoQueueItem) -> bool {
    item.source_count > 0
        && item.near_term_weight != Some(CapitalAbsorptionNearTermSupplyWeight::Expired)
        && (item.status >= CapitalAbsorptionIpoQueueStatus::NearTerm
            || item.event_type == CapitalAbsorptionObservationEventType::Confirmed)
}

fn is_observation_asset(item: &CapitalAbsorptionIpoQueueItem) -> bool {
    item.source_count > 0 && item.lifecycle_status >= CapitalAbsorptionIpoLifecycleStatus::Listed
}

fn observation_watchlist_item(
    item: &CapitalAbsorptionIpoQueueItem,
) -> CapitalAbsorptionObservationWatchlistItem {
    let post_ipo = item.lifecycle_status >= CapitalAbsorptionIpoLifecycleStatus::Listed;
    let observation_day = post_ipo.then_some(item.observation_day).flatten();
    let review_window_days = post_ipo.then_some(90);
    let review_candidate = observation_day.is_some_and(|day| day >= 90);
    CapitalAbsorptionObservationWatchlistItem {
        issuer: item.issuer.clone(),
        lifecycle_status: item.lifecycle_status,
        observation_day,
        review_window_days,
        review_candidate,
    }
}

fn observation_day(
    observed_at: Option<NaiveDate>,
    latest_observed_at: Option<NaiveDate>,
) -> Option<i64> {
    match (observed_at, latest_observed_at) {
        (Some(observed_at), Some(latest_observed_at)) => Some(
            latest_observed_at
                .signed_duration_since(observed_at)
                .num_days()
                .max(0)
                + 1,
        ),
        _ => None,
    }
}

fn near_term_weight(
    status: CapitalAbsorptionIpoQueueStatus,
    observed_at: Option<NaiveDate>,
    latest_observed_at: Option<NaiveDate>,
) -> Option<CapitalAbsorptionNearTermSupplyWeight> {
    if status < CapitalAbsorptionIpoQueueStatus::NearTerm {
        return None;
    }
    match observation_day(observed_at, latest_observed_at) {
        Some(day) if day <= 30 => Some(CapitalAbsorptionNearTermSupplyWeight::High),
        Some(day) if day <= 90 => Some(CapitalAbsorptionNearTermSupplyWeight::Medium),
        Some(_) => Some(CapitalAbsorptionNearTermSupplyWeight::Expired),
        None => Some(CapitalAbsorptionNearTermSupplyWeight::High),
    }
}

fn lifecycle_status_from_queue_status(
    status: CapitalAbsorptionIpoQueueStatus,
    event_type: CapitalAbsorptionObservationEventType,
    observed_at: Option<NaiveDate>,
    latest_observed_at: Option<NaiveDate>,
) -> CapitalAbsorptionIpoLifecycleStatus {
    if status >= CapitalAbsorptionIpoQueueStatus::Ipo {
        return match observation_day(observed_at, latest_observed_at) {
            Some(day) if day >= 180 => CapitalAbsorptionIpoLifecycleStatus::Graduated,
            Some(day) if day >= 30 => CapitalAbsorptionIpoLifecycleStatus::Observed,
            _ => CapitalAbsorptionIpoLifecycleStatus::Listed,
        };
    }
    if event_type == CapitalAbsorptionObservationEventType::Confirmed {
        CapitalAbsorptionIpoLifecycleStatus::Confirmed
    } else if event_type == CapitalAbsorptionObservationEventType::Reported
        || status >= CapitalAbsorptionIpoQueueStatus::Reported
    {
        CapitalAbsorptionIpoLifecycleStatus::Reported
    } else {
        CapitalAbsorptionIpoLifecycleStatus::Rumor
    }
}

fn build_upcoming_supply_timeline(
    near_term_supply: &[CapitalAbsorptionIpoQueueItem],
    future_queue: &[CapitalAbsorptionIpoQueueItem],
) -> Vec<CapitalAbsorptionSupplyTimelineItem> {
    let mut timeline = Vec::new();
    for item in near_term_supply.iter().filter(|item| item.source_count > 0) {
        timeline.push(CapitalAbsorptionSupplyTimelineItem {
            issuer: item.issuer.clone(),
            bucket: CapitalAbsorptionSupplyTimelineBucket::Next30Days,
            lifecycle_status: item.lifecycle_status,
        });
    }
    for item in future_queue.iter().filter(|item| item.source_count > 0) {
        let bucket = match item.status {
            CapitalAbsorptionIpoQueueStatus::Preparation
            | CapitalAbsorptionIpoQueueStatus::PreIpo => {
                CapitalAbsorptionSupplyTimelineBucket::Next12Months
            }
            CapitalAbsorptionIpoQueueStatus::Rumor | CapitalAbsorptionIpoQueueStatus::Reported => {
                CapitalAbsorptionSupplyTimelineBucket::Unknown
            }
            CapitalAbsorptionIpoQueueStatus::NearTerm
            | CapitalAbsorptionIpoQueueStatus::Filed
            | CapitalAbsorptionIpoQueueStatus::Ipo => {
                CapitalAbsorptionSupplyTimelineBucket::Next30Days
            }
        };
        timeline.push(CapitalAbsorptionSupplyTimelineItem {
            issuer: item.issuer.clone(),
            bucket,
            lifecycle_status: item.lifecycle_status,
        });
    }
    timeline.sort_by(|a, b| {
        timeline_bucket_order(a.bucket)
            .cmp(&timeline_bucket_order(b.bucket))
            .then_with(|| a.issuer.cmp(&b.issuer))
    });
    timeline
}

fn timeline_bucket_order(bucket: CapitalAbsorptionSupplyTimelineBucket) -> u8 {
    match bucket {
        CapitalAbsorptionSupplyTimelineBucket::Next30Days => 0,
        CapitalAbsorptionSupplyTimelineBucket::Next12Months => 1,
        CapitalAbsorptionSupplyTimelineBucket::Unknown => 2,
    }
}

fn build_ipo_queue_history(
    future_queue: &[CapitalAbsorptionIpoQueueItem],
    events: &[&CapitalAbsorptionAutoEvent],
) -> Vec<CapitalAbsorptionIpoQueueHistoryPoint> {
    let Some(latest_observed_at) = events.iter().map(|event| event.observed_at).max() else {
        return Vec::new();
    };
    let first_history_date = latest_observed_at - Duration::days(29);
    let days = latest_observed_at
        .signed_duration_since(first_history_date)
        .num_days();
    (0..=days)
        .map(|offset| first_history_date + Duration::days(offset))
        .map(|observed_at| {
            let mut issuers = events
                .iter()
                .filter(|event| event.observed_at <= observed_at)
                .filter(|event| is_ai_ipo_candidate(&event.subject))
                .filter(|event| {
                    future_queue
                        .iter()
                        .any(|item| same_issuer(&event.subject, &item.issuer))
                })
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

fn classify_potential_supply_pressure(
    near_term_supply: &[CapitalAbsorptionIpoQueueItem],
    future_queue: &[CapitalAbsorptionIpoQueueItem],
) -> CapitalAbsorptionPotentialSupplyPressure {
    let near_term_supply_count = near_term_supply
        .iter()
        .filter(|item| item.source_count > 0)
        .count();
    let future_queue_count = future_queue
        .iter()
        .filter(|item| item.source_count > 0)
        .count();
    let reported_count = future_queue
        .iter()
        .filter(|item| {
            item.source_count > 0
                && (item.status >= CapitalAbsorptionIpoQueueStatus::Reported
                    || item.event_type >= CapitalAbsorptionObservationEventType::Reported)
        })
        .count();
    let confirmed_count = near_term_supply
        .iter()
        .filter(|item| {
            item.source_count > 0
                && (item.status >= CapitalAbsorptionIpoQueueStatus::Filed
                    || item.event_type == CapitalAbsorptionObservationEventType::Confirmed)
        })
        .count();
    let drivers = build_potential_supply_pressure_drivers(near_term_supply, future_queue);
    let level = if near_term_supply_count >= 2
        || (near_term_supply_count >= 1 && future_queue_count >= 2)
        || reported_count >= 3
        || future_queue_count >= 4
    {
        CapitalAbsorptionPotentialSupplyPressureLevel::Elevated
    } else if near_term_supply_count > 0 || reported_count > 0 || future_queue_count > 0 {
        CapitalAbsorptionPotentialSupplyPressureLevel::Normal
    } else {
        CapitalAbsorptionPotentialSupplyPressureLevel::Low
    };
    CapitalAbsorptionPotentialSupplyPressure {
        level,
        near_term_supply_count,
        future_queue_count,
        queue_count: future_queue_count,
        reported_count,
        confirmed_count,
        drivers,
    }
}

fn build_potential_supply_pressure_drivers(
    near_term_supply: &[CapitalAbsorptionIpoQueueItem],
    future_queue: &[CapitalAbsorptionIpoQueueItem],
) -> Vec<CapitalAbsorptionPotentialSupplyPressureDriver> {
    let mut drivers = Vec::new();
    for item in near_term_supply.iter().filter(|item| item.source_count > 0) {
        drivers.push(CapitalAbsorptionPotentialSupplyPressureDriver {
            label: format!("{} IPO", item.issuer),
            strength: match item.near_term_weight {
                Some(CapitalAbsorptionNearTermSupplyWeight::Medium) => {
                    CapitalAbsorptionPressureDriverStrength::Medium
                }
                _ => CapitalAbsorptionPressureDriverStrength::High,
            },
        });
    }
    for item in future_queue.iter().filter(|item| item.source_count > 0) {
        drivers.push(CapitalAbsorptionPotentialSupplyPressureDriver {
            label: format!("{} IPO Discussion", item.issuer),
            strength: pressure_driver_strength_from_item(item),
        });
    }
    drivers.sort_by(|a, b| {
        pressure_driver_strength_rank(b.strength)
            .cmp(&pressure_driver_strength_rank(a.strength))
            .then_with(|| a.label.cmp(&b.label))
    });
    drivers
}

fn pressure_driver_strength_from_item(
    item: &CapitalAbsorptionIpoQueueItem,
) -> CapitalAbsorptionPressureDriverStrength {
    if item.status >= CapitalAbsorptionIpoQueueStatus::Preparation
        || item.event_type >= CapitalAbsorptionObservationEventType::Reported
    {
        CapitalAbsorptionPressureDriverStrength::Medium
    } else {
        CapitalAbsorptionPressureDriverStrength::Low
    }
}

fn pressure_driver_strength_rank(strength: CapitalAbsorptionPressureDriverStrength) -> u8 {
    match strength {
        CapitalAbsorptionPressureDriverStrength::High => 3,
        CapitalAbsorptionPressureDriverStrength::Medium => 2,
        CapitalAbsorptionPressureDriverStrength::Low => 1,
    }
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
    AI_IPO_CANDIDATES
        .iter()
        .any(|issuer| same_issuer(subject, issuer))
}

fn queue_status_from_event(event: &CapitalAbsorptionAutoEvent) -> CapitalAbsorptionIpoQueueStatus {
    let lower = event.description.to_ascii_lowercase();
    let status = if lower.contains("listed")
        || lower.contains("debut")
        || lower.contains("begins trading")
        || lower.contains("starts trading")
        || lower.contains("completed ipo")
        || lower.contains("completes ipo")
        || lower.contains("priced ipo")
        || lower.contains("prices ipo")
        || lower.contains("ipo priced")
        || lower.contains("expected to price")
    {
        CapitalAbsorptionIpoQueueStatus::Ipo
    } else if lower.contains("filed")
        || lower.contains("files to go public")
        || lower.contains("files for ipo")
        || lower.contains("filed for ipo")
        || lower.contains("s-1")
    {
        CapitalAbsorptionIpoQueueStatus::Filed
    } else if lower.contains("pre-ipo")
        || lower.contains("pre ipo")
        || lower.contains("as soon as")
        || lower.contains("targeting")
        || lower.contains("target ipo")
        || lower.contains("expected in")
        || lower.contains("expected ipo")
        || contains_any(
            &lower,
            &[
                "ipo valuation",
                "listing valuation",
                "ipo terms",
                "listing terms",
            ],
        )
        || lower.contains("roadshow")
    {
        CapitalAbsorptionIpoQueueStatus::PreIpo
    } else if lower.contains("prepares")
        || lower.contains("preparing")
        || lower.contains("preparation")
        || lower.contains("readiness")
        || lower.contains("hire")
        || lower.contains("hiring")
        || lower.contains("banker")
        || lower.contains("adviser")
        || lower.contains("advisor")
    {
        CapitalAbsorptionIpoQueueStatus::Preparation
    } else if lower.contains("reported")
        || lower.contains("report says")
        || lower.contains("according to")
        || lower.contains("plans")
        || lower.contains("expected")
        || lower.contains("candidate")
        || lower.contains("discussion")
        || lower.contains("considering")
    {
        CapitalAbsorptionIpoQueueStatus::Reported
    } else {
        CapitalAbsorptionIpoQueueStatus::Rumor
    };
    if event.event_type == CapitalAbsorptionObservationEventType::Confirmed
        && status < CapitalAbsorptionIpoQueueStatus::NearTerm
    {
        CapitalAbsorptionIpoQueueStatus::NearTerm
    } else {
        status
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
    } else if contains_any(
        &lower,
        &[
            "announces",
            "announced",
            "confirmed",
            "confirms",
            "priced",
            "prices ipo",
            "priced ipo",
            "begins trading",
            "listed",
            "completed",
            " filed ",
            "filed for",
            "files to raise",
            "filed to raise",
            "s-1",
            "launches",
            "issued",
            "issues",
            "issuance",
            "establishes",
            "established",
        ],
    ) {
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
            "public listing",
            "direct listing",
            "stock listing",
            "ipo listing",
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
            "follow-on offering",
            "at-the-market",
            "atm program",
            "atm offering",
            "equity raise",
            "convertible debt",
            "convertible notes",
            "convertible senior notes",
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
            "confirmed",
            "confirms",
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
            "issued",
            "issues",
            "issuance",
            "establishes",
            "established",
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
            "convertible debt issuance",
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
    ) && extract_confirmed_financing_amount(CapitalAbsorptionAutoEventCategory::IpoSupply, text)
        .is_some()
}

fn extract_confirmed_financing_amount(
    category: CapitalAbsorptionAutoEventCategory,
    text: &str,
) -> Option<f64> {
    match category {
        CapitalAbsorptionAutoEventCategory::IpoSupply => {
            extract_usd_billions_near_confirmed_ipo_amount_context(text)
        }
        CapitalAbsorptionAutoEventCategory::MegaCapFinancing
        | CapitalAbsorptionAutoEventCategory::SecondaryLiquidity => extract_usd_billions(text),
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

fn extract_usd_billions_near_confirmed_ipo_amount_context(text: &str) -> Option<f64> {
    let tokens = text
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '$'))
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();
    for index in 0..tokens.len().saturating_sub(1) {
        let number = tokens[index].trim_start_matches('$').parse::<f64>().ok();
        let unit = tokens[index + 1].as_str();
        let Some(number) = number else {
            continue;
        };
        let amount = if unit.starts_with("billion") || unit == "bn" {
            Some(number)
        } else if unit.starts_with("million") || unit == "mn" {
            Some(number / 1000.0)
        } else {
            None
        };
        let Some(amount) = amount else {
            continue;
        };
        let start = index.saturating_sub(5);
        let end = (index + 7).min(tokens.len());
        let context = tokens[start..end].join(" ");
        if contains_any(
            &context,
            &[
                "valuation",
                "valued",
                "worth",
                "market cap",
                "expected value",
                "projected valuation",
                "target valuation",
            ],
        ) {
            continue;
        }
        if contains_any(
            &context,
            &[
                "raise",
                "raises",
                "proceeds",
                "gross proceeds",
                "offering size",
                "priced",
                "prices",
            ],
        ) {
            return Some(amount);
        }
    }
    None
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
        assert_eq!(snapshot.ipo_queue_history.len(), 30);
        assert_eq!(
            snapshot
                .ipo_queue_history
                .last()
                .map(|point| point.queue_size),
            Some(1)
        );
        assert_eq!(
            snapshot
                .ai_ipo_queue
                .iter()
                .find(|item| item.issuer == "SpaceX")
                .map(|item| item.status),
            Some(CapitalAbsorptionIpoQueueStatus::Reported)
        );
        assert_eq!(
            snapshot.potential_supply_pressure.level,
            CapitalAbsorptionPotentialSupplyPressureLevel::Normal
        );
        assert_eq!(snapshot.potential_supply_pressure.queue_count, 1);
        assert_eq!(snapshot.potential_supply_pressure.reported_count, 1);
        assert_eq!(snapshot.potential_supply_pressure.confirmed_count, 0);
    }

    #[test]
    fn confirmed_spacex_moves_to_near_term_supply_with_driver_and_timeline() {
        let mut spacex = event("SpaceX", 0.0);
        spacex.category = CapitalAbsorptionAutoEventCategory::IpoSupply;
        spacex.supply_kind = CapitalAbsorptionSupplyKind::Potential;
        spacex.event_type = CapitalAbsorptionObservationEventType::Confirmed;
        spacex.description = "SpaceX IPO confirmed for near-term supply window".to_string();
        spacex.amount_usd_b = None;

        let mut openai = event("OpenAI", 0.0);
        openai.category = CapitalAbsorptionAutoEventCategory::IpoSupply;
        openai.supply_kind = CapitalAbsorptionSupplyKind::Potential;
        openai.event_type = CapitalAbsorptionObservationEventType::Reported;
        openai.description = "OpenAI IPO discussion continues".to_string();
        openai.amount_usd_b = None;

        let snapshot = build_capital_absorption_snapshot_from_events(
            vec![spacex, openai],
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "fixture".to_string(),
            },
        );

        assert_eq!(
            snapshot
                .near_term_supply
                .iter()
                .map(|item| item.issuer.as_str())
                .collect::<Vec<_>>(),
            vec!["SpaceX"]
        );
        assert_eq!(
            snapshot.near_term_supply.first().map(|item| item.status),
            Some(CapitalAbsorptionIpoQueueStatus::NearTerm)
        );
        assert_eq!(
            snapshot
                .near_term_supply
                .first()
                .map(|item| item.event_type),
            Some(CapitalAbsorptionObservationEventType::Confirmed)
        );
        assert!(snapshot
            .ai_ipo_queue
            .iter()
            .any(|item| item.issuer == "OpenAI"));
        assert!(snapshot.observation_watchlist.iter().any(|item| {
            item.issuer == "OpenAI"
                && item.lifecycle_status == CapitalAbsorptionIpoLifecycleStatus::Reported
                && item.observation_day.is_none()
                && item.review_window_days.is_none()
        }));
        assert!(!snapshot
            .ai_ipo_queue
            .iter()
            .any(|item| item.issuer == "SpaceX"));
        assert_eq!(snapshot.potential_supply_pressure.near_term_supply_count, 1);
        assert_eq!(snapshot.potential_supply_pressure.future_queue_count, 1);
        assert_eq!(
            snapshot
                .potential_supply_pressure
                .drivers
                .first()
                .map(|driver| { (driver.label.as_str(), driver.strength) }),
            Some(("SpaceX IPO", CapitalAbsorptionPressureDriverStrength::High))
        );
        assert_eq!(
            snapshot
                .upcoming_supply_timeline
                .iter()
                .find(|item| item.issuer == "SpaceX")
                .map(|item| item.bucket),
            Some(CapitalAbsorptionSupplyTimelineBucket::Next30Days)
        );
    }

    #[test]
    fn listed_spacex_moves_from_ipo_queue_to_observation_watchlist() {
        let mut spacex = event("SpaceX", 0.0);
        spacex.category = CapitalAbsorptionAutoEventCategory::IpoSupply;
        spacex.supply_kind = CapitalAbsorptionSupplyKind::Potential;
        spacex.event_type = CapitalAbsorptionObservationEventType::Confirmed;
        spacex.description = "SpaceX completed IPO and begins trading".to_string();
        spacex.amount_usd_b = None;
        spacex.observed_at = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();

        let snapshot = build_capital_absorption_snapshot_from_events(
            vec![spacex],
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "fixture".to_string(),
            },
        );

        assert!(snapshot.near_term_supply.is_empty());
        assert!(!snapshot
            .ai_ipo_queue
            .iter()
            .any(|item| item.issuer == "SpaceX"));
        assert_eq!(
            snapshot
                .observation_watchlist
                .iter()
                .map(|item| {
                    (
                        item.issuer.as_str(),
                        item.lifecycle_status,
                        item.observation_day,
                        item.review_window_days,
                        item.review_candidate,
                    )
                })
                .collect::<Vec<_>>(),
            vec![(
                "SpaceX",
                CapitalAbsorptionIpoLifecycleStatus::Listed,
                Some(1),
                Some(90),
                false,
            )]
        );
        assert_eq!(snapshot.potential_supply_pressure.near_term_supply_count, 0);
        assert!(snapshot.potential_supply_pressure.drivers.is_empty());
    }

    #[test]
    fn post_ipo_observation_day_marks_review_candidate_after_ninety_days() {
        let mut spacex = event("SpaceX", 0.0);
        spacex.category = CapitalAbsorptionAutoEventCategory::IpoSupply;
        spacex.supply_kind = CapitalAbsorptionSupplyKind::Potential;
        spacex.event_type = CapitalAbsorptionObservationEventType::Confirmed;
        spacex.description = "SpaceX completed IPO and begins trading".to_string();
        spacex.amount_usd_b = None;
        spacex.observed_at = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();

        let mut openai = event("OpenAI", 0.0);
        openai.category = CapitalAbsorptionAutoEventCategory::IpoSupply;
        openai.supply_kind = CapitalAbsorptionSupplyKind::Potential;
        openai.event_type = CapitalAbsorptionObservationEventType::Reported;
        openai.description = "OpenAI IPO reported by multiple outlets".to_string();
        openai.amount_usd_b = None;
        openai.observed_at = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();

        let snapshot = build_capital_absorption_snapshot_from_events(
            vec![spacex, openai],
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "fixture".to_string(),
            },
        );

        let spacex = snapshot
            .observation_watchlist
            .iter()
            .find(|item| item.issuer == "SpaceX")
            .expect("SpaceX should remain in observation watchlist");
        assert_eq!(
            spacex.lifecycle_status,
            CapitalAbsorptionIpoLifecycleStatus::Observed
        );
        assert_eq!(spacex.observation_day, Some(90));
        assert!(spacex.review_candidate);
    }

    #[test]
    fn ipo_queue_history_keeps_recent_thirty_day_window() {
        let mut first = event("SpaceX", 0.0);
        first.category = CapitalAbsorptionAutoEventCategory::IpoSupply;
        first.supply_kind = CapitalAbsorptionSupplyKind::Potential;
        first.event_type = CapitalAbsorptionObservationEventType::Reported;
        first.description = "SpaceX IPO reported as preparation continues".to_string();
        first.amount_usd_b = None;
        first.observed_at = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let mut second = first.clone();
        second.subject = "Anthropic".to_string();
        second.description = "Anthropic IPO discussion grows".to_string();
        second.observed_at = NaiveDate::from_ymd_opt(2026, 6, 6).unwrap();

        let snapshot = build_capital_absorption_snapshot_from_events(
            vec![first, second],
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "fixture".to_string(),
            },
        );

        assert_eq!(snapshot.ipo_queue_history.len(), 30);
        assert_eq!(
            snapshot
                .ipo_queue_history
                .first()
                .map(|point| point.queue_size),
            Some(0)
        );
        assert_eq!(
            snapshot
                .ipo_queue_history
                .last()
                .map(|point| point.queue_size),
            Some(2)
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

    #[test]
    fn classifies_confirmed_non_ipo_actual_supply_allowlist() {
        for (headline, expected_amount) in [
            (
                "Alphabet confirmed $2 billion equity raise for AI infrastructure",
                2.0,
            ),
            ("Nvidia announced $3 billion secondary offering", 3.0),
            ("Microsoft priced $4 billion follow-on offering", 4.0),
            ("Amazon issued $5 billion convertible debt issuance", 5.0),
            ("Meta establishes $6 billion ATM program", 6.0),
        ] {
            let event = classify_capital_absorption_news_observation(
                "Market",
                headline,
                "",
                NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
                None,
            )
            .expect("confirmed supply event should be observed");

            assert_eq!(event.supply_kind, CapitalAbsorptionSupplyKind::Actual);
            assert_eq!(
                event.event_type,
                CapitalAbsorptionObservationEventType::Confirmed
            );
            assert_eq!(event.amount_usd_b, Some(expected_amount));
        }
    }

    #[test]
    fn classifies_filed_ipo_with_confirmed_amount_as_actual_supply() {
        let event = classify_capital_absorption_news_observation(
            "Market",
            "Figure files to raise $750 million in IPO",
            "The S-1 confirms gross proceeds from the offering.",
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
            None,
        )
        .expect("filed IPO with confirmed amount should be observed");

        assert_eq!(event.subject, "Figure");
        assert_eq!(event.supply_kind, CapitalAbsorptionSupplyKind::Actual);
        assert_eq!(
            event.event_type,
            CapitalAbsorptionObservationEventType::Confirmed
        );
        assert_eq!(event.amount_usd_b, Some(0.75));
    }

    #[test]
    fn keeps_private_ipo_expectation_and_valuation_out_of_actual_supply() {
        let event = classify_capital_absorption_news_observation(
            "Market",
            "Stripe IPO expected at $90 billion valuation",
            "The company remains an IPO candidate without confirmed proceeds.",
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
            None,
        )
        .expect("private IPO expectation should remain potential");

        assert_eq!(event.subject, "Stripe");
        assert_eq!(event.supply_kind, CapitalAbsorptionSupplyKind::Potential);
        assert_eq!(event.amount_usd_b, None);
    }

    #[test]
    fn filed_ipo_with_proceeds_and_valuation_uses_proceeds_only() {
        let event = classify_capital_absorption_news_observation(
            "Market",
            "Figure files to raise $750 million in IPO at $10 billion valuation",
            "The S-1 confirms gross proceeds from the offering.",
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
            None,
        )
        .expect("filed IPO proceeds should be actual supply even when valuation is mentioned");

        assert_eq!(event.subject, "Figure");
        assert_eq!(event.supply_kind, CapitalAbsorptionSupplyKind::Actual);
        assert_eq!(event.amount_usd_b, Some(0.75));
    }

    #[test]
    fn generic_job_listing_for_ai_issuer_is_not_potential_supply() {
        let event = classify_capital_absorption_news_observation(
            "Market",
            "OpenAI posts new job listing for infrastructure team",
            "The role description mentions AI compute and data center operations.",
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
            None,
        );

        assert!(event.is_none());
    }

    #[test]
    fn filters_weak_ipo_related_recommendation_articles() {
        let event = classify_capital_absorption_news_observation(
            "Market",
            "3 stocks to buy before the Anthropic IPO",
            "A Wall Street analyst research call mentions related tickers.",
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
            None,
        );

        assert!(event.is_none());
    }

    #[test]
    fn classifies_ipo_stage_independently_from_event_type() {
        let cases = [
            (
                "SpaceX IPO rumor circulates after private investor comments",
                1,
                CapitalAbsorptionObservationEventType::Rumor,
                CapitalAbsorptionIpoQueueStatus::Rumor,
            ),
            (
                "OpenAI IPO reported by multiple outlets",
                2,
                CapitalAbsorptionObservationEventType::Reported,
                CapitalAbsorptionIpoQueueStatus::Reported,
            ),
            (
                "Anthropic hires bankers for IPO preparation",
                1,
                CapitalAbsorptionObservationEventType::Reported,
                CapitalAbsorptionIpoQueueStatus::Preparation,
            ),
            (
                "SpaceX targets IPO as soon as 2027 at updated valuation",
                1,
                CapitalAbsorptionObservationEventType::Reported,
                CapitalAbsorptionIpoQueueStatus::PreIpo,
            ),
            (
                "Anthropic IPO discussion grows after private valuation reaches $60 billion",
                1,
                CapitalAbsorptionObservationEventType::Reported,
                CapitalAbsorptionIpoQueueStatus::Reported,
            ),
            (
                "OpenAI IPO valuation terms become central to listing discussions",
                1,
                CapitalAbsorptionObservationEventType::Reported,
                CapitalAbsorptionIpoQueueStatus::PreIpo,
            ),
            (
                "Figure files for IPO with S-1 registration statement",
                1,
                CapitalAbsorptionObservationEventType::Confirmed,
                CapitalAbsorptionIpoQueueStatus::Filed,
            ),
            (
                "Stripe completed IPO and begins trading",
                1,
                CapitalAbsorptionObservationEventType::Confirmed,
                CapitalAbsorptionIpoQueueStatus::Ipo,
            ),
        ];

        for (description, source_count, event_type, expected_status) in cases {
            let mut event = event("SpaceX", 0.0);
            event.category = CapitalAbsorptionAutoEventCategory::IpoSupply;
            event.supply_kind = CapitalAbsorptionSupplyKind::Potential;
            event.event_type = event_type;
            event.description = description.to_string();
            event.amount_usd_b = None;
            event.source_count = source_count;

            assert_eq!(queue_status_from_event(&event), expected_status);
        }
    }

    #[test]
    fn private_valuation_discussion_does_not_become_pre_ipo_stage() {
        let mut event = event("Anthropic", 0.0);
        event.category = CapitalAbsorptionAutoEventCategory::IpoSupply;
        event.supply_kind = CapitalAbsorptionSupplyKind::Potential;
        event.event_type = CapitalAbsorptionObservationEventType::Reported;
        event.description =
            "Anthropic IPO discussion grows after private valuation reaches $60 billion"
                .to_string();
        event.amount_usd_b = None;

        assert_eq!(
            queue_status_from_event(&event),
            CapitalAbsorptionIpoQueueStatus::Reported
        );
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

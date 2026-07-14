use crate::config;
use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoEvent, CapitalAbsorptionAutoSnapshot, CapitalAbsorptionIpoLifecycleStatus,
    CapitalAbsorptionIpoQueueHistoryPoint, CapitalAbsorptionIpoQueueItem,
    CapitalAbsorptionIpoQueueStatus, CapitalAbsorptionObservationEventType,
    CapitalAbsorptionObservationWatchlistItem, CapitalAbsorptionPotentialSupplyPressure,
    CapitalAbsorptionPotentialSupplyPressureLevel, CapitalAbsorptionPotentialSupplyTrend,
    CapitalAbsorptionSourceHealth, CapitalAbsorptionSupplyEventCounts, CapitalAbsorptionSupplyKind,
    CapitalAbsorptionSupplyTimelineBucket, CapitalAbsorptionSupplyTimelineItem,
};
use crate::features::shared::interface::i18n::Language;
use std::collections::BTreeMap;

use super::capital_absorption_i18n::*;
use super::capital_absorption_supply_phase_read_model::build_supply_phase_view_model;

pub(crate) fn build_capital_absorption_report_from_config(
    manual: Option<&config::CapitalAbsorptionConfig>,
    auto_snapshot: Option<&CapitalAbsorptionAutoSnapshot>,
    language: Language,
) -> String {
    let manual = manual.filter(|capital_absorption| capital_absorption.enable.unwrap_or(true));
    let snapshot = if let Some(auto_snapshot) = auto_snapshot.filter(|snapshot| {
        snapshot.source_status.status != CapitalAbsorptionSourceHealth::Unavailable
    }) {
        CapitalAbsorptionRenderSnapshot::from_auto(auto_snapshot, language)
    } else if let Some(manual) = manual {
        CapitalAbsorptionRenderSnapshot::from_config(manual, language)
    } else if let Some(auto_snapshot) = auto_snapshot {
        CapitalAbsorptionRenderSnapshot::from_auto(auto_snapshot, language)
    } else {
        return capital_absorption_empty(language).to_string();
    };

    let mut out = String::new();
    out.push_str(capital_absorption_title(language));
    out.push_str("\n\n");
    if let Some(source_status) = &snapshot.source_status {
        out.push_str(&format!(
            "{} {} · {}\n\n",
            capital_absorption_source_label(language),
            source_status.provider,
            source_status.message
        ));
    }
    out.push_str(&format!(
        "{} {}\n\n",
        capital_absorption_status_label(language),
        snapshot.status
    ));
    push_supply_event_counts(&mut out, &snapshot.supply_event_counts, language);
    push_actual_capital_supply(
        &mut out,
        &snapshot.capital_demand,
        &snapshot.observed_events,
        language,
    );
    push_potential_supply_trend(&mut out, snapshot.potential_supply_trend, language);
    push_potential_supply_pressure(
        &mut out,
        &snapshot.potential_supply_pressure,
        &snapshot.near_term_supply,
        &snapshot.ai_ipo_queue,
        &snapshot.capital_demand,
        language,
    );
    let supply_phase = build_supply_phase_view_model(
        snapshot.potential_supply_pressure.level,
        snapshot.potential_supply_trend,
        language,
    );
    out.push_str(&format!("{}\n\n", supply_phase.title));
    out.push_str(&format!(
        "{} {}\n\n",
        supply_phase.phase_label, supply_phase.phase_value
    ));
    out.push_str(&format!(
        "{} {}\n\n",
        supply_phase.summary_label, supply_phase.summary_value
    ));
    push_supply_queue(
        &mut out,
        capital_absorption_near_term_supply_label(language),
        &snapshot.near_term_supply,
        language,
    );
    push_ai_ipo_queue(&mut out, &snapshot.ai_ipo_queue, language);
    push_upcoming_supply_timeline(&mut out, &snapshot.upcoming_supply_timeline, language);
    push_observation_watchlist(&mut out, &snapshot.observation_watchlist, language);
    push_ipo_queue_history(&mut out, &snapshot.ipo_queue_history, language);
    push_capital_absorption_events(&mut out, &snapshot.observed_events, language);
    push_capital_supply(&mut out, &snapshot.capital_supply, language);
    push_capital_absorption_ratio(&mut out, &snapshot.absorption_ratio, language);
    out.push_str(&format!(
        "{} {}\n\n",
        capital_absorption_structural_impact_label(language),
        snapshot.structural_impact
    ));
    out.push_str(&supply_phase.boundary);
    out
}

struct CapitalAbsorptionRenderSnapshot {
    source_status: Option<CapitalAbsorptionRenderSourceStatus>,
    status: String,
    observed_events: Vec<CapitalAbsorptionRenderEvent>,
    supply_event_counts: CapitalAbsorptionSupplyEventCounts,
    near_term_supply: Vec<CapitalAbsorptionIpoQueueItem>,
    ai_ipo_queue: Vec<CapitalAbsorptionIpoQueueItem>,
    upcoming_supply_timeline: Vec<CapitalAbsorptionSupplyTimelineItem>,
    observation_watchlist: Vec<CapitalAbsorptionObservationWatchlistItem>,
    ipo_queue_history: Vec<CapitalAbsorptionIpoQueueHistoryPoint>,
    potential_supply_trend: CapitalAbsorptionPotentialSupplyTrend,
    potential_supply_pressure: CapitalAbsorptionPotentialSupplyPressure,
    capital_demand: CapitalDemandRenderSnapshot,
    capital_supply: CapitalSupplyRenderSnapshot,
    absorption_ratio: CapitalAbsorptionRenderRatio,
    structural_impact: String,
}

struct CapitalAbsorptionRenderSourceStatus {
    provider: String,
    message: String,
}

struct CapitalAbsorptionRenderEvent {
    category: String,
    subject: String,
    description: String,
    source_count: usize,
    supply_kind: CapitalAbsorptionSupplyKind,
    amount_usd_b: Option<f64>,
}

struct CapitalDemandRenderSnapshot {
    rolling_12m_usd_b: Option<f64>,
    ipo_financing_usd_b: Option<f64>,
    secondary_offering_usd_b: Option<f64>,
    convertible_debt_usd_b: Option<f64>,
    ai_related_financing_usd_b: Option<f64>,
}

struct CapitalSupplyRenderSnapshot {
    rolling_12m_usd_b: Option<f64>,
    score: Option<f64>,
    trend: String,
    etf_net_inflow_usd_b: Option<f64>,
    mutual_fund_net_inflow_usd_b: Option<f64>,
    pension_allocation_flow_usd_b: Option<f64>,
    foreign_capital_inflow_usd_b: Option<f64>,
    corporate_buyback_usd_b: Option<f64>,
}

struct CapitalAbsorptionRenderRatio {
    value: Option<f64>,
    state: String,
}

impl CapitalAbsorptionRenderSnapshot {
    fn from_config(value: &config::CapitalAbsorptionConfig, language: Language) -> Self {
        let observed_events = value
            .observed_events
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|event| CapitalAbsorptionRenderEvent::from_config(event, language))
            .collect::<Vec<_>>();
        Self {
            source_status: None,
            status: capital_absorption_status_value(capped_config_status(value.status), language),
            supply_event_counts: supply_event_counts_from_render_events(&observed_events),
            near_term_supply: Vec::new(),
            ai_ipo_queue: default_capital_absorption_ipo_queue(),
            upcoming_supply_timeline: Vec::new(),
            observation_watchlist: Vec::new(),
            ipo_queue_history: Vec::new(),
            potential_supply_trend: CapitalAbsorptionPotentialSupplyTrend::Stable,
            potential_supply_pressure: default_capital_absorption_potential_supply_pressure(),
            observed_events,
            capital_demand: CapitalDemandRenderSnapshot::from_config(
                &value.capital_demand,
                language,
            ),
            capital_supply: CapitalSupplyRenderSnapshot::from_config(
                &value.capital_supply,
                language,
            ),
            absorption_ratio: CapitalAbsorptionRenderRatio {
                value: value.absorption_ratio.value,
                state: capital_absorption_ratio_state_value(value.absorption_ratio.state, language),
            },
            structural_impact: capital_absorption_structural_impact_value(
                value.structural_impact.as_deref(),
                language,
            ),
        }
    }

    fn from_auto(value: &CapitalAbsorptionAutoSnapshot, language: Language) -> Self {
        Self {
            source_status: Some(CapitalAbsorptionRenderSourceStatus {
                provider: value.source_status.provider.clone(),
                message: value.source_status.message.clone(),
            }),
            status: capital_absorption_auto_status_value(value.status, language),
            supply_event_counts: value.supply_event_counts.clone(),
            near_term_supply: value.near_term_supply.clone(),
            ai_ipo_queue: value.ai_ipo_queue.clone(),
            upcoming_supply_timeline: value.upcoming_supply_timeline.clone(),
            observation_watchlist: value.observation_watchlist.clone(),
            ipo_queue_history: value.ipo_queue_history.clone(),
            potential_supply_trend: value.potential_supply_trend,
            potential_supply_pressure: value.potential_supply_pressure.clone(),
            observed_events: value
                .observed_events
                .iter()
                .map(|event| CapitalAbsorptionRenderEvent::from_auto(event, language))
                .collect(),
            capital_demand: CapitalDemandRenderSnapshot::from_auto(&value.capital_demand, language),
            capital_supply: CapitalSupplyRenderSnapshot::from_auto(&value.capital_supply, language),
            absorption_ratio: CapitalAbsorptionRenderRatio {
                value: value.absorption_ratio.value,
                state: capital_absorption_auto_ratio_state_value(
                    value.absorption_ratio.state,
                    language,
                ),
            },
            structural_impact: capital_absorption_observation_only_value(language).to_string(),
        }
    }
}

impl CapitalAbsorptionRenderEvent {
    fn from_config(value: &config::CapitalAbsorptionEventConfig, language: Language) -> Self {
        let supply_kind = match value.category {
            config::CapitalAbsorptionEventCategory::IpoSupply => {
                CapitalAbsorptionSupplyKind::Potential
            }
            config::CapitalAbsorptionEventCategory::MegaCapFinancing
            | config::CapitalAbsorptionEventCategory::SecondaryLiquidity => {
                CapitalAbsorptionSupplyKind::Actual
            }
        };
        Self {
            category: capital_absorption_event_category_value(value.category, language),
            subject: value.subject.clone(),
            description: value.description.clone(),
            source_count: 1,
            supply_kind,
            amount_usd_b: value.amount_usd_b,
        }
    }

    fn from_auto(value: &CapitalAbsorptionAutoEvent, language: Language) -> Self {
        Self {
            category: capital_absorption_auto_event_category_value(value.category, language),
            subject: value.subject.clone(),
            description: value.description.clone(),
            source_count: value.source_count,
            supply_kind: value.supply_kind,
            amount_usd_b: value.amount_usd_b,
        }
    }
}

impl CapitalDemandRenderSnapshot {
    fn from_config(value: &config::CapitalDemandConfig, language: Language) -> Self {
        let _current_phase_hides_demand_trend_and_score = (
            capital_absorption_trend_value(value.trend, language),
            value.score,
        );
        Self {
            rolling_12m_usd_b: value.rolling_12m_usd_b,
            ipo_financing_usd_b: value.ipo_financing_usd_b,
            secondary_offering_usd_b: value.secondary_offering_usd_b,
            convertible_debt_usd_b: value.convertible_debt_usd_b,
            ai_related_financing_usd_b: value.ai_related_financing_usd_b,
        }
    }

    fn from_auto(
        value: &crate::features::research::application::capital_absorption::CapitalDemandAutoSnapshot,
        language: Language,
    ) -> Self {
        let _current_phase_hides_demand_trend_and_score = (
            capital_absorption_auto_trend_value(value.trend, language),
            value.score,
        );
        Self {
            rolling_12m_usd_b: value.rolling_12m_usd_b,
            ipo_financing_usd_b: value.ipo_financing_usd_b,
            secondary_offering_usd_b: value.secondary_offering_usd_b,
            convertible_debt_usd_b: value.convertible_debt_usd_b,
            ai_related_financing_usd_b: value.ai_related_financing_usd_b,
        }
    }
}

impl CapitalSupplyRenderSnapshot {
    fn from_config(value: &config::CapitalSupplyConfig, language: Language) -> Self {
        Self {
            rolling_12m_usd_b: value.rolling_12m_usd_b,
            score: value.score,
            trend: capital_absorption_trend_value(value.trend, language),
            etf_net_inflow_usd_b: value.etf_net_inflow_usd_b,
            mutual_fund_net_inflow_usd_b: value.mutual_fund_net_inflow_usd_b,
            pension_allocation_flow_usd_b: value.pension_allocation_flow_usd_b,
            foreign_capital_inflow_usd_b: value.foreign_capital_inflow_usd_b,
            corporate_buyback_usd_b: value.corporate_buyback_usd_b,
        }
    }

    fn from_auto(
        value: &crate::features::research::application::capital_absorption::CapitalSupplyAutoSnapshot,
        language: Language,
    ) -> Self {
        Self {
            rolling_12m_usd_b: value.rolling_12m_usd_b,
            score: value.score,
            trend: capital_absorption_auto_trend_value(value.trend, language),
            etf_net_inflow_usd_b: value.etf_net_inflow_usd_b,
            mutual_fund_net_inflow_usd_b: value.mutual_fund_net_inflow_usd_b,
            pension_allocation_flow_usd_b: value.pension_allocation_flow_usd_b,
            foreign_capital_inflow_usd_b: value.foreign_capital_inflow_usd_b,
            corporate_buyback_usd_b: value.corporate_buyback_usd_b,
        }
    }
}

fn push_capital_absorption_events(
    out: &mut String,
    events: &[CapitalAbsorptionRenderEvent],
    language: Language,
) {
    out.push_str(capital_absorption_events_label(language));
    out.push_str(":\n");
    if events.is_empty() {
        out.push_str(capital_absorption_no_events(language));
        out.push('\n');
        return;
    }
    out.push_str(&format!(
        "{}:\n",
        capital_absorption_discovery_new_label(language)
    ));
    for (subject, source_count) in discovery_summary_counts(events) {
        out.push_str(&format!("- {subject} x{source_count}\n"));
    }
    out.push_str(&format!(
        "{}:\n- {}\n",
        capital_absorption_discovery_upgraded_label(language),
        capital_absorption_none_label(language)
    ));
    out.push_str(&format!(
        "{}:\n- {}\n",
        capital_absorption_discovery_downgraded_label(language),
        capital_absorption_none_label(language)
    ));
    out.push_str(&format!(
        "{}:\n- {}\n",
        capital_absorption_discovery_disappeared_label(language),
        capital_absorption_none_label(language)
    ));
    out.push('\n');
}

fn discovery_summary_counts(events: &[CapitalAbsorptionRenderEvent]) -> Vec<(String, usize)> {
    let mut counts = BTreeMap::new();
    for event in events {
        *counts.entry(event.subject.clone()).or_insert(0) += event.source_count.max(1);
    }
    counts.into_iter().collect()
}

fn push_actual_capital_supply(
    out: &mut String,
    demand: &CapitalDemandRenderSnapshot,
    events: &[CapitalAbsorptionRenderEvent],
    language: Language,
) {
    out.push_str(capital_absorption_actual_supply_label(language));
    out.push_str(":\n");
    push_optional_usd(
        out,
        capital_absorption_observed_actual_amount_label(language),
        demand.rolling_12m_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_ipo_label(language),
        demand.ipo_financing_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_secondary_label(language),
        demand.secondary_offering_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_convertible_label(language),
        demand.convertible_debt_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_ai_related_label(language),
        demand.ai_related_financing_usd_b,
    );
    if demand.rolling_12m_usd_b.is_none()
        && demand.ipo_financing_usd_b.is_none()
        && demand.secondary_offering_usd_b.is_none()
        && demand.convertible_debt_usd_b.is_none()
        && demand.ai_related_financing_usd_b.is_none()
    {
        out.push_str(capital_absorption_no_actual_supply(language));
        out.push('\n');
    }

    let mut contributors = Vec::new();
    for event in events {
        if event.supply_kind == CapitalAbsorptionSupplyKind::Actual {
            if let Some(amount) = event.amount_usd_b {
                if amount > 0.0 {
                    let family =
                        match crate::features::research::domain::capital_absorption::event_family(
                            &event.description,
                        ) {
                            "convertible_debt" => "Convertible Debt",
                            "secondary_offering" => "Secondary Offering",
                            "ipo" => "IPO",
                            "secondary_liquidity" => "Secondary Liquidity",
                            _ => "Financing",
                        };
                    contributors.push(format!(
                        "{} {}: {}",
                        event.subject,
                        family,
                        format_usd(amount)
                    ));
                }
            }
        }
    }

    if !contributors.is_empty() {
        out.push('\n');
        out.push_str(capital_absorption_actual_supply_contributors_label(
            language,
        ));
        out.push_str("\n\n");
        for c in contributors {
            out.push_str(&format!("* {}\n", c));
        }
    }
    out.push('\n');
}

fn push_potential_supply_trend(
    out: &mut String,
    trend: CapitalAbsorptionPotentialSupplyTrend,
    language: Language,
) {
    out.push_str(capital_absorption_potential_supply_trend_label(language));
    out.push_str(":\n");
    out.push_str(&format!(
        "- {} {}\n\n",
        capital_absorption_trend_label(language),
        capital_absorption_potential_supply_trend_value(trend, language)
    ));
}

fn push_potential_supply_pressure(
    out: &mut String,
    pressure: &CapitalAbsorptionPotentialSupplyPressure,
    near_term_supply: &[CapitalAbsorptionIpoQueueItem],
    future_queue: &[CapitalAbsorptionIpoQueueItem],
    demand: &CapitalDemandRenderSnapshot,
    language: Language,
) {
    out.push_str(capital_absorption_potential_supply_pressure_label(language));
    out.push_str(":\n");
    out.push_str(&format!(
        "- {} {}\n",
        capital_absorption_pressure_level_label(language),
        capital_absorption_potential_supply_pressure_level_value(pressure.level, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_near_term_supply_count_label(language),
        pressure.near_term_supply_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_queue_count_label(language),
        pressure.future_queue_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_reported_count_label(language),
        pressure.reported_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_confirmed_count_label(language),
        pressure.confirmed_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_supply_interpretation_label(language),
        capital_absorption_supply_interpretation_value(pressure.level, language)
    ));

    let mut reasons = Vec::new();
    if let Some(actual_usd) = demand.rolling_12m_usd_b {
        if actual_usd > 0.0 {
            let actual_supply_label = match language {
                Language::ZhCn => "Actual Supply",
                Language::EnUs => "Actual Supply",
                Language::JaJp => "Actual Supply",
            };
            reasons.push(format!(
                "{}: {}",
                actual_supply_label,
                format_usd(actual_usd)
            ));
        }
    }

    let mut queue_items = near_term_supply
        .iter()
        .chain(future_queue.iter())
        .filter(|item| item.source_count > 0)
        .cloned()
        .collect::<Vec<_>>();

    queue_items.sort_by(|a, b| {
        b.event_type
            .cmp(&a.event_type)
            .then_with(|| b.source_count.cmp(&a.source_count))
            .then_with(|| a.issuer.cmp(&b.issuer))
    });

    for driver in &pressure.drivers {
        reasons.push(format!(
            "{} ({})",
            driver.label,
            capital_absorption_pressure_driver_strength_value(driver.strength, language)
        ));
    }
    for item in queue_items {
        if pressure
            .drivers
            .iter()
            .any(|driver| driver.label.starts_with(&item.issuer))
        {
            continue;
        }
        reasons.push(format!(
            "{} ({})",
            item.issuer,
            capital_absorption_event_type_value(item.event_type, language)
        ));
    }

    let reasons_to_show = reasons.into_iter().take(5).collect::<Vec<_>>();
    if !reasons_to_show.is_empty() {
        out.push('\n');
        out.push_str(capital_absorption_drivers_label(language));
        out.push_str(":\n");
        for r in reasons_to_show {
            out.push_str(&format!("* {}\n", r));
        }
    }
    out.push('\n');
}

fn capital_absorption_supply_interpretation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Supply Interpretation",
        Language::EnUs => "Supply Interpretation",
        Language::JaJp => "Supply Interpretation",
    }
}

fn capital_absorption_supply_interpretation_value(
    level: CapitalAbsorptionPotentialSupplyPressureLevel,
    language: Language,
) -> &'static str {
    match level {
        CapitalAbsorptionPotentialSupplyPressureLevel::Low => match language {
            Language::ZhCn => {
                "Current supply remains absorbable. No abnormal dilution pressure detected."
            }
            Language::EnUs => {
                "Current supply remains absorbable. No abnormal dilution pressure detected."
            }
            Language::JaJp => {
                "Current supply remains absorbable. No abnormal dilution pressure detected."
            }
        },
        CapitalAbsorptionPotentialSupplyPressureLevel::Normal => match language {
            Language::ZhCn => "Supply pressure is increasing. Absorption is still manageable.",
            Language::EnUs => "Supply pressure is increasing. Absorption is still manageable.",
            Language::JaJp => "Supply pressure is increasing. Absorption is still manageable.",
        },
        CapitalAbsorptionPotentialSupplyPressureLevel::Elevated => match language {
            Language::ZhCn => "Supply pressure is increasing. Watch market absorption capability.",
            Language::EnUs => "Supply pressure is increasing. Watch market absorption capability.",
            Language::JaJp => "Supply pressure is increasing. Watch market absorption capability.",
        },
    }
}

fn push_supply_event_counts(
    out: &mut String,
    counts: &CapitalAbsorptionSupplyEventCounts,
    language: Language,
) {
    out.push_str(capital_absorption_supply_event_count_label(language));
    out.push_str(":\n");
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_mega_cap_financing_count_label(language),
        counts.mega_cap_financing
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_secondary_offering_count_label(language),
        counts.secondary_offering
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        capital_absorption_convertible_debt_count_label(language),
        counts.convertible_debt
    ));
    out.push_str(&format!(
        "- {}: {}\n\n",
        capital_absorption_secondary_liquidity_count_label(language),
        counts.secondary_liquidity
    ));
}

fn push_ai_ipo_queue(
    out: &mut String,
    queue: &[CapitalAbsorptionIpoQueueItem],
    language: Language,
) {
    if queue.is_empty() {
        out.push_str("Future Queue details unavailable.\n\n");
        return;
    }
    push_supply_queue(
        out,
        capital_absorption_ai_ipo_queue_label(language),
        queue,
        language,
    );
}

fn push_supply_queue(
    out: &mut String,
    label: &str,
    queue: &[CapitalAbsorptionIpoQueueItem],
    language: Language,
) {
    if queue.is_empty() {
        out.push_str(label);
        out.push_str(": unavailable\n\n");
        return;
    }
    out.push_str(label);
    out.push_str(":\n");
    for item in queue.iter().take(3) {
        let source_quality = if item.source_count > 0 {
            format!(
                "{} {}",
                capital_absorption_sources_count_label(language),
                item.source_count
            )
        } else {
            "unavailable".to_string()
        };
        let expected_window = item
            .observation_day
            .map(|days| format!("within {days} days"))
            .unwrap_or_else(|| "future window".to_string());
        out.push_str(&format!(
            "- Subject: {} · Event Type: {} · Expected Window: {} · Status: {} · Source Quality: {} · Lifecycle: {}\n",
            item.issuer,
            capital_absorption_event_type_value(item.event_type, language),
            expected_window,
            capital_absorption_ipo_queue_status_value(item.status, language),
            source_quality,
            capital_absorption_lifecycle_status_value(item.lifecycle_status, language)
        ));
    }
    out.push('\n');
}

fn push_upcoming_supply_timeline(
    out: &mut String,
    timeline: &[CapitalAbsorptionSupplyTimelineItem],
    language: Language,
) {
    if timeline.is_empty() {
        return;
    }
    out.push_str(capital_absorption_upcoming_supply_timeline_label(language));
    out.push_str(":\n");
    for (bucket, label) in [
        (
            CapitalAbsorptionSupplyTimelineBucket::Next30Days,
            "0-30 Days",
        ),
        (
            CapitalAbsorptionSupplyTimelineBucket::Next12Months,
            "1-12 Months",
        ),
        (CapitalAbsorptionSupplyTimelineBucket::Unknown, "Unknown"),
    ] {
        out.push_str(label);
        out.push_str(":\n");
        let mut issuers = timeline
            .iter()
            .filter(|item| item.bucket == bucket)
            .map(|item| {
                (
                    item.issuer.clone(),
                    capital_absorption_lifecycle_status_value(item.lifecycle_status, language),
                )
            })
            .collect::<Vec<_>>();
        issuers.sort();
        issuers.dedup();
        if issuers.is_empty() {
            out.push_str(&format!("- {}\n", capital_absorption_none_label(language)));
        } else {
            for (issuer, status) in issuers {
                out.push_str(&format!("- {issuer} ({status})\n"));
            }
        }
    }
    out.push('\n');
}

fn push_observation_watchlist(
    out: &mut String,
    watchlist: &[CapitalAbsorptionObservationWatchlistItem],
    language: Language,
) {
    if watchlist.is_empty() {
        return;
    }
    out.push_str(capital_absorption_observation_watchlist_label(language));
    out.push_str(":\n");
    for item in watchlist {
        let status = capital_absorption_lifecycle_status_value(item.lifecycle_status, language);
        let observation_day = item
            .observation_day
            .map(|day| {
                format!(
                    " · {}: {}",
                    capital_absorption_observation_day_label(language),
                    day
                )
            })
            .unwrap_or_default();
        let review_window = item
            .review_window_days
            .map(|days| {
                format!(
                    " · {}: {} {}",
                    capital_absorption_review_window_label(language),
                    days,
                    capital_absorption_days_unit(language)
                )
            })
            .unwrap_or_default();
        let review_candidate = if item.review_candidate {
            format!(" · {}", capital_absorption_review_candidate_label(language))
        } else {
            String::new()
        };
        out.push_str(&format!(
            "- {}: Status {}{}{}{}\n",
            item.issuer, status, observation_day, review_window, review_candidate
        ));
    }
    out.push('\n');
}

fn push_ipo_queue_history(
    out: &mut String,
    history: &[CapitalAbsorptionIpoQueueHistoryPoint],
    language: Language,
) {
    if history.is_empty() {
        return;
    }
    out.push_str(capital_absorption_ipo_queue_history_label(language));
    out.push_str(":\n");
    for point in history {
        out.push_str(&format!(
            "- {} · {} = {}\n",
            point.observed_at,
            capital_absorption_queue_size_label(language),
            point.queue_size
        ));
    }
    out.push('\n');
}

fn push_capital_supply(out: &mut String, supply: &CapitalSupplyRenderSnapshot, language: Language) {
    out.push_str(capital_absorption_supply_label(language));
    out.push_str(":\n");
    out.push_str(&format!(
        "- {} {}\n",
        capital_absorption_trend_label(language),
        supply.trend
    ));
    push_optional_usd(
        out,
        capital_absorption_rolling_12m_label(language),
        supply.rolling_12m_usd_b,
    );
    push_optional_score(out, capital_absorption_score_label(language), supply.score);
    push_optional_usd(
        out,
        capital_absorption_etf_label(language),
        supply.etf_net_inflow_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_mutual_fund_label(language),
        supply.mutual_fund_net_inflow_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_pension_label(language),
        supply.pension_allocation_flow_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_foreign_capital_label(language),
        supply.foreign_capital_inflow_usd_b,
    );
    push_optional_usd(
        out,
        capital_absorption_buyback_label(language),
        supply.corporate_buyback_usd_b,
    );
    out.push('\n');
}

fn push_capital_absorption_ratio(
    out: &mut String,
    ratio: &CapitalAbsorptionRenderRatio,
    language: Language,
) {
    let _configured_ratio_is_intentionally_hidden = (&ratio.value, &ratio.state);
    out.push_str(&format!(
        "{} {}\n\n",
        capital_absorption_ratio_label(language),
        capital_absorption_ratio_disabled_label(language)
    ));
}

fn supply_event_counts_from_render_events(
    events: &[CapitalAbsorptionRenderEvent],
) -> CapitalAbsorptionSupplyEventCounts {
    let actual_events = events
        .iter()
        .filter(|event| event.supply_kind == CapitalAbsorptionSupplyKind::Actual)
        .collect::<Vec<_>>();
    CapitalAbsorptionSupplyEventCounts {
        mega_cap_financing: actual_events
            .iter()
            .filter(|event| event.category.contains("Mega Cap"))
            .count(),
        ai_ipo_candidate: actual_events
            .iter()
            .filter(|event| event.category.contains("IPO"))
            .count(),
        secondary_offering: actual_events
            .iter()
            .filter(|event| event.description.to_ascii_lowercase().contains("secondary"))
            .count(),
        convertible_debt: actual_events
            .iter()
            .filter(|event| {
                event
                    .description
                    .to_ascii_lowercase()
                    .contains("convertible")
            })
            .count(),
        secondary_liquidity: actual_events
            .iter()
            .filter(|event| {
                event.category.contains("Secondary Liquidity")
                    || event.category.contains("二级流动性")
                    || event.category.contains("セカンダリー流動性")
            })
            .count(),
    }
}

fn default_capital_absorption_ipo_queue() -> Vec<CapitalAbsorptionIpoQueueItem> {
    ["Anthropic", "OpenAI", "Databricks", "Stripe", "Figure"]
        .iter()
        .map(|issuer| CapitalAbsorptionIpoQueueItem {
            issuer: (*issuer).to_string(),
            status: CapitalAbsorptionIpoQueueStatus::Rumor,
            source_count: 0,
            event_type: CapitalAbsorptionObservationEventType::Rumor,
            lifecycle_status: CapitalAbsorptionIpoLifecycleStatus::Rumor,
            observed_at: None,
            observation_day: None,
            near_term_weight: None,
        })
        .collect()
}

fn default_capital_absorption_potential_supply_pressure() -> CapitalAbsorptionPotentialSupplyPressure
{
    CapitalAbsorptionPotentialSupplyPressure {
        level: CapitalAbsorptionPotentialSupplyPressureLevel::Low,
        near_term_supply_count: 0,
        future_queue_count: 0,
        queue_count: 0,
        reported_count: 0,
        confirmed_count: 0,
        drivers: Vec::new(),
    }
}

fn push_optional_usd(out: &mut String, label: &str, value: Option<f64>) {
    if let Some(value) = value {
        out.push_str(&format!("- {label} ${value:.1}B\n"));
    }
}

fn push_optional_score(out: &mut String, label: &str, value: Option<f64>) {
    if let Some(value) = value {
        out.push_str(&format!("- {label} {value:.2}\n"));
    }
}

fn format_usd(val: f64) -> String {
    if (val - val.round()).abs() < 0.01 {
        format!("${:.0}B", val)
    } else {
        format!("${:.1}B", val)
    }
}

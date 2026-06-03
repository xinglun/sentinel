use chrono::NaiveDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionAutoStatus {
    Normal,
    Watch,
    Active,
    Stressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionAutoRatioState {
    Low,
    Neutral,
    Elevated,
    Stressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionAutoTrend {
    Decreasing,
    Stable,
    Increasing,
    Accelerating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapitalAbsorptionAutoEventCategory {
    MegaCapFinancing,
    IpoSupply,
    SecondaryLiquidity,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionAutoEvent {
    pub category: CapitalAbsorptionAutoEventCategory,
    pub subject: String,
    pub description: String,
    pub amount_usd_b: Option<f64>,
    pub ai_capex_related: bool,
    pub source_url: Option<String>,
    pub observed_at: NaiveDate,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CapitalAbsorptionAutoSnapshot {
    pub source_status: CapitalAbsorptionSourceStatus,
    pub status: CapitalAbsorptionAutoStatus,
    pub observed_events: Vec<CapitalAbsorptionAutoEvent>,
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
    let demand_total = sum_amounts(&events);
    let ai_related = sum_amounts(events.iter().filter(|event| event.ai_capex_related));
    let ipo_total = sum_amounts(
        events
            .iter()
            .filter(|event| event.category == CapitalAbsorptionAutoEventCategory::IpoSupply),
    );
    let secondary_total =
        sum_amounts(events.iter().filter(|event| {
            event.category == CapitalAbsorptionAutoEventCategory::MegaCapFinancing
        }));
    let convertible_total = sum_amounts(events.iter().filter(|event| {
        event
            .description
            .to_ascii_lowercase()
            .contains("convertible")
    }));
    let unique_subjects = unique_subject_count(&events);
    let status = classify_status(&events, demand_total, unique_subjects);
    let demand_trend = if matches!(status, CapitalAbsorptionAutoStatus::Stressed) {
        CapitalAbsorptionAutoTrend::Accelerating
    } else if events.is_empty() {
        CapitalAbsorptionAutoTrend::Stable
    } else {
        CapitalAbsorptionAutoTrend::Increasing
    };
    let supply_trend = if events
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
        CapitalAbsorptionAutoStatus::Active => CapitalAbsorptionAutoRatioState::Elevated,
        CapitalAbsorptionAutoStatus::Stressed => CapitalAbsorptionAutoRatioState::Stressed,
    };
    CapitalAbsorptionAutoSnapshot {
        source_status,
        status,
        observed_events: events,
        capital_demand: CapitalDemandAutoSnapshot {
            rolling_12m_usd_b: demand_total,
            score: demand_total.map(|value| (value / 100.0).min(1.0)),
            trend: demand_trend,
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
    demand_total: Option<f64>,
    unique_subjects: usize,
) -> CapitalAbsorptionAutoStatus {
    if demand_total.is_some_and(|value| value >= 200.0) || events.len() >= 5 {
        CapitalAbsorptionAutoStatus::Stressed
    } else if unique_subjects >= 2 || events.len() >= 3 {
        CapitalAbsorptionAutoStatus::Active
    } else if !events.is_empty() {
        CapitalAbsorptionAutoStatus::Watch
    } else {
        CapitalAbsorptionAutoStatus::Normal
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
    fn classifies_multiple_subjects_as_active_without_trade_signal() {
        let events = vec![event("GOOG", 80.0), event("MSFT", 40.0)];

        let snapshot = build_capital_absorption_snapshot_from_events(
            events,
            CapitalAbsorptionSourceStatus {
                provider: "fixture".to_string(),
                status: CapitalAbsorptionSourceHealth::Succeeded,
                message: "fixture".to_string(),
            },
        );

        assert_eq!(snapshot.status, CapitalAbsorptionAutoStatus::Active);
        assert_eq!(snapshot.capital_demand.rolling_12m_usd_b, Some(120.0));
        assert_eq!(
            snapshot.absorption_ratio.state,
            CapitalAbsorptionAutoRatioState::Elevated
        );
        assert_eq!(snapshot.structural_impact, "Observation Only");
    }

    fn event(subject: &str, amount_usd_b: f64) -> CapitalAbsorptionAutoEvent {
        CapitalAbsorptionAutoEvent {
            category: CapitalAbsorptionAutoEventCategory::MegaCapFinancing,
            subject: subject.to_string(),
            description: "secondary offering for AI capex".to_string(),
            amount_usd_b: Some(amount_usd_b),
            ai_capex_related: true,
            source_url: None,
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        }
    }
}

use crate::features::radar::interface::presentation::{
    SignalContextCoverage, SignalContextInformationLevel, SignalContextItem,
    SignalContextSourceStatus, SignalContextV1,
};
use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
use crate::features::research::interface::macro_event_observation::MarketReaction;
use chrono::NaiveDate;

#[derive(Debug, Clone, Default)]
pub(crate) struct SignalContextCoverageInput {
    pub market_date: String,
    pub scheduled_macro: Vec<SignalContextItem>,
    pub corporate_events: Vec<SignalContextItem>,
    pub geopolitical_events: Vec<SignalContextItem>,
    pub commodity_events: Vec<SignalContextItem>,
    pub rates_credit_events: Vec<SignalContextItem>,
    pub market_structure_events: Vec<SignalContextItem>,
    pub coverage: SignalContextCoverage,
    pub observed_market_reactions: Vec<MarketReaction>,
    pub event_time_utc: Option<String>,
    pub event_time_market_tz: Option<String>,
    pub report_generated_at: Option<String>,
}

pub(crate) fn build_signal_context_v1(input: SignalContextCoverageInput) -> SignalContextV1 {
    let mut coverage = input.coverage;
    coverage.overall = aggregate_coverage([
        coverage.scheduled_macro,
        coverage.corporate,
        coverage.geopolitical,
        coverage.commodity,
        coverage.rates_credit,
        coverage.market_structure,
    ]);
    let mut all = Vec::new();
    all.extend(input.scheduled_macro.iter().cloned());
    all.extend(input.corporate_events.iter().cloned());
    all.extend(input.geopolitical_events.iter().cloned());
    all.extend(input.commodity_events.iter().cloned());
    all.extend(input.rates_credit_events.iter().cloned());
    all.extend(input.market_structure_events.iter().cloned());

    let primary_context = all.iter().cloned().max_by(|left, right| {
        item_rank(left)
            .cmp(&item_rank(right))
            .then_with(|| left.observed_at.cmp(&right.observed_at))
            .then_with(|| left.title.cmp(&right.title))
    });
    let primary_key = primary_context.as_ref().map(|item| item.title.clone());
    let secondary_contexts = all
        .iter()
        .filter(|item| primary_key.as_deref() != Some(item.title.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let overall_information_content = overall_information_content(&all, coverage.overall);
    let context_quality = context_quality(primary_context.as_ref(), coverage.overall);

    SignalContextV1 {
        market_date: input.market_date,
        scheduled_macro: input.scheduled_macro,
        corporate_events: input.corporate_events,
        geopolitical_events: input.geopolitical_events,
        commodity_events: input.commodity_events,
        rates_credit_events: input.rates_credit_events,
        market_structure_events: input.market_structure_events,
        primary_context,
        secondary_contexts,
        overall_information_content,
        context_quality,
        coverage,
        observed_market_reactions: input.observed_market_reactions,
        event_time_utc: input.event_time_utc,
        event_time_market_tz: input.event_time_market_tz,
        report_generated_at: input.report_generated_at,
        ..Default::default()
    }
}

pub(crate) fn build_v1_from_event_context(
    as_of_date: NaiveDate,
    event_context: &SignalContextEventReadModel,
) -> SignalContextV1 {
    let scheduled_macro = event_context
        .timeline_entries
        .iter()
        .filter(|entry| entry.event_date == as_of_date)
        .map(|entry| SignalContextItem {
            context_type:
                crate::features::radar::interface::presentation::SignalContextType::ScheduledMacro,
            title: entry.event_name.clone(),
            information_content: if entry.high_information {
                SignalContextInformationLevel::High
            } else {
                SignalContextInformationLevel::Medium
            },
            market_relevance: if entry.high_information {
                SignalContextInformationLevel::High
            } else {
                SignalContextInformationLevel::Medium
            },
            evidence_quality: SignalContextInformationLevel::High,
            lifecycle:
                crate::features::radar::interface::presentation::SignalContextLifecycle::Released,
            event_fact: entry.summary.clone(),
            observed_at: format!("{}T00:00:00Z", as_of_date),
            source_published_at: format!("{}T00:00:00Z", as_of_date),
            market_date: as_of_date.to_string(),
            evidence: vec![
                crate::features::research::interface::macro_event_observation::EvidenceRecord {
                    source: entry.source.clone(),
                    source_url: String::new(),
                    timestamp: format!("{}T00:00:00Z", as_of_date),
                    source_published_at: format!("{}T00:00:00Z", as_of_date),
                    event_type: entry.event_type.clone(),
                    subject: entry.event_name.clone(),
                    importance: format!("{:?}", entry.importance),
                },
            ],
        })
        .collect::<Vec<_>>();
    let scheduled_status = match event_context.source_health {
        MacroEventSourceHealth::Succeeded => SignalContextSourceStatus::Healthy,
        MacroEventSourceHealth::Partial => SignalContextSourceStatus::Partial,
        MacroEventSourceHealth::Unavailable => SignalContextSourceStatus::Unavailable,
    };
    let coverage = SignalContextCoverage {
        scheduled_macro: scheduled_status,
        overall: aggregate_coverage([
            scheduled_status,
            SignalContextSourceStatus::Unavailable,
            SignalContextSourceStatus::Unavailable,
            SignalContextSourceStatus::Unavailable,
            SignalContextSourceStatus::Unavailable,
            SignalContextSourceStatus::Unavailable,
        ]),
        ..SignalContextCoverage::default()
    };
    build_signal_context_v1(SignalContextCoverageInput {
        market_date: as_of_date.to_string(),
        scheduled_macro,
        coverage,
        ..SignalContextCoverageInput::default()
    })
}

pub(crate) fn aggregate_coverage(
    statuses: [SignalContextSourceStatus; 6],
) -> SignalContextSourceStatus {
    if statuses
        .iter()
        .all(|status| *status == SignalContextSourceStatus::Healthy)
    {
        SignalContextSourceStatus::Healthy
    } else if statuses.contains(&SignalContextSourceStatus::Unavailable) {
        SignalContextSourceStatus::Unavailable
    } else if statuses.contains(&SignalContextSourceStatus::Degraded) {
        SignalContextSourceStatus::Degraded
    } else {
        SignalContextSourceStatus::Partial
    }
}

#[allow(dead_code)]
pub(crate) fn classify_lifecycle(
    event_date: chrono::NaiveDate,
    market_date: chrono::NaiveDate,
    active_repricing: bool,
    days_since_observation: i64,
) -> crate::features::radar::interface::presentation::SignalContextLifecycle {
    use crate::features::radar::interface::presentation::SignalContextLifecycle;
    if event_date > market_date {
        SignalContextLifecycle::Upcoming
    } else if event_date == market_date {
        if active_repricing {
            SignalContextLifecycle::ActiveRepricing
        } else {
            SignalContextLifecycle::Released
        }
    } else if active_repricing && days_since_observation <= 3 {
        SignalContextLifecycle::Aftermath
    } else {
        SignalContextLifecycle::Expired
    }
}

fn item_rank(item: &SignalContextItem) -> (u8, u8, u8) {
    (
        information_rank(item.information_content),
        information_rank(item.market_relevance),
        information_rank(item.evidence_quality) * 10 + context_type_rank(item.context_type),
    )
}

fn context_type_rank(
    value: crate::features::radar::interface::presentation::SignalContextType,
) -> u8 {
    use crate::features::radar::interface::presentation::SignalContextType;
    match value {
        SignalContextType::ScheduledMacro => 6,
        SignalContextType::Geopolitical => 5,
        SignalContextType::Commodity => 4,
        SignalContextType::RatesCredit => 3,
        SignalContextType::Corporate => 2,
        SignalContextType::MarketStructure => 1,
    }
}

fn information_rank(value: SignalContextInformationLevel) -> u8 {
    match value {
        SignalContextInformationLevel::High => 4,
        SignalContextInformationLevel::Medium => 3,
        SignalContextInformationLevel::Low => 2,
        SignalContextInformationLevel::Unavailable => 0,
    }
}

fn overall_information_content(
    items: &[SignalContextItem],
    coverage: SignalContextSourceStatus,
) -> SignalContextInformationLevel {
    if items
        .iter()
        .any(|item| item.information_content == SignalContextInformationLevel::High)
    {
        return SignalContextInformationLevel::High;
    }
    if items
        .iter()
        .any(|item| item.information_content == SignalContextInformationLevel::Medium)
    {
        return SignalContextInformationLevel::Medium;
    }
    if coverage == SignalContextSourceStatus::Healthy {
        SignalContextInformationLevel::Low
    } else {
        SignalContextInformationLevel::Unavailable
    }
}

fn context_quality(
    primary: Option<&SignalContextItem>,
    coverage: SignalContextSourceStatus,
) -> crate::features::radar::interface::presentation::SignalContextQuality {
    use crate::features::radar::interface::presentation::SignalContextQuality;
    match (primary, coverage) {
        (Some(item), SignalContextSourceStatus::Healthy)
            if item.evidence_quality == SignalContextInformationLevel::High =>
        {
            SignalContextQuality::High
        }
        (Some(item), _) if information_rank(item.evidence_quality) >= 3 => {
            SignalContextQuality::Medium
        }
        (Some(_), SignalContextSourceStatus::Healthy) => SignalContextQuality::Low,
        _ => SignalContextQuality::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::interface::presentation::{
        SignalContextInformationLevel, SignalContextType,
    };

    fn item(title: &str, level: SignalContextInformationLevel) -> SignalContextItem {
        SignalContextItem {
            context_type: SignalContextType::ScheduledMacro,
            title: title.to_string(),
            information_content: level,
            market_relevance: level,
            evidence_quality: SignalContextInformationLevel::High,
            event_fact: title.to_string(),
            observed_at: "2026-08-07T12:30:00Z".to_string(),
            source_published_at: "2026-08-07T12:30:00Z".to_string(),
            market_date: "2026-08-07".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn payroll_is_high_primary_when_rates_is_secondary() {
        let input = SignalContextCoverageInput {
            market_date: "2026-08-07".to_string(),
            coverage: healthy_coverage(),
            scheduled_macro: vec![item(
                "US Employment Report",
                SignalContextInformationLevel::High,
            )],
            rates_credit_events: vec![item(
                "Treasury yield repricing",
                SignalContextInformationLevel::High,
            )],
            ..Default::default()
        };
        let snapshot = build_signal_context_v1(input);
        assert_eq!(
            snapshot.overall_information_content,
            SignalContextInformationLevel::High
        );
        assert_eq!(
            snapshot.primary_context.as_ref().unwrap().title,
            "US Employment Report"
        );
        assert_eq!(
            snapshot.context_quality,
            crate::features::radar::interface::presentation::SignalContextQuality::High
        );
    }

    #[test]
    fn partial_coverage_cannot_be_low_or_high_quality() {
        let mut input = SignalContextCoverageInput::default();
        input.coverage.overall = SignalContextSourceStatus::Partial;
        let snapshot = build_signal_context_v1(input);
        assert_eq!(
            snapshot.overall_information_content,
            SignalContextInformationLevel::Unavailable
        );
        assert_eq!(
            snapshot.context_quality,
            crate::features::radar::interface::presentation::SignalContextQuality::Unavailable
        );
    }

    #[test]
    fn geopolitical_and_commodity_shocks_do_not_fall_back_to_none() {
        let mut input = SignalContextCoverageInput {
            market_date: "2026-08-10".to_string(),
            coverage: healthy_coverage(),
            geopolitical_events: vec![item(
                "Hormuz shipping risk",
                SignalContextInformationLevel::High,
            )],
            commodity_events: vec![item("Oil shock", SignalContextInformationLevel::High)],
            ..Default::default()
        };
        input.geopolitical_events[0].context_type = SignalContextType::Geopolitical;
        input.commodity_events[0].context_type = SignalContextType::Commodity;
        let snapshot = build_signal_context_v1(input);
        assert_eq!(
            snapshot.overall_information_content,
            SignalContextInformationLevel::High
        );
        assert_eq!(
            snapshot.primary_context.as_ref().unwrap().title,
            "Hormuz shipping risk"
        );
        assert_eq!(snapshot.secondary_contexts.len(), 1);
    }

    #[test]
    fn lifecycle_does_not_repeat_old_event_as_new_event() {
        let market_date = chrono::NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();
        let event_date = chrono::NaiveDate::from_ymd_opt(2026, 8, 7).unwrap();
        assert_eq!(
            classify_lifecycle(event_date, market_date, true, 2),
            crate::features::radar::interface::presentation::SignalContextLifecycle::Aftermath
        );
        assert_eq!(
            classify_lifecycle(event_date, market_date, false, 3),
            crate::features::radar::interface::presentation::SignalContextLifecycle::Expired
        );
    }

    fn healthy_coverage() -> SignalContextCoverage {
        SignalContextCoverage {
            scheduled_macro: SignalContextSourceStatus::Healthy,
            corporate: SignalContextSourceStatus::Healthy,
            geopolitical: SignalContextSourceStatus::Healthy,
            commodity: SignalContextSourceStatus::Healthy,
            rates_credit: SignalContextSourceStatus::Healthy,
            market_structure: SignalContextSourceStatus::Healthy,
            overall: SignalContextSourceStatus::Healthy,
        }
    }
}

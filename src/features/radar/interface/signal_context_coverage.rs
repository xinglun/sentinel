use crate::features::radar::interface::presentation::{
    SignalContextCoverage, SignalContextInformationLevel, SignalContextItem,
    SignalContextSourceStatus, SignalContextV1,
};
use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
use crate::features::research::interface::macro_event_observation::MacroEventImportance;
use crate::features::research::interface::macro_event_observation::MacroEventSourceHealth;
use crate::features::research::interface::macro_event_observation::MarketReaction;
use chrono::NaiveDate;
use serde_json;
use std::env;
use std::fs;

const EXTERNAL_SIGNAL_CONTEXT_PATH_ENV: &str = "SENTINEL_SIGNAL_CONTEXT_JSON_PATH";

#[cfg(test)]
pub(crate) static SIGNAL_CONTEXT_ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    let primary_context = all
        .iter()
        .filter(|item| {
            matches!(
                item.information_content,
                SignalContextInformationLevel::High | SignalContextInformationLevel::Medium
            )
        })
        .cloned()
        .max_by(|left, right| {
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
    let macro_context = apply_runtime_source_coverage(
        build_macro_v1_from_event_context(as_of_date, event_context),
        event_context.runtime_coverage.as_ref(),
    );
    let external = env::var(EXTERNAL_SIGNAL_CONTEXT_PATH_ENV)
        .ok()
        .and_then(|path| {
            load_external_signal_context_from_path(&path, as_of_date)
                .ok()
                .flatten()
        });
    build_v1_from_event_context_with_external(macro_context, external)
}

fn apply_runtime_source_coverage(
    mut snapshot: SignalContextV1,
    runtime_coverage: Option<&SignalContextCoverage>,
) -> SignalContextV1 {
    let Some(runtime_coverage) = runtime_coverage else {
        return snapshot;
    };

    snapshot.coverage.corporate = runtime_coverage.corporate;
    snapshot.coverage.geopolitical = runtime_coverage.geopolitical;
    snapshot.coverage.commodity = runtime_coverage.commodity;
    snapshot.coverage.rates_credit = runtime_coverage.rates_credit;
    snapshot.coverage.market_structure = runtime_coverage.market_structure;
    snapshot.coverage.overall = aggregate_coverage([
        snapshot.coverage.scheduled_macro,
        snapshot.coverage.corporate,
        snapshot.coverage.geopolitical,
        snapshot.coverage.commodity,
        snapshot.coverage.rates_credit,
        snapshot.coverage.market_structure,
    ]);
    snapshot
}

fn build_v1_from_event_context_with_external(
    macro_context: SignalContextV1,
    external: Option<SignalContextV1>,
) -> SignalContextV1 {
    external
        .map(|external| merge_external_context(macro_context.clone(), external))
        .unwrap_or(macro_context)
}

fn build_macro_v1_from_event_context(
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
            information_content: macro_information_level(entry),
            market_relevance: macro_information_level(entry),
            evidence_quality: SignalContextInformationLevel::High,
            lifecycle: macro_lifecycle(entry),
            event_fact: entry
                .actual_value
                .as_ref()
                .map(|actual| format!("{}; Actual: {}", entry.summary, actual))
                .unwrap_or_else(|| entry.summary.clone()),
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
            expected_value: entry.expected_value.clone(),
            actual_value: entry.actual_value.clone(),
            surprise: entry.surprise.clone(),
            reason: entry.reason.clone(),
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

fn macro_information_level(
    entry: &crate::features::radar::interface::signal_context_event_read_model::SignalContextTimelineEntry,
) -> SignalContextInformationLevel {
    if entry.actual_value.is_some() || entry.high_information {
        return SignalContextInformationLevel::High;
    }
    if entry.lifecycle.eq_ignore_ascii_case("UPCOMING") {
        return SignalContextInformationLevel::Medium;
    }
    match entry.importance {
        Some(MacroEventImportance::High | MacroEventImportance::Medium) => {
            SignalContextInformationLevel::Medium
        }
        Some(MacroEventImportance::Low) | None => SignalContextInformationLevel::Low,
        Some(MacroEventImportance::Critical) => SignalContextInformationLevel::High,
    }
}

fn macro_lifecycle(
    entry: &crate::features::radar::interface::signal_context_event_read_model::SignalContextTimelineEntry,
) -> crate::features::radar::interface::presentation::SignalContextLifecycle {
    match entry.lifecycle.to_ascii_uppercase().as_str() {
        "UPCOMING" => {
            crate::features::radar::interface::presentation::SignalContextLifecycle::Upcoming
        }
        "RELEASED" | "COMPARED" => {
            crate::features::radar::interface::presentation::SignalContextLifecycle::Released
        }
        "ACTIVE_REPRICING" => {
            crate::features::radar::interface::presentation::SignalContextLifecycle::ActiveRepricing
        }
        "AFTERMATH" => {
            crate::features::radar::interface::presentation::SignalContextLifecycle::Aftermath
        }
        _ => crate::features::radar::interface::presentation::SignalContextLifecycle::Expired,
    }
}

/// 構造化された外部ソースを読み込み、日付または証拠が不十分な場合は fail-closed とする。
pub(crate) fn load_external_signal_context_from_path(
    path: &str,
    as_of_date: NaiveDate,
) -> Result<Option<SignalContextV1>, String> {
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let context =
        serde_json::from_str::<SignalContextV1>(&raw).map_err(|error| error.to_string())?;
    if context.market_date != as_of_date.to_string() {
        return Err(format!(
            "market_date mismatch: expected {}, got {}",
            as_of_date, context.market_date
        ));
    }
    let context = filter_external_context(context, as_of_date);
    if all_context_items(&context).iter().any(|item| {
        matches!(
            item.information_content,
            SignalContextInformationLevel::High | SignalContextInformationLevel::Medium
        ) && (item.evidence.is_empty()
            || item.evidence.iter().any(|evidence| {
                evidence.source.trim().is_empty()
                    || evidence.timestamp.trim().is_empty()
                    || evidence.event_type.trim().is_empty()
                    || evidence.subject.trim().is_empty()
                    || evidence.importance.trim().is_empty()
            }))
    }) {
        return Err("HIGH/MEDIUM context is missing EvidenceRecord fields".to_string());
    }
    Ok(Some(context))
}

fn all_context_items(context: &SignalContextV1) -> Vec<&SignalContextItem> {
    context
        .scheduled_macro
        .iter()
        .chain(context.corporate_events.iter())
        .chain(context.geopolitical_events.iter())
        .chain(context.commodity_events.iter())
        .chain(context.rates_credit_events.iter())
        .chain(context.market_structure_events.iter())
        .collect()
}

fn filter_external_context(mut context: SignalContextV1, as_of_date: NaiveDate) -> SignalContextV1 {
    let keep = |item: &SignalContextItem| {
        item.market_date == as_of_date.to_string()
            && !matches!(
                item.lifecycle,
                crate::features::radar::interface::presentation::SignalContextLifecycle::Expired
            )
    };
    context.scheduled_macro.retain(keep);
    context.corporate_events.retain(keep);
    context.geopolitical_events.retain(keep);
    context.commodity_events.retain(keep);
    context.rates_credit_events.retain(keep);
    context.market_structure_events.retain(keep);
    context
}

fn merge_external_context(
    macro_context: SignalContextV1,
    mut external: SignalContextV1,
) -> SignalContextV1 {
    let mut scheduled_macro = external.scheduled_macro;
    for item in macro_context.scheduled_macro {
        if !scheduled_macro
            .iter()
            .any(|existing| existing.title == item.title)
        {
            scheduled_macro.push(item);
        }
    }
    external.scheduled_macro = scheduled_macro;
    external.coverage.scheduled_macro = macro_context.coverage.scheduled_macro;
    build_signal_context_v1(SignalContextCoverageInput {
        market_date: external.market_date,
        scheduled_macro: external.scheduled_macro,
        corporate_events: external.corporate_events,
        geopolitical_events: external.geopolitical_events,
        commodity_events: external.commodity_events,
        rates_credit_events: external.rates_credit_events,
        market_structure_events: external.market_structure_events,
        coverage: external.coverage,
        observed_market_reactions: external.observed_market_reactions,
        event_time_utc: external.event_time_utc,
        event_time_market_tz: external.event_time_market_tz,
        report_generated_at: external.report_generated_at,
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
    use crate::features::research::interface::macro_event_observation::EvidenceRecord;

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
            lifecycle:
                crate::features::radar::interface::presentation::SignalContextLifecycle::Released,
            ..Default::default()
        }
    }

    #[test]
    fn external_context_requires_matching_market_date_and_traceable_evidence() {
        let mut context = SignalContextV1 {
            market_date: "2026-08-07".to_string(),
            scheduled_macro: vec![item(
                "US Employment Report",
                SignalContextInformationLevel::High,
            )],
            ..Default::default()
        };
        context.scheduled_macro[0].evidence = vec![EvidenceRecord {
            source: "official-employment-source".to_string(),
            source_url: "https://example.invalid/payroll".to_string(),
            timestamp: "2026-08-07T12:30:00Z".to_string(),
            source_published_at: "2026-08-07T12:30:00Z".to_string(),
            event_type: "EMPLOYMENT".to_string(),
            subject: "US Employment Report".to_string(),
            importance: "HIGH".to_string(),
        }];
        let path = std::env::temp_dir().join(format!(
            "sentinel-signal-context-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, serde_json::to_vec(&context).unwrap()).unwrap();
        let loaded = load_external_signal_context_from_path(
            path.to_str().unwrap(),
            NaiveDate::from_ymd_opt(2026, 8, 7).unwrap(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(loaded.scheduled_macro.len(), 1);
        assert!(load_external_signal_context_from_path(
            path.to_str().unwrap(),
            NaiveDate::from_ymd_opt(2026, 8, 8).unwrap(),
        )
        .is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn runtime_merge_promotes_external_geopolitical_context() {
        let mut geopolitical = item("Hormuz shipping risk", SignalContextInformationLevel::High);
        geopolitical.context_type = SignalContextType::Geopolitical;
        geopolitical.evidence = vec![EvidenceRecord {
            source: "official-shipping-source".to_string(),
            source_url: String::new(),
            timestamp: "2026-08-10T12:00:00Z".to_string(),
            source_published_at: "2026-08-10T12:00:00Z".to_string(),
            event_type: "GEOPOLITICAL".to_string(),
            subject: geopolitical.title.clone(),
            importance: "HIGH".to_string(),
        }];
        let external = SignalContextV1 {
            market_date: "2026-08-10".to_string(),
            geopolitical_events: vec![geopolitical],
            coverage: healthy_coverage(),
            ..Default::default()
        };
        let merged = build_v1_from_event_context_with_external(
            SignalContextV1 {
                market_date: "2026-08-10".to_string(),
                ..Default::default()
            },
            Some(external),
        );
        assert_eq!(
            merged
                .primary_context
                .as_ref()
                .map(|item| item.title.as_str()),
            Some("Hormuz shipping risk")
        );
        assert_eq!(
            merged.overall_information_content,
            SignalContextInformationLevel::High
        );
        assert_eq!(merged.decision_weight, 0);
        assert!(!merged.trade_signal);
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
    fn known_cpi_upcoming_is_not_collapsed_to_no_event() {
        let mut cpi = item("US CPI", SignalContextInformationLevel::Medium);
        cpi.lifecycle =
            crate::features::radar::interface::presentation::SignalContextLifecycle::Upcoming;
        cpi.market_date = "2026-08-12".to_string();
        cpi.expected_value = Some("2.9%".to_string());
        let snapshot = build_signal_context_v1(SignalContextCoverageInput {
            market_date: "2026-08-12".to_string(),
            scheduled_macro: vec![cpi],
            coverage: SignalContextCoverage {
                scheduled_macro: SignalContextSourceStatus::Healthy,
                ..Default::default()
            },
            ..Default::default()
        });

        let primary = snapshot
            .primary_context
            .expect("known CPI must remain visible");
        assert_eq!(primary.title, "US CPI");
        assert_eq!(
            primary.lifecycle,
            crate::features::radar::interface::presentation::SignalContextLifecycle::Upcoming
        );
        assert_eq!(
            primary.information_content,
            SignalContextInformationLevel::Medium
        );
        assert_eq!(snapshot.decision_weight, 0);
        assert!(!snapshot.trade_signal);
    }

    #[test]
    fn released_cpi_without_actual_is_degraded_but_keeps_event_fact() {
        let mut cpi = item("US CPI", SignalContextInformationLevel::High);
        cpi.actual_value = None;
        cpi.reason = Some("EVENT_DATA_UNAVAILABLE".to_string());
        let snapshot = build_signal_context_v1(SignalContextCoverageInput {
            market_date: "2026-08-12".to_string(),
            scheduled_macro: vec![cpi],
            coverage: SignalContextCoverage {
                scheduled_macro: SignalContextSourceStatus::Partial,
                ..Default::default()
            },
            ..Default::default()
        });

        let primary = snapshot
            .primary_context
            .expect("released CPI must remain visible");
        assert_eq!(primary.title, "US CPI");
        assert_eq!(primary.reason.as_deref(), Some("EVENT_DATA_UNAVAILABLE"));
        assert_eq!(
            snapshot.context_quality,
            crate::features::radar::interface::presentation::SignalContextQuality::Medium
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
    fn low_importance_macro_event_remains_low_information() {
        let event_context = SignalContextEventReadModel {
            source_health: MacroEventSourceHealth::Succeeded,
            timeline_entries: vec![
                crate::features::radar::interface::signal_context_event_read_model::SignalContextTimelineEntry {
                    event_date: NaiveDate::from_ymd_opt(2026, 8, 7).unwrap(),
                    event_name: "Minor survey release".to_string(),
                    event_type: "OTHER".to_string(),
                    source: "official-calendar".to_string(),
                    importance: Some(MacroEventImportance::Low),
                    lifecycle: "Released".to_string(),
                    summary: "Minor survey release".to_string(),
                    high_information: false,
                    expected_value: None,
                    actual_value: None,
                    surprise: None,
                    reason: None,
                },
            ],
            ..Default::default()
        };
        let snapshot = build_macro_v1_from_event_context(
            NaiveDate::from_ymd_opt(2026, 8, 7).unwrap(),
            &event_context,
        );
        assert_eq!(
            snapshot.scheduled_macro[0].information_content,
            SignalContextInformationLevel::Low
        );
        assert!(snapshot.primary_context.is_none());
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

    #[test]
    fn runtime_source_coverage_is_applied_without_inventing_events() {
        let event_context = SignalContextEventReadModel {
            runtime_coverage: Some(SignalContextCoverage {
                corporate: SignalContextSourceStatus::Healthy,
                rates_credit: SignalContextSourceStatus::Healthy,
                ..SignalContextCoverage::default()
            }),
            ..Default::default()
        };

        let snapshot = build_v1_from_event_context(
            NaiveDate::from_ymd_opt(2026, 8, 12).unwrap(),
            &event_context,
        );

        assert_eq!(
            snapshot.coverage.corporate,
            SignalContextSourceStatus::Healthy
        );
        assert_eq!(
            snapshot.coverage.rates_credit,
            SignalContextSourceStatus::Healthy
        );
        assert!(snapshot.corporate_events.is_empty());
        assert!(snapshot.rates_credit_events.is_empty());
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

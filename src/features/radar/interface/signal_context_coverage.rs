use crate::features::radar::interface::presentation::{
    SignalContextCoverage, SignalContextInformationLevel, SignalContextItem,
    SignalContextSourceStatus, SignalContextV1,
};
use crate::features::radar::interface::signal_context_event_read_model::SignalContextEventReadModel;
use crate::features::research::application::corporate_event_evidence_resolver::{
    CorporateEventEvidence, CorporateEventEvidenceLifecycle, CorporateEventEvidenceResolution,
    EvidenceConfidence, ExternalCorporateEventEnrichment,
};
use crate::features::research::application::corporate_event_provider::{
    CorporateEventObservation, CorporateEventProviderHealth, CorporateEventProviderReadModel,
    CorporateEventReleaseWindow, CorporateEventSource, CorporateEventSourceKind,
};
use crate::features::research::interface::macro_event_observation::EvidenceRecord;
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
    let macro_context = apply_corporate_event_provider_context(
        as_of_date,
        macro_context,
        &event_context.corporate_event_provider,
    );
    let macro_context = apply_corporate_event_evidence_context(
        as_of_date,
        macro_context,
        &event_context.corporate_event_evidence,
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

pub(crate) fn attach_corporate_event_evidence(
    mut event_context: SignalContextEventReadModel,
    resolution: CorporateEventEvidenceResolution,
) -> SignalContextEventReadModel {
    event_context.corporate_event_evidence = resolution;
    event_context
}

/// 外部 Signal Context の corporate item を Resolver の enrichment contract へ変換する。
pub(crate) fn external_corporate_event_enrichments(
    context: &SignalContextV1,
) -> Vec<ExternalCorporateEventEnrichment> {
    context
        .corporate_events
        .iter()
        .filter_map(|item| {
            let symbol = item.symbol.as_deref()?.trim().to_ascii_uppercase();
            let event_date = NaiveDate::parse_from_str(&item.market_date, "%Y-%m-%d").ok()?;
            let evidence = item.evidence.first()?;
            let observed_at = chrono::DateTime::parse_from_rfc3339(&item.observed_at)
                .ok()?
                .with_timezone(&chrono::Utc);
            let raw_theme = evidence.event_type.trim();
            let theme = (!raw_theme.is_empty() && !raw_theme.eq_ignore_ascii_case("EARNINGS"))
                .then(|| raw_theme.to_string());
            Some(ExternalCorporateEventEnrichment {
                symbol,
                event_type: crate::features::research::application::corporate_event_provider::CorporateEventType::Earnings,
                event_date,
                theme,
                importance: parse_evidence_confidence(&evidence.importance),
                structured_explanation: item
                    .reason
                    .clone()
                    .or_else(|| (!item.event_fact.trim().is_empty()).then(|| item.event_fact.clone())),
                source: CorporateEventSource {
                    provider_id: "external-signal-context".to_string(),
                    source_kind: CorporateEventSourceKind::ExternalFixture,
                    source_url: (!evidence.source_url.trim().is_empty())
                        .then(|| evidence.source_url.clone()),
                },
                observed_at,
            })
        })
        .collect()
}

fn parse_evidence_confidence(value: &str) -> Option<EvidenceConfidence> {
    match value.trim().to_ascii_uppercase().as_str() {
        "HIGH" => Some(EvidenceConfidence::High),
        "MEDIUM" => Some(EvidenceConfidence::Medium),
        "LOW" => Some(EvidenceConfidence::Low),
        _ => None,
    }
}

fn apply_corporate_event_provider_context(
    as_of_date: NaiveDate,
    snapshot: SignalContextV1,
    provider: &CorporateEventProviderReadModel,
) -> SignalContextV1 {
    if provider.source.is_empty() && provider.diagnostic.is_none() && provider.events.is_empty() {
        return snapshot;
    }
    let mut coverage = snapshot.coverage;
    coverage.corporate = match provider.health {
        CorporateEventProviderHealth::Healthy => SignalContextSourceStatus::Healthy,
        CorporateEventProviderHealth::Unavailable => SignalContextSourceStatus::Unavailable,
    };
    let mut corporate_events = Vec::new();
    for item in provider
        .events
        .iter()
        .map(|event| corporate_event_item(event, as_of_date))
    {
        if let Some(index) = corporate_events
            .iter()
            .position(|existing| corporate_events_match(existing, &item))
        {
            merge_provider_corporate_facts(&mut corporate_events[index], &item);
        } else {
            corporate_events.push(item);
        }
    }
    build_signal_context_v1(SignalContextCoverageInput {
        market_date: snapshot.market_date,
        scheduled_macro: snapshot.scheduled_macro,
        corporate_events,
        geopolitical_events: snapshot.geopolitical_events,
        commodity_events: snapshot.commodity_events,
        rates_credit_events: snapshot.rates_credit_events,
        market_structure_events: snapshot.market_structure_events,
        coverage,
        observed_market_reactions: snapshot.observed_market_reactions,
        event_time_utc: snapshot.event_time_utc,
        event_time_market_tz: snapshot.event_time_market_tz,
        report_generated_at: snapshot.report_generated_at,
    })
}

fn apply_corporate_event_evidence_context(
    as_of_date: NaiveDate,
    snapshot: SignalContextV1,
    resolution: &CorporateEventEvidenceResolution,
) -> SignalContextV1 {
    if resolution.events.is_empty() {
        return snapshot;
    }
    let mut corporate_events = snapshot.corporate_events.clone();
    for evidence in resolution
        .events
        .iter()
        .filter(|evidence| evidence.lifecycle != CorporateEventEvidenceLifecycle::Unavailable)
    {
        let item = canonical_corporate_event_item(evidence, as_of_date);
        if let Some(index) = corporate_events
            .iter()
            .position(|existing| corporate_events_match(existing, &item))
        {
            merge_provider_corporate_facts(&mut corporate_events[index], &item);
        } else {
            corporate_events.push(item);
        }
    }
    let mut coverage = snapshot.coverage;
    coverage.corporate = canonical_corporate_coverage(resolution);
    build_signal_context_v1(SignalContextCoverageInput {
        market_date: snapshot.market_date,
        scheduled_macro: snapshot.scheduled_macro,
        corporate_events,
        geopolitical_events: snapshot.geopolitical_events,
        commodity_events: snapshot.commodity_events,
        rates_credit_events: snapshot.rates_credit_events,
        market_structure_events: snapshot.market_structure_events,
        coverage,
        observed_market_reactions: snapshot.observed_market_reactions,
        event_time_utc: snapshot.event_time_utc,
        event_time_market_tz: snapshot.event_time_market_tz,
        report_generated_at: snapshot.report_generated_at,
    })
}

fn canonical_corporate_event_item(
    evidence: &CorporateEventEvidence,
    as_of_date: NaiveDate,
) -> SignalContextItem {
    let title = format!("{} EARNINGS", evidence.subject);
    let level = evidence
        .importance
        .or(Some(evidence.confidence))
        .map(evidence_confidence_level)
        .unwrap_or(SignalContextInformationLevel::Unavailable);
    let lifecycle = match evidence.lifecycle {
        CorporateEventEvidenceLifecycle::Scheduled
        | CorporateEventEvidenceLifecycle::PendingConfirmation => {
            crate::features::radar::interface::presentation::SignalContextLifecycle::Upcoming
        }
        CorporateEventEvidenceLifecycle::Confirmed
        | CorporateEventEvidenceLifecycle::Historical => {
            crate::features::radar::interface::presentation::SignalContextLifecycle::Released
        }
        CorporateEventEvidenceLifecycle::Unavailable => {
            crate::features::radar::interface::presentation::SignalContextLifecycle::Expired
        }
    };
    let event_date = evidence
        .confirmed_event_date
        .or(evidence.expected_date)
        .unwrap_or(as_of_date);
    let sources = evidence
        .evidence
        .iter()
        .map(|item| item.source.provider_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let mut facts = vec![format!(
        "{title} lifecycle: {:?}; expected date: {}; confirmed date: {}; sources: {}",
        evidence.lifecycle,
        evidence
            .expected_date
            .map(|date| date.to_string())
            .unwrap_or_else(|| "UNAVAILABLE".to_string()),
        evidence
            .confirmed_event_date
            .map(|date| date.to_string())
            .unwrap_or_else(|| "UNAVAILABLE".to_string()),
        if sources.is_empty() {
            "UNAVAILABLE"
        } else {
            &sources
        },
    )];
    if let Some(theme) = &evidence.theme {
        facts.push(format!("theme: {theme}"));
    }
    if let Some(explanation) = &evidence.structured_explanation {
        facts.push(format!("external explanation: {explanation}"));
    }
    if let Some(expected_value) = &evidence.expected_value {
        facts.push(format!("expected value: {expected_value}"));
    }
    if let Some(actual_value) = &evidence.actual_value {
        facts.push(format!("actual value: {actual_value}"));
    }
    for diagnostic in &evidence.diagnostics {
        facts.push(format!("diagnostic: {:?}", diagnostic.code));
    }
    SignalContextItem {
        context_type: crate::features::radar::interface::presentation::SignalContextType::Corporate,
        title: title.clone(),
        symbol: Some(evidence.subject.clone()),
        information_content: level,
        market_relevance: level,
        evidence_quality: if evidence.confirmed_at.is_some() {
            SignalContextInformationLevel::High
        } else {
            level
        },
        lifecycle,
        event_fact: facts.join("; "),
        observed_at: evidence
            .confirmed_at
            .or_else(|| evidence.evidence.first().map(|item| item.observed_at))
            .map(|timestamp| timestamp.to_rfc3339())
            .unwrap_or_default(),
        source_published_at: evidence
            .confirmed_at
            .map(|timestamp| timestamp.to_rfc3339())
            .unwrap_or_default(),
        market_date: event_date.to_string(),
        evidence: evidence
            .evidence
            .iter()
            .map(|item| EvidenceRecord {
                source: item.source.provider_id.clone(),
                source_url: item.source.source_url.clone().unwrap_or_default(),
                timestamp: item.observed_at.to_rfc3339(),
                source_published_at: item
                    .source_timestamp
                    .map(|timestamp| timestamp.to_rfc3339())
                    .unwrap_or_default(),
                event_type: "EARNINGS".to_string(),
                subject: title.clone(),
                importance: format!("{:?}", evidence.importance.unwrap_or(evidence.confidence)),
            })
            .collect(),
        expected_value: evidence.expected_value.clone(),
        actual_value: evidence.actual_value.clone(),
        reason: (!evidence.diagnostics.is_empty()).then(|| {
            evidence
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect::<Vec<_>>()
                .join("; ")
        }),
        ..Default::default()
    }
}

fn evidence_confidence_level(value: EvidenceConfidence) -> SignalContextInformationLevel {
    match value {
        EvidenceConfidence::High => SignalContextInformationLevel::High,
        EvidenceConfidence::Medium => SignalContextInformationLevel::Medium,
        EvidenceConfidence::Low => SignalContextInformationLevel::Low,
        EvidenceConfidence::Unavailable => SignalContextInformationLevel::Unavailable,
    }
}

fn canonical_corporate_coverage(
    resolution: &CorporateEventEvidenceResolution,
) -> SignalContextSourceStatus {
    if resolution.provider_health.iter().all(|provider| {
        provider.health
            == crate::features::research::application::corporate_event_evidence_resolver::CorporateEventEvidenceProviderHealth::Healthy
    }) {
        SignalContextSourceStatus::Healthy
    } else if resolution.events.iter().any(|event| {
        event.lifecycle != CorporateEventEvidenceLifecycle::Unavailable
    }) {
        SignalContextSourceStatus::Degraded
    } else {
        SignalContextSourceStatus::Unavailable
    }
}

pub(crate) fn corporate_event_item(
    event: &CorporateEventObservation,
    as_of_date: NaiveDate,
) -> SignalContextItem {
    let has_actual = event.eps_actual.is_some() || event.revenue_actual.is_some();
    let information_content = if has_actual {
        SignalContextInformationLevel::High
    } else {
        SignalContextInformationLevel::Medium
    };
    let title = format!("{} EARNINGS", event.symbol);
    let release_window = match event.release_window {
        CorporateEventReleaseWindow::BeforeMarketOpen => "BMO",
        CorporateEventReleaseWindow::AfterMarketClose => "AMC",
        CorporateEventReleaseWindow::DuringMarketHours => "DMH",
        CorporateEventReleaseWindow::Unknown => "UNKNOWN",
    };
    let event_fact = format!(
        "{title} release window: {release_window}; timezone: {}; FY{} Q{}{}",
        event.market_timezone,
        event.fiscal_year,
        event.fiscal_quarter,
        render_earnings_values(event),
    );
    let importance = if has_actual { "HIGH" } else { "MEDIUM" };
    SignalContextItem {
        context_type: crate::features::radar::interface::presentation::SignalContextType::Corporate,
        title: title.clone(),
        symbol: Some(event.symbol.clone()),
        information_content,
        market_relevance: information_content,
        evidence_quality: SignalContextInformationLevel::High,
        lifecycle: if event.market_date <= as_of_date {
            crate::features::radar::interface::presentation::SignalContextLifecycle::Released
        } else {
            crate::features::radar::interface::presentation::SignalContextLifecycle::Upcoming
        },
        event_fact,
        observed_at: event.observed_at.clone(),
        source_published_at: String::new(),
        market_date: event.market_date.to_string(),
        evidence: vec![EvidenceRecord {
            source: corporate_event_source_label(&event.source),
            source_url: event.source.source_url.clone().unwrap_or_default(),
            timestamp: event.observed_at.clone(),
            source_published_at: String::new(),
            event_type: "EARNINGS".to_string(),
            subject: title,
            importance: importance.to_string(),
        }],
        actual_value: event.revenue_actual.map(|value| value.to_string()),
        expected_value: event.revenue_estimate.map(|value| value.to_string()),
        ..Default::default()
    }
}

fn corporate_event_source_label(source: &CorporateEventSource) -> String {
    if source.provider_id == "finnhub"
        && source.source_kind == CorporateEventSourceKind::EarningsCalendar
    {
        "Finnhub Earnings Calendar".to_string()
    } else {
        source.provider_id.clone()
    }
}

fn render_earnings_values(event: &CorporateEventObservation) -> String {
    let mut values = Vec::new();
    if let Some(value) = event.eps_actual {
        values.push(format!("EPS actual: {value}"));
    }
    if let Some(value) = event.eps_estimate {
        values.push(format!("EPS estimate: {value}"));
    }
    if let Some(value) = event.revenue_actual {
        values.push(format!("Revenue actual: {value}"));
    }
    if let Some(value) = event.revenue_estimate {
        values.push(format!("Revenue estimate: {value}"));
    }
    if values.is_empty() {
        String::new()
    } else {
        format!("; {}", values.join("; "))
    }
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
            symbol: None,
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
    let provider_corporate_coverage = macro_context.coverage.corporate;
    let provider_corporate_events = macro_context.corporate_events;
    let external_has_corporate_events = !external.corporate_events.is_empty();
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
    let mut corporate_events = external.corporate_events;
    for item in provider_corporate_events {
        if let Some(index) = corporate_events
            .iter()
            .position(|existing| corporate_events_match(existing, &item))
        {
            merge_provider_corporate_facts(&mut corporate_events[index], &item);
        } else {
            corporate_events.push(item);
        }
    }
    external.corporate_events = corporate_events;
    if !external_has_corporate_events {
        external.coverage.corporate = provider_corporate_coverage;
    }
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

pub(crate) fn corporate_events_match(left: &SignalContextItem, right: &SignalContextItem) -> bool {
    if left.context_type
        != crate::features::radar::interface::presentation::SignalContextType::Corporate
        || right.context_type
            != crate::features::radar::interface::presentation::SignalContextType::Corporate
        || left.market_date != right.market_date
    {
        return false;
    }

    match (
        normalized_symbol(left.symbol.as_deref()),
        normalized_symbol(right.symbol.as_deref()),
    ) {
        (Some(left_symbol), Some(right_symbol)) => {
            left_symbol == right_symbol && event_kind(left) == event_kind(right)
        }
        _ => normalized_text(&left.title) == normalized_text(&right.title),
    }
}

fn merge_provider_corporate_facts(
    external_or_existing: &mut SignalContextItem,
    provider: &SignalContextItem,
) {
    if external_or_existing.symbol.is_none() {
        external_or_existing.symbol = provider.symbol.clone();
    }
    if external_or_existing.event_fact.trim().is_empty() {
        external_or_existing.event_fact = provider.event_fact.clone();
    } else if !provider.event_fact.trim().is_empty()
        && !external_or_existing
            .event_fact
            .contains(provider.event_fact.trim())
    {
        external_or_existing.event_fact = format!(
            "{} Provider facts: {}",
            external_or_existing.event_fact.trim_end_matches([' ', ';']),
            provider.event_fact.trim()
        );
    }
    if external_or_existing.observed_at.trim().is_empty() {
        external_or_existing.observed_at = provider.observed_at.clone();
    }
    if external_or_existing.source_published_at.trim().is_empty() {
        external_or_existing.source_published_at = provider.source_published_at.clone();
    }
    if external_or_existing.actual_value.is_none() {
        external_or_existing.actual_value = provider.actual_value.clone();
    }
    if external_or_existing.expected_value.is_none() {
        external_or_existing.expected_value = provider.expected_value.clone();
    }
    for evidence in &provider.evidence {
        if !external_or_existing.evidence.iter().any(|existing| {
            existing.source == evidence.source
                && existing.source_url == evidence.source_url
                && existing.timestamp == evidence.timestamp
                && existing.event_type == evidence.event_type
                && existing.subject == evidence.subject
                && existing.importance == evidence.importance
        }) {
            external_or_existing.evidence.push(evidence.clone());
        }
    }
}

fn normalized_symbol(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_uppercase())
}

fn event_kind(item: &SignalContextItem) -> String {
    item.title
        .split_whitespace()
        .last()
        .map(normalized_text)
        .unwrap_or_default()
}

fn normalized_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
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
    use crate::features::research::application::corporate_event_provider::{
        CorporateEventObservation, CorporateEventProviderHealth, CorporateEventProviderReadModel,
        CorporateEventReleaseWindow, CorporateEventSource, CorporateEventSourceKind,
    };
    use crate::features::research::interface::macro_event_observation::EvidenceRecord;

    fn finnhub_source(url: &str) -> CorporateEventSource {
        CorporateEventSource {
            provider_id: "finnhub".to_string(),
            source_kind: CorporateEventSourceKind::EarningsCalendar,
            source_url: Some(url.to_string()),
        }
    }

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
    fn provider_event_projects_to_high_corporate_signal_context() {
        let market_date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        let event_context = SignalContextEventReadModel {
            corporate_event_provider: CorporateEventProviderReadModel {
                health: CorporateEventProviderHealth::Healthy,
                source: finnhub_source(
                    "https://finnhub.io/api/v1/calendar/earnings?from=2026-08-27&to=2026-08-27",
                ),
                retrieved_at: "2026-08-27T20:00:00Z".to_string(),
                diagnostic: None,
                events: vec![CorporateEventObservation {
                    symbol: "NVDA".to_string(),
                    market_date,
                    market_timezone: "America/New_York".to_string(),
                    release_window: CorporateEventReleaseWindow::AfterMarketClose,
                    fiscal_quarter: 2,
                    fiscal_year: 2027,
                    eps_actual: None,
                    eps_estimate: Some(1.04),
                    revenue_actual: Some(96_200_000_000.0),
                    revenue_estimate: Some(95_000_000_000.0),
                    source: finnhub_source(
                        "https://finnhub.io/api/v1/calendar/earnings?from=2026-08-27&to=2026-08-27",
                    ),
                    observed_at: "2026-08-27T20:00:00Z".to_string(),
                }],
            },
            ..Default::default()
        };

        let snapshot = build_v1_from_event_context(market_date, &event_context);
        let event = snapshot
            .corporate_events
            .first()
            .expect("provider event must be projected");

        assert_eq!(event.title, "NVDA EARNINGS");
        assert_eq!(event.context_type, SignalContextType::Corporate);
        assert_eq!(
            event.information_content,
            SignalContextInformationLevel::High
        );
        assert_eq!(event.market_date, "2026-08-27");
        assert!(event.event_fact.contains("AMC"));
        assert!(event.event_fact.contains("America/New_York"));
        assert_eq!(event.evidence[0].event_type, "EARNINGS");
        assert_eq!(snapshot.decision_weight, 0);
        assert!(!snapshot.trade_signal);
        assert_eq!(snapshot.gate_effect, "none");
        assert_eq!(snapshot.execution_effect, "none");
        assert_eq!(snapshot.position_sizing_effect, "none");
        let baseline = SignalContextV1::default();
        assert_eq!(
            (
                snapshot.decision_weight,
                snapshot.trade_signal,
                snapshot.gate_effect.as_str(),
                snapshot.execution_effect.as_str(),
                snapshot.position_sizing_effect.as_str(),
            ),
            (
                baseline.decision_weight,
                baseline.trade_signal,
                baseline.gate_effect.as_str(),
                baseline.execution_effect.as_str(),
                baseline.position_sizing_effect.as_str(),
            )
        );
    }

    #[test]
    fn unavailable_provider_keeps_corporate_context_fail_closed() {
        let market_date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        let event_context = SignalContextEventReadModel {
            corporate_event_provider: CorporateEventProviderReadModel::unavailable(
                finnhub_source(
                    "https://finnhub.io/api/v1/calendar/earnings?from=2026-08-27&to=2026-08-27",
                ),
                "Finnhub API key is not configured",
            ),
            ..Default::default()
        };

        let snapshot = build_v1_from_event_context(market_date, &event_context);

        assert!(snapshot.corporate_events.is_empty());
        assert_eq!(
            snapshot.coverage.corporate,
            crate::features::radar::interface::presentation::SignalContextSourceStatus::Unavailable
        );
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
    fn runtime_merge_preserves_external_corporate_enrichment_without_duplicate() {
        let market_date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        let provider_event = corporate_event_item(
            &CorporateEventObservation {
                symbol: "NVDA".to_string(),
                market_date,
                market_timezone: "America/New_York".to_string(),
                release_window: CorporateEventReleaseWindow::AfterMarketClose,
                fiscal_quarter: 2,
                fiscal_year: 2027,
                revenue_actual: Some(96_200_000_000.0),
                revenue_estimate: Some(95_000_000_000.0),
                source: finnhub_source("https://finnhub.io/api/v1/calendar/earnings"),
                observed_at: "2026-08-27T00:00:00Z".to_string(),
                ..Default::default()
            },
            market_date,
        );
        let mut provider_only_event = provider_event.clone();
        provider_only_event.symbol = Some("MSFT".to_string());
        provider_only_event.title = "MSFT EARNINGS".to_string();
        let mut external_event = item("NVDA EARNINGS", SignalContextInformationLevel::High);
        external_event.context_type = SignalContextType::Corporate;
        external_event.market_date = market_date.to_string();
        external_event.event_fact = "NVIDIA earnings; AI INFRASTRUCTURE".to_string();
        external_event.evidence = vec![EvidenceRecord {
            source: "manual-corporate-context".to_string(),
            source_url: "https://example.invalid/nvda".to_string(),
            timestamp: "2026-08-27T20:00:00Z".to_string(),
            source_published_at: "2026-08-27T20:00:00Z".to_string(),
            event_type: "EARNINGS".to_string(),
            subject: "NVDA EARNINGS".to_string(),
            importance: "HIGH".to_string(),
        }];
        let merged = merge_external_context(
            SignalContextV1 {
                market_date: market_date.to_string(),
                corporate_events: vec![provider_event, provider_only_event],
                coverage: SignalContextCoverage {
                    corporate: SignalContextSourceStatus::Healthy,
                    ..Default::default()
                },
                ..Default::default()
            },
            SignalContextV1 {
                market_date: market_date.to_string(),
                corporate_events: vec![external_event],
                coverage: SignalContextCoverage {
                    corporate: SignalContextSourceStatus::Healthy,
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        assert_eq!(merged.corporate_events.len(), 2);
        let merged_nvda = merged
            .corporate_events
            .iter()
            .find(|event| event.title == "NVDA EARNINGS")
            .expect("external NVDA enrichment must remain");
        assert!(merged_nvda
            .event_fact
            .starts_with("NVIDIA earnings; AI INFRASTRUCTURE"));
        assert_eq!(merged_nvda.evidence[0].source, "manual-corporate-context");
        assert!(merged_nvda.event_fact.contains("AMC"));
        assert!(merged_nvda.event_fact.contains("FY2027 Q2"));
        assert!(merged_nvda
            .evidence
            .iter()
            .any(|evidence| evidence.source == "Finnhub Earnings Calendar"));
        assert!(merged
            .corporate_events
            .iter()
            .any(|event| event.title == "MSFT EARNINGS"));
    }

    #[test]
    fn provider_projection_deduplicates_same_symbol_date_and_event_kind() {
        let market_date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        let first = CorporateEventObservation {
            symbol: "NVDA".to_string(),
            market_date,
            market_timezone: "America/New_York".to_string(),
            release_window: CorporateEventReleaseWindow::AfterMarketClose,
            fiscal_quarter: 2,
            fiscal_year: 2027,
            revenue_actual: Some(96_200_000_000.0),
            source: finnhub_source("https://finnhub.io/api/v1/calendar/earnings"),
            observed_at: "2026-08-27T20:00:00Z".to_string(),
            ..Default::default()
        };
        let mut second = first.clone();
        second.eps_actual = Some(1.05);
        second.observed_at = "2026-08-27T20:01:00Z".to_string();
        let snapshot = build_v1_from_event_context(
            market_date,
            &SignalContextEventReadModel {
                corporate_event_provider: CorporateEventProviderReadModel {
                    health: CorporateEventProviderHealth::Healthy,
                    events: vec![first, second],
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        assert_eq!(snapshot.corporate_events.len(), 1);
        let event = &snapshot.corporate_events[0];
        assert!(event.event_fact.contains("Revenue actual"));
        assert!(event.event_fact.contains("EPS actual"));
        assert_eq!(event.evidence.len(), 2);
    }

    #[test]
    fn runtime_merge_matches_provider_ticker_to_external_company_name_by_symbol() {
        let market_date = NaiveDate::from_ymd_opt(2026, 8, 27).unwrap();
        let external = load_external_signal_context_from_path(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/signal_context/2026-08-27-nvidia-earnings.json"
            ),
            market_date,
        )
        .expect("NVIDIA fixture must deserialize")
        .expect("NVIDIA fixture must be present");
        let provider_event = corporate_event_item(
            &CorporateEventObservation {
                symbol: "NVDA".to_string(),
                market_date,
                market_timezone: "America/New_York".to_string(),
                release_window: CorporateEventReleaseWindow::AfterMarketClose,
                fiscal_quarter: 2,
                fiscal_year: 2027,
                revenue_actual: Some(96_200_000_000.0),
                source: finnhub_source("https://finnhub.io/api/v1/calendar/earnings"),
                ..Default::default()
            },
            market_date,
        );

        let merged = merge_external_context(
            SignalContextV1 {
                market_date: market_date.to_string(),
                corporate_events: vec![provider_event],
                coverage: SignalContextCoverage {
                    corporate: SignalContextSourceStatus::Healthy,
                    ..Default::default()
                },
                ..Default::default()
            },
            external,
        );

        assert_eq!(merged.corporate_events.len(), 1);
        assert_eq!(merged.corporate_events[0].title, "NVIDIA EARNINGS");
        assert_eq!(
            merged.corporate_events[0].evidence[0].event_type,
            "AI_INFRASTRUCTURE"
        );
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

    #[test]
    fn external_corporate_context_projects_to_resolver_enrichment() {
        let context = load_external_signal_context_from_path(
            "tests/fixtures/signal_context/2026-08-27-nvidia-earnings.json",
            NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
        )
        .unwrap()
        .unwrap();

        let enrichments = external_corporate_event_enrichments(&context);

        assert_eq!(enrichments.len(), 1);
        assert_eq!(enrichments[0].symbol, "NVDA");
        assert_eq!(enrichments[0].theme.as_deref(), Some("AI_INFRASTRUCTURE"));
        assert_eq!(enrichments[0].source.provider_id, "external-signal-context");
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

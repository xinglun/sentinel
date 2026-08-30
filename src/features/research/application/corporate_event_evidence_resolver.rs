use super::corporate_event_provider::{
    CorporateEventProviderHealth, CorporateEventProviderReadModel, CorporateEventSource,
    CorporateEventType, ExpectedCorporateEventProviderHealth,
    ExpectedCorporateEventProviderReadModel,
};
use super::official_disclosure_provider::{
    OfficialDisclosureKind, OfficialDisclosureProviderHealth, OfficialDisclosureProviderReadModel,
};
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::{BTreeMap, BTreeSet};

/// canonical corporate event の lifecycle。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CorporateEventEvidenceLifecycle {
    Scheduled,
    PendingConfirmation,
    Confirmed,
    Historical,
    Unavailable,
}

/// evidence の相対的な確度。売買判断の重みではない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceConfidence {
    High,
    Medium,
    Low,
    Unavailable,
}

/// Resolver が返す provider health。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CorporateEventEvidenceProviderHealth {
    Healthy,
    Partial,
    Stale,
    Unavailable,
}

/// evidence の provenance と cutoff 用時刻。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CorporateEventEvidenceRef {
    pub source: CorporateEventSource,
    pub event_date: NaiveDate,
    pub observed_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub source_timestamp: Option<DateTime<Utc>>,
    pub fact_kind: String,
}

/// Resolver が黙って補正しなかった不一致または採用不能理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceDiagnosticCode {
    ProviderDateConflict,
    ExpectedConfirmedDateDifference,
    EvidenceAfterReportRun,
    UntraceableEvidence,
    InvalidTimestamp,
    ResolverUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceDiagnostic {
    pub code: EvidenceDiagnosticCode,
    pub message: String,
}

/// 既存の Signal Context JSON から ACL が変換する企業イベント enrichment。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalCorporateEventEnrichment {
    pub symbol: String,
    pub event_type: CorporateEventType,
    pub event_date: NaiveDate,
    pub theme: Option<String>,
    pub importance: Option<EvidenceConfidence>,
    pub structured_explanation: Option<String>,
    pub source: CorporateEventSource,
    pub observed_at: DateTime<Utc>,
}

/// 複数 source を統合した企業イベントの canonical evidence。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CorporateEventEvidence {
    pub subject: String,
    pub event_type: CorporateEventType,
    pub lifecycle: CorporateEventEvidenceLifecycle,
    pub expected_date: Option<NaiveDate>,
    pub confirmed_event_date: Option<NaiveDate>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub confidence: EvidenceConfidence,
    pub evidence: Vec<CorporateEventEvidenceRef>,
    pub diagnostics: Vec<EvidenceDiagnostic>,
    pub expected_value: Option<String>,
    pub actual_value: Option<String>,
    pub theme: Option<String>,
    pub importance: Option<EvidenceConfidence>,
    pub structured_explanation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CorporateEventEvidenceProviderHealthRecord {
    pub provider_id: String,
    pub health: CorporateEventEvidenceProviderHealth,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct CorporateEventEvidenceResolution {
    pub events: Vec<CorporateEventEvidence>,
    pub provider_health: Vec<CorporateEventEvidenceProviderHealthRecord>,
}

/// Resolver application port の入力。provider concrete 型はここへ漏らさない。
pub(crate) struct CorporateEventEvidenceResolverInput<'a> {
    pub subjects: &'a [String],
    pub expected: &'a ExpectedCorporateEventProviderReadModel,
    pub official: &'a OfficialDisclosureProviderReadModel,
    pub aggregator: &'a CorporateEventProviderReadModel,
    pub enrichments: &'a [ExternalCorporateEventEnrichment],
    pub external_diagnostic: Option<&'a str>,
    pub report_run_at: DateTime<Utc>,
}

/// Provider の優先順位を上書きではなく evidence の併合として扱う Resolver。
pub(crate) struct CorporateEventEvidenceResolver;

impl CorporateEventEvidenceResolver {
    pub(crate) fn resolve(
        input: CorporateEventEvidenceResolverInput<'_>,
    ) -> CorporateEventEvidenceResolution {
        let report_date = input.report_run_at.date_naive();
        let mut accumulators = BTreeMap::<String, EvidenceAccumulator>::new();
        for subject in input.subjects {
            let symbol = normalize_symbol(subject);
            if !symbol.is_empty() {
                accumulators.entry(symbol).or_default();
            }
        }

        for event in &input.expected.events {
            let symbol = normalize_symbol(&event.symbol);
            if symbol.is_empty() {
                continue;
            }
            let accumulator = accumulators.entry(symbol).or_default();
            if event.observed_at > input.report_run_at {
                accumulator.diagnostics.push(EvidenceDiagnostic {
                    code: EvidenceDiagnosticCode::EvidenceAfterReportRun,
                    message: "expected event was observed after the report cutoff".to_string(),
                });
                continue;
            }
            accumulator.expected_dates.insert(event.expected_date);
            accumulator.evidence.push(CorporateEventEvidenceRef {
                source: event.source.clone(),
                event_date: event.expected_date,
                observed_at: event.observed_at,
                accepted_at: None,
                source_timestamp: None,
                fact_kind: "ExpectedEvent".to_string(),
            });
        }

        for observation in &input.official.observations {
            let symbol = normalize_symbol(&observation.symbol);
            if symbol.is_empty() {
                continue;
            }
            let accumulator = accumulators.entry(symbol).or_default();
            if !officially_visible(observation, input.report_run_at) {
                accumulator.diagnostics.push(EvidenceDiagnostic {
                    code: EvidenceDiagnosticCode::EvidenceAfterReportRun,
                    message: "official evidence was accepted after the report cutoff".to_string(),
                });
                continue;
            }
            if observation.disclosure_kind != OfficialDisclosureKind::EarningsRelated {
                continue;
            }
            accumulator.confirmed_dates.insert(observation.filing_date);
            accumulator.confirmed_at =
                min_datetime(accumulator.confirmed_at, observation.accepted_at);
            accumulator.evidence.push(CorporateEventEvidenceRef {
                source: observation.source.clone(),
                event_date: observation.filing_date,
                observed_at: observation.accepted_at.unwrap_or(observation.retrieved_at),
                accepted_at: observation.accepted_at,
                source_timestamp: observation.accepted_at,
                fact_kind: "OfficialEarningsDisclosure".to_string(),
            });
        }

        for observation in &input.aggregator.events {
            let symbol = normalize_symbol(&observation.symbol);
            let Some(observed_at) = parse_timestamp(&observation.observed_at) else {
                if !symbol.is_empty() {
                    accumulators
                        .entry(symbol)
                        .or_default()
                        .diagnostics
                        .push(EvidenceDiagnostic {
                            code: EvidenceDiagnosticCode::InvalidTimestamp,
                            message: "aggregator observation timestamp is invalid".to_string(),
                        });
                }
                continue;
            };
            if symbol.is_empty() {
                continue;
            }
            let accumulator = accumulators.entry(symbol).or_default();
            if observed_at > input.report_run_at {
                accumulator.diagnostics.push(EvidenceDiagnostic {
                    code: EvidenceDiagnosticCode::EvidenceAfterReportRun,
                    message: "aggregator observation was observed after the report cutoff"
                        .to_string(),
                });
                continue;
            }
            accumulator.aggregator_dates.insert(observation.market_date);
            accumulator.actual_value = observation.revenue_actual.map(|value| value.to_string());
            accumulator.expected_value =
                observation.revenue_estimate.map(|value| value.to_string());
            accumulator.evidence.push(CorporateEventEvidenceRef {
                source: observation.source.clone(),
                event_date: observation.market_date,
                observed_at,
                accepted_at: None,
                source_timestamp: Some(observed_at),
                fact_kind: "AggregatorObserved".to_string(),
            });
        }

        for enrichment in input.enrichments {
            let symbol = normalize_symbol(&enrichment.symbol);
            if symbol.is_empty() {
                continue;
            }
            let accumulator = accumulators.entry(symbol).or_default();
            if enrichment.source.is_empty() {
                accumulator.diagnostics.push(EvidenceDiagnostic {
                    code: EvidenceDiagnosticCode::UntraceableEvidence,
                    message: "external enrichment has no source metadata".to_string(),
                });
                continue;
            }
            if enrichment.observed_at > input.report_run_at {
                accumulator.diagnostics.push(EvidenceDiagnostic {
                    code: EvidenceDiagnosticCode::EvidenceAfterReportRun,
                    message: "external enrichment was observed after the report cutoff".to_string(),
                });
                continue;
            }
            accumulator.enrichment = Some(enrichment.clone());
            accumulator.evidence.push(CorporateEventEvidenceRef {
                source: enrichment.source.clone(),
                event_date: enrichment.event_date,
                observed_at: enrichment.observed_at,
                accepted_at: None,
                source_timestamp: Some(enrichment.observed_at),
                fact_kind: "ExternalEnrichment".to_string(),
            });
        }

        let events = accumulators
            .into_iter()
            .map(|(symbol, accumulator)| accumulator.into_evidence(symbol, report_date))
            .collect();

        CorporateEventEvidenceResolution {
            events,
            provider_health: provider_health(&input),
        }
    }

    pub(crate) fn unavailable(
        subjects: &[String],
        diagnostic: impl Into<String>,
    ) -> CorporateEventEvidenceResolution {
        let diagnostic = diagnostic.into();
        let events = subjects
            .iter()
            .map(|subject| CorporateEventEvidence {
                subject: normalize_symbol(subject),
                event_type: CorporateEventType::Earnings,
                lifecycle: CorporateEventEvidenceLifecycle::Unavailable,
                expected_date: None,
                confirmed_event_date: None,
                confirmed_at: None,
                confidence: EvidenceConfidence::Unavailable,
                evidence: Vec::new(),
                diagnostics: vec![EvidenceDiagnostic {
                    code: EvidenceDiagnosticCode::ResolverUnavailable,
                    message: diagnostic.clone(),
                }],
                expected_value: None,
                actual_value: None,
                theme: None,
                importance: None,
                structured_explanation: None,
            })
            .filter(|event| !event.subject.is_empty())
            .collect();
        let provider_health = [
            "alpha_vantage",
            "sec-edgar",
            "finnhub",
            "external-signal-context",
        ]
        .into_iter()
        .map(|provider_id| CorporateEventEvidenceProviderHealthRecord {
            provider_id: provider_id.to_string(),
            health: CorporateEventEvidenceProviderHealth::Unavailable,
            diagnostic: Some(diagnostic.clone()),
        })
        .collect();
        CorporateEventEvidenceResolution {
            events,
            provider_health,
        }
    }
}

#[derive(Default)]
struct EvidenceAccumulator {
    expected_dates: BTreeSet<NaiveDate>,
    aggregator_dates: BTreeSet<NaiveDate>,
    confirmed_dates: BTreeSet<NaiveDate>,
    confirmed_at: Option<DateTime<Utc>>,
    evidence: Vec<CorporateEventEvidenceRef>,
    diagnostics: Vec<EvidenceDiagnostic>,
    expected_value: Option<String>,
    actual_value: Option<String>,
    enrichment: Option<ExternalCorporateEventEnrichment>,
}

impl EvidenceAccumulator {
    fn into_evidence(mut self, symbol: String, report_date: NaiveDate) -> CorporateEventEvidence {
        self.evidence.sort_by(|left, right| {
            left.event_date
                .cmp(&right.event_date)
                .then_with(|| left.source.provider_id.cmp(&right.source.provider_id))
                .then_with(|| left.fact_kind.cmp(&right.fact_kind))
        });
        let has_event_evidence = !self.evidence.is_empty();
        let expected_date = single_date(&self.expected_dates);
        let confirmed_event_date = single_date(&self.confirmed_dates);
        let lifecycle = if !self.confirmed_dates.is_empty() {
            if confirmed_event_date.is_some_and(|date| date < report_date) {
                CorporateEventEvidenceLifecycle::Historical
            } else {
                CorporateEventEvidenceLifecycle::Confirmed
            }
        } else if has_event_evidence {
            let known_dates = self
                .expected_dates
                .iter()
                .chain(self.aggregator_dates.iter())
                .chain(
                    self.enrichment
                        .iter()
                        .map(|enrichment| &enrichment.event_date),
                );
            if known_dates
                .clone()
                .next()
                .is_some_and(|date| *date > report_date)
            {
                CorporateEventEvidenceLifecycle::Scheduled
            } else {
                CorporateEventEvidenceLifecycle::PendingConfirmation
            }
        } else {
            CorporateEventEvidenceLifecycle::Unavailable
        };
        let mut diagnostics = self.diagnostics;
        let mut non_official_dates = self.expected_dates.clone();
        non_official_dates.extend(self.aggregator_dates.iter().copied());
        if non_official_dates.len() > 1 {
            diagnostics.push(EvidenceDiagnostic {
                code: EvidenceDiagnosticCode::ProviderDateConflict,
                message: "expected and aggregator dates disagree; no date was selected".to_string(),
            });
        }
        if expected_date.is_some()
            && confirmed_event_date.is_some()
            && expected_date != confirmed_event_date
        {
            diagnostics.push(EvidenceDiagnostic {
                code: EvidenceDiagnosticCode::ExpectedConfirmedDateDifference,
                message: "expected date and official confirmed date differ".to_string(),
            });
        }
        let confidence = if !self.confirmed_dates.is_empty() {
            EvidenceConfidence::High
        } else if !self.expected_dates.is_empty() {
            EvidenceConfidence::Medium
        } else if !self.aggregator_dates.is_empty() {
            EvidenceConfidence::Low
        } else {
            EvidenceConfidence::Unavailable
        };
        let (theme, importance, structured_explanation) = self
            .enrichment
            .map(|enrichment| {
                (
                    enrichment.theme,
                    enrichment.importance,
                    enrichment.structured_explanation,
                )
            })
            .unwrap_or((None, None, None));
        CorporateEventEvidence {
            subject: symbol,
            event_type: CorporateEventType::Earnings,
            lifecycle,
            expected_date,
            confirmed_event_date,
            confirmed_at: self.confirmed_at,
            confidence,
            evidence: self.evidence,
            diagnostics,
            expected_value: self.expected_value,
            actual_value: self.actual_value,
            theme,
            importance,
            structured_explanation,
        }
    }
}

fn provider_health(
    input: &CorporateEventEvidenceResolverInput<'_>,
) -> Vec<CorporateEventEvidenceProviderHealthRecord> {
    vec![
        CorporateEventEvidenceProviderHealthRecord {
            provider_id: input.expected.source.provider_id.clone(),
            health: expected_health(input.expected, input.report_run_at),
            diagnostic: input.expected.diagnostic.clone(),
        },
        CorporateEventEvidenceProviderHealthRecord {
            provider_id: "sec-edgar".to_string(),
            health: official_health(input.official),
            diagnostic: input.official.diagnostic.clone(),
        },
        CorporateEventEvidenceProviderHealthRecord {
            provider_id: input.aggregator.source.provider_id.clone(),
            health: match input.aggregator.health {
                CorporateEventProviderHealth::Healthy => {
                    CorporateEventEvidenceProviderHealth::Healthy
                }
                CorporateEventProviderHealth::Unavailable => {
                    CorporateEventEvidenceProviderHealth::Unavailable
                }
            },
            diagnostic: input.aggregator.diagnostic.clone(),
        },
        CorporateEventEvidenceProviderHealthRecord {
            provider_id: "external-signal-context".to_string(),
            health: if input.external_diagnostic.is_some() || input.enrichments.is_empty() {
                CorporateEventEvidenceProviderHealth::Unavailable
            } else {
                CorporateEventEvidenceProviderHealth::Healthy
            },
            diagnostic: input.external_diagnostic.map(str::to_string),
        },
    ]
}

fn expected_health(
    model: &ExpectedCorporateEventProviderReadModel,
    report_run_at: DateTime<Utc>,
) -> CorporateEventEvidenceProviderHealth {
    match model.health {
        ExpectedCorporateEventProviderHealth::Unavailable => {
            CorporateEventEvidenceProviderHealth::Unavailable
        }
        ExpectedCorporateEventProviderHealth::Healthy => {
            if model.fetched_at.is_some_and(|fetched_at| {
                report_run_at.signed_duration_since(fetched_at).num_hours() > 24
            }) {
                CorporateEventEvidenceProviderHealth::Stale
            } else {
                CorporateEventEvidenceProviderHealth::Healthy
            }
        }
    }
}

fn official_health(
    model: &OfficialDisclosureProviderReadModel,
) -> CorporateEventEvidenceProviderHealth {
    match model.health {
        OfficialDisclosureProviderHealth::Healthy => {
            if model.unavailable_symbols.is_empty() {
                CorporateEventEvidenceProviderHealth::Healthy
            } else if !model.observations.is_empty() {
                CorporateEventEvidenceProviderHealth::Partial
            } else {
                CorporateEventEvidenceProviderHealth::Unavailable
            }
        }
        OfficialDisclosureProviderHealth::Unavailable => {
            if !model.observations.is_empty() {
                CorporateEventEvidenceProviderHealth::Partial
            } else {
                CorporateEventEvidenceProviderHealth::Unavailable
            }
        }
    }
}

fn officially_visible(
    observation: &super::official_disclosure_provider::OfficialDisclosureObservation,
    report_run_at: DateTime<Utc>,
) -> bool {
    observation.accepted_at.unwrap_or(observation.retrieved_at) <= report_run_at
}

fn normalize_symbol(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

fn single_date(dates: &BTreeSet<NaiveDate>) -> Option<NaiveDate> {
    (dates.len() == 1).then(|| *dates.iter().next().expect("one date must exist"))
}

fn min_datetime(
    left: Option<DateTime<Utc>>,
    right: Option<DateTime<Utc>>,
) -> Option<DateTime<Utc>> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CorporateEventEvidenceLifecycle, CorporateEventEvidenceResolver,
        CorporateEventEvidenceResolverInput, EvidenceConfidence, EvidenceDiagnosticCode,
        ExternalCorporateEventEnrichment,
    };
    use crate::features::research::application::corporate_event_provider::{
        CorporateEventObservation, CorporateEventProviderHealth, CorporateEventProviderReadModel,
        CorporateEventReleaseWindow, CorporateEventSource, CorporateEventSourceKind,
        ExpectedCorporateEvent, ExpectedCorporateEventProviderHealth,
        ExpectedCorporateEventProviderReadModel, FiscalPeriod,
    };
    use crate::features::research::application::official_disclosure_provider::{
        OfficialDisclosureKind, OfficialDisclosureObservation, OfficialDisclosureProviderHealth,
        OfficialDisclosureProviderReadModel,
    };
    use chrono::{DateTime, NaiveDate, TimeZone, Utc};
    use std::collections::BTreeMap;

    const REPORT_RUN_AT: &str = "2026-08-27T18:00:00Z";

    fn run_at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(REPORT_RUN_AT)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn source(provider_id: &str, source_kind: CorporateEventSourceKind) -> CorporateEventSource {
        CorporateEventSource {
            provider_id: provider_id.to_string(),
            source_kind,
            source_url: Some(format!("https://example.test/{provider_id}")),
        }
    }

    fn subjects() -> Vec<String> {
        vec!["NVDA".to_string()]
    }

    fn expected_model(
        events: Vec<ExpectedCorporateEvent>,
    ) -> ExpectedCorporateEventProviderReadModel {
        ExpectedCorporateEventProviderReadModel {
            health: ExpectedCorporateEventProviderHealth::Healthy,
            source: source("alpha_vantage", CorporateEventSourceKind::EarningsCalendar),
            fetched_at: Some(run_at()),
            diagnostic: None,
            events,
        }
    }

    fn expected(date: &str, observed_at: &str) -> ExpectedCorporateEvent {
        ExpectedCorporateEvent {
            symbol: "NVDA".to_string(),
            event_type: super::super::corporate_event_provider::CorporateEventType::Earnings,
            expected_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            fiscal_period: Some(FiscalPeriod {
                quarter: 2,
                year: 2027,
            }),
            source: source("alpha_vantage", CorporateEventSourceKind::EarningsCalendar),
            observed_at: DateTime::parse_from_rfc3339(observed_at)
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    fn official_model(
        observations: Vec<OfficialDisclosureObservation>,
    ) -> OfficialDisclosureProviderReadModel {
        OfficialDisclosureProviderReadModel {
            health: OfficialDisclosureProviderHealth::Healthy,
            observations,
            unavailable_symbols: BTreeMap::new(),
            retrieved_at: Some(run_at()),
            diagnostic: None,
        }
    }

    fn official(
        kind: OfficialDisclosureKind,
        filing_date: &str,
        accepted_at: &str,
    ) -> OfficialDisclosureObservation {
        OfficialDisclosureObservation {
            symbol: "NVDA".to_string(),
            cik: "0001045810".to_string(),
            form: "8-K".to_string(),
            accession_number: "0001045810-26-000001".to_string(),
            filing_date: NaiveDate::parse_from_str(filing_date, "%Y-%m-%d").unwrap(),
            report_date: Some(NaiveDate::from_ymd_opt(2026, 7, 31).unwrap()),
            accepted_at: Some(
                DateTime::parse_from_rfc3339(accepted_at)
                    .unwrap()
                    .with_timezone(&Utc),
            ),
            primary_document: Some("nvda-20260827.htm".to_string()),
            disclosure_kind: kind,
            source: source("sec-edgar", CorporateEventSourceKind::OfficialFiling),
            retrieved_at: run_at(),
        }
    }

    fn aggregator_model(events: Vec<CorporateEventObservation>) -> CorporateEventProviderReadModel {
        CorporateEventProviderReadModel {
            health: CorporateEventProviderHealth::Healthy,
            source: source("finnhub", CorporateEventSourceKind::EarningsCalendar),
            retrieved_at: REPORT_RUN_AT.to_string(),
            diagnostic: None,
            events,
        }
    }

    fn aggregator(date: &str) -> CorporateEventObservation {
        CorporateEventObservation {
            symbol: "NVDA".to_string(),
            market_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            market_timezone: "America/New_York".to_string(),
            release_window: CorporateEventReleaseWindow::AfterMarketClose,
            fiscal_quarter: 2,
            fiscal_year: 2027,
            eps_actual: Some(1.04),
            eps_estimate: Some(1.01),
            revenue_actual: Some(96_200_000_000.0),
            revenue_estimate: Some(95_000_000_000.0),
            source: source("finnhub", CorporateEventSourceKind::EarningsCalendar),
            observed_at: REPORT_RUN_AT.to_string(),
        }
    }

    fn resolve(
        expected_events: Vec<ExpectedCorporateEvent>,
        official_observations: Vec<OfficialDisclosureObservation>,
        aggregator_events: Vec<CorporateEventObservation>,
        enrichments: Vec<ExternalCorporateEventEnrichment>,
        report_run_at: DateTime<Utc>,
    ) -> super::CorporateEventEvidenceResolution {
        CorporateEventEvidenceResolver::resolve(CorporateEventEvidenceResolverInput {
            subjects: &subjects(),
            expected: &expected_model(expected_events),
            official: &official_model(official_observations),
            aggregator: &aggregator_model(aggregator_events),
            enrichments: &enrichments,
            external_diagnostic: None,
            report_run_at,
        })
    }

    #[test]
    fn alpha_only_is_scheduled_and_not_confirmed() {
        let resolution = resolve(
            vec![expected("2026-08-28", REPORT_RUN_AT)],
            vec![],
            vec![],
            vec![],
            run_at(),
        );
        let event = &resolution.events[0];

        assert_eq!(event.lifecycle, CorporateEventEvidenceLifecycle::Scheduled);
        assert_eq!(event.expected_date.unwrap().to_string(), "2026-08-28");
        assert_eq!(event.confirmed_at, None);
        assert_eq!(event.confidence, EvidenceConfidence::Medium);
    }

    #[test]
    fn sec_earnings_related_confirms_and_retains_expected_evidence() {
        let resolution = resolve(
            vec![expected("2026-08-27", REPORT_RUN_AT)],
            vec![official(
                OfficialDisclosureKind::EarningsRelated,
                "2026-08-28",
                REPORT_RUN_AT,
            )],
            vec![],
            vec![],
            run_at(),
        );
        let event = &resolution.events[0];

        assert_eq!(event.lifecycle, CorporateEventEvidenceLifecycle::Confirmed);
        assert_eq!(event.expected_date.unwrap().to_string(), "2026-08-27");
        assert_eq!(
            event.confirmed_event_date.unwrap().to_string(),
            "2026-08-28"
        );
        assert_eq!(event.evidence.len(), 2);
        assert!(event.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == EvidenceDiagnosticCode::ExpectedConfirmedDateDifference
        }));
    }

    #[test]
    fn non_earnings_filing_does_not_confirm_earnings() {
        let resolution = resolve(
            vec![expected("2026-08-28", REPORT_RUN_AT)],
            vec![official(
                OfficialDisclosureKind::QuarterlyReport,
                "2026-08-27",
                REPORT_RUN_AT,
            )],
            vec![],
            vec![],
            run_at(),
        );

        assert_eq!(
            resolution.events[0].lifecycle,
            CorporateEventEvidenceLifecycle::Scheduled
        );
        assert_eq!(resolution.events[0].confirmed_at, None);
    }

    #[test]
    fn finnhub_only_remains_pending_confirmation() {
        let resolution = resolve(
            vec![],
            vec![],
            vec![aggregator("2026-08-27")],
            vec![],
            run_at(),
        );

        assert_eq!(
            resolution.events[0].lifecycle,
            CorporateEventEvidenceLifecycle::PendingConfirmation
        );
        assert_eq!(resolution.events[0].confirmed_at, None);
        assert_eq!(
            resolution.events[0].actual_value.as_deref(),
            Some("96200000000")
        );
        assert_eq!(
            resolution.events[0].expected_value.as_deref(),
            Some("95000000000")
        );
    }

    #[test]
    fn non_official_date_conflict_is_retained_without_guessing() {
        let resolution = resolve(
            vec![expected("2026-08-27", REPORT_RUN_AT)],
            vec![],
            vec![aggregator("2026-08-28")],
            vec![],
            run_at(),
        );
        let event = &resolution.events[0];

        assert_eq!(
            event.lifecycle,
            CorporateEventEvidenceLifecycle::PendingConfirmation
        );
        assert_eq!(event.expected_date.unwrap().to_string(), "2026-08-27");
        assert!(event.confirmed_event_date.is_none());
        assert_eq!(event.evidence.len(), 2);
        assert!(event
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == EvidenceDiagnosticCode::ProviderDateConflict }));
    }

    #[test]
    fn post_cutoff_evidence_is_not_visible() {
        let cutoff = Utc.with_ymd_and_hms(2026, 8, 27, 15, 0, 0).unwrap();
        let resolution = resolve(
            vec![expected("2026-08-28", "2026-08-27T14:00:00Z")],
            vec![official(
                OfficialDisclosureKind::EarningsRelated,
                "2026-08-27",
                "2026-08-27T20:00:00Z",
            )],
            vec![aggregator("2026-08-27")],
            vec![],
            cutoff,
        );

        assert_eq!(
            resolution.events[0].lifecycle,
            CorporateEventEvidenceLifecycle::Scheduled
        );
        assert_eq!(resolution.events[0].evidence.len(), 1);
        assert_eq!(
            resolution.events[0].evidence[0].source.provider_id,
            "alpha_vantage"
        );
    }

    #[test]
    fn external_enrichment_is_retained_without_symbol_theme_inference() {
        let enrichment = ExternalCorporateEventEnrichment {
            symbol: "NVDA".to_string(),
            event_type: super::super::corporate_event_provider::CorporateEventType::Earnings,
            event_date: NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
            theme: Some("AI_INFRASTRUCTURE".to_string()),
            importance: Some(EvidenceConfidence::High),
            structured_explanation: Some("External explanation".to_string()),
            source: source("fixture", CorporateEventSourceKind::ExternalFixture),
            observed_at: run_at(),
        };
        let resolution = resolve(vec![], vec![], vec![], vec![enrichment], run_at());
        let event = &resolution.events[0];

        assert_eq!(event.theme.as_deref(), Some("AI_INFRASTRUCTURE"));
        assert_eq!(event.importance, Some(EvidenceConfidence::High));
        assert!(event
            .evidence
            .iter()
            .any(|evidence| evidence.source.provider_id == "fixture"));
    }

    #[test]
    fn subject_without_visible_evidence_is_unavailable() {
        let resolution = resolve(vec![], vec![], vec![], vec![], run_at());

        assert_eq!(
            resolution.events[0].lifecycle,
            CorporateEventEvidenceLifecycle::Unavailable
        );
        assert!(resolution.events[0].evidence.is_empty());
        assert!(resolution.events[0].diagnostics.is_empty());
    }
}

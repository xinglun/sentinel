use crate::features::radar::interface::presentation::{
    SignalContextPrimaryContext, SignalContextQuality,
};
use crate::features::research::domain::expectation::{
    ExpectationEventType, ExpectationLifecycleState,
};
use crate::features::research::interface::expectation_report_builder::ExpectationLayerSnapshot;
use crate::features::research::interface::macro_event_calendar_adapter::MacroEventCalendarReadModel;
use crate::features::research::interface::macro_event_observation::{
    FutureCalendarKind, FutureCalendarObservation, MacroEventImportance,
    MacroEventInformationContent, MacroEventLifecycle, MacroEventSourceHealth, MacroEventType,
};
use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub(crate) struct SignalContextEventReadModelInput<'a> {
    pub as_of_date: NaiveDate,
    pub expectation_snapshot: Option<&'a ExpectationLayerSnapshot>,
    pub future_calendar: Option<&'a MacroEventCalendarReadModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SignalContextEventReadModel {
    pub source_health: MacroEventSourceHealth,
    pub source_attempts: usize,
    pub source_successes: usize,
    pub source_failures: usize,
    pub source_diagnostic: Option<String>,
    pub index_reconstitution: SignalContextEventSlot,
    pub etf_rebalance: SignalContextEventSlot,
    pub holiday_liquidity: SignalContextEventSlot,
    pub pre_earnings_waiting: SignalContextEventSlot,
    pub major_event_waiting: SignalContextEventSlot,
    pub macro_event: SignalContextEventSlot,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum SignalContextEventSlot {
    #[default]
    Unavailable,
    Loaded(Option<SignalContextEvidence>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SignalContextEvidence {
    pub detected: bool,
    pub quality: SignalContextQuality,
    pub source: SignalContextEvidenceSource,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum SignalContextEvidenceSource {
    Calendar,
    ExpectationLifecycle,
    MacroSchedule,
    ManualObservation,
    Unknown,
}

pub(crate) fn build_signal_context_event_read_model(
    input: SignalContextEventReadModelInput<'_>,
) -> SignalContextEventReadModel {
    let pre_earnings_waiting = input
        .expectation_snapshot
        .map(|snapshot| build_pre_earnings_waiting_slot(snapshot, input.as_of_date))
        .unwrap_or(SignalContextEventSlot::Unavailable);
    let future_calendar = input
        .future_calendar
        .map(|calendar| build_future_calendar_slots(calendar, input.as_of_date))
        .unwrap_or_default();

    SignalContextEventReadModel {
        source_health: input
            .future_calendar
            .map(|calendar| calendar.source_health)
            .unwrap_or(MacroEventSourceHealth::Unavailable),
        source_attempts: input
            .future_calendar
            .map(|calendar| calendar.source_attempts)
            .unwrap_or(0),
        source_successes: input
            .future_calendar
            .map(|calendar| calendar.source_successes)
            .unwrap_or(0),
        source_failures: input
            .future_calendar
            .map(|calendar| calendar.source_failures)
            .unwrap_or(0),
        source_diagnostic: input
            .future_calendar
            .and_then(|calendar| calendar.diagnostic.clone()),
        index_reconstitution: future_calendar.index_reconstitution,
        etf_rebalance: future_calendar.etf_rebalance,
        holiday_liquidity: future_calendar.holiday_liquidity,
        pre_earnings_waiting,
        major_event_waiting: future_calendar.major_event_waiting,
        macro_event: future_calendar.macro_event,
    }
}

impl SignalContextEventReadModel {
    pub(crate) fn has_loaded_context(&self) -> bool {
        self.index_reconstitution.is_loaded()
            || self.etf_rebalance.is_loaded()
            || self.holiday_liquidity.is_loaded()
            || self.pre_earnings_waiting.is_loaded()
            || self.major_event_waiting.is_loaded()
            || self.macro_event.is_loaded()
    }

    pub(crate) fn detected_primary_context(&self) -> Option<SignalContextPrimaryContext> {
        if self.macro_event.is_detected() {
            return Some(SignalContextPrimaryContext::MacroEvent);
        }
        if self.major_event_waiting.is_detected() {
            return Some(SignalContextPrimaryContext::MajorEventWaiting);
        }
        if self.pre_earnings_waiting.is_detected() {
            return Some(SignalContextPrimaryContext::PreEarningsWaiting);
        }
        if self.index_reconstitution.is_detected() {
            return Some(SignalContextPrimaryContext::IndexReconstitution);
        }
        if self.etf_rebalance.is_detected() {
            return Some(SignalContextPrimaryContext::EtfRebalance);
        }
        if self.holiday_liquidity.is_detected() {
            return Some(SignalContextPrimaryContext::HolidayLiquidity);
        }
        None
    }

    pub(crate) fn evidence_quality_for(
        &self,
        context: SignalContextPrimaryContext,
    ) -> Option<SignalContextQuality> {
        self.slot_for(context)
            .and_then(|slot| slot.evidence())
            .map(|evidence| evidence.quality)
    }

    fn slot_for(&self, context: SignalContextPrimaryContext) -> Option<&SignalContextEventSlot> {
        match context {
            SignalContextPrimaryContext::IndexReconstitution => Some(&self.index_reconstitution),
            SignalContextPrimaryContext::EtfRebalance => Some(&self.etf_rebalance),
            SignalContextPrimaryContext::HolidayLiquidity => Some(&self.holiday_liquidity),
            SignalContextPrimaryContext::PreEarningsWaiting => Some(&self.pre_earnings_waiting),
            SignalContextPrimaryContext::MajorEventWaiting => Some(&self.major_event_waiting),
            SignalContextPrimaryContext::MacroEvent => Some(&self.macro_event),
            SignalContextPrimaryContext::QuarterEndRebalancing
            | SignalContextPrimaryContext::MonthEndRebalancing
            | SignalContextPrimaryContext::None => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct FutureCalendarSlots {
    index_reconstitution: SignalContextEventSlot,
    etf_rebalance: SignalContextEventSlot,
    holiday_liquidity: SignalContextEventSlot,
    major_event_waiting: SignalContextEventSlot,
    macro_event: SignalContextEventSlot,
}

impl SignalContextEventSlot {
    fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded(_))
    }

    fn is_detected(&self) -> bool {
        self.evidence().is_some_and(|evidence| evidence.detected)
    }

    fn evidence(&self) -> Option<&SignalContextEvidence> {
        match self {
            Self::Loaded(Some(evidence)) if evidence.detected => Some(evidence),
            _ => None,
        }
    }
}

fn build_pre_earnings_waiting_slot(
    snapshot: &ExpectationLayerSnapshot,
    as_of_date: NaiveDate,
) -> SignalContextEventSlot {
    let evidence = snapshot
        .observations
        .iter()
        .filter(|observation| observation.lifecycle_state == ExpectationLifecycleState::Pending)
        .filter(|observation| {
            is_near_term_pending_observation(observation.period.as_str(), as_of_date)
        })
        .find(|observation| is_pre_earnings_candidate(observation.event_type))
        .map(|observation| SignalContextEvidence {
            detected: true,
            quality: SignalContextQuality::Low,
            source: SignalContextEvidenceSource::ExpectationLifecycle,
            summary: format!(
                "{} / {} / {:?} pending",
                observation.subject, observation.period, observation.event_type
            ),
        });

    SignalContextEventSlot::Loaded(evidence)
}

fn build_future_calendar_slots(
    calendar: &MacroEventCalendarReadModel,
    as_of_date: NaiveDate,
) -> FutureCalendarSlots {
    if calendar.source_health == MacroEventSourceHealth::Unavailable
        && calendar.observations.is_empty()
    {
        return FutureCalendarSlots::default();
    }

    let mut slots = FutureCalendarSlots {
        index_reconstitution: SignalContextEventSlot::Loaded(None),
        etf_rebalance: SignalContextEventSlot::Loaded(None),
        holiday_liquidity: SignalContextEventSlot::Loaded(None),
        major_event_waiting: SignalContextEventSlot::Loaded(None),
        macro_event: SignalContextEventSlot::Loaded(None),
    };
    for observation in &calendar.observations {
        if observation.source_health == MacroEventSourceHealth::Unavailable {
            continue;
        }
        match observation.kind {
            FutureCalendarKind::MacroEvent => {
                if matches!(
                    observation.lifecycle,
                    MacroEventLifecycle::Upcoming | MacroEventLifecycle::Released
                ) && matches_macro_event_importance(observation.importance)
                    && observation.information_content == MacroEventInformationContent::High
                    && is_macro_event_window_hit(observation, as_of_date)
                    && matches_macro_event_type(observation.event_type)
                {
                    slots.macro_event =
                        SignalContextEventSlot::Loaded(Some(SignalContextEvidence {
                            detected: true,
                            quality: macro_event_quality(observation.source_health),
                            source: SignalContextEvidenceSource::Calendar,
                            summary: format!(
                                "{} / {} / {}",
                                observation.event_name, observation.event_date, observation.source
                            ),
                        }));
                }
            }
            FutureCalendarKind::MajorEventWaiting => {
                if matches!(
                    observation.lifecycle,
                    MacroEventLifecycle::Upcoming | MacroEventLifecycle::Released
                ) && matches_macro_event_importance(observation.importance)
                    && observation.information_content == MacroEventInformationContent::High
                    && is_future_event_window_hit(observation, as_of_date)
                {
                    slots.major_event_waiting =
                        SignalContextEventSlot::Loaded(Some(SignalContextEvidence {
                            detected: true,
                            quality: macro_event_quality(observation.source_health),
                            source: SignalContextEvidenceSource::Calendar,
                            summary: format!(
                                "{} / {} / {}",
                                observation.event_name, observation.event_date, observation.source
                            ),
                        }));
                }
            }
            FutureCalendarKind::IndexReconstitution => {
                if is_exact_day(observation.event_date, as_of_date) {
                    slots.index_reconstitution =
                        SignalContextEventSlot::Loaded(Some(SignalContextEvidence {
                            detected: true,
                            quality: macro_event_quality(observation.source_health),
                            source: SignalContextEvidenceSource::Calendar,
                            summary: format!(
                                "{} / {} / {}",
                                observation.event_name, observation.event_date, observation.source
                            ),
                        }));
                }
            }
            FutureCalendarKind::EtfRebalance => {
                if is_exact_day(observation.event_date, as_of_date) {
                    slots.etf_rebalance =
                        SignalContextEventSlot::Loaded(Some(SignalContextEvidence {
                            detected: true,
                            quality: macro_event_quality(observation.source_health),
                            source: SignalContextEvidenceSource::Calendar,
                            summary: format!(
                                "{} / {} / {}",
                                observation.event_name, observation.event_date, observation.source
                            ),
                        }));
                }
            }
            FutureCalendarKind::HolidayLiquidity => {
                if is_exact_day(observation.event_date, as_of_date) {
                    slots.holiday_liquidity =
                        SignalContextEventSlot::Loaded(Some(SignalContextEvidence {
                            detected: true,
                            quality: macro_event_quality(observation.source_health),
                            source: SignalContextEvidenceSource::Calendar,
                            summary: format!(
                                "{} / {} / {}",
                                observation.event_name, observation.event_date, observation.source
                            ),
                        }));
                }
            }
            FutureCalendarKind::PreEarningsWaiting => {}
        }
    }

    slots
}

fn is_near_term_pending_observation(period: &str, as_of_date: NaiveDate) -> bool {
    let Some(event_date) = parse_period_date(period) else {
        return false;
    };
    let days = event_date.signed_duration_since(as_of_date).num_days();
    days > 0 && days <= PRE_EARNINGS_WAITING_WINDOW_DAYS
}

fn is_pre_earnings_candidate(event_type: ExpectationEventType) -> bool {
    matches!(
        event_type,
        ExpectationEventType::DeliveryConsensus
            | ExpectationEventType::EarningsConsensus
            | ExpectationEventType::RevenueConsensus
            | ExpectationEventType::MarginConsensus
            | ExpectationEventType::CloudGrowthConsensus
            | ExpectationEventType::CapexConsensus
            | ExpectationEventType::ProductEventExpectation
            | ExpectationEventType::UserGrowthConsensus
            | ExpectationEventType::ProcedureGrowthConsensus
    )
}

fn matches_macro_event_type(event_type: MacroEventType) -> bool {
    matches!(
        event_type,
        MacroEventType::Cpi
            | MacroEventType::CoreCpi
            | MacroEventType::Ppi
            | MacroEventType::Pce
            | MacroEventType::CorePce
            | MacroEventType::NonfarmPayrolls
            | MacroEventType::UnemploymentRate
            | MacroEventType::Jolts
            | MacroEventType::Gdp
            | MacroEventType::FomcRateDecision
            | MacroEventType::FomcMinutes
            | MacroEventType::FedChairSpeech
            | MacroEventType::TreasuryAuction
            | MacroEventType::IsmManufacturing
            | MacroEventType::IsmServices
            | MacroEventType::RetailSales
    )
}

fn matches_macro_event_importance(importance: MacroEventImportance) -> bool {
    matches!(
        importance,
        MacroEventImportance::High | MacroEventImportance::Critical
    )
}

fn is_macro_event_window_hit(
    observation: &FutureCalendarObservation,
    as_of_date: NaiveDate,
) -> bool {
    observation.event_date == as_of_date
}

fn is_future_event_window_hit(
    observation: &FutureCalendarObservation,
    as_of_date: NaiveDate,
) -> bool {
    observation.event_date > as_of_date
        && observation
            .event_date
            .signed_duration_since(as_of_date)
            .num_days()
            <= MAJOR_EVENT_WAITING_WINDOW_DAYS
}

fn is_exact_day(event_date: NaiveDate, as_of_date: NaiveDate) -> bool {
    event_date == as_of_date
}

fn macro_event_quality(source_health: MacroEventSourceHealth) -> SignalContextQuality {
    match source_health {
        MacroEventSourceHealth::Succeeded => SignalContextQuality::High,
        MacroEventSourceHealth::Partial => SignalContextQuality::Medium,
        MacroEventSourceHealth::Unavailable => SignalContextQuality::Unavailable,
    }
}

fn parse_period_date(value: &str) -> Option<NaiveDate> {
    if let Ok(date) = NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d") {
        return Some(date);
    }

    let value = value.trim();
    let (year_part, quarter_part) = value.split_once('Q')?;
    let year = year_part.parse::<i32>().ok()?;
    let quarter = quarter_part.parse::<u32>().ok()?;
    match quarter {
        1 => NaiveDate::from_ymd_opt(year, 3, 31),
        2 => NaiveDate::from_ymd_opt(year, 6, 30),
        3 => NaiveDate::from_ymd_opt(year, 9, 30),
        4 => NaiveDate::from_ymd_opt(year, 12, 31),
        _ => None,
    }
}

const PRE_EARNINGS_WAITING_WINDOW_DAYS: i64 = 21;
const MAJOR_EVENT_WAITING_WINDOW_DAYS: i64 = 14;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::expectation::{
        ExpectationEventType, ExpectationLifecycleState, ExpectationObservation,
        ExpectationPressure, RevisionDirection, SourceHealth, SurpriseState,
    };
    use crate::features::research::interface::expectation_report_builder::ExpectationLayerSnapshot;
    use crate::features::research::interface::macro_event_calendar_adapter::{
        load_macro_event_calendar_from_json_str, MacroEventCalendarReadModel,
    };
    use crate::features::research::interface::macro_event_observation::{
        MacroEventImportance, MacroEventInformationContent, MacroEventLifecycle,
        MacroEventObservation, MacroEventSourceHealth, MacroEventSurpriseState, MacroEventType,
    };
    use chrono::NaiveDate;

    fn build_macro_event_observation(
        event_date: NaiveDate,
        importance: MacroEventImportance,
        lifecycle: MacroEventLifecycle,
        source_health: MacroEventSourceHealth,
        information_content: MacroEventInformationContent,
    ) -> MacroEventObservation {
        MacroEventObservation {
            event_id: "fomc-2026-06-18".to_string(),
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            event_date,
            event_time: Some("14:00".to_string()),
            timezone: "America/New_York".to_string(),
            country: "US".to_string(),
            event_type: MacroEventType::FomcRateDecision,
            event_name: "FOMC Rate Decision".to_string(),
            source: "Federal Reserve".to_string(),
            source_url: "https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm"
                .to_string(),
            importance,
            lifecycle,
            expected_value: Some("5.25%".to_string()),
            actual_value: None,
            previous_value: Some("5.50%".to_string()),
            unit: Some("%".to_string()),
            surprise_state: MacroEventSurpriseState::NotAvailable,
            information_content,
            source_health,
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 17).unwrap(),
        }
    }

    fn macro_event_calendar(observation: MacroEventObservation) -> MacroEventCalendarReadModel {
        MacroEventCalendarReadModel::from_observations(
            observation.as_of_date,
            "inline".to_string(),
            vec![observation],
        )
    }

    fn expectation_snapshot_with_pending_observation(period: &str) -> ExpectationLayerSnapshot {
        let as_of_date = NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
        ExpectationLayerSnapshot {
            as_of_date,
            decision_weight_percent: 0,
            trade_signal: false,
            gate_effect: "none".to_string(),
            execution_effect: "none".to_string(),
            position_sizing_effect: "none".to_string(),
            observations: vec![ExpectationObservation {
                subject: "TSLA".to_string(),
                period: period.to_string(),
                as_of_date,
                event_type: ExpectationEventType::DeliveryConsensus,
                lifecycle_state: ExpectationLifecycleState::Pending,
                expected_value: "~401k deliveries".to_string(),
                actual_value: "未発表".to_string(),
                result: None,
                surprise_percent: None,
                market_reaction: None,
                released_at: None,
                archived_at: None,
                unit: "deliveries".to_string(),
                consensus_source: "fixture".to_string(),
                estimate_count: 0,
                estimate_high: None,
                estimate_low: None,
                estimate_median: None,
                estimate_average: None,
                revision_direction: RevisionDirection::Unknown,
                surprise_state: SurpriseState::NotReleased,
                expectation_pressure: ExpectationPressure::Low,
                confidence: None,
                source_health: SourceHealth::Succeeded,
                interpretation: "fixture".to_string(),
                observed_at: as_of_date,
            }],
        }
    }

    fn future_calendar_fact(
        kind: FutureCalendarKind,
        event_date: NaiveDate,
        importance: MacroEventImportance,
        lifecycle: MacroEventLifecycle,
        source_health: MacroEventSourceHealth,
        information_content: MacroEventInformationContent,
    ) -> FutureCalendarObservation {
        FutureCalendarObservation {
            kind,
            event_id: format!("fact-{:?}-{}", kind, event_date),
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            event_date,
            event_time: Some("08:30".to_string()),
            timezone: "America/New_York".to_string(),
            country: "US".to_string(),
            event_type: MacroEventType::Gdp,
            event_name: format!("{:?}", kind),
            source: "Official Calendar".to_string(),
            source_url: "https://example.com/calendar".to_string(),
            importance,
            lifecycle,
            expected_value: None,
            actual_value: None,
            previous_value: None,
            unit: None,
            surprise_state: MacroEventSurpriseState::NotAvailable,
            information_content,
            source_health,
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
        }
    }

    fn future_calendar_with_fact(fact: FutureCalendarObservation) -> MacroEventCalendarReadModel {
        MacroEventCalendarReadModel::from_observations(
            fact.as_of_date,
            "inline".to_string(),
            vec![fact],
        )
    }

    #[test]
    fn build_macro_event_slot_from_json_observation() {
        let raw = serde_json::json!({
            "event_id": "cpi-2026-06-18",
            "as_of_date": "2026-06-18",
            "event_date": "2026-06-18",
            "event_time": "08:30",
            "timezone": "America/New_York",
            "country": "US",
            "event_type": "CPI",
            "event_name": "CPI Release",
            "source": "BLS",
            "source_url": "https://www.bls.gov/schedule/news_release/cpi.htm",
            "importance": "HIGH",
            "lifecycle": "UPCOMING",
            "expected_value": "2.9%",
            "actual_value": null,
            "previous_value": "2.8%",
            "unit": "%",
            "surprise_state": "NOT_AVAILABLE",
            "information_content": "HIGH",
            "source_health": "SUCCEEDED",
            "observed_at": "2026-06-17"
        });
        let calendar = load_macro_event_calendar_from_json_str(
            &raw.to_string(),
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            "inline-json",
        );
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: None,
            future_calendar: Some(&calendar),
        });

        assert_eq!(
            read_model.detected_primary_context(),
            Some(SignalContextPrimaryContext::MacroEvent)
        );
        assert_eq!(
            read_model.evidence_quality_for(SignalContextPrimaryContext::MacroEvent),
            Some(SignalContextQuality::High)
        );
    }

    #[test]
    fn macro_event_with_low_importance_does_not_trigger_macro_event() {
        let observation = build_macro_event_observation(
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            MacroEventImportance::Low,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: observation.as_of_date,
            expectation_snapshot: None,
            future_calendar: Some(&macro_event_calendar(observation)),
        });

        assert_eq!(read_model.detected_primary_context(), None);
        assert_eq!(
            read_model.evidence_quality_for(SignalContextPrimaryContext::MacroEvent),
            None
        );
    }

    #[test]
    fn macro_event_with_archived_lifecycle_does_not_trigger_macro_event() {
        let observation = build_macro_event_observation(
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            MacroEventImportance::Critical,
            MacroEventLifecycle::Archived,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::High,
        );
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: observation.as_of_date,
            expectation_snapshot: None,
            future_calendar: Some(&macro_event_calendar(observation)),
        });

        assert_eq!(read_model.detected_primary_context(), None);
    }

    #[test]
    fn macro_event_unavailable_keeps_none_unknown() {
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: None,
            future_calendar: Some(&MacroEventCalendarReadModel::unavailable(
                NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
                "missing".to_string(),
            )),
        });

        assert_eq!(read_model.detected_primary_context(), None);
        assert_eq!(
            read_model.evidence_quality_for(SignalContextPrimaryContext::MacroEvent),
            None
        );
    }

    #[test]
    fn macro_event_requires_same_day_or_window_hit() {
        let observation = build_macro_event_observation(
            NaiveDate::from_ymd_opt(2026, 6, 19).unwrap(),
            MacroEventImportance::Critical,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::High,
        );
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: None,
            future_calendar: Some(&macro_event_calendar(observation)),
        });

        assert_eq!(read_model.detected_primary_context(), None);
    }

    #[test]
    fn pre_earnings_waiting_loaded_from_near_term_pending_observation() {
        let snapshot = expectation_snapshot_with_pending_observation("2026-07-09");
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: Some(&snapshot),
            future_calendar: None,
        });

        assert_eq!(
            read_model.detected_primary_context(),
            Some(SignalContextPrimaryContext::PreEarningsWaiting)
        );
    }

    #[test]
    fn pre_earnings_waiting_excludes_outside_window_pending_observation() {
        let snapshot = expectation_snapshot_with_pending_observation("2026-07-10");
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: Some(&snapshot),
            future_calendar: None,
        });

        assert_eq!(read_model.detected_primary_context(), None);
    }

    #[test]
    fn pre_earnings_waiting_excludes_same_day_pending_observation() {
        let snapshot = expectation_snapshot_with_pending_observation("2026-06-18");
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: Some(&snapshot),
            future_calendar: None,
        });

        assert_eq!(read_model.detected_primary_context(), None);
    }

    #[test]
    fn index_reconstitution_loaded_from_future_calendar() {
        let fact = future_calendar_fact(
            FutureCalendarKind::IndexReconstitution,
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            MacroEventImportance::High,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let as_of_date = fact.as_of_date;
        let calendar = future_calendar_with_fact(fact);
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date,
            expectation_snapshot: None,
            future_calendar: Some(&calendar),
        });

        assert_eq!(
            read_model.detected_primary_context(),
            Some(SignalContextPrimaryContext::IndexReconstitution)
        );
    }

    #[test]
    fn index_reconstitution_excludes_off_by_one_day() {
        let fact = future_calendar_fact(
            FutureCalendarKind::IndexReconstitution,
            NaiveDate::from_ymd_opt(2026, 6, 19).unwrap(),
            MacroEventImportance::High,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let calendar = future_calendar_with_fact(fact);
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: None,
            future_calendar: Some(&calendar),
        });

        assert_eq!(read_model.detected_primary_context(), None);
    }

    #[test]
    fn etf_rebalance_loaded_from_future_calendar() {
        let fact = future_calendar_fact(
            FutureCalendarKind::EtfRebalance,
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            MacroEventImportance::High,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let as_of_date = fact.as_of_date;
        let calendar = future_calendar_with_fact(fact);
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date,
            expectation_snapshot: None,
            future_calendar: Some(&calendar),
        });

        assert_eq!(
            read_model.detected_primary_context(),
            Some(SignalContextPrimaryContext::EtfRebalance)
        );
    }

    #[test]
    fn etf_rebalance_excludes_off_by_one_day() {
        let fact = future_calendar_fact(
            FutureCalendarKind::EtfRebalance,
            NaiveDate::from_ymd_opt(2026, 6, 19).unwrap(),
            MacroEventImportance::High,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let calendar = future_calendar_with_fact(fact);
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: None,
            future_calendar: Some(&calendar),
        });

        assert_eq!(read_model.detected_primary_context(), None);
    }

    #[test]
    fn holiday_liquidity_loaded_from_future_calendar() {
        let fact = future_calendar_fact(
            FutureCalendarKind::HolidayLiquidity,
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            MacroEventImportance::High,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let as_of_date = fact.as_of_date;
        let calendar = future_calendar_with_fact(fact);
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date,
            expectation_snapshot: None,
            future_calendar: Some(&calendar),
        });

        assert_eq!(
            read_model.detected_primary_context(),
            Some(SignalContextPrimaryContext::HolidayLiquidity)
        );
    }

    #[test]
    fn holiday_liquidity_excludes_off_by_one_day() {
        let fact = future_calendar_fact(
            FutureCalendarKind::HolidayLiquidity,
            NaiveDate::from_ymd_opt(2026, 6, 19).unwrap(),
            MacroEventImportance::High,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::Low,
        );
        let calendar = future_calendar_with_fact(fact);
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: None,
            future_calendar: Some(&calendar),
        });

        assert_eq!(read_model.detected_primary_context(), None);
    }

    #[test]
    fn major_event_waiting_loaded_from_future_calendar() {
        let fact = future_calendar_fact(
            FutureCalendarKind::MajorEventWaiting,
            NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
            MacroEventImportance::Critical,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::High,
        );
        let calendar = future_calendar_with_fact(fact);
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: None,
            future_calendar: Some(&calendar),
        });

        assert_eq!(
            read_model.detected_primary_context(),
            Some(SignalContextPrimaryContext::MajorEventWaiting)
        );
    }

    #[test]
    fn major_event_waiting_excludes_same_day_future_calendar() {
        let fact = future_calendar_fact(
            FutureCalendarKind::MajorEventWaiting,
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            MacroEventImportance::Critical,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::High,
        );
        let calendar = future_calendar_with_fact(fact);
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: None,
            future_calendar: Some(&calendar),
        });

        assert_eq!(read_model.detected_primary_context(), None);
    }

    #[test]
    fn major_event_waiting_excludes_outside_window_future_calendar() {
        let fact = future_calendar_fact(
            FutureCalendarKind::MajorEventWaiting,
            NaiveDate::from_ymd_opt(2026, 7, 3).unwrap(),
            MacroEventImportance::Critical,
            MacroEventLifecycle::Upcoming,
            MacroEventSourceHealth::Succeeded,
            MacroEventInformationContent::High,
        );
        let calendar = future_calendar_with_fact(fact);
        let read_model = build_signal_context_event_read_model(SignalContextEventReadModelInput {
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            expectation_snapshot: None,
            future_calendar: Some(&calendar),
        });

        assert_eq!(read_model.detected_primary_context(), None);
    }
}

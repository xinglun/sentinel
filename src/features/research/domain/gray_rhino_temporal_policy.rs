use crate::features::research::domain::gray_rhino::{GrayRhinoAssessment, RhinoEscalationState};
use crate::features::research::domain::gray_rhino_evidence::{
    is_scoreable_evidence_record, GrayRhinoEvidenceCategory, GrayRhinoEvidenceRecord,
    GrayRhinoRiskEffect,
};
use chrono::{Duration, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemporalTrend {
    Rising,
    Stable,
    Falling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemperatureLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemperatureVelocity {
    Falling,
    Stable,
    Rising,
    Accelerating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstitutionalResponseState {
    Strong,
    Adequate,
    Weak,
    NoData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EscalationVelocity {
    pub delta_score: i32,
    pub delta_days: i64,
    pub changed_dimension_count: usize,
    pub trend: TemporalTrend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceAcceleration {
    pub recent_count: usize,
    pub prior_count: usize,
    pub trend: TemporalTrend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstitutionalResponseSummary {
    pub mitigating_count: usize,
    pub amplifying_count: usize,
    pub state: InstitutionalResponseState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrayRhinoTemporalSummary {
    pub temperature: TemperatureLevel,
    pub velocity: TemperatureVelocity,
    pub escalation_velocity: Option<EscalationVelocity>,
    pub evidence_acceleration: EvidenceAcceleration,
    pub institutional_response: InstitutionalResponseSummary,
}

pub(crate) fn build_temporal_summary(
    assessment: Option<&GrayRhinoAssessment>,
    records: &[GrayRhinoEvidenceRecord],
    as_of_date: NaiveDate,
) -> GrayRhinoTemporalSummary {
    let escalation_velocity = assessment.and_then(compute_escalation_velocity);
    let evidence_acceleration = compute_evidence_acceleration(records, as_of_date, 14);
    let temperature = compute_temperature(assessment, &evidence_acceleration);
    let velocity =
        compute_temperature_velocity(escalation_velocity.as_ref(), &evidence_acceleration);
    GrayRhinoTemporalSummary {
        temperature,
        velocity,
        escalation_velocity,
        evidence_acceleration,
        institutional_response: compute_institutional_response(records),
    }
}

fn compute_temperature(
    assessment: Option<&GrayRhinoAssessment>,
    acceleration: &EvidenceAcceleration,
) -> TemperatureLevel {
    match assessment
        .map(|value| value.current.escalation.escalation_state)
        .unwrap_or(RhinoEscalationState::Background)
    {
        RhinoEscalationState::Critical => TemperatureLevel::Critical,
        RhinoEscalationState::Expanding | RhinoEscalationState::Normalized => {
            TemperatureLevel::High
        }
        RhinoEscalationState::Visible => TemperatureLevel::Medium,
        RhinoEscalationState::Background => {
            if acceleration.recent_count > 0 {
                TemperatureLevel::Medium
            } else {
                TemperatureLevel::Low
            }
        }
    }
}

fn compute_temperature_velocity(
    escalation_velocity: Option<&EscalationVelocity>,
    acceleration: &EvidenceAcceleration,
) -> TemperatureVelocity {
    match escalation_velocity.map(|value| value.trend) {
        Some(TemporalTrend::Rising) if acceleration.trend == TemporalTrend::Rising => {
            TemperatureVelocity::Accelerating
        }
        Some(TemporalTrend::Rising) => TemperatureVelocity::Rising,
        Some(TemporalTrend::Falling) => TemperatureVelocity::Falling,
        Some(TemporalTrend::Stable) | None => match acceleration.trend {
            TemporalTrend::Rising => TemperatureVelocity::Rising,
            TemporalTrend::Falling => TemperatureVelocity::Falling,
            TemporalTrend::Stable => TemperatureVelocity::Stable,
        },
    }
}

fn compute_escalation_velocity(assessment: &GrayRhinoAssessment) -> Option<EscalationVelocity> {
    let previous = assessment.previous.as_ref()?;
    let current = &assessment.current;
    let delta_score =
        current.escalation.escalation_score() - previous.escalation.escalation_score();
    let delta_days = (current.as_of_date - previous.as_of_date).num_days().max(0);
    Some(EscalationVelocity {
        delta_score,
        delta_days,
        changed_dimension_count: assessment.changed_dimension_keys().len(),
        trend: classify_score_delta(delta_score).or_else_state_rank(
            state_rank(current.escalation.escalation_state)
                - state_rank(previous.escalation.escalation_state),
        ),
    })
}

fn compute_evidence_acceleration(
    records: &[GrayRhinoEvidenceRecord],
    as_of_date: NaiveDate,
    window_days: i64,
) -> EvidenceAcceleration {
    let recent_start = as_of_date - Duration::days(window_days - 1);
    let prior_start = as_of_date - Duration::days(window_days * 2 - 1);
    let prior_end = recent_start - Duration::days(1);

    let recent_count = records
        .iter()
        .filter(|record| is_scoreable_evidence_record(record))
        .filter(|record| {
            record.source.observed_at >= recent_start && record.source.observed_at <= as_of_date
        })
        .count();
    let prior_count = records
        .iter()
        .filter(|record| is_scoreable_evidence_record(record))
        .filter(|record| {
            record.source.observed_at >= prior_start && record.source.observed_at <= prior_end
        })
        .count();

    EvidenceAcceleration {
        recent_count,
        prior_count,
        trend: classify_count_delta(recent_count, prior_count),
    }
}

fn compute_institutional_response(
    records: &[GrayRhinoEvidenceRecord],
) -> InstitutionalResponseSummary {
    let mitigating_count = records
        .iter()
        .filter(|record| record.category == GrayRhinoEvidenceCategory::InstitutionalMaturity)
        .filter(|record| record.risk_effect == GrayRhinoRiskEffect::Mitigating)
        .count();
    let amplifying_count = records
        .iter()
        .filter(|record| record.category == GrayRhinoEvidenceCategory::InstitutionalMaturity)
        .filter(|record| record.risk_effect == GrayRhinoRiskEffect::Amplifying)
        .count();
    let state = match (mitigating_count, amplifying_count) {
        (0, 0) => InstitutionalResponseState::NoData,
        (mitigating, amplifying) if mitigating > amplifying => InstitutionalResponseState::Strong,
        (mitigating, amplifying) if mitigating == amplifying => {
            InstitutionalResponseState::Adequate
        }
        _ => InstitutionalResponseState::Weak,
    };

    InstitutionalResponseSummary {
        mitigating_count,
        amplifying_count,
        state,
    }
}

fn classify_score_delta(delta: i32) -> TemporalTrend {
    match delta.cmp(&0) {
        std::cmp::Ordering::Greater => TemporalTrend::Rising,
        std::cmp::Ordering::Equal => TemporalTrend::Stable,
        std::cmp::Ordering::Less => TemporalTrend::Falling,
    }
}

fn classify_count_delta(recent: usize, prior: usize) -> TemporalTrend {
    match recent.cmp(&prior) {
        std::cmp::Ordering::Greater => TemporalTrend::Rising,
        std::cmp::Ordering::Equal => TemporalTrend::Stable,
        std::cmp::Ordering::Less => TemporalTrend::Falling,
    }
}

fn state_rank(state: RhinoEscalationState) -> i32 {
    match state {
        RhinoEscalationState::Background => 0,
        RhinoEscalationState::Visible => 1,
        RhinoEscalationState::Expanding => 2,
        RhinoEscalationState::Normalized => 3,
        RhinoEscalationState::Critical => 4,
    }
}

trait TemporalTrendExt {
    fn or_else_state_rank(self, state_delta: i32) -> Self;
}

impl TemporalTrendExt for TemporalTrend {
    fn or_else_state_rank(self, state_delta: i32) -> Self {
        if self != TemporalTrend::Stable {
            return self;
        }
        classify_score_delta(state_delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::gray_rhino::{
        GrayRhinoAssessmentSnapshot, GrayRhinoEscalation, GrayRhinoObservationSource, RiskLevel,
    };
    use crate::features::research::domain::gray_rhino_evidence::{
        GrayRhinoEvidenceSourceType, GrayRhinoSourceReference,
    };

    fn escalation(score: RiskLevel) -> GrayRhinoEscalation {
        escalation_with_state(
            if score == RiskLevel::High {
                RhinoEscalationState::Critical
            } else {
                RhinoEscalationState::Visible
            },
            score,
        )
    }

    fn escalation_with_state(
        escalation_state: RhinoEscalationState,
        score: RiskLevel,
    ) -> GrayRhinoEscalation {
        GrayRhinoEscalation {
            escalation_state,
            risk_expansion_rate: score,
            constraint_growth_rate: RiskLevel::Low,
            dependency_centralization: score,
            awareness_decay: RiskLevel::Low,
            narrative_overconfidence: RiskLevel::Low,
            single_point_fragility: RiskLevel::Low,
            fallback_survivability_risk: RiskLevel::Low,
            notes: vec![],
            suppressed_note_count: 0,
        }
    }

    fn assessment_with_state(state: RhinoEscalationState) -> GrayRhinoAssessment {
        GrayRhinoAssessment {
            current: GrayRhinoAssessmentSnapshot {
                schema_version: 1,
                as_of_date: NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
                source: GrayRhinoObservationSource::EvidenceStore,
                escalation: escalation_with_state(state, RiskLevel::Moderate),
            },
            previous: None,
        }
    }

    fn snapshot(date: &str, level: RiskLevel) -> GrayRhinoAssessmentSnapshot {
        GrayRhinoAssessmentSnapshot {
            schema_version: 1,
            as_of_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            source: GrayRhinoObservationSource::EvidenceStore,
            escalation: escalation(level),
        }
    }

    fn record(
        date: &str,
        category: GrayRhinoEvidenceCategory,
        risk_effect: GrayRhinoRiskEffect,
    ) -> GrayRhinoEvidenceRecord {
        let observed_at = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        GrayRhinoEvidenceRecord {
            subject: "MSFT".to_string(),
            category,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::OperatorCuratedSource,
                source_title: "source".to_string(),
                publisher: "operator".to_string(),
                source_url: Some("https://example.com".to_string()),
                repository_path: None,
                observed_at,
                retrieved_at: observed_at,
            },
            confidence: 0.8,
            risk_effect,
            extraction_note: "note".to_string(),
            structural_fact: "fact".to_string(),
        }
    }

    #[test]
    fn escalation_velocity_rises_from_previous_snapshot() {
        let assessment = GrayRhinoAssessment {
            current: snapshot("2026-05-15", RiskLevel::High),
            previous: Some(snapshot("2026-05-01", RiskLevel::Moderate)),
        };

        let velocity = compute_escalation_velocity(&assessment).unwrap();

        assert_eq!(velocity.trend, TemporalTrend::Rising);
        assert_eq!(velocity.delta_days, 14);
        assert!(velocity.delta_score > 0);
        assert!(velocity.changed_dimension_count > 0);
    }

    #[test]
    fn evidence_acceleration_compares_recent_and_prior_windows() {
        let as_of = NaiveDate::from_ymd_opt(2026, 5, 28).unwrap();
        let records = vec![
            record(
                "2026-05-27",
                GrayRhinoEvidenceCategory::DependencyConcentration,
                GrayRhinoRiskEffect::Amplifying,
            ),
            record(
                "2026-05-21",
                GrayRhinoEvidenceCategory::GovernanceConcentration,
                GrayRhinoRiskEffect::Amplifying,
            ),
            record(
                "2026-05-10",
                GrayRhinoEvidenceCategory::GovernanceConcentration,
                GrayRhinoRiskEffect::Amplifying,
            ),
        ];

        let acceleration = compute_evidence_acceleration(&records, as_of, 14);

        assert_eq!(acceleration.recent_count, 2);
        assert_eq!(acceleration.prior_count, 1);
        assert_eq!(acceleration.trend, TemporalTrend::Rising);
    }

    #[test]
    fn temperature_contract_covers_all_levels() {
        let as_of = NaiveDate::from_ymd_opt(2026, 5, 28).unwrap();
        let recent_records = vec![record(
            "2026-05-27",
            GrayRhinoEvidenceCategory::DependencyConcentration,
            GrayRhinoRiskEffect::Amplifying,
        )];

        assert_eq!(
            build_temporal_summary(None, &[], as_of).temperature,
            TemperatureLevel::Low
        );
        assert_eq!(
            build_temporal_summary(None, &recent_records, as_of).temperature,
            TemperatureLevel::Medium
        );
        assert_eq!(
            build_temporal_summary(
                Some(&assessment_with_state(RhinoEscalationState::Visible)),
                &[],
                as_of
            )
            .temperature,
            TemperatureLevel::Medium
        );
        assert_eq!(
            build_temporal_summary(
                Some(&assessment_with_state(RhinoEscalationState::Expanding)),
                &[],
                as_of
            )
            .temperature,
            TemperatureLevel::High
        );
        assert_eq!(
            build_temporal_summary(
                Some(&assessment_with_state(RhinoEscalationState::Normalized)),
                &[],
                as_of
            )
            .temperature,
            TemperatureLevel::High
        );
        assert_eq!(
            build_temporal_summary(
                Some(&assessment_with_state(RhinoEscalationState::Critical)),
                &[],
                as_of
            )
            .temperature,
            TemperatureLevel::Critical
        );
    }

    #[test]
    fn temperature_velocity_contract_covers_all_values() {
        let rising_escalation = EscalationVelocity {
            delta_score: 1,
            delta_days: 7,
            changed_dimension_count: 1,
            trend: TemporalTrend::Rising,
        };
        let falling_escalation = EscalationVelocity {
            trend: TemporalTrend::Falling,
            ..rising_escalation.clone()
        };
        let stable_escalation = EscalationVelocity {
            trend: TemporalTrend::Stable,
            ..rising_escalation.clone()
        };
        let rising_evidence = EvidenceAcceleration {
            recent_count: 2,
            prior_count: 1,
            trend: TemporalTrend::Rising,
        };
        let falling_evidence = EvidenceAcceleration {
            recent_count: 1,
            prior_count: 2,
            trend: TemporalTrend::Falling,
        };
        let stable_evidence = EvidenceAcceleration {
            recent_count: 1,
            prior_count: 1,
            trend: TemporalTrend::Stable,
        };

        assert_eq!(
            compute_temperature_velocity(Some(&rising_escalation), &rising_evidence),
            TemperatureVelocity::Accelerating
        );
        assert_eq!(
            compute_temperature_velocity(Some(&rising_escalation), &stable_evidence),
            TemperatureVelocity::Rising
        );
        assert_eq!(
            compute_temperature_velocity(Some(&falling_escalation), &rising_evidence),
            TemperatureVelocity::Falling
        );
        assert_eq!(
            compute_temperature_velocity(Some(&stable_escalation), &rising_evidence),
            TemperatureVelocity::Rising
        );
        assert_eq!(
            compute_temperature_velocity(None, &falling_evidence),
            TemperatureVelocity::Falling
        );
        assert_eq!(
            compute_temperature_velocity(None, &stable_evidence),
            TemperatureVelocity::Stable
        );
    }

    #[test]
    fn institutional_response_is_weak_when_gaps_outnumber_mitigation() {
        let records = vec![
            record(
                "2026-05-20",
                GrayRhinoEvidenceCategory::InstitutionalMaturity,
                GrayRhinoRiskEffect::Amplifying,
            ),
            record(
                "2026-05-21",
                GrayRhinoEvidenceCategory::InstitutionalMaturity,
                GrayRhinoRiskEffect::Amplifying,
            ),
            record(
                "2026-05-22",
                GrayRhinoEvidenceCategory::InstitutionalMaturity,
                GrayRhinoRiskEffect::Mitigating,
            ),
        ];

        let response = compute_institutional_response(&records);

        assert_eq!(response.state, InstitutionalResponseState::Weak);
        assert_eq!(response.amplifying_count, 2);
        assert_eq!(response.mitigating_count, 1);
    }
}

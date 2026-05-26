#[cfg(test)]
use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateKind, GrayRhinoCandidateScope, GrayRhinoCandidateState,
};
pub(crate) use crate::features::research::domain::gray_rhino_monitoring_policy::{
    evaluate_gray_rhino_monitoring_states, GrayRhinoMonitoringDirection, GrayRhinoMonitoringStatus,
};
#[cfg(test)]
use chrono::NaiveDate;

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        subject: &str,
        state: GrayRhinoCandidateState,
        observed_at: NaiveDate,
    ) -> GrayRhinoCandidate {
        GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Company,
            kind: GrayRhinoCandidateKind::GovernanceConcentration,
            subject: subject.to_string(),
            state,
            evidence: vec!["governance candidate".to_string()],
            watch_triggers: vec!["proxy update".to_string()],
            source_title: "Proxy".to_string(),
            observed_at,
            source_published_at: Some(observed_at),
            last_confirmed_at: Some(observed_at),
            resolved_at: None,
        }
    }

    #[test]
    fn repeated_visible_candidate_intensifies_to_expanding() {
        let as_of_date = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
        let statuses = evaluate_gray_rhino_monitoring_states(
            &[
                candidate(
                    "TSLA",
                    GrayRhinoCandidateState::Visible,
                    NaiveDate::from_ymd_opt(2026, 5, 24).unwrap(),
                ),
                candidate("TSLA", GrayRhinoCandidateState::Visible, as_of_date),
            ],
            as_of_date,
        );

        assert_eq!(statuses.len(), 1);
        assert_eq!(
            statuses[0].current_state,
            GrayRhinoCandidateState::Expanding
        );
        assert_eq!(
            statuses[0].direction,
            GrayRhinoMonitoringDirection::Intensifying
        );
    }

    #[test]
    fn stale_persistent_candidate_cools_without_auto_resolving() {
        let candidate = candidate(
            "TSLA",
            GrayRhinoCandidateState::Expanding,
            NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        );

        let cooling = evaluate_gray_rhino_monitoring_states(
            std::slice::from_ref(&candidate),
            NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        );
        assert_eq!(cooling[0].current_state, GrayRhinoCandidateState::Cooling);

        let stale = evaluate_gray_rhino_monitoring_states(
            std::slice::from_ref(&candidate),
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        );
        assert_eq!(stale[0].current_state, GrayRhinoCandidateState::Cooling);
        assert_eq!(stale[0].direction, GrayRhinoMonitoringDirection::Cooling);
    }

    #[test]
    fn gray_rhino_time_model_persistent_governance_does_not_auto_resolve() {
        let as_of_date = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
        let statuses = evaluate_gray_rhino_monitoring_states(
            &[candidate(
                "GOOG",
                GrayRhinoCandidateState::Visible,
                NaiveDate::from_ymd_opt(2026, 4, 24).unwrap(),
            )],
            as_of_date,
        );

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].current_state, GrayRhinoCandidateState::Cooling);
        assert_eq!(statuses[0].direction, GrayRhinoMonitoringDirection::Cooling);
        assert_eq!(statuses[0].stale_days, 31);
    }

    #[test]
    fn gray_rhino_time_model_explicit_resolved_candidate_resolves() {
        let as_of_date = NaiveDate::from_ymd_opt(2026, 5, 25).unwrap();
        let mut resolved = candidate("GOOG", GrayRhinoCandidateState::Resolved, as_of_date);
        resolved.resolved_at = Some(as_of_date);
        let statuses = evaluate_gray_rhino_monitoring_states(&[resolved], as_of_date);

        assert_eq!(statuses[0].current_state, GrayRhinoCandidateState::Resolved);
        assert_eq!(
            statuses[0].direction,
            GrayRhinoMonitoringDirection::Resolved
        );
    }

    #[test]
    fn gray_rhino_lifecycle_stale_expanding_stays_cooling() {
        let candidate = candidate(
            "TSLA",
            GrayRhinoCandidateState::Expanding,
            NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        );

        let stale = evaluate_gray_rhino_monitoring_states(
            std::slice::from_ref(&candidate),
            NaiveDate::from_ymd_opt(2026, 6, 5).unwrap(),
        );

        assert_eq!(stale[0].current_state, GrayRhinoCandidateState::Cooling);
        assert_eq!(stale[0].direction, GrayRhinoMonitoringDirection::Cooling);
    }
}

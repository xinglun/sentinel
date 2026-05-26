use crate::features::research::domain::gray_rhino_candidate::GrayRhinoCandidate;
#[cfg(test)]
use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidateKind, GrayRhinoCandidateScope, GrayRhinoCandidateState,
};
use crate::features::research::domain::gray_rhino_discovery_policy::{
    self, GrayRhinoDiscoveryFacts,
};
use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrayRhinoDiscoveryInput {
    pub subject: String,
    pub source_title: String,
    pub observed_at: NaiveDate,
    pub text: String,
}

pub fn discover_gray_rhino_candidates(input: &GrayRhinoDiscoveryInput) -> Vec<GrayRhinoCandidate> {
    gray_rhino_discovery_policy::discover_gray_rhino_candidates(&GrayRhinoDiscoveryFacts {
        subject: &input.subject,
        source_title: &input.source_title,
        observed_at: input.observed_at,
        text: &input.text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_company_governance_control_from_prospectus_text() {
        let candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "SpaceX".to_string(),
            source_title: "Prospectus excerpt".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "The founder controls majority voting power through class B shares. The board is controlled and no independent directors provide effective checks.".to_string(),
        });

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].scope, GrayRhinoCandidateScope::Company);
        assert_eq!(
            candidates[0].kind,
            GrayRhinoCandidateKind::GovernanceConcentration
        );
        assert_eq!(candidates[0].state, GrayRhinoCandidateState::Expanding);
    }

    #[test]
    fn discovers_market_liquidity_and_narrative_candidates() {
        let candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "Market".to_string(),
            source_title: "FRED and Finnhub sources".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "Market narrative concentration is high. Liquidity tightened and rate pressure elevated. Capex payback risk is rising.".to_string(),
        });

        assert!(candidates
            .iter()
            .any(|candidate| candidate.kind == GrayRhinoCandidateKind::NarrativeCrowding));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.kind == GrayRhinoCandidateKind::LiquidityFragility));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.kind == GrayRhinoCandidateKind::CapexPaybackFragility));
    }

    #[test]
    fn fred_threshold_critical_terms_set_critical_state() {
        let candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "Market".to_string(),
            source_title: "FRED threshold assessment".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "rate pressure critical. credit stress critical. capex payback critical."
                .to_string(),
        });

        assert!(candidates.iter().any(|candidate| {
            candidate.kind == GrayRhinoCandidateKind::LiquidityFragility
                && candidate.state == GrayRhinoCandidateState::Critical
        }));
        assert!(candidates.iter().any(|candidate| {
            candidate.kind == GrayRhinoCandidateKind::CapexPaybackFragility
                && candidate.state == GrayRhinoCandidateState::Critical
        }));
    }

    #[test]
    fn governance_discovery_rejects_historic_control_without_current_constraint() {
        let candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "EXAMPLE".to_string(),
            source_title: "Old filing excerpt".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "The founder previously controlled the company before the recapitalization."
                .to_string(),
        });

        assert!(candidates.is_empty());
    }

    #[test]
    fn gray_rhino_noise_rejects_neutral_finnhub_boilerplate() {
        let candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "NVDA".to_string(),
            source_title: "Finnhub company news".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "Finnhub narrative source for NVDA. Purpose: normalize company news for structural-risk discovery.".to_string(),
        });

        assert!(candidates.is_empty());
    }

    #[test]
    fn discovers_institutional_and_redundancy_gaps() {
        let candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "EXAMPLE".to_string(),
            source_title: "Annual report".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "The issuer has no succession plan. A single point of failure remains and failover not tested.".to_string(),
        });

        assert!(candidates
            .iter()
            .any(|candidate| candidate.kind == GrayRhinoCandidateKind::InstitutionalMaturityGap));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.kind == GrayRhinoCandidateKind::RedundancyGap));
    }
}

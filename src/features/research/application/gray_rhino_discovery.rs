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

    #[test]
    fn test_discovers_company_ai_monetization_states() {
        // Test MSFT with capex rising significantly (Expanding)
        let candidates_msft = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "MSFT".to_string(),
            source_title: "Earnings Call".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "AI-related capex is rising significantly but copilot adoption remains pilot-stage. Multiple companies see capex payback fragility.".to_string(),
        });
        assert_eq!(candidates_msft.len(), 1);
        assert_eq!(candidates_msft[0].scope, GrayRhinoCandidateScope::Company);
        assert_eq!(candidates_msft[0].subject, "MSFT");
        assert_eq!(candidates_msft[0].state, GrayRhinoCandidateState::Expanding);
        assert!(candidates_msft[0].evidence.contains(
            &"AI-related capital spending fails to connect to revenue growth".to_string()
        ));

        // Test GOOG with profitability under pressure (Visible)
        let candidates_goog = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "GOOG".to_string(),
            source_title: "Quarterly report".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "AI investment compresses margin, putting profitability under pressure."
                .to_string(),
        });
        assert_eq!(candidates_goog.len(), 1);
        assert_eq!(candidates_goog[0].state, GrayRhinoCandidateState::Visible);
        assert!(candidates_goog[0]
            .evidence
            .contains(&"AI investment persistently compresses margin".to_string()));

        // Test NVDA with critical (Critical)
        let candidates_nvda = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "NVDA".to_string(),
            source_title: "Market review".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "Management ROI caveat: payback horizon extension makes capex payback critical."
                .to_string(),
        });
        assert_eq!(candidates_nvda.len(), 1);
        assert_eq!(candidates_nvda[0].state, GrayRhinoCandidateState::Critical);
    }

    #[test]
    fn test_rejects_forbidden_narrative_for_ai_monetization() {
        // Contains "ai will definitely fail"
        let candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "MSFT".to_string(),
            source_title: "Opinion piece".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "AI monetization fragility is visible, but AI will definitely fail anyway."
                .to_string(),
        });
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_unknown_and_narrative_only_evidence_rejected() {
        // Without allowed trigger facts, only generic narrative
        let candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "GOOG".to_string(),
            source_title: "Generic discussion".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "Some people are discussing tech stocks in the market.".to_string(),
        });
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_positive_or_neutral_narrative_is_ignored() {
        // Positive validation narrative (contains validate / margin expansion / robust)
        let candidates_positive = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "MSFT".to_string(),
            source_title: "PR Release".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "revenue growth and margin expansion validate AI monetization and show robust success".to_string(),
        });
        assert!(candidates_positive.is_empty());

        // Neutral narrative without negative sentiment words (just capex growth/timing/pilot stage, no risk/pressure)
        let candidates_neutral = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "GOOG".to_string(),
            source_title: "Fact sheet".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "We discuss our AI-related capex growth and monetization timing as we enter pilot stage.".to_string(),
        });
        assert!(candidates_neutral.is_empty());
    }

    #[test]
    fn test_critical_state_binding() {
        // "critical" used in positive/neutral growth context without any risk concept
        // It contains "validate", so it will be ignored directly
        let candidates_neutral_crit = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "MSFT".to_string(),
            source_title: "Tech speech".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "AI investment is critical to our long-term growth and success.".to_string(),
        });
        assert!(candidates_neutral_crit.is_empty());

        // "critical" used in neutral context (no validate) but no risk concept
        let candidates_neutral_crit2 = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "GOOG".to_string(),
            source_title: "Tech speech".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "This critical infrastructure investment is underway.".to_string(),
        });
        assert!(candidates_neutral_crit2.is_empty());

        // "critical" used with capex risk context -> triggers and becomes Critical state
        let candidates_real_crit = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "META".to_string(),
            source_title: "Urgent memo".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "We see payback risk and capital spending margin compression is in a critical stage.".to_string(),
        });
        assert_eq!(candidates_real_crit.len(), 1);
        assert_eq!(
            candidates_real_crit[0].state,
            GrayRhinoCandidateState::Critical
        );
    }

    #[test]
    fn test_mixed_signals_with_risk_predominance() {
        // Contains positive word "validates" but negated by "no longer", and has strong risk "margin compression"
        let candidates_negated_pos = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "MSFT".to_string(),
            source_title: "Earnings commentary".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "revenue growth no longer validates AI monetization and margin compression is rising".to_string(),
        });
        assert_eq!(candidates_negated_pos.len(), 1);
        assert_eq!(
            candidates_negated_pos[0].state,
            GrayRhinoCandidateState::Visible
        );
        assert!(candidates_negated_pos[0]
            .evidence
            .contains(&"AI investment persistently compresses margin".to_string()));

        // Contains positive word "accelerated" but transitioned by "but", and has strong risk "payback disappointment"
        let candidates_but_transition = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
            subject: "GOOG".to_string(),
            source_title: "Analysts review".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            text: "AI adoption accelerated, but payback disappointment remains".to_string(),
        });
        assert_eq!(candidates_but_transition.len(), 1);
        assert_eq!(
            candidates_but_transition[0].state,
            GrayRhinoCandidateState::Visible
        );
        assert!(candidates_but_transition[0].evidence.contains(
            &"Weakening prospects for returns on AI infrastructure investment".to_string()
        ));
    }
}

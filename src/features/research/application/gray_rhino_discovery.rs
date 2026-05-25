use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateKind, GrayRhinoCandidateScope, GrayRhinoCandidateState,
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
    let text = normalize(&input.text);
    let mut candidates = Vec::new();

    if contains_any(
        &text,
        &[
            "dual class",
            "super voting",
            "ten votes per share",
            "majority voting power",
            "controls more than 50",
            "has the ability to control",
            "will control",
            "continues to control",
            "controlled company exemption",
        ],
    ) && contains_any(
        &text,
        &["founder", "chief executive", "class b", "voting control"],
    ) && contains_any(
        &text,
        &[
            "no independent directors",
            "board is controlled",
            "limited ability to influence",
            "controlled company exemption",
            "check-and-balance weakness",
        ],
    ) {
        candidates.push(GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Company,
            kind: GrayRhinoCandidateKind::GovernanceConcentration,
            subject: input.subject.clone(),
            state: if contains_any(
                &text,
                &[
                    "majority voting power",
                    "controls more than 50",
                    "will control",
                ],
            ) {
                GrayRhinoCandidateState::Expanding
            } else {
                GrayRhinoCandidateState::Visible
            },
            evidence: vec![
                "Founder or single-party voting control detected.".to_string(),
                "Governance check-and-balance weakness detected.".to_string(),
            ],
            watch_triggers: vec![
                "IPO voting terms".to_string(),
                "board composition changes".to_string(),
                "related-party transactions".to_string(),
                "founder control changes".to_string(),
            ],
            source_title: input.source_title.clone(),
            observed_at: input.observed_at,
            source_published_at: Some(input.observed_at),
            last_confirmed_at: Some(input.observed_at),
            resolved_at: None,
        });
    }

    if contains_any(
        &text,
        &[
            "sole supplier",
            "single supplier",
            "single cloud provider",
            "dependent on one",
            "no alternative supplier",
            "single provider dependency",
        ],
    ) {
        candidates.push(GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Company,
            kind: GrayRhinoCandidateKind::DependencyConcentration,
            subject: input.subject.clone(),
            state: GrayRhinoCandidateState::Visible,
            evidence: vec!["Single dependency or missing fallback detected.".to_string()],
            watch_triggers: vec![
                "supplier disruption".to_string(),
                "cloud outage".to_string(),
                "fallback disclosure change".to_string(),
            ],
            source_title: input.source_title.clone(),
            observed_at: input.observed_at,
            source_published_at: Some(input.observed_at),
            last_confirmed_at: Some(input.observed_at),
            resolved_at: None,
        });
    }

    if contains_any(
        &text,
        &[
            "no succession plan",
            "succession gap",
            "weak oversight",
            "audit committee weakness",
            "compliance maturity is low",
        ],
    ) {
        candidates.push(GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Company,
            kind: GrayRhinoCandidateKind::InstitutionalMaturityGap,
            subject: input.subject.clone(),
            state: if contains_any(&text, &["material weakness", "no succession plan"]) {
                GrayRhinoCandidateState::Expanding
            } else {
                GrayRhinoCandidateState::Visible
            },
            evidence: vec!["Institutional maturity gap detected.".to_string()],
            watch_triggers: vec![
                "succession disclosure".to_string(),
                "audit committee remediation".to_string(),
                "compliance program update".to_string(),
            ],
            source_title: input.source_title.clone(),
            observed_at: input.observed_at,
            source_published_at: Some(input.observed_at),
            last_confirmed_at: Some(input.observed_at),
            resolved_at: None,
        });
    }

    if contains_any(
        &text,
        &[
            "no fallback",
            "fallback unavailable",
            "failover not tested",
            "single point of failure",
            "redundancy gap",
        ],
    ) {
        candidates.push(GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Company,
            kind: GrayRhinoCandidateKind::RedundancyGap,
            subject: input.subject.clone(),
            state: if contains_any(&text, &["single point of failure", "failover not tested"]) {
                GrayRhinoCandidateState::Expanding
            } else {
                GrayRhinoCandidateState::Visible
            },
            evidence: vec!["Fallback or redundancy gap detected.".to_string()],
            watch_triggers: vec![
                "fallback provider disclosure".to_string(),
                "failover test evidence".to_string(),
                "recovery path update".to_string(),
            ],
            source_title: input.source_title.clone(),
            observed_at: input.observed_at,
            source_published_at: Some(input.observed_at),
            last_confirmed_at: Some(input.observed_at),
            resolved_at: None,
        });
    }

    if contains_any(
        &text,
        &[
            "mega cap concentration",
            "narrow leadership",
            "market breadth deteriorated",
            "liquidity tightened",
            "capex payback risk",
        ],
    ) {
        candidates.push(GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Market,
            kind: GrayRhinoCandidateKind::MarketConcentration,
            subject: "Market".to_string(),
            state: GrayRhinoCandidateState::Visible,
            evidence: vec!["Market-level structural concentration text detected.".to_string()],
            watch_triggers: vec![
                "breadth deterioration".to_string(),
                "liquidity tightening".to_string(),
                "capex payback disappointment".to_string(),
            ],
            source_title: input.source_title.clone(),
            observed_at: input.observed_at,
            source_published_at: Some(input.observed_at),
            last_confirmed_at: Some(input.observed_at),
            resolved_at: None,
        });
    }

    if contains_any(
        &text,
        &[
            "narrative overcrowding",
            "narrative concentration",
            "crowded ai trade",
            "market narrative concentration is high",
        ],
    ) {
        candidates.push(GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Market,
            kind: GrayRhinoCandidateKind::NarrativeCrowding,
            subject: "Market".to_string(),
            state: GrayRhinoCandidateState::Visible,
            evidence: vec!["Narrative crowding detected from source text.".to_string()],
            watch_triggers: vec![
                "headline concentration".to_string(),
                "single-theme leadership".to_string(),
                "positioning reversal".to_string(),
            ],
            source_title: input.source_title.clone(),
            observed_at: input.observed_at,
            source_published_at: Some(input.observed_at),
            last_confirmed_at: Some(input.observed_at),
            resolved_at: None,
        });
    }

    if contains_any(
        &text,
        &[
            "liquidity fragility",
            "liquidity fragility critical",
            "liquidity tightened",
            "liquidity absorption elevated",
            "liquidity absorption critical",
            "rate pressure elevated",
            "rate pressure critical",
            "credit stress watch",
            "credit stress critical",
            "yield curve constraint",
            "yield curve constraint critical",
        ],
    ) {
        candidates.push(GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Market,
            kind: GrayRhinoCandidateKind::LiquidityFragility,
            subject: "Market".to_string(),
            state: if contains_any(
                &text,
                &[
                    "liquidity fragility critical",
                    "liquidity absorption critical",
                    "rate pressure critical",
                    "credit stress critical",
                    "yield curve constraint critical",
                ],
            ) {
                GrayRhinoCandidateState::Critical
            } else if contains_any(
                &text,
                &[
                    "credit stress watch",
                    "rate pressure elevated",
                    "liquidity absorption elevated",
                ],
            ) {
                GrayRhinoCandidateState::Expanding
            } else {
                GrayRhinoCandidateState::Visible
            },
            evidence: vec!["Liquidity or rate-pressure fragility detected.".to_string()],
            watch_triggers: vec![
                "yield curve deterioration".to_string(),
                "credit spread widening".to_string(),
                "central-bank liquidity shift".to_string(),
            ],
            source_title: input.source_title.clone(),
            observed_at: input.observed_at,
            source_published_at: Some(input.observed_at),
            last_confirmed_at: Some(input.observed_at),
            resolved_at: None,
        });
    }

    if contains_any(
        &text,
        &[
            "capex payback risk",
            "capex payback critical",
            "payback disappointment",
            "capital expenditure payback delayed",
            "infrastructure build-out without utilization",
        ],
    ) {
        candidates.push(GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Market,
            kind: GrayRhinoCandidateKind::CapexPaybackFragility,
            subject: "Market".to_string(),
            state: if contains_any(&text, &["capex payback critical"]) {
                GrayRhinoCandidateState::Critical
            } else if contains_any(&text, &["capex payback risk"]) {
                GrayRhinoCandidateState::Expanding
            } else {
                GrayRhinoCandidateState::Visible
            },
            evidence: vec!["Capex payback fragility detected.".to_string()],
            watch_triggers: vec![
                "utilization gap".to_string(),
                "earnings disappointment".to_string(),
                "capex guidance revision".to_string(),
            ],
            source_title: input.source_title.clone(),
            observed_at: input.observed_at,
            source_published_at: Some(input.observed_at),
            last_confirmed_at: Some(input.observed_at),
            resolved_at: None,
        });
    }

    candidates
}

pub fn render_gray_rhino_inline_reference(candidates: &[GrayRhinoCandidate]) -> String {
    if candidates.is_empty() {
        return "Gray Rhino Inline Reference: none auto-discovered.\nBoundary: reference only; no trading, Gate, trend, or market-state mutation.".to_string();
    }
    let mut out = String::from("Gray Rhino Inline Reference (semantic isolation)\n");
    for candidate in candidates {
        out.push_str(&format!(
            "- {} / {:?} / {:?} / {:?}: {}\n",
            candidate.subject,
            candidate.scope,
            candidate.kind,
            candidate.state,
            candidate.evidence.join(" ")
        ));
        if !candidate.watch_triggers.is_empty() {
            out.push_str(&format!(
                "  Trigger watch: {}\n",
                candidate.watch_triggers.join(" / ")
            ));
        }
    }
    out.push_str("Boundary: reference only; no trading, Gate, trend, or market-state mutation.");
    out
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
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
    fn renders_reference_without_signal_terms() {
        let rendered = render_gray_rhino_inline_reference(&[]);

        assert!(rendered.contains("reference only"));
        assert!(!rendered.contains("BUY"));
        assert!(!rendered.contains("SELL"));
        assert!(!rendered.contains("trend_cohesion"));
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

use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateKind, GrayRhinoCandidateScope, GrayRhinoCandidateState,
};
use chrono::NaiveDate;

pub(crate) struct GrayRhinoDiscoveryFacts<'a> {
    pub subject: &'a str,
    pub source_title: &'a str,
    pub observed_at: NaiveDate,
    pub text: &'a str,
}

/// 構造事実から候補種別と lifecycle state を判定する domain policy。
pub(crate) fn discover_gray_rhino_candidates(
    facts: &GrayRhinoDiscoveryFacts<'_>,
) -> Vec<GrayRhinoCandidate> {
    let text = normalize(facts.text);
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
        candidates.push(candidate(
            GrayRhinoCandidateScope::Company,
            GrayRhinoCandidateKind::GovernanceConcentration,
            facts.subject,
            if contains_any(
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
            vec![
                "Founder or single-party voting control detected.",
                "Governance check-and-balance weakness detected.",
            ],
            vec![
                "IPO voting terms",
                "board composition changes",
                "related-party transactions",
                "founder control changes",
            ],
            facts,
        ));
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
        candidates.push(candidate(
            GrayRhinoCandidateScope::Company,
            GrayRhinoCandidateKind::DependencyConcentration,
            facts.subject,
            GrayRhinoCandidateState::Visible,
            vec!["Single dependency or missing fallback detected."],
            vec![
                "supplier disruption",
                "cloud outage",
                "fallback disclosure change",
            ],
            facts,
        ));
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
        candidates.push(candidate(
            GrayRhinoCandidateScope::Company,
            GrayRhinoCandidateKind::InstitutionalMaturityGap,
            facts.subject,
            if contains_any(&text, &["material weakness", "no succession plan"]) {
                GrayRhinoCandidateState::Expanding
            } else {
                GrayRhinoCandidateState::Visible
            },
            vec!["Institutional maturity gap detected."],
            vec![
                "succession disclosure",
                "audit committee remediation",
                "compliance program update",
            ],
            facts,
        ));
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
        candidates.push(candidate(
            GrayRhinoCandidateScope::Company,
            GrayRhinoCandidateKind::RedundancyGap,
            facts.subject,
            if contains_any(&text, &["single point of failure", "failover not tested"]) {
                GrayRhinoCandidateState::Expanding
            } else {
                GrayRhinoCandidateState::Visible
            },
            vec!["Fallback or redundancy gap detected."],
            vec![
                "fallback provider disclosure",
                "failover test evidence",
                "recovery path update",
            ],
            facts,
        ));
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
        candidates.push(candidate(
            GrayRhinoCandidateScope::Market,
            GrayRhinoCandidateKind::MarketConcentration,
            "Market",
            GrayRhinoCandidateState::Visible,
            vec!["Market-level structural concentration text detected."],
            vec![
                "breadth deterioration",
                "liquidity tightening",
                "capex payback disappointment",
            ],
            facts,
        ));
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
        candidates.push(candidate(
            GrayRhinoCandidateScope::Market,
            GrayRhinoCandidateKind::NarrativeCrowding,
            "Market",
            GrayRhinoCandidateState::Visible,
            vec!["Narrative crowding detected from source text."],
            vec![
                "headline concentration",
                "single-theme leadership",
                "positioning reversal",
            ],
            facts,
        ));
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
        candidates.push(candidate(
            GrayRhinoCandidateScope::Market,
            GrayRhinoCandidateKind::LiquidityFragility,
            "Market",
            if contains_any(
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
            vec!["Liquidity or rate-pressure fragility detected."],
            vec![
                "yield curve deterioration",
                "credit spread widening",
                "central-bank liquidity shift",
            ],
            facts,
        ));
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
        candidates.push(candidate(
            GrayRhinoCandidateScope::Market,
            GrayRhinoCandidateKind::CapexPaybackFragility,
            "Market",
            if contains_any(&text, &["capex payback critical"]) {
                GrayRhinoCandidateState::Critical
            } else if contains_any(&text, &["capex payback risk"]) {
                GrayRhinoCandidateState::Expanding
            } else {
                GrayRhinoCandidateState::Visible
            },
            vec!["Capex payback fragility detected."],
            vec![
                "utilization gap",
                "earnings disappointment",
                "capex guidance revision",
            ],
            facts,
        ));
    }

    candidates
}

fn candidate(
    scope: GrayRhinoCandidateScope,
    kind: GrayRhinoCandidateKind,
    subject: &str,
    state: GrayRhinoCandidateState,
    evidence: Vec<&str>,
    watch_triggers: Vec<&str>,
    facts: &GrayRhinoDiscoveryFacts<'_>,
) -> GrayRhinoCandidate {
    GrayRhinoCandidate {
        scope,
        kind,
        subject: subject.to_string(),
        state,
        evidence: evidence.into_iter().map(str::to_string).collect(),
        watch_triggers: watch_triggers.into_iter().map(str::to_string).collect(),
        source_title: facts.source_title.to_string(),
        observed_at: facts.observed_at,
        source_published_at: Some(facts.observed_at),
        last_confirmed_at: Some(facts.observed_at),
        resolved_at: None,
    }
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

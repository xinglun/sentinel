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

    let subject_upper = facts.subject.to_uppercase();
    let is_target = subject_upper == "MARKET"
        || subject_upper == "MSFT"
        || subject_upper == "GOOG"
        || subject_upper == "META"
        || subject_upper == "AMZN"
        || subject_upper == "NVDA";

    if is_target {
        let has_positive_validation = contains_any(
            &text,
            &[
                "validate",
                "validates",
                "margin expansion",
                "strong growth",
                "robust",
                "success",
                "successful",
                "accelerate",
            ],
        ) && !contains_any(
            &text,
            &[
                "no longer",
                "not",
                "fails to",
                "fail to",
                "failed to",
                "disappoint",
                "but",
                "however",
            ],
        );

        let is_capex_payback_triggered = {
            let has_strong_risk = contains_any(
                &text,
                &[
                    "capex payback risk",
                    "capex payback critical",
                    "payback disappointment",
                    "capital expenditure payback delayed",
                    "infrastructure build-out without utilization",
                    "monetization fragility",
                    "capex payback fragility",
                    "fails to connect to revenue growth",
                    "fails to keep pace",
                    "ai investment compresses margin",
                    "margin compression",
                    "profitability under pressure",
                    "management roi caveat",
                    "payback horizon extension",
                    "enterprise adoption remains pilot-stage",
                    "capital spending fails to connect to revenue growth",
                ],
            );

            let has_neutral_concept = contains_any(
                &text,
                &[
                    "ai-related capex",
                    "capex rises significantly",
                    "capex increases significantly",
                    "capex is rising",
                    "capex growth",
                    "monetization timing",
                    "pilot stage",
                    "experiment stage",
                    "experiment-stage",
                ],
            );

            let has_negative_sentiment = contains_any(
                &text,
                &[
                    "pressure",
                    "risk",
                    "concern",
                    "slowing",
                    "weak",
                    "fragile",
                    "delay",
                    "disappoint",
                    "compress",
                ],
            );

            has_strong_risk
                || (has_neutral_concept && has_negative_sentiment && !has_positive_validation)
        };

        let has_forbidden_narrative = contains_any(
            &text,
            &[
                "ai will definitely fail",
                "ai is bound to fail",
                "systemic economic crisis has begun",
                "economic crisis has started",
                "most of us gdp is driven by ai",
                "us gdp is mostly driven by ai",
                "ai monetization failure will cause systemic crisis",
            ],
        );

        if is_capex_payback_triggered && !has_forbidden_narrative {
            let scope = if subject_upper == "MARKET" {
                GrayRhinoCandidateScope::Market
            } else {
                GrayRhinoCandidateScope::Company
            };

            let mut state = GrayRhinoCandidateState::Visible;
            let text_lower = text.to_lowercase();
            let has_critical_concept = contains_any(
                &text,
                &["capex", "payback", "monetization", "margin", "capital"],
            );
            if contains_any(&text, &["capex payback critical"])
                || (text_lower.contains("critical") && has_critical_concept)
            {
                state = GrayRhinoCandidateState::Critical;
            } else if contains_any(
                &text,
                &[
                    "capex payback risk",
                    "expanding",
                    "spreads",
                    "multiple companies",
                ],
            ) {
                state = GrayRhinoCandidateState::Expanding;
            } else if contains_any(&text, &["resolved", "significantly improved"]) {
                state = GrayRhinoCandidateState::Resolved;
            } else if contains_any(&text, &["cooling", "stabilizing"]) {
                state = GrayRhinoCandidateState::Cooling;
            } else if contains_any(&text, &["discussion", "narrative", "sentiment"]) {
                state = GrayRhinoCandidateState::Background;
            } else if contains_any(
                &text,
                &[
                    "visible",
                    "delayed",
                    "fragility",
                    "disappointment",
                    "compression",
                ],
            ) {
                state = GrayRhinoCandidateState::Visible;
            }

            let mut evidence = vec![
                "Capex payback fragility detected.".to_string(),
                "Visible structural risk candidate: AI monetization may be failing to keep pace with capital intensity.".to_string(),
                "Current evidence suggests payback fragility, not confirmed systemic failure.".to_string(),
                "Watch whether revenue realization, margin structure, and adoption breadth can validate current investment levels.".to_string(),
            ];

            if contains_any(&text, &["capex", "capital spending"]) {
                evidence.push(
                    "AI-related capital spending fails to connect to revenue growth".to_string(),
                );
            }
            if contains_any(&text, &["revenue", "returns", "payback"]) {
                evidence.push(
                    "Weakening prospects for returns on AI infrastructure investment".to_string(),
                );
            }
            if contains_any(&text, &["margin", "compress"]) {
                evidence.push("AI investment persistently compresses margin".to_string());
            }

            let watch_triggers = vec![
                "AI revenue growth vs AI capex growth".to_string(),
                "margin compression from AI investment".to_string(),
                "management ROI caveat / payback horizon extension".to_string(),
                "enterprise adoption remains pilot-stage".to_string(),
                "cloud / copilot / ad AI monetization misses prior narrative".to_string(),
            ];

            candidates.push(GrayRhinoCandidate {
                scope,
                kind: GrayRhinoCandidateKind::CapexPaybackFragility,
                subject: facts.subject.to_string(),
                state,
                evidence,
                watch_triggers,
                source_title: facts.source_title.to_string(),
                observed_at: facts.observed_at,
                source_published_at: Some(facts.observed_at),
                last_confirmed_at: Some(facts.observed_at),
                resolved_at: None,
            });
        }
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

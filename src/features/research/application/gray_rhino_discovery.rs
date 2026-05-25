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
            "voting power",
            "controlled",
            "dual class",
            "super voting",
            "ten votes per share",
            "no independent directors",
            "board is controlled",
        ],
    ) && contains_any(
        &text,
        &["founder", "chief executive", "class b", "voting control"],
    ) {
        candidates.push(GrayRhinoCandidate {
            scope: GrayRhinoCandidateScope::Company,
            kind: GrayRhinoCandidateKind::GovernanceConcentration,
            subject: input.subject.clone(),
            state: if contains_any(&text, &["majority voting power", "controls more than 50"]) {
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
}

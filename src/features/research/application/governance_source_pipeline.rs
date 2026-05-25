use crate::features::research::application::governance_evidence::{
    ingest_governance_concentration_evidence, GovernanceEvidenceRepository,
};
use crate::features::research::domain::governance_source::{
    GovernanceSourceDocument, GovernanceSourceKind,
};
use crate::features::research::domain::gray_rhino_evidence::{
    GovernanceConcentrationEvidence, GovernanceConcentrationMetrics, GrayRhinoEvidenceSourceType,
    GrayRhinoSourceReference,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceSourceCollectionRequest {
    pub symbol: Option<String>,
    pub local_file: Option<String>,
    pub observed_at: NaiveDate,
    pub retrieved_at: NaiveDate,
    pub lookback_days: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceEvidenceRejectionDetail {
    pub source_title: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceSourceCollectionSummary {
    pub source_count: usize,
    pub accepted_count: usize,
    pub saved_count: usize,
    pub rejected: Vec<GovernanceEvidenceRejectionDetail>,
    pub latest_observed_at: Option<NaiveDate>,
}

#[async_trait]
pub trait GovernanceSourceAdapter {
    async fn fetch_governance_sources(
        &self,
        request: &GovernanceSourceCollectionRequest,
    ) -> Result<Vec<GovernanceSourceDocument>>;
}

pub async fn collect_governance_concentration_sources(
    adapter: &dyn GovernanceSourceAdapter,
    repository: &dyn GovernanceEvidenceRepository,
    request: GovernanceSourceCollectionRequest,
) -> Result<GovernanceSourceCollectionSummary> {
    let documents = adapter.fetch_governance_sources(&request).await?;
    let mut accepted_count = 0;
    let mut saved_count = 0;
    let mut rejected = Vec::new();
    let mut latest_observed_at = None;

    for document in &documents {
        latest_observed_at = Some(
            latest_observed_at
                .map(|latest: NaiveDate| latest.max(document.observed_at))
                .unwrap_or(document.observed_at),
        );
        match extract_governance_concentration_evidence(document)
            .and_then(|evidence| ingest_governance_concentration_evidence(repository, evidence))
        {
            Ok(outcome) => {
                accepted_count += 1;
                if outcome.saved {
                    saved_count += 1;
                }
            }
            Err(err) => rejected.push(GovernanceEvidenceRejectionDetail {
                source_title: document.source_title.clone(),
                reason: err.to_string(),
            }),
        }
    }

    Ok(GovernanceSourceCollectionSummary {
        source_count: documents.len(),
        accepted_count,
        saved_count,
        rejected,
        latest_observed_at,
    })
}

pub fn extract_governance_concentration_evidence(
    document: &GovernanceSourceDocument,
) -> Result<GovernanceConcentrationEvidence> {
    document
        .validate()
        .map_err(|err| anyhow!("Invalid governance source document: {:?}", err))?;
    let metrics = GovernanceConcentrationMetrics {
        founder_voting_power: parse_percent_metric(
            &document.content,
            &[
                "founder_voting_power",
                "founder voting power",
                "founder voting control",
            ],
        ),
        independent_board_ratio: parse_ratio_metric(
            &document.content,
            &[
                "independent_board_ratio",
                "independent board ratio",
                "board independence ratio",
            ],
        ),
        dual_class_structure: parse_bool_metric(
            &document.content,
            &[
                "dual_class_structure",
                "dual class structure",
                "dual class shares",
            ],
        ),
        super_voting_rights: parse_bool_metric(
            &document.content,
            &[
                "super_voting_rights",
                "super voting rights",
                "super-voting rights",
            ],
        ),
        succession_disclosure: parse_bool_metric(
            &document.content,
            &[
                "succession_disclosure",
                "succession disclosure",
                "succession plan",
            ],
        ),
    };
    let evidence = GovernanceConcentrationEvidence {
        subject: document.subject.clone(),
        source: GrayRhinoSourceReference {
            source_type: match document.source_kind {
                GovernanceSourceKind::SecFiling => GrayRhinoEvidenceSourceType::RegulatoryFiling,
                GovernanceSourceKind::LocalGovernanceDocument => {
                    GrayRhinoEvidenceSourceType::GovernanceDocument
                }
            },
            source_title: document.source_title.clone(),
            publisher: document.publisher.clone(),
            source_url: document.source_url.clone(),
            repository_path: document.repository_path.clone(),
            observed_at: document.observed_at,
            retrieved_at: document.retrieved_at,
        },
        confidence: 0.9,
        extraction_note: "Deterministic governance source adapter extracted structured metrics."
            .to_string(),
        structural_fact: "Governance source discloses machine-readable concentration metrics."
            .to_string(),
        metrics,
    };
    evidence
        .validate()
        .map_err(|err| anyhow!("Invalid extracted governance evidence: {:?}", err))?;
    Ok(evidence)
}

fn parse_percent_metric(content: &str, labels: &[&str]) -> Option<f64> {
    parse_number_after_labels(content, labels).map(|value| {
        if value <= 1.0 && content.contains('%') {
            value * 100.0
        } else {
            value
        }
    })
}

fn parse_ratio_metric(content: &str, labels: &[&str]) -> Option<f64> {
    parse_number_after_labels(content, labels)
        .map(|value| if value > 1.0 { value / 100.0 } else { value })
}

fn parse_number_after_labels(content: &str, labels: &[&str]) -> Option<f64> {
    let lower = content.to_lowercase();
    for label in labels {
        if let Some(start) = lower.find(&label.to_lowercase()) {
            let tail = &content[start + label.len()..];
            if let Some(value) = first_number(tail) {
                return Some(value);
            }
        }
    }
    None
}

fn first_number(value: &str) -> Option<f64> {
    let mut raw = String::new();
    let mut started = false;
    for ch in value.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            raw.push(ch);
            started = true;
        } else if started {
            break;
        }
    }
    raw.parse::<f64>().ok()
}

fn parse_bool_metric(content: &str, labels: &[&str]) -> Option<bool> {
    let lower = content.to_lowercase();
    for label in labels {
        let normalized_label = label.to_lowercase();
        if let Some(start) = lower.find(&normalized_label) {
            let tail = lower[start + normalized_label.len()..]
                .split([';', '\n', '\r'])
                .next()
                .unwrap_or("");
            if tail.contains("false") || tail.contains("no") || tail.contains("not disclosed") {
                return Some(false);
            }
            if tail.contains("true") || tail.contains("yes") || tail.contains("disclosed") {
                return Some(true);
            }
            return Some(true);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::governance_source::GovernanceSourceKind;

    fn document(content: &str) -> GovernanceSourceDocument {
        GovernanceSourceDocument {
            subject: "Example issuer".to_string(),
            source_kind: GovernanceSourceKind::LocalGovernanceDocument,
            source_title: "Proxy statement".to_string(),
            publisher: "Example issuer".to_string(),
            source_url: Some("https://example.com/proxy".to_string()),
            repository_path: Some("gray_rhino_sources/governance/proxy.html".to_string()),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            content: content.to_string(),
        }
    }

    #[test]
    fn extracts_structured_governance_metrics_without_narrative() {
        let evidence = extract_governance_concentration_evidence(&document(
            "founder_voting_power: 61.2%; independent_board_ratio: 0.42; dual_class_structure: true; super_voting_rights: yes; succession_disclosure: false",
        ))
        .unwrap();

        assert_eq!(evidence.metrics.founder_voting_power, Some(61.2));
        assert_eq!(evidence.metrics.independent_board_ratio, Some(0.42));
        assert_eq!(evidence.metrics.dual_class_structure, Some(true));
        assert_eq!(evidence.metrics.super_voting_rights, Some(true));
        assert_eq!(evidence.metrics.succession_disclosure, Some(false));
    }

    #[test]
    fn rejects_source_without_governance_metrics() {
        let err = extract_governance_concentration_evidence(&document(
            "This document contains only generic governance prose.",
        ))
        .unwrap_err();

        assert!(err.to_string().contains("MissingGovernanceMetric"));
    }
}

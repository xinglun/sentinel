use crate::features::research::application::governance_evidence::{
    ingest_governance_concentration_evidence, GovernanceEvidenceRepository,
};
use crate::features::research::domain::governance_source::{
    GovernanceExtractionAuditRecord, GovernanceMetricAuditEntry, GovernanceMetricAuditStatus,
    GovernanceReplayRejectionKind, GovernanceSourceDocument, GovernanceSourceKind,
    GovernanceSourceManifest,
};
use crate::features::research::domain::gray_rhino_evidence::{
    GovernanceConcentrationEvidence, GovernanceConcentrationMetrics, GrayRhinoEvidenceSourceType,
    GrayRhinoSourceReference,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use sha2::{Digest, Sha256};

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

#[derive(Debug, Clone, PartialEq)]
pub struct GovernanceSourceCollectionSummary {
    pub source_count: usize,
    pub accepted_count: usize,
    pub saved_count: usize,
    pub rejected: Vec<GovernanceEvidenceRejectionDetail>,
    pub latest_observed_at: Option<NaiveDate>,
    pub manifest_count: usize,
    pub audit_count: usize,
    pub coverage_ratio: f64,
}

#[async_trait]
pub trait GovernanceSourceAdapter {
    async fn fetch_governance_sources(
        &self,
        request: &GovernanceSourceCollectionRequest,
    ) -> Result<Vec<GovernanceSourceDocument>>;
}

/// Governance source の manifest / audit ledger 永続化 port。
pub trait GovernanceSourceAuditRepository {
    fn save_governance_source_manifest(&self, manifest: &GovernanceSourceManifest) -> Result<bool>;
    fn save_governance_extraction_audit(
        &self,
        record: &GovernanceExtractionAuditRecord,
    ) -> Result<bool>;
    fn load_governance_extraction_audits(&self) -> Result<Vec<GovernanceExtractionAuditRecord>>;
}

pub trait GovernanceEvidenceAuditRepository:
    GovernanceEvidenceRepository + GovernanceSourceAuditRepository
{
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct GovernanceFieldCoverage {
    pub metric: String,
    pub extracted_count: usize,
    pub missing_count: usize,
    pub invalid_count: usize,
    pub coverage_ratio: f64,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct GovernanceReplayCoverageReport {
    pub source_count: usize,
    pub accepted_count: usize,
    pub rejected_count: usize,
    pub metric_coverage: Vec<GovernanceFieldCoverage>,
}

impl<T> GovernanceEvidenceAuditRepository for T where
    T: GovernanceEvidenceRepository + GovernanceSourceAuditRepository
{
}

pub async fn collect_governance_concentration_sources(
    adapter: &dyn GovernanceSourceAdapter,
    repository: &dyn GovernanceEvidenceAuditRepository,
    request: GovernanceSourceCollectionRequest,
) -> Result<GovernanceSourceCollectionSummary> {
    let documents = adapter.fetch_governance_sources(&request).await?;
    let mut accepted_count = 0;
    let mut saved_count = 0;
    let mut rejected = Vec::new();
    let mut latest_observed_at = None;
    let mut manifest_count = 0;
    let mut audit_count = 0;

    for document in &documents {
        latest_observed_at = Some(
            latest_observed_at
                .map(|latest: NaiveDate| latest.max(document.observed_at))
                .unwrap_or(document.observed_at),
        );
        if repository
            .save_governance_source_manifest(&build_governance_source_manifest(document))
            .is_ok_and(|saved| saved)
        {
            manifest_count += 1;
        }

        let audit = build_governance_extraction_audit(document, None);
        match extract_governance_concentration_evidence(document)
            .and_then(|evidence| ingest_governance_concentration_evidence(repository, evidence))
        {
            Ok(outcome) => {
                accepted_count += 1;
                if outcome.saved {
                    saved_count += 1;
                }
                if repository
                    .save_governance_extraction_audit(&GovernanceExtractionAuditRecord {
                        accepted: true,
                        rejection_reason: None,
                        ..audit
                    })
                    .is_ok_and(|saved| saved)
                {
                    audit_count += 1;
                }
            }
            Err(err) => {
                let reason = err.to_string();
                if repository
                    .save_governance_extraction_audit(&GovernanceExtractionAuditRecord {
                        accepted: false,
                        rejection_reason: Some(reason.clone()),
                        ..audit
                    })
                    .is_ok_and(|saved| saved)
                {
                    audit_count += 1;
                }
                rejected.push(GovernanceEvidenceRejectionDetail {
                    source_title: document.source_title.clone(),
                    reason,
                });
            }
        }
    }
    let coverage_ratio = if documents.is_empty() {
        0.0
    } else {
        accepted_count as f64 / documents.len() as f64
    };

    Ok(GovernanceSourceCollectionSummary {
        source_count: documents.len(),
        accepted_count,
        saved_count,
        rejected,
        latest_observed_at,
        manifest_count,
        audit_count,
        coverage_ratio,
    })
}

pub fn build_governance_source_manifest(
    document: &GovernanceSourceDocument,
) -> GovernanceSourceManifest {
    GovernanceSourceManifest {
        subject: document.subject.clone(),
        source_kind: document.source_kind,
        source_title: document.source_title.clone(),
        publisher: document.publisher.clone(),
        source_url: document.source_url.clone(),
        repository_path: document.repository_path.clone(),
        observed_at: document.observed_at,
        retrieved_at: document.retrieved_at,
        content_sha256: sha256_hex(&document.content),
    }
}

pub fn build_governance_extraction_audit(
    document: &GovernanceSourceDocument,
    rejection_reason: Option<String>,
) -> GovernanceExtractionAuditRecord {
    let metrics = extract_metric_audit_entries(&document.content);
    let accepted = metrics
        .iter()
        .any(|metric| metric.status == GovernanceMetricAuditStatus::Extracted);
    GovernanceExtractionAuditRecord {
        subject: document.subject.clone(),
        source_title: document.source_title.clone(),
        observed_at: document.observed_at,
        retrieved_at: document.retrieved_at,
        metrics,
        accepted,
        rejection_reason,
    }
}

#[allow(dead_code)]
pub fn build_governance_field_coverage_report(
    audits: &[GovernanceExtractionAuditRecord],
) -> GovernanceReplayCoverageReport {
    let metric_names = [
        "founder_voting_power",
        "independent_board_ratio",
        "dual_class_structure",
        "super_voting_rights",
        "succession_disclosure",
    ];
    let metric_coverage = metric_names
        .iter()
        .map(|metric| {
            let mut extracted_count = 0;
            let mut missing_count = 0;
            let mut invalid_count = 0;
            for audit in audits {
                if let Some(entry) = audit.metrics.iter().find(|entry| entry.metric == *metric) {
                    match entry.status {
                        GovernanceMetricAuditStatus::Extracted => extracted_count += 1,
                        GovernanceMetricAuditStatus::Missing => missing_count += 1,
                        GovernanceMetricAuditStatus::Invalid => invalid_count += 1,
                    }
                } else {
                    missing_count += 1;
                }
            }
            let coverage_ratio = if audits.is_empty() {
                0.0
            } else {
                extracted_count as f64 / audits.len() as f64
            };
            GovernanceFieldCoverage {
                metric: (*metric).to_string(),
                extracted_count,
                missing_count,
                invalid_count,
                coverage_ratio,
            }
        })
        .collect();

    GovernanceReplayCoverageReport {
        source_count: audits.len(),
        accepted_count: audits.iter().filter(|audit| audit.accepted).count(),
        rejected_count: audits.iter().filter(|audit| !audit.accepted).count(),
        metric_coverage,
    }
}

#[allow(dead_code)]
pub fn classify_governance_rejection(reason: &str) -> GovernanceReplayRejectionKind {
    if reason.contains("MissingGovernanceMetric") {
        GovernanceReplayRejectionKind::MetriclessSource
    } else if reason.contains("Invalid governance source document") {
        GovernanceReplayRejectionKind::SourceInvalid
    } else {
        GovernanceReplayRejectionKind::ExtractionInvalid
    }
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

fn extract_metric_audit_entries(content: &str) -> Vec<GovernanceMetricAuditEntry> {
    vec![
        number_metric_audit(
            "founder_voting_power",
            parse_percent_metric(
                content,
                &[
                    "founder_voting_power",
                    "founder voting power",
                    "founder voting control",
                ],
            ),
        ),
        number_metric_audit(
            "independent_board_ratio",
            parse_ratio_metric(
                content,
                &[
                    "independent_board_ratio",
                    "independent board ratio",
                    "board independence ratio",
                ],
            ),
        ),
        bool_metric_audit(
            "dual_class_structure",
            parse_bool_metric(
                content,
                &[
                    "dual_class_structure",
                    "dual class structure",
                    "dual class shares",
                ],
            ),
        ),
        bool_metric_audit(
            "super_voting_rights",
            parse_bool_metric(
                content,
                &[
                    "super_voting_rights",
                    "super voting rights",
                    "super-voting rights",
                ],
            ),
        ),
        bool_metric_audit(
            "succession_disclosure",
            parse_bool_metric(
                content,
                &[
                    "succession_disclosure",
                    "succession disclosure",
                    "succession plan",
                ],
            ),
        ),
    ]
}

fn number_metric_audit(metric: &str, value: Option<f64>) -> GovernanceMetricAuditEntry {
    GovernanceMetricAuditEntry {
        metric: metric.to_string(),
        status: value
            .map(|_| GovernanceMetricAuditStatus::Extracted)
            .unwrap_or(GovernanceMetricAuditStatus::Missing),
        value: value.map(|value| value.to_string()),
        reason: value.is_none().then(|| "metric not found".to_string()),
    }
}

fn bool_metric_audit(metric: &str, value: Option<bool>) -> GovernanceMetricAuditEntry {
    GovernanceMetricAuditEntry {
        metric: metric.to_string(),
        status: value
            .map(|_| GovernanceMetricAuditStatus::Extracted)
            .unwrap_or(GovernanceMetricAuditStatus::Missing),
        value: value.map(|value| value.to_string()),
        reason: value.is_none().then(|| "metric not found".to_string()),
    }
}

fn sha256_hex(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
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

    #[test]
    fn manifest_records_replayable_source_hash() {
        let manifest = build_governance_source_manifest(&document("founder_voting_power: 61.2%"));

        assert_eq!(manifest.subject, "Example issuer");
        assert_eq!(manifest.content_sha256.len(), 64);
    }

    #[test]
    fn audit_records_metric_level_extraction_status() {
        let audit = build_governance_extraction_audit(
            &document("founder_voting_power: 61.2%; dual_class_structure: true"),
            None,
        );

        assert!(audit
            .metrics
            .iter()
            .any(|metric| metric.metric == "founder_voting_power"
                && metric.status == GovernanceMetricAuditStatus::Extracted));
        assert!(audit
            .metrics
            .iter()
            .any(|metric| metric.metric == "succession_disclosure"
                && metric.status == GovernanceMetricAuditStatus::Missing));
    }

    #[test]
    fn coverage_report_summarizes_fixture_replay_quality() {
        let accepted = build_governance_extraction_audit(
            &document("founder_voting_power: 61.2%; dual_class_structure: true"),
            None,
        );
        let rejected = GovernanceExtractionAuditRecord {
            accepted: false,
            rejection_reason: Some(
                "Invalid extracted governance evidence: MissingGovernanceMetric".to_string(),
            ),
            ..build_governance_extraction_audit(&document("generic governance prose only"), None)
        };

        let report = build_governance_field_coverage_report(&[accepted, rejected]);

        assert_eq!(report.source_count, 2);
        assert_eq!(report.accepted_count, 1);
        assert!(report.metric_coverage.iter().any(|metric| {
            metric.metric == "founder_voting_power"
                && metric.extracted_count == 1
                && metric.coverage_ratio == 0.5
        }));
    }

    #[test]
    fn rejection_taxonomy_distinguishes_metricless_sources() {
        assert_eq!(
            classify_governance_rejection(
                "Invalid extracted governance evidence: MissingGovernanceMetric"
            ),
            GovernanceReplayRejectionKind::MetriclessSource
        );
    }
}

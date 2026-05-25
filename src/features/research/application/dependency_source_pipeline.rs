use crate::features::research::application::dependency_evidence::{
    ingest_dependency_concentration_evidence, DependencyEvidenceRepository,
};
use crate::features::research::domain::dependency_source::{
    DependencyExtractionAuditRecord, DependencyMetricAuditEntry, DependencyMetricAuditStatus,
    DependencyReplayRejectionKind, DependencySourceDocument, DependencySourceManifest,
};
use crate::features::research::domain::gray_rhino_evidence::{
    DependencyConcentrationEvidence, DependencyConcentrationKind, DependencyConcentrationMetrics,
    GrayRhinoEvidenceSourceType, GrayRhinoSourceReference,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::NaiveDate;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySourceCollectionRequest {
    pub symbol: Option<String>,
    pub local_file: Option<String>,
    pub observed_at: NaiveDate,
    pub retrieved_at: NaiveDate,
    pub persist_evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyEvidenceRejectionDetail {
    pub subject: String,
    pub source_title: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DependencyFieldCoverage {
    pub metric: String,
    pub extracted_count: usize,
    pub missing_count: usize,
    pub coverage_ratio: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DependencySourceCollectionSummary {
    pub source_count: usize,
    pub accepted_count: usize,
    pub saved_count: usize,
    pub manifest_count: usize,
    pub audit_count: usize,
    pub rejected: Vec<DependencyEvidenceRejectionDetail>,
    pub latest_observed_at: Option<NaiveDate>,
    pub metric_coverage: Vec<DependencyFieldCoverage>,
}

#[async_trait]
pub trait DependencySourceAdapter {
    async fn fetch_dependency_sources(
        &self,
        request: &DependencySourceCollectionRequest,
    ) -> Result<Vec<DependencySourceDocument>>;
}

pub trait DependencySourceAuditRepository {
    fn save_dependency_source_manifest(&self, manifest: &DependencySourceManifest) -> Result<bool>;
    fn save_dependency_extraction_audit(
        &self,
        record: &DependencyExtractionAuditRecord,
    ) -> Result<bool>;
}

pub trait DependencyEvidenceAuditRepository:
    DependencyEvidenceRepository + DependencySourceAuditRepository
{
}

impl<T> DependencyEvidenceAuditRepository for T where
    T: DependencyEvidenceRepository + DependencySourceAuditRepository
{
}

pub async fn collect_dependency_concentration_sources(
    adapter: &dyn DependencySourceAdapter,
    repository: &dyn DependencyEvidenceAuditRepository,
    request: DependencySourceCollectionRequest,
) -> Result<DependencySourceCollectionSummary> {
    let documents = adapter.fetch_dependency_sources(&request).await?;
    let mut accepted_count = 0;
    let mut saved_count = 0;
    let mut manifest_count = 0;
    let mut audit_count = 0;
    let mut rejected = Vec::new();
    let mut audits = Vec::new();
    let mut latest_observed_at = None;

    for document in documents {
        let manifest = build_dependency_source_manifest(&document);
        if repository.save_dependency_source_manifest(&manifest)? {
            manifest_count += 1;
        }
        let extraction = extract_dependency_concentration_evidence(&document);
        let audit = build_dependency_extraction_audit(&document, extraction.as_ref().err());
        if repository.save_dependency_extraction_audit(&audit)? {
            audit_count += 1;
        }
        audits.push(audit);
        latest_observed_at = latest_observed_at
            .map(|latest: NaiveDate| latest.max(document.observed_at))
            .or(Some(document.observed_at));
        match extraction {
            Ok(evidence) => {
                accepted_count += 1;
                if request.persist_evidence
                    && ingest_dependency_concentration_evidence(repository, evidence)?.saved
                {
                    saved_count += 1;
                }
            }
            Err(err) => rejected.push(DependencyEvidenceRejectionDetail {
                subject: document.subject.clone(),
                source_title: document.source_title.clone(),
                reason: err.to_string(),
            }),
        }
    }

    Ok(DependencySourceCollectionSummary {
        source_count: audits.len(),
        accepted_count,
        saved_count,
        manifest_count,
        audit_count,
        rejected,
        latest_observed_at,
        metric_coverage: build_dependency_field_coverage(&audits),
    })
}

pub fn extract_dependency_concentration_evidence(
    document: &DependencySourceDocument,
) -> Result<DependencyConcentrationEvidence> {
    document
        .validate()
        .map_err(|err| anyhow!("Invalid dependency source document: {:?}", err))?;
    let text = normalize_dependency_text(&document.content);
    let metrics = DependencyConcentrationMetrics {
        dependency_kind: parse_dependency_kind(&text)
            .unwrap_or(DependencyConcentrationKind::Supplier),
        dependency_name: parse_dependency_name(&text).unwrap_or_else(|| document.subject.clone()),
        concentration_ratio: parse_ratio_metric(
            &text,
            &["concentration_ratio", "dependency ratio"],
        ),
        single_point_of_failure: parse_bool_metric(
            &text,
            &["single_point_of_failure", "single point of failure"],
        ),
        fallback_disclosed: parse_bool_metric(
            &text,
            &[
                "fallback_disclosed",
                "fallback disclosed",
                "fallback available",
            ],
        ),
    };
    let evidence = DependencyConcentrationEvidence {
        subject: document.subject.clone(),
        source: GrayRhinoSourceReference {
            source_type: GrayRhinoEvidenceSourceType::OperatorCuratedSource,
            source_title: document.source_title.clone(),
            publisher: document.publisher.clone(),
            source_url: document.source_url.clone(),
            repository_path: document.repository_path.clone(),
            observed_at: document.observed_at,
            retrieved_at: document.retrieved_at,
        },
        confidence: 0.84,
        extraction_note: "Deterministic dependency source adapter extracted structured metrics."
            .to_string(),
        structural_fact: "Dependency source discloses machine-readable concentration metrics."
            .to_string(),
        metrics,
    };
    evidence
        .validate()
        .map_err(|err| anyhow!("Invalid extracted dependency evidence: {:?}", err))?;
    Ok(evidence)
}

#[allow(dead_code)]
pub fn classify_dependency_rejection(reason: &str) -> DependencyReplayRejectionKind {
    if reason.contains("MissingDependencyMetric") {
        DependencyReplayRejectionKind::MetriclessSource
    } else if reason.contains("Invalid dependency source document") {
        DependencyReplayRejectionKind::SourceInvalid
    } else {
        DependencyReplayRejectionKind::ExtractionInvalid
    }
}

fn build_dependency_source_manifest(
    document: &DependencySourceDocument,
) -> DependencySourceManifest {
    DependencySourceManifest {
        subject: document.subject.clone(),
        source_kind: document.source_kind,
        source_title: document.source_title.clone(),
        publisher: document.publisher.clone(),
        source_url: document.source_url.clone(),
        repository_path: document.repository_path.clone(),
        observed_at: document.observed_at,
        retrieved_at: document.retrieved_at,
        content_sha256: format!("{:x}", Sha256::digest(document.content.as_bytes())),
    }
}

fn build_dependency_extraction_audit(
    document: &DependencySourceDocument,
    rejection: Option<&anyhow::Error>,
) -> DependencyExtractionAuditRecord {
    let text = normalize_dependency_text(&document.content);
    DependencyExtractionAuditRecord {
        subject: document.subject.clone(),
        source_title: document.source_title.clone(),
        observed_at: document.observed_at,
        retrieved_at: document.retrieved_at,
        metrics: vec![
            string_metric_audit(
                "dependency_kind",
                parse_dependency_kind(&text).map(|v| format!("{:?}", v)),
            ),
            string_metric_audit("dependency_name", parse_dependency_name(&text)),
            number_metric_audit(
                "concentration_ratio",
                parse_ratio_metric(&text, &["concentration_ratio", "dependency ratio"]),
            ),
            bool_metric_audit(
                "single_point_of_failure",
                parse_bool_metric(
                    &text,
                    &["single_point_of_failure", "single point of failure"],
                ),
            ),
            bool_metric_audit(
                "fallback_disclosed",
                parse_bool_metric(
                    &text,
                    &[
                        "fallback_disclosed",
                        "fallback disclosed",
                        "fallback available",
                    ],
                ),
            ),
        ],
        accepted: rejection.is_none(),
        rejection_reason: rejection.map(|err| err.to_string()),
    }
}

fn build_dependency_field_coverage(
    audits: &[DependencyExtractionAuditRecord],
) -> Vec<DependencyFieldCoverage> {
    let metrics = [
        "dependency_kind",
        "dependency_name",
        "concentration_ratio",
        "single_point_of_failure",
        "fallback_disclosed",
    ];
    metrics
        .iter()
        .map(|metric| {
            let extracted_count = audits
                .iter()
                .flat_map(|audit| &audit.metrics)
                .filter(|entry| {
                    entry.metric == *metric
                        && entry.status == DependencyMetricAuditStatus::Extracted
                })
                .count();
            let missing_count = audits.len().saturating_sub(extracted_count);
            DependencyFieldCoverage {
                metric: (*metric).to_string(),
                extracted_count,
                missing_count,
                coverage_ratio: if audits.is_empty() {
                    0.0
                } else {
                    extracted_count as f64 / audits.len() as f64
                },
            }
        })
        .collect()
}

fn string_metric_audit(metric: &str, value: Option<String>) -> DependencyMetricAuditEntry {
    DependencyMetricAuditEntry {
        metric: metric.to_string(),
        status: value
            .as_ref()
            .map(|_| DependencyMetricAuditStatus::Extracted)
            .unwrap_or(DependencyMetricAuditStatus::Missing),
        value,
        reason: None,
    }
}

fn number_metric_audit(metric: &str, value: Option<f64>) -> DependencyMetricAuditEntry {
    string_metric_audit(metric, value.map(|value| value.to_string()))
}

fn bool_metric_audit(metric: &str, value: Option<bool>) -> DependencyMetricAuditEntry {
    string_metric_audit(metric, value.map(|value| value.to_string()))
}

fn parse_dependency_kind(text: &str) -> Option<DependencyConcentrationKind> {
    parse_label_value(text, &["dependency_kind", "dependency kind"]).and_then(|value| {
        match value.trim().to_lowercase().as_str() {
            "infrastructure" => Some(DependencyConcentrationKind::Infrastructure),
            "compute" => Some(DependencyConcentrationKind::Compute),
            "cloud" => Some(DependencyConcentrationKind::Cloud),
            "launch" => Some(DependencyConcentrationKind::Launch),
            "supplier" => Some(DependencyConcentrationKind::Supplier),
            "ecosystem" => Some(DependencyConcentrationKind::Ecosystem),
            _ => None,
        }
    })
}

fn parse_dependency_name(text: &str) -> Option<String> {
    parse_label_value(
        text,
        &[
            "dependency_name",
            "dependency name",
            "supplier",
            "cloud provider",
        ],
    )
    .filter(|value| !value.trim().is_empty())
}

fn parse_ratio_metric(text: &str, labels: &[&str]) -> Option<f64> {
    parse_label_value(text, labels).and_then(|value| {
        let number = first_number(&value)?;
        Some(if number > 1.0 { number / 100.0 } else { number })
    })
}

fn parse_bool_metric(text: &str, labels: &[&str]) -> Option<bool> {
    parse_label_value(text, labels).and_then(|value| {
        let lower = value.to_lowercase();
        if lower.contains("true") || lower.contains("yes") || lower.contains("disclosed") {
            Some(true)
        } else if lower.contains("false") || lower.contains("no") || lower.contains("not disclosed")
        {
            Some(false)
        } else {
            None
        }
    })
}

fn parse_label_value(text: &str, labels: &[&str]) -> Option<String> {
    for label in labels {
        if let Some(start) = text.find(label) {
            let tail = &text[start + label.len()..];
            let tail = tail.trim_start_matches([':', '=', ' ', '-']);
            let value = tail
                .split([';', '\n', '\r'])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !value.is_empty() {
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

fn normalize_dependency_text(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::dependency_source::DependencySourceKind;
    use std::cell::RefCell;

    #[derive(Default)]
    struct InMemoryRepository {
        evidence: RefCell<
            Vec<crate::features::research::domain::gray_rhino_evidence::GrayRhinoEvidenceRecord>,
        >,
        manifests: RefCell<Vec<DependencySourceManifest>>,
        audits: RefCell<Vec<DependencyExtractionAuditRecord>>,
    }

    impl DependencyEvidenceRepository for InMemoryRepository {
        fn save_dependency_evidence(
            &self,
            record: &crate::features::research::domain::gray_rhino_evidence::GrayRhinoEvidenceRecord,
        ) -> Result<bool> {
            self.evidence.borrow_mut().push(record.clone());
            Ok(true)
        }

        fn load_dependency_evidence(
            &self,
        ) -> Result<
            Vec<crate::features::research::domain::gray_rhino_evidence::GrayRhinoEvidenceRecord>,
        > {
            Ok(self.evidence.borrow().clone())
        }
    }

    impl DependencySourceAuditRepository for InMemoryRepository {
        fn save_dependency_source_manifest(
            &self,
            manifest: &DependencySourceManifest,
        ) -> Result<bool> {
            self.manifests.borrow_mut().push(manifest.clone());
            Ok(true)
        }

        fn save_dependency_extraction_audit(
            &self,
            record: &DependencyExtractionAuditRecord,
        ) -> Result<bool> {
            self.audits.borrow_mut().push(record.clone());
            Ok(true)
        }
    }

    fn document(content: &str) -> DependencySourceDocument {
        DependencySourceDocument {
            subject: "Example issuer".to_string(),
            source_kind: DependencySourceKind::LocalDependencyDocument,
            source_title: "Dependency disclosure".to_string(),
            publisher: "Example issuer".to_string(),
            source_url: Some("https://example.com/dependency".to_string()),
            repository_path: None,
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            content: content.to_string(),
        }
    }

    #[test]
    fn extracts_dependency_metrics_from_structured_source() {
        let evidence = extract_dependency_concentration_evidence(&document(
            "dependency_kind: Supplier; dependency_name: Example supplier; concentration_ratio: 0.70; single_point_of_failure: true; fallback_disclosed: false",
        ))
        .unwrap();

        assert_eq!(
            evidence.metrics.dependency_kind,
            DependencyConcentrationKind::Supplier
        );
        assert_eq!(evidence.metrics.concentration_ratio, Some(0.70));
        assert_eq!(evidence.metrics.single_point_of_failure, Some(true));
        assert_eq!(evidence.metrics.fallback_disclosed, Some(false));
    }

    #[test]
    fn rejects_metricless_dependency_source() {
        let err = extract_dependency_concentration_evidence(&document(
            "This is generic supplier narrative without structured dependency metrics.",
        ))
        .unwrap_err();

        assert!(err.to_string().contains("MissingDependencyMetric"));
    }

    #[test]
    fn rejection_taxonomy_distinguishes_metricless_dependency_sources() {
        assert_eq!(
            classify_dependency_rejection(
                "Invalid extracted dependency evidence: MissingDependencyMetric"
            ),
            DependencyReplayRejectionKind::MetriclessSource
        );
    }

    #[tokio::test]
    async fn collection_reports_coverage_and_rejections() {
        struct StaticAdapter;

        #[async_trait]
        impl DependencySourceAdapter for StaticAdapter {
            async fn fetch_dependency_sources(
                &self,
                _request: &DependencySourceCollectionRequest,
            ) -> Result<Vec<DependencySourceDocument>> {
                Ok(vec![
                    document("dependency_kind: Supplier; dependency_name: Supplier A; concentration_ratio: 0.7"),
                    document("generic dependency narrative"),
                ])
            }
        }

        let repository = InMemoryRepository::default();
        let summary = collect_dependency_concentration_sources(
            &StaticAdapter,
            &repository,
            DependencySourceCollectionRequest {
                symbol: Some("EXAMPLE".to_string()),
                local_file: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                persist_evidence: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(summary.source_count, 2);
        assert_eq!(summary.accepted_count, 1);
        assert_eq!(summary.rejected.len(), 1);
        assert!(summary.metric_coverage.iter().any(|metric| {
            metric.metric == "concentration_ratio" && metric.extracted_count == 1
        }));
    }
}

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencySourceKind {
    LocalDependencyDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencySourceDocument {
    pub subject: String,
    pub source_kind: DependencySourceKind,
    pub source_title: String,
    pub publisher: String,
    pub source_url: Option<String>,
    pub repository_path: Option<String>,
    pub observed_at: NaiveDate,
    pub retrieved_at: NaiveDate,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencySourceManifest {
    pub subject: String,
    pub source_kind: DependencySourceKind,
    pub source_title: String,
    pub publisher: String,
    pub source_url: Option<String>,
    pub repository_path: Option<String>,
    pub observed_at: NaiveDate,
    pub retrieved_at: NaiveDate,
    pub content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyMetricAuditStatus {
    Extracted,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyMetricAuditEntry {
    pub metric: String,
    pub status: DependencyMetricAuditStatus,
    pub value: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyExtractionAuditRecord {
    pub subject: String,
    pub source_title: String,
    pub observed_at: NaiveDate,
    pub retrieved_at: NaiveDate,
    pub metrics: Vec<DependencyMetricAuditEntry>,
    pub accepted: bool,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyReplayRejectionKind {
    MetriclessSource,
    SourceInvalid,
    ExtractionInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySourceValidationError {
    MissingSubject,
    MissingTitle,
    MissingPublisher,
    MissingReference,
    MissingContent,
}

impl DependencySourceDocument {
    pub fn validate(&self) -> Result<(), DependencySourceValidationError> {
        if self.subject.trim().is_empty() {
            return Err(DependencySourceValidationError::MissingSubject);
        }
        if self.source_title.trim().is_empty() {
            return Err(DependencySourceValidationError::MissingTitle);
        }
        if self.publisher.trim().is_empty() {
            return Err(DependencySourceValidationError::MissingPublisher);
        }
        if self.source_url.is_none() && self.repository_path.is_none() {
            return Err(DependencySourceValidationError::MissingReference);
        }
        if self.content.trim().is_empty() {
            return Err(DependencySourceValidationError::MissingContent);
        }
        Ok(())
    }
}

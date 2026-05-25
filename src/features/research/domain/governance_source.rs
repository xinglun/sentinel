use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GovernanceSourceKind {
    SecFiling,
    LocalGovernanceDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceSourceDocument {
    pub subject: String,
    pub source_kind: GovernanceSourceKind,
    pub source_title: String,
    pub publisher: String,
    pub source_url: Option<String>,
    pub repository_path: Option<String>,
    pub observed_at: NaiveDate,
    pub retrieved_at: NaiveDate,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceSourceManifest {
    pub subject: String,
    pub source_kind: GovernanceSourceKind,
    pub source_title: String,
    pub publisher: String,
    pub source_url: Option<String>,
    pub repository_path: Option<String>,
    pub observed_at: NaiveDate,
    pub retrieved_at: NaiveDate,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GovernanceMetricAuditStatus {
    Extracted,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceMetricAuditEntry {
    pub metric: String,
    pub status: GovernanceMetricAuditStatus,
    pub value: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceExtractionAuditRecord {
    pub subject: String,
    pub source_title: String,
    pub observed_at: NaiveDate,
    pub retrieved_at: NaiveDate,
    pub metrics: Vec<GovernanceMetricAuditEntry>,
    pub accepted: bool,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovernanceSourceRejection {
    MissingContent,
    MissingSubject,
    MissingSourceReference,
    MissingSourceTitle,
    MissingPublisher,
}

impl GovernanceSourceDocument {
    pub fn validate(&self) -> Result<(), GovernanceSourceRejection> {
        if self.subject.trim().is_empty() {
            return Err(GovernanceSourceRejection::MissingSubject);
        }
        if self.source_title.trim().is_empty() {
            return Err(GovernanceSourceRejection::MissingSourceTitle);
        }
        if self.publisher.trim().is_empty() {
            return Err(GovernanceSourceRejection::MissingPublisher);
        }
        if self.source_url.is_none() && self.repository_path.is_none() {
            return Err(GovernanceSourceRejection::MissingSourceReference);
        }
        if self.content.trim().is_empty() {
            return Err(GovernanceSourceRejection::MissingContent);
        }
        Ok(())
    }
}

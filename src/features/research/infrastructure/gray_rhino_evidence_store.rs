use crate::features::research::application::dependency_evidence::DependencyEvidenceRepository;
use crate::features::research::application::dependency_source_pipeline::DependencySourceAuditRepository;
use crate::features::research::application::governance_evidence::GovernanceEvidenceRepository;
use crate::features::research::application::governance_source_pipeline::GovernanceSourceAuditRepository;
use crate::features::research::application::institutional_evidence::InstitutionalEvidenceRepository;
use crate::features::research::application::redundancy_evidence::RedundancyEvidenceRepository;
use crate::features::research::domain::dependency_source::{
    DependencyExtractionAuditRecord, DependencySourceManifest,
};
use crate::features::research::domain::governance_source::{
    GovernanceExtractionAuditRecord, GovernanceSourceManifest,
};
use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceCategory, GrayRhinoEvidenceRecord,
};
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 灰色のサイ evidence を追記型 JSONL として保存する。
pub(crate) struct GrayRhinoEvidenceStore {
    path: PathBuf,
    manifest_path: PathBuf,
    audit_path: PathBuf,
    dependency_manifest_path: PathBuf,
    dependency_audit_path: PathBuf,
}

impl GrayRhinoEvidenceStore {
    pub(crate) fn new(save_dir: &Path) -> Self {
        Self {
            path: save_dir.join("gray_rhino_evidence.jsonl"),
            manifest_path: save_dir.join("gray_rhino_governance_source_manifest.jsonl"),
            audit_path: save_dir.join("gray_rhino_governance_extraction_audit.jsonl"),
            dependency_manifest_path: save_dir.join("gray_rhino_dependency_source_manifest.jsonl"),
            dependency_audit_path: save_dir.join("gray_rhino_dependency_extraction_audit.jsonl"),
        }
    }
}

impl GovernanceEvidenceRepository for GrayRhinoEvidenceStore {
    fn save_governance_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool> {
        let records = self.load_governance_evidence()?;
        if records.iter().any(|existing| existing == record) {
            return Ok(false);
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("Failed to open {}", self.path.display()))?;
        writeln!(file, "{}", serde_json::to_string(record)?)?;
        Ok(true)
    }

    fn load_governance_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>> {
        Ok(load_jsonl::<GrayRhinoEvidenceRecord>(&self.path)?
            .into_iter()
            .filter(|record| record.category == GrayRhinoEvidenceCategory::GovernanceConcentration)
            .collect())
    }
}

impl DependencyEvidenceRepository for GrayRhinoEvidenceStore {
    fn save_dependency_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool> {
        append_unique_jsonl(&self.path, record)
    }

    fn load_dependency_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>> {
        Ok(load_jsonl::<GrayRhinoEvidenceRecord>(&self.path)?
            .into_iter()
            .filter(|record| record.category == GrayRhinoEvidenceCategory::DependencyConcentration)
            .collect())
    }
}

impl InstitutionalEvidenceRepository for GrayRhinoEvidenceStore {
    fn save_institutional_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool> {
        append_unique_jsonl(&self.path, record)
    }

    fn load_institutional_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>> {
        Ok(load_jsonl::<GrayRhinoEvidenceRecord>(&self.path)?
            .into_iter()
            .filter(|record| record.category == GrayRhinoEvidenceCategory::InstitutionalMaturity)
            .collect())
    }
}

impl RedundancyEvidenceRepository for GrayRhinoEvidenceStore {
    fn save_redundancy_evidence(&self, record: &GrayRhinoEvidenceRecord) -> Result<bool> {
        append_unique_jsonl(&self.path, record)
    }

    fn load_redundancy_evidence(&self) -> Result<Vec<GrayRhinoEvidenceRecord>> {
        Ok(load_jsonl::<GrayRhinoEvidenceRecord>(&self.path)?
            .into_iter()
            .filter(|record| record.category == GrayRhinoEvidenceCategory::Redundancy)
            .collect())
    }
}

impl GovernanceSourceAuditRepository for GrayRhinoEvidenceStore {
    fn save_governance_source_manifest(&self, manifest: &GovernanceSourceManifest) -> Result<bool> {
        append_unique_jsonl(&self.manifest_path, manifest)
    }

    fn save_governance_extraction_audit(
        &self,
        record: &GovernanceExtractionAuditRecord,
    ) -> Result<bool> {
        append_unique_jsonl(&self.audit_path, record)
    }

    fn load_governance_extraction_audits(&self) -> Result<Vec<GovernanceExtractionAuditRecord>> {
        load_jsonl(&self.audit_path)
    }
}

impl DependencySourceAuditRepository for GrayRhinoEvidenceStore {
    fn save_dependency_source_manifest(&self, manifest: &DependencySourceManifest) -> Result<bool> {
        append_unique_jsonl(&self.dependency_manifest_path, manifest)
    }

    fn save_dependency_extraction_audit(
        &self,
        record: &DependencyExtractionAuditRecord,
    ) -> Result<bool> {
        append_unique_jsonl(&self.dependency_audit_path, record)
    }
}

fn append_unique_jsonl<T>(path: &Path, value: &T) -> Result<bool>
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + PartialEq,
{
    let records: Vec<T> = load_jsonl(path)?;
    if records.iter().any(|existing| existing == value) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    writeln!(file, "{}", serde_json::to_string(value)?)?;
    Ok(true)
}

fn load_jsonl<T>(path: &Path) -> Result<Vec<T>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .with_context(|| format!("Failed to parse JSONL in {}", path.display()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::research::domain::governance_source::{
        GovernanceExtractionAuditRecord, GovernanceMetricAuditEntry, GovernanceMetricAuditStatus,
        GovernanceSourceKind, GovernanceSourceManifest,
    };
    use crate::features::research::domain::gray_rhino_evidence::{
        GrayRhinoEvidenceCategory, GrayRhinoEvidenceSourceType, GrayRhinoRiskEffect,
        GrayRhinoSourceReference,
    };
    use chrono::NaiveDate;
    use tempfile::tempdir;

    fn record() -> GrayRhinoEvidenceRecord {
        GrayRhinoEvidenceRecord {
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            source: GrayRhinoSourceReference {
                source_type: GrayRhinoEvidenceSourceType::GovernanceDocument,
                source_title: "Proxy statement".to_string(),
                publisher: "Example issuer".to_string(),
                source_url: Some("https://example.com/proxy".to_string()),
                repository_path: None,
                observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
                retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            },
            confidence: 0.9,
            risk_effect: GrayRhinoRiskEffect::Amplifying,
            extraction_note: "Proxy statement discloses voting rights.".to_string(),
            structural_fact: "Dual class shares create unequal voting rights.".to_string(),
        }
    }

    #[test]
    fn appends_governance_evidence_without_duplicates() {
        let dir = tempdir().unwrap();
        let store = GrayRhinoEvidenceStore::new(dir.path());
        let record = record();

        assert!(store.save_governance_evidence(&record).unwrap());
        assert!(!store.save_governance_evidence(&record).unwrap());

        assert_eq!(store.load_governance_evidence().unwrap(), vec![record]);
    }

    #[test]
    fn appends_dependency_evidence_without_duplicates_and_filters_category() {
        let dir = tempdir().unwrap();
        let store = GrayRhinoEvidenceStore::new(dir.path());
        let mut dependency = record();
        dependency.category = GrayRhinoEvidenceCategory::DependencyConcentration;
        dependency.source.source_type = GrayRhinoEvidenceSourceType::SupplierDisclosure;
        dependency.source.source_title = "Supplier dependency disclosure".to_string();
        dependency.structural_fact =
            "Critical supplier dependency has no disclosed fallback.".to_string();
        let governance = record();

        assert!(store.save_governance_evidence(&governance).unwrap());
        assert!(store.save_dependency_evidence(&dependency).unwrap());
        assert!(!store.save_dependency_evidence(&dependency).unwrap());

        assert_eq!(store.load_dependency_evidence().unwrap(), vec![dependency]);
    }

    #[test]
    fn appends_manifest_and_audit_without_duplicates() {
        let dir = tempdir().unwrap();
        let store = GrayRhinoEvidenceStore::new(dir.path());
        let manifest = GovernanceSourceManifest {
            subject: "EXAMPLE".to_string(),
            source_kind: GovernanceSourceKind::LocalGovernanceDocument,
            source_title: "Proxy statement".to_string(),
            publisher: "EXAMPLE".to_string(),
            source_url: None,
            repository_path: Some("gray_rhino_sources/governance/EXAMPLE/proxy.txt".to_string()),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            content_sha256: "a".repeat(64),
        };
        let audit = GovernanceExtractionAuditRecord {
            subject: "EXAMPLE".to_string(),
            source_title: "Proxy statement".to_string(),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            metrics: vec![GovernanceMetricAuditEntry {
                metric: "founder_voting_power".to_string(),
                status: GovernanceMetricAuditStatus::Extracted,
                value: Some("61.2".to_string()),
                reason: None,
            }],
            accepted: true,
            rejection_reason: None,
        };

        assert!(store.save_governance_source_manifest(&manifest).unwrap());
        assert!(!store.save_governance_source_manifest(&manifest).unwrap());
        assert!(store.save_governance_extraction_audit(&audit).unwrap());
        assert!(!store.save_governance_extraction_audit(&audit).unwrap());
        assert_eq!(
            store.load_governance_extraction_audits().unwrap(),
            vec![audit]
        );
    }
}

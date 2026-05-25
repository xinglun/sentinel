use crate::features::research::application::dependency_evidence::DependencyEvidenceRepository;
use crate::features::research::application::governance_evidence::GovernanceEvidenceRepository;
use crate::features::research::application::governance_source_pipeline::GovernanceSourceAuditRepository;
use crate::features::research::application::gray_rhino_daily_report::GrayRhinoDailyReportRepository;
use crate::features::research::application::gray_rhino_discovery::{
    discover_gray_rhino_candidates, GrayRhinoDiscoveryInput,
};
use crate::features::research::application::institutional_evidence::InstitutionalEvidenceRepository;
use crate::features::research::application::redundancy_evidence::RedundancyEvidenceRepository;
use crate::features::research::domain::governance_source::GovernanceExtractionAuditRecord;
use crate::features::research::domain::gray_rhino::GrayRhinoAssessmentSnapshot;
use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateScope,
};
use crate::features::research::domain::gray_rhino_evidence::GrayRhinoEvidenceRecord;
use crate::features::research::infrastructure::gray_rhino_candidate_store::GrayRhinoCandidateStore;
use crate::features::research::infrastructure::gray_rhino_evidence_store::GrayRhinoEvidenceStore;
use crate::features::research::infrastructure::gray_rhino_snapshot_store::GrayRhinoSnapshotStore;
use anyhow::Result;
use chrono::NaiveDate;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) struct FileGrayRhinoDailyReportRepository {
    save_dir: PathBuf,
}

impl FileGrayRhinoDailyReportRepository {
    pub(crate) fn new(save_dir: &Path) -> Self {
        Self {
            save_dir: save_dir.to_path_buf(),
        }
    }
}

impl GrayRhinoDailyReportRepository for FileGrayRhinoDailyReportRepository {
    fn load_previous_snapshot(
        &self,
        as_of_date: NaiveDate,
    ) -> Result<Option<GrayRhinoAssessmentSnapshot>> {
        GrayRhinoSnapshotStore::new(&self.save_dir).load_latest_before(as_of_date)
    }

    fn save_snapshot_if_changed(&self, snapshot: &GrayRhinoAssessmentSnapshot) -> Result<()> {
        GrayRhinoSnapshotStore::new(&self.save_dir).save_if_changed(snapshot)
    }

    fn load_evidence_records(&self) -> Result<Vec<GrayRhinoEvidenceRecord>> {
        let store = GrayRhinoEvidenceStore::new(&self.save_dir);
        let mut records = store.load_governance_evidence()?;
        records.extend(store.load_dependency_evidence()?);
        records.extend(store.load_institutional_evidence()?);
        records.extend(store.load_redundancy_evidence()?);
        Ok(records)
    }

    fn load_governance_audits(&self) -> Result<Vec<GovernanceExtractionAuditRecord>> {
        GrayRhinoEvidenceStore::new(&self.save_dir).load_governance_extraction_audits()
    }

    fn collect_auto_candidates(
        &self,
        watch_symbols: &[String],
        as_of_date: NaiveDate,
    ) -> Result<Vec<GrayRhinoCandidate>> {
        let source_roots = [
            self.save_dir.join("gray_rhino_sources"),
            self.save_dir.join("gray_rhino_raw_sources"),
        ];
        let mut files = Vec::new();
        for root in source_roots {
            collect_text_files(&root, &mut files);
        }
        let default_subject = watch_symbols
            .first()
            .cloned()
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let mut candidates = Vec::new();
        for path in files {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let path_text = path.to_string_lossy().to_string();
            let path_components = path
                .components()
                .filter_map(|component| component.as_os_str().to_str())
                .map(|component| component.to_uppercase())
                .collect::<Vec<_>>();
            let subject = watch_symbols
                .iter()
                .find(|symbol| {
                    let symbol = symbol.to_uppercase();
                    path_components.iter().any(|component| {
                        component == &symbol || component.starts_with(&format!("{symbol}_"))
                    })
                })
                .cloned()
                .or_else(|| {
                    path.parent()
                        .and_then(|parent| parent.file_name())
                        .and_then(|name| name.to_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| default_subject.clone());
            let source_is_typed_company_cache = path.components().any(|component| {
                matches!(
                    component.as_os_str().to_str(),
                    Some("governance" | "narrative")
                )
            });
            if source_is_typed_company_cache
                && !watch_symbols
                    .iter()
                    .any(|watch_symbol| watch_symbol.eq_ignore_ascii_case(&subject))
            {
                continue;
            }
            candidates.extend(discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
                subject,
                source_title: path_text,
                observed_at: as_of_date,
                text,
            }));
        }
        if let Ok(persisted_candidates) =
            GrayRhinoCandidateStore::new(&self.save_dir).load_candidates()
        {
            candidates.extend(
                persisted_candidates.into_iter().filter(|candidate| {
                    candidate_in_current_report_scope(candidate, watch_symbols)
                }),
            );
        }
        Ok(candidates)
    }

    fn load_backfill_ops_view(&self) -> Option<Value> {
        load_latest_jsonl_value(&self.save_dir.join("gray_rhino_backfill_runs.jsonl"))
    }

    fn load_discovery_ops_view(&self) -> Option<Value> {
        load_latest_jsonl_value(&self.save_dir.join("gray_rhino_discovery_runs.jsonl"))
    }
}

fn candidate_in_current_report_scope(
    candidate: &GrayRhinoCandidate,
    watch_symbols: &[String],
) -> bool {
    candidate.scope == GrayRhinoCandidateScope::Market
        || watch_symbols
            .iter()
            .any(|symbol| symbol.eq_ignore_ascii_case(&candidate.subject))
}

fn collect_text_files(path: &Path, out: &mut Vec<PathBuf>) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if metadata.is_file() {
        if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("txt" | "md" | "html" | "htm")
        ) {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_text_files(&entry.path(), out);
    }
}

fn load_latest_jsonl_value(path: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    let latest = raw.lines().rev().find(|line| !line.trim().is_empty())?;
    serde_json::from_str(latest).ok()
}

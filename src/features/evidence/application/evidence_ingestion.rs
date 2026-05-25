use crate::features::evidence::application::evidence::EvidenceRepository;
use crate::features::evidence::domain::collection_policy::EvidenceCollectionPolicy;
use crate::features::evidence::domain::evidence::{AutomatedEvidenceRecord, EvidenceSourceType};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;

/// 取得済み source document の application DTO。
#[derive(Debug, Clone)]
pub struct SourceDocument {
    pub title: String,
    pub content: String,
    pub url: String,
    pub source_type: EvidenceSourceType,
    pub symbol: String,
    pub metadata: HashMap<String, String>,
}

/// 外部 source から document を取得する port。
#[async_trait]
pub trait SourceFetcher {
    async fn fetch(
        &self,
        url: &str,
        symbol: &str,
        source_type: EvidenceSourceType,
        days: usize,
    ) -> anyhow::Result<SourceDocument>;
}

/// SourceDocument から domain evidence を抽出する port。
pub trait EvidenceExtractor {
    fn extract(&self, doc: &SourceDocument) -> Vec<AutomatedEvidenceRecord>;
}

/// Evidence collection use case の入力。
#[derive(Debug, Clone)]
pub struct CollectEvidenceRequest {
    pub url: String,
    pub symbol: String,
    pub source_type: EvidenceSourceType,
    pub days: usize,
    pub persist: bool,
    pub retention_days: Option<i64>,
}

/// Evidence collection use case の結果。
#[derive(Debug, Clone)]
pub struct CollectEvidenceOutcome {
    pub document: SourceDocument,
    pub records: Vec<AutomatedEvidenceRecord>,
    pub saved_count: usize,
    pub cleanup_count: usize,
}

/// Evidence source 取得、抽出、永続化を orchestration する use case。
pub async fn collect_evidence_from_source(
    fetcher: &dyn SourceFetcher,
    extractor: &dyn EvidenceExtractor,
    repository: Option<&dyn EvidenceRepository>,
    request: CollectEvidenceRequest,
) -> Result<CollectEvidenceOutcome> {
    let document = fetcher
        .fetch(
            &request.url,
            &request.symbol,
            request.source_type,
            request.days,
        )
        .await?;
    let mut records = extractor.extract(&document);
    normalize_dedupe_keys(&mut records);

    let mut cleanup_count = 0;
    let mut saved_count = 0;
    let policy = EvidenceCollectionPolicy::new(request.persist, request.retention_days);
    if policy.requires_repository() {
        let repository = repository.ok_or_else(|| {
            anyhow!("EvidenceRepository is required when persistence is requested")
        })?;
        if let Some(retention_days) = policy.retention_days() {
            cleanup_count = repository.cleanup_old_records(retention_days)?;
        }
        saved_count = repository.save_records(&records)?;
    }

    Ok(CollectEvidenceOutcome {
        document,
        records,
        saved_count,
        cleanup_count,
    })
}

/// 保存前の自動 evidence dedupe key を application boundary で正規化する。
pub fn normalize_dedupe_keys(records: &mut [AutomatedEvidenceRecord]) {
    for record in records.iter_mut() {
        record.generate_auto_dedupe_key();
    }
}

/// Batch evidence collection の対象。
#[derive(Debug, Clone)]
pub struct BatchEvidenceTarget {
    pub symbol: String,
    pub url: String,
}

/// Batch evidence collection use case の入力。
#[derive(Debug, Clone)]
pub struct BatchCollectEvidenceRequest {
    pub targets: Vec<BatchEvidenceTarget>,
    pub source_type: EvidenceSourceType,
    pub days: usize,
    pub persist: bool,
    pub retention_days: Option<i64>,
}

/// Symbol 単位の取得失敗情報。
#[derive(Debug, Clone)]
pub struct BatchEvidenceFailure {
    pub symbol: String,
    pub error: String,
}

/// Batch evidence collection use case の結果。
#[derive(Debug, Clone)]
pub struct BatchCollectEvidenceOutcome {
    pub records: Vec<AutomatedEvidenceRecord>,
    pub success_count: usize,
    pub failure_count: usize,
    pub failures: Vec<BatchEvidenceFailure>,
    pub saved_count: usize,
    pub cleanup_count: usize,
}

/// 複数 symbol の evidence collection を orchestration する use case。
pub async fn collect_evidence_batch(
    fetcher: &dyn SourceFetcher,
    extractor: &dyn EvidenceExtractor,
    repository: Option<&dyn EvidenceRepository>,
    request: BatchCollectEvidenceRequest,
) -> Result<BatchCollectEvidenceOutcome> {
    let mut records = Vec::new();
    let mut success_count = 0;
    let mut failures = Vec::new();

    for target in &request.targets {
        match collect_evidence_from_source(
            fetcher,
            extractor,
            None,
            CollectEvidenceRequest {
                url: target.url.clone(),
                symbol: target.symbol.clone(),
                source_type: request.source_type,
                days: request.days,
                persist: false,
                retention_days: None,
            },
        )
        .await
        {
            Ok(outcome) => {
                records.extend(outcome.records);
                success_count += 1;
            }
            Err(error) => failures.push(BatchEvidenceFailure {
                symbol: target.symbol.clone(),
                error: error.to_string(),
            }),
        }
    }

    normalize_dedupe_keys(&mut records);

    let mut cleanup_count = 0;
    let mut saved_count = 0;
    let policy = EvidenceCollectionPolicy::new(request.persist, request.retention_days);
    if policy.requires_repository() {
        let repository = repository.ok_or_else(|| {
            anyhow!("EvidenceRepository is required when persistence is requested")
        })?;
        if let Some(retention_days) = policy.retention_days() {
            cleanup_count = repository.cleanup_old_records(retention_days)?;
        }
        saved_count = repository.save_records(&records)?;
    }

    Ok(BatchCollectEvidenceOutcome {
        records,
        success_count,
        failure_count: failures.len(),
        failures,
        saved_count,
        cleanup_count,
    })
}

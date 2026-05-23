use crate::domain::evidence::{AutomatedEvidenceRecord, EvidenceSourceType};
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

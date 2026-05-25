use crate::features::research::application::governance_source_pipeline::{
    GovernanceSourceAdapter, GovernanceSourceCollectionRequest,
};
use crate::features::research::domain::governance_source::{
    GovernanceSourceDocument, GovernanceSourceKind,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::{Local, NaiveDate};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// GovernanceConcentration 用の source document adapter。
pub(crate) struct GovernanceDocumentSourceAdapter {
    user_agent: Option<String>,
    cache_dir: PathBuf,
    base_url_data: String,
    base_url_www: String,
}

impl GovernanceDocumentSourceAdapter {
    pub(crate) fn new(user_agent: Option<String>, save_dir: &Path) -> Self {
        Self {
            user_agent,
            cache_dir: save_dir.join("gray_rhino_sources").join("governance"),
            base_url_data: "https://data.sec.gov".to_string(),
            base_url_www: "https://www.sec.gov".to_string(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_base_urls(mut self, data: String, www: String) -> Self {
        self.base_url_data = data;
        self.base_url_www = www;
        self
    }

    async fn fetch_local_document(
        &self,
        request: &GovernanceSourceCollectionRequest,
        file: &str,
    ) -> Result<GovernanceSourceDocument> {
        let source_path = PathBuf::from(file);
        let content = tokio::fs::read_to_string(&source_path)
            .await
            .with_context(|| format!("Failed to read governance source file: {}", file))?;
        let subject = request
            .symbol
            .clone()
            .unwrap_or_else(|| "LOCAL".to_string());
        let cached_path = self
            .cache_document(
                &subject,
                source_path.file_name().and_then(|name| name.to_str()),
                &content,
            )
            .await?;
        Ok(GovernanceSourceDocument {
            subject: subject.clone(),
            source_kind: GovernanceSourceKind::LocalGovernanceDocument,
            source_title: source_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("local governance document")
                .to_string(),
            publisher: subject,
            source_url: None,
            repository_path: Some(cached_path),
            observed_at: request.observed_at,
            retrieved_at: request.retrieved_at,
            content,
        })
    }

    async fn fetch_sec_document(
        &self,
        request: &GovernanceSourceCollectionRequest,
    ) -> Result<GovernanceSourceDocument> {
        let symbol = request
            .symbol
            .as_deref()
            .ok_or_else(|| anyhow!("--symbol is required for SEC governance collection"))?;
        let user_agent = self
            .user_agent
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                anyhow!("SEC user_agent is not configured. Set SEC_USER_AGENT env or config.toml")
            })?;
        let cik = self.get_cik(symbol, user_agent).await?;
        let submissions_url = format!("{}/submissions/CIK{}.json", self.base_url_data, cik);
        let submissions = self.sec_get_json(&submissions_url, user_agent).await?;
        let filing = select_governance_filing(&submissions, request.lookback_days)
            .ok_or_else(|| anyhow!("No governance SEC filing found for {}", symbol))?;
        let accession_no_dashes = filing.accession_number.replace('-', "");
        let cik_no_zeros = cik.trim_start_matches('0');
        let doc_url = format!(
            "{}/Archives/edgar/data/{}/{}/{}",
            self.base_url_www, cik_no_zeros, accession_no_dashes, filing.primary_document
        );
        let content = self.sec_get_text(&doc_url, user_agent).await?;
        let cached_path = self
            .cache_document(symbol, Some(&filing.primary_document), &content)
            .await?;

        Ok(GovernanceSourceDocument {
            subject: symbol.to_string(),
            source_kind: GovernanceSourceKind::SecFiling,
            source_title: format!("SEC {} for {}", filing.form, symbol),
            publisher: "SEC EDGAR".to_string(),
            source_url: Some(doc_url),
            repository_path: Some(cached_path),
            observed_at: filing.filing_date,
            retrieved_at: request.retrieved_at,
            content,
        })
    }

    async fn get_cik(&self, symbol: &str, user_agent: &str) -> Result<String> {
        let url = format!("{}/files/company_tickers.json", self.base_url_www);
        let data = self.sec_get_json(&url, user_agent).await?;
        let target = symbol.to_uppercase();
        data.as_object()
            .and_then(|items| {
                items.values().find_map(|item| {
                    let ticker = item.get("ticker")?.as_str()?.to_uppercase();
                    if ticker == target {
                        let cik = item.get("cik_str")?.as_u64()?;
                        Some(format!("{cik:010}"))
                    } else {
                        None
                    }
                })
            })
            .ok_or_else(|| anyhow!("CIK not found for symbol: {}", symbol))
    }

    async fn sec_get_json(&self, url: &str, user_agent: &str) -> Result<Value> {
        let text = self.sec_get_text(url, user_agent).await?;
        serde_json::from_str(&text).with_context(|| format!("Failed to parse SEC JSON: {}", url))
    }

    async fn sec_get_text(&self, url: &str, user_agent: &str) -> Result<String> {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let response = client.get(url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "SEC governance source returned error: {}. URL: {}",
                response.status(),
                url
            ));
        }
        response
            .text()
            .await
            .with_context(|| format!("Failed to read SEC response: {}", url))
    }

    async fn cache_document(
        &self,
        subject: &str,
        file_name: Option<&str>,
        content: &str,
    ) -> Result<String> {
        let subject_dir = self.cache_dir.join(sanitize_path_segment(subject));
        tokio::fs::create_dir_all(&subject_dir)
            .await
            .with_context(|| format!("Failed to create {}", subject_dir.display()))?;
        let name = file_name
            .map(sanitize_path_segment)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "governance-source.txt".to_string());
        let target = subject_dir.join(name);
        tokio::fs::write(&target, content)
            .await
            .with_context(|| format!("Failed to write {}", target.display()))?;
        Ok(target.to_string_lossy().to_string())
    }
}

#[async_trait]
impl GovernanceSourceAdapter for GovernanceDocumentSourceAdapter {
    async fn fetch_governance_sources(
        &self,
        request: &GovernanceSourceCollectionRequest,
    ) -> Result<Vec<GovernanceSourceDocument>> {
        if let Some(file) = request.local_file.as_deref() {
            return Ok(vec![self.fetch_local_document(request, file).await?]);
        }
        Ok(vec![self.fetch_sec_document(request).await?])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecFilingCandidate {
    form: String,
    filing_date: NaiveDate,
    accession_number: String,
    primary_document: String,
}

fn select_governance_filing(value: &Value, lookback_days: usize) -> Option<SecFilingCandidate> {
    let recent = value.get("filings")?.get("recent")?;
    let forms = recent.get("form")?.as_array()?;
    let dates = recent.get("filingDate")?.as_array()?;
    let accessions = recent.get("accessionNumber")?.as_array()?;
    let primary_documents = recent.get("primaryDocument")?.as_array()?;
    let limit_date = (Local::now() - chrono::Duration::days(lookback_days as i64)).date_naive();

    for idx in 0..forms.len() {
        let form = forms.get(idx)?.as_str()?;
        if !matches!(form, "DEF 14A" | "10-K" | "20-F" | "S-1") {
            continue;
        }
        let filing_date = NaiveDate::parse_from_str(dates.get(idx)?.as_str()?, "%Y-%m-%d").ok()?;
        if filing_date < limit_date {
            continue;
        }
        return Some(SecFilingCandidate {
            form: form.to_string(),
            filing_date,
            accession_number: accessions.get(idx)?.as_str()?.to_string(),
            primary_document: primary_documents.get(idx)?.as_str()?.to_string(),
        });
    }
    None
}

fn sanitize_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn local_document_adapter_caches_source_for_replay() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("proxy.txt");
        tokio::fs::write(&source, "founder_voting_power: 61.2%")
            .await
            .unwrap();
        let adapter = GovernanceDocumentSourceAdapter::new(None, dir.path());
        let request = GovernanceSourceCollectionRequest {
            symbol: Some("EXAMPLE".to_string()),
            local_file: Some(source.to_string_lossy().to_string()),
            observed_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            retrieved_at: NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            lookback_days: 365,
            persist_evidence: true,
        };

        let documents = adapter.fetch_governance_sources(&request).await.unwrap();

        assert_eq!(documents.len(), 1);
        let cached = documents[0].repository_path.as_ref().unwrap();
        assert!(Path::new(cached).exists());
        assert_eq!(
            documents[0].source_kind,
            GovernanceSourceKind::LocalGovernanceDocument
        );
    }

    #[test]
    fn selects_proxy_statement_before_non_governance_forms() {
        let raw = serde_json::json!({
            "filings": {
                "recent": {
                    "form": ["8-K", "DEF 14A"],
                    "filingDate": ["2026-05-20", "2026-05-21"],
                    "accessionNumber": ["0000000000-26-000001", "0000000000-26-000002"],
                    "primaryDocument": ["current.htm", "proxy.htm"]
                }
            }
        });

        let filing = select_governance_filing(&raw, 3650).unwrap();

        assert_eq!(filing.form, "DEF 14A");
        assert_eq!(filing.primary_document, "proxy.htm");
    }
}

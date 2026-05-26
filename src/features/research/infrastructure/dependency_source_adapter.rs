use crate::features::research::application::dependency_source_pipeline::{
    DependencySourceAdapter, DependencySourceCollectionRequest,
};
use crate::features::research::domain::dependency_source::{
    DependencySourceDocument, DependencySourceKind,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use sha2::{Digest, Sha256};

pub(crate) struct LocalDependencySourceAdapter;

#[async_trait]
impl DependencySourceAdapter for LocalDependencySourceAdapter {
    async fn fetch_dependency_sources(
        &self,
        request: &DependencySourceCollectionRequest,
    ) -> Result<Vec<DependencySourceDocument>> {
        let subject = request
            .symbol
            .clone()
            .unwrap_or_else(|| "UNKNOWN".to_string());
        if let Some(url) = request.source_url.as_ref() {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .context("Failed to build dependency source HTTP client")?;
            let mut last_error = None;
            let mut content = None;
            for _attempt in 0..3 {
                match client.get(url).send().await {
                    Ok(response) => match response.error_for_status() {
                        Ok(response) => match response.text().await {
                            Ok(body) => {
                                content = Some(body);
                                break;
                            }
                            Err(err) => last_error = Some(err.into()),
                        },
                        Err(err) => last_error = Some(err.into()),
                    },
                    Err(err) => last_error = Some(err.into()),
                }
            }
            let content = content.ok_or_else(|| {
                last_error.unwrap_or_else(|| anyhow!("Failed to fetch dependency source URL"))
            })?;
            if let Some(cache_dir) = request.source_cache_dir.as_ref() {
                let cache_dir = std::path::PathBuf::from(cache_dir);
                tokio::fs::create_dir_all(&cache_dir)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to create dependency cache dir: {}",
                            cache_dir.display()
                        )
                    })?;
                let cache_name = format!("{:x}.txt", Sha256::digest(url.as_bytes()));
                tokio::fs::write(cache_dir.join(cache_name), &content)
                    .await
                    .with_context(|| "Failed to cache dependency source body")?;
            }
            let parsed_url = reqwest::Url::parse(url).ok();
            let publisher = parsed_url
                .as_ref()
                .and_then(|parsed| parsed.host_str())
                .unwrap_or("unknown dependency publisher")
                .to_string();
            return Ok(vec![DependencySourceDocument {
                subject: subject.clone(),
                source_kind: DependencySourceKind::LiveDependencyDisclosure,
                source_title: format!("Dependency disclosure: {publisher}"),
                publisher,
                source_url: Some(url.to_string()),
                repository_path: None,
                observed_at: request.observed_at,
                retrieved_at: request.retrieved_at,
                content,
            }]);
        }
        let file = request.local_file.as_ref().ok_or_else(|| {
            anyhow!("--file or --url is required for dependency source collection")
        })?;
        let path = std::path::PathBuf::from(file);
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read dependency source file: {}", file))?;
        Ok(vec![DependencySourceDocument {
            subject: subject.clone(),
            source_kind: DependencySourceKind::LocalDependencyDocument,
            source_title: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("dependency_source")
                .to_string(),
            publisher: subject,
            source_url: None,
            repository_path: Some(file.to_string()),
            observed_at: request.observed_at,
            retrieved_at: request.retrieved_at,
            content,
        }])
    }
}

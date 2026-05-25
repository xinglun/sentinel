use crate::config;
use crate::features::research::application::governance_source_pipeline::{
    GovernanceSourceAdapter, GovernanceSourceCollectionRequest,
};
use crate::features::research::application::gray_rhino_source_collection::{
    CollectGrayRhinoSourcesUseCase, GrayRhinoCandidateRepositoryPort, GrayRhinoDiscoveryRunRecord,
    GrayRhinoDiscoveryRunRepositoryPort, GrayRhinoFetchOutcome, GrayRhinoFetchedSource,
    GrayRhinoSourceCollectionOutcome, GrayRhinoSourceCollectionRequest, GrayRhinoSourceFetcherPort,
    GrayRhinoSourceProvider,
};
use crate::features::research::domain::gray_rhino_candidate::GrayRhinoCandidate;
use crate::features::research::infrastructure::gray_rhino_candidate_store::GrayRhinoCandidateStore;
use crate::features::research::infrastructure::sec_governance_source_adapter::GovernanceDocumentSourceAdapter;
use anyhow::{anyhow, Context, Result};
use chrono::Duration;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

const FRED_SERIES: &[&str] = &[
    "DGS10",
    "DGS2",
    "T10Y2Y",
    "FEDFUNDS",
    "BAMLH0A0HYM2",
    "WALCL",
    "RRPONTSYD",
];

pub(crate) struct FileGrayRhinoSourceCollector<'a> {
    app_config: &'a config::AppConfig,
}

impl<'a> FileGrayRhinoSourceCollector<'a> {
    pub(crate) fn new(app_config: &'a config::AppConfig) -> Self {
        Self { app_config }
    }
}

pub(crate) struct FileGrayRhinoCandidateRepository<'a> {
    save_dir: &'a Path,
}

impl<'a> FileGrayRhinoCandidateRepository<'a> {
    pub(crate) fn new(save_dir: &'a Path) -> Self {
        Self { save_dir }
    }
}

pub(crate) struct FileGrayRhinoDiscoveryRunRepository;

pub(crate) async fn collect_gray_rhino_sources(
    app_config: &config::AppConfig,
    request: GrayRhinoSourceCollectionRequest,
) -> Result<Vec<GrayRhinoSourceCollectionOutcome>> {
    let fetcher = FileGrayRhinoSourceCollector::new(app_config);
    let candidate_repository = FileGrayRhinoCandidateRepository::new(&request.save_dir);
    let run_repository = FileGrayRhinoDiscoveryRunRepository;
    CollectGrayRhinoSourcesUseCase::new(&fetcher, &candidate_repository, &run_repository)
        .collect(&request)
        .await
}

impl GrayRhinoSourceFetcherPort for FileGrayRhinoSourceCollector<'_> {
    async fn fetch_sources(
        &self,
        request: &GrayRhinoSourceCollectionRequest,
    ) -> Result<Vec<GrayRhinoFetchOutcome>> {
        match request.provider {
            GrayRhinoSourceProvider::Sec => collect_sec_sources(self.app_config, request).await,
            GrayRhinoSourceProvider::Finnhub => {
                collect_finnhub_sources(self.app_config, request).await
            }
            GrayRhinoSourceProvider::Fred => collect_fred_sources(self.app_config, request).await,
        }
    }
}

impl GrayRhinoCandidateRepositoryPort for FileGrayRhinoCandidateRepository<'_> {
    fn save_candidates(&self, candidates: &[GrayRhinoCandidate]) -> Result<usize> {
        GrayRhinoCandidateStore::new(self.save_dir).save_candidates(candidates)
    }
}

impl GrayRhinoDiscoveryRunRepositoryPort for FileGrayRhinoDiscoveryRunRepository {
    async fn append_discovery_run(
        &self,
        request: &GrayRhinoSourceCollectionRequest,
        outcomes: &[GrayRhinoSourceCollectionOutcome],
    ) -> Result<()> {
        append_discovery_run(request, outcomes).await
    }
}

async fn collect_sec_sources(
    app_config: &config::AppConfig,
    request: &GrayRhinoSourceCollectionRequest,
) -> Result<Vec<GrayRhinoFetchOutcome>> {
    let subjects = configured_subjects(app_config, &request.symbols);
    if request.dry_run {
        return Ok(subjects
            .into_iter()
            .map(|subject| planned_outcome(subject, "SEC filing fetch planned"))
            .collect());
    }
    let adapter = GovernanceDocumentSourceAdapter::new(
        app_config.sec.as_ref().map(|sec| sec.user_agent.clone()),
        &request.save_dir,
    );
    let mut outcomes = Vec::new();
    for subject in subjects {
        let collection_request = GovernanceSourceCollectionRequest {
            symbol: Some(subject.clone()),
            local_file: None,
            observed_at: request.as_of_date,
            retrieved_at: request.as_of_date,
            lookback_days: request.lookback_days,
            persist_evidence: false,
        };
        match adapter.fetch_governance_sources(&collection_request).await {
            Ok(documents) => {
                for document in documents {
                    outcomes.push(GrayRhinoFetchOutcome::Accepted(GrayRhinoFetchedSource {
                        subject: document.subject,
                        source_title: document.source_title,
                        source_published_at: document.observed_at,
                        content_sha256: Some(content_sha256(&document.content)),
                        content: document.content,
                        source_url: document.source_url,
                        repository_path: document.repository_path,
                    }));
                }
            }
            Err(err) => outcomes.push(rejected_outcome(subject, "fetch_failure", err.to_string())),
        }
    }
    Ok(outcomes)
}

async fn collect_finnhub_sources(
    app_config: &config::AppConfig,
    request: &GrayRhinoSourceCollectionRequest,
) -> Result<Vec<GrayRhinoFetchOutcome>> {
    let subjects = configured_subjects(app_config, &request.symbols);
    if request.dry_run {
        return Ok(subjects
            .into_iter()
            .map(|subject| planned_outcome(subject, "Finnhub narrative fetch planned"))
            .collect());
    }
    let token = app_config
        .finnhub
        .as_ref()
        .map(|config| config.finnhub_api_key.as_str())
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| anyhow!("Finnhub API key is not configured"))?;
    let to = request.as_of_date;
    let from = to - Duration::days(request.lookback_days as i64);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let mut outcomes = Vec::new();
    for subject in subjects {
        let url = format!(
            "https://finnhub.io/api/v1/company-news?symbol={}&from={}&to={}&token={}",
            subject, from, to, token
        );
        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                let raw = response.text().await?;
                let content = render_finnhub_narrative_text(&subject, &raw)?;
                let repository_path = cache_source(
                    &request.save_dir,
                    "narrative",
                    &subject,
                    &format!("finnhub_news_{}.txt", to),
                    &content,
                )
                .await?;
                outcomes.push(GrayRhinoFetchOutcome::Accepted(GrayRhinoFetchedSource {
                    subject: subject.clone(),
                    source_title: "Finnhub company news".to_string(),
                    source_published_at: request.as_of_date,
                    content_sha256: Some(content_sha256(&content)),
                    content,
                    source_url: Some(redact_token(&url)),
                    repository_path: Some(repository_path),
                }));
            }
            Ok(response) => outcomes.push(rejected_outcome(
                subject,
                "fetch_failure",
                format!("Finnhub returned {}", response.status()),
            )),
            Err(err) => outcomes.push(rejected_outcome(subject, "fetch_failure", err.to_string())),
        }
    }
    Ok(outcomes)
}

async fn collect_fred_sources(
    app_config: &config::AppConfig,
    request: &GrayRhinoSourceCollectionRequest,
) -> Result<Vec<GrayRhinoFetchOutcome>> {
    if request.dry_run {
        return Ok(vec![planned_outcome(
            "Market".to_string(),
            "FRED macro series fetch planned",
        )]);
    }
    let token = app_config
        .fred
        .as_ref()
        .map(|config| config.fred_api_key.as_str())
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| anyhow!("FRED API key is not configured"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let mut series_payloads = Vec::new();
    for series in FRED_SERIES {
        let url = format!(
            "https://api.stlouisfed.org/fred/series/observations?series_id={}&api_key={}&file_type=json&sort_order=desc&limit=5",
            series, token
        );
        let response = client.get(&url).send().await?;
        if !response.status().is_success() {
            return Ok(vec![rejected_outcome(
                "Market".to_string(),
                "fetch_failure",
                format!("FRED {} returned {}", series, response.status()),
            )]);
        }
        series_payloads.push((series.to_string(), response.text().await?));
    }
    let content = render_fred_macro_text(&series_payloads)?;
    let repository_path = cache_source(
        &request.save_dir,
        "macro",
        "Market",
        &format!("fred_{}.txt", request.as_of_date),
        &content,
    )
    .await?;
    Ok(vec![GrayRhinoFetchOutcome::Accepted(
        GrayRhinoFetchedSource {
            subject: "Market".to_string(),
            source_title: "FRED macro series".to_string(),
            source_published_at: request.as_of_date,
            content_sha256: Some(content_sha256(&content)),
            content,
            source_url: Some("https://fred.stlouisfed.org/".to_string()),
            repository_path: Some(repository_path),
        },
    )])
}

pub(crate) fn render_finnhub_narrative_text(symbol: &str, raw_json: &str) -> Result<String> {
    let items: Vec<serde_json::Value> =
        serde_json::from_str(raw_json).context("Failed to parse Finnhub news JSON")?;
    let mut out = format!("Finnhub narrative source for {symbol}\n");
    out.push_str("Purpose: normalize company news for structural-risk discovery.\n");
    for item in items.iter().take(20) {
        let headline = item
            .get("headline")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let summary = item
            .get("summary")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let url = item
            .get("url")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        out.push_str(&format!(
            "- headline: {headline}\n  summary: {summary}\n  url: {url}\n"
        ));
    }
    Ok(out)
}

pub(crate) fn render_fred_macro_text(series_payloads: &[(String, String)]) -> Result<String> {
    let mut out = String::from("FRED macro structural source\n");
    out.push_str("Observed dimensions: rate pressure, liquidity fragility, credit stress, yield curve constraint.\n");
    let mut observations = BTreeMap::new();
    for (series, payload) in series_payloads {
        let value: serde_json::Value = serde_json::from_str(payload)
            .with_context(|| format!("Failed to parse FRED {series}"))?;
        let latest = value
            .get("observations")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        out.push_str(&format!(
            "- FRED series {series}: latest observation {latest}\n"
        ));
        observations.insert(series.as_str(), fred_numeric_observations(payload)?);
    }
    out.push_str("FRED threshold assessment:\n");
    append_fred_threshold_assessment(&mut out, &observations);
    Ok(out)
}

fn append_fred_threshold_assessment(out: &mut String, observations: &BTreeMap<&str, Vec<f64>>) {
    let dgs10 = latest_value(observations, "DGS10");
    let fedfunds = latest_value(observations, "FEDFUNDS");
    let curve = latest_value(observations, "T10Y2Y");
    let high_yield_spread = latest_value(observations, "BAMLH0A0HYM2");
    let walcl_change = latest_change_ratio(observations, "WALCL");
    let rrp = latest_value(observations, "RRPONTSYD");

    if dgs10.is_some_and(|value| value >= 5.0) || fedfunds.is_some_and(|value| value >= 5.25) {
        out.push_str("- rate pressure critical: DGS10 >= 5.00 or FEDFUNDS >= 5.25.\n");
        out.push_str("- capex payback critical: financing hurdle is materially elevated.\n");
    } else if dgs10.is_some_and(|value| value >= 4.5) || fedfunds.is_some_and(|value| value >= 5.0)
    {
        out.push_str("- rate pressure elevated: DGS10 >= 4.50 or FEDFUNDS >= 5.00.\n");
        out.push_str("- capex payback risk: financing hurdle is elevated.\n");
    }

    if curve.is_some_and(|value| value <= -1.0) {
        out.push_str("- yield curve constraint critical: T10Y2Y <= -1.00.\n");
    } else if curve.is_some_and(|value| value <= -0.5) {
        out.push_str("- yield curve constraint: T10Y2Y <= -0.50.\n");
    }

    if high_yield_spread.is_some_and(|value| value >= 7.0) {
        out.push_str("- credit stress critical: high-yield spread >= 7.00.\n");
    } else if high_yield_spread.is_some_and(|value| value >= 5.0) {
        out.push_str("- credit stress watch: high-yield spread >= 5.00.\n");
    }

    if walcl_change.is_some_and(|ratio| ratio <= -0.05) {
        out.push_str("- liquidity fragility critical: WALCL contracted by at least 5% from the prior observation.\n");
    } else if walcl_change.is_some_and(|ratio| ratio <= -0.02) {
        out.push_str(
            "- liquidity tightened: WALCL contracted by at least 2% from the prior observation.\n",
        );
    }

    if rrp.is_some_and(|value| value >= 2_000.0) {
        out.push_str("- liquidity absorption critical: RRPONTSYD >= 2000.\n");
    } else if rrp.is_some_and(|value| value >= 1_000.0) {
        out.push_str("- liquidity absorption elevated: RRPONTSYD >= 1000.\n");
    }

    if out.ends_with("FRED threshold assessment:\n") {
        out.push_str("- threshold status neutral: no configured FRED macro threshold breached.\n");
    }
}

fn latest_value(observations: &BTreeMap<&str, Vec<f64>>, series: &str) -> Option<f64> {
    observations
        .get(series)
        .and_then(|values| values.first())
        .copied()
}

fn latest_change_ratio(observations: &BTreeMap<&str, Vec<f64>>, series: &str) -> Option<f64> {
    let values = observations.get(series)?;
    let latest = *values.first()?;
    let previous = *values.get(1)?;
    if previous == 0.0 {
        return None;
    }
    Some((latest - previous) / previous)
}

fn fred_numeric_observations(payload: &str) -> Result<Vec<f64>> {
    let value: serde_json::Value =
        serde_json::from_str(payload).context("Failed to parse FRED observations JSON")?;
    Ok(value
        .get("observations")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|observation| {
            observation
                .get("value")
                .and_then(|value| value.as_str())
                .and_then(|value| value.parse::<f64>().ok())
        })
        .collect())
}

fn configured_subjects(app_config: &config::AppConfig, requested: &[String]) -> Vec<String> {
    if !requested.is_empty() {
        return requested
            .iter()
            .map(|symbol| symbol.trim().to_uppercase())
            .filter(|symbol| !symbol.is_empty())
            .collect();
    }
    app_config
        .watchlist
        .iter()
        .filter(|entry| entry.enable)
        .map(|entry| entry.symbol.to_uppercase())
        .collect()
}

async fn cache_source(
    save_dir: &Path,
    category: &str,
    subject: &str,
    file_name: &str,
    content: &str,
) -> Result<String> {
    let dir = save_dir
        .join("gray_rhino_sources")
        .join(category)
        .join(sanitize_path_segment(subject));
    tokio::fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("Failed to create {}", dir.display()))?;
    let path = dir.join(sanitize_path_segment(file_name));
    tokio::fs::write(&path, content)
        .await
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(path.to_string_lossy().to_string())
}

async fn append_discovery_run(
    request: &GrayRhinoSourceCollectionRequest,
    outcomes: &[GrayRhinoSourceCollectionOutcome],
) -> Result<()> {
    let path = request.save_dir.join("gray_rhino_discovery_runs.jsonl");
    tokio::fs::create_dir_all(&request.save_dir).await?;
    let record = GrayRhinoDiscoveryRunRecord {
        run_id: format!("{}-{}", request.provider.as_str(), request.as_of_date),
        provider: request.provider,
        as_of_date: request.as_of_date,
        dry_run: request.dry_run,
        source_count: outcomes.len(),
        accepted: outcomes.iter().filter(|outcome| outcome.accepted).count(),
        rejected: outcomes.iter().filter(|outcome| !outcome.accepted).count(),
        candidate_count: outcomes.iter().map(|outcome| outcome.candidate_count).sum(),
        outcomes: outcomes.to_vec(),
    };
    let line = serde_json::to_string(&record)?;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    use tokio::io::AsyncWriteExt;
    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    Ok(())
}

fn planned_outcome(subject: String, message: &str) -> GrayRhinoFetchOutcome {
    GrayRhinoFetchOutcome::Planned {
        subject,
        message: message.to_string(),
    }
}

fn rejected_outcome(subject: String, taxonomy: &str, message: String) -> GrayRhinoFetchOutcome {
    GrayRhinoFetchOutcome::Rejected {
        subject,
        taxonomy: taxonomy.to_string(),
        message,
    }
}

fn content_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
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

fn redact_token(url: &str) -> String {
    url.split("&token=")
        .next()
        .map(|prefix| format!("{prefix}&token=REDACTED"))
        .unwrap_or_else(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finnhub_text_preserves_narrative_crowding_terms() {
        let text = render_finnhub_narrative_text(
            "NVDA",
            r#"[{"headline":"AI narrative overcrowding expands around mega cap leadership","summary":"Market narrative concentration is high and investors cite single supplier dependency.","url":"https://example.com"}]"#,
        )
        .unwrap();

        assert!(text.contains("narrative overcrowding"));
        assert!(text.contains("single supplier dependency"));
    }

    #[test]
    fn fred_text_emits_market_gray_rhino_terms() {
        let payload = r#"{"observations":[{"date":"2026-05-25","value":"4.50"}]}"#.to_string();
        let text = render_fred_macro_text(&[("DGS10".to_string(), payload)]).unwrap();

        assert!(text.contains("FRED threshold assessment"));
        assert!(text.contains("rate pressure elevated"));
        assert!(text.contains("capex payback risk"));
    }

    #[test]
    fn fred_threshold_assessment_emits_critical_terms() {
        let dgs10 = r#"{"observations":[{"date":"2026-05-25","value":"5.10"}]}"#.to_string();
        let spread = r#"{"observations":[{"date":"2026-05-25","value":"7.20"}]}"#.to_string();
        let walcl = r#"{"observations":[{"date":"2026-05-25","value":"900"},{"date":"2026-05-18","value":"1000"}]}"#.to_string();
        let text = render_fred_macro_text(&[
            ("DGS10".to_string(), dgs10),
            ("BAMLH0A0HYM2".to_string(), spread),
            ("WALCL".to_string(), walcl),
        ])
        .unwrap();

        assert!(text.contains("rate pressure critical"));
        assert!(text.contains("credit stress critical"));
        assert!(text.contains("liquidity fragility critical"));
    }
}

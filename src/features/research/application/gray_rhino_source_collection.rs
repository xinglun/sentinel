use crate::features::research::application::gray_rhino_discovery::{
    discover_gray_rhino_candidates, GrayRhinoDiscoveryInput,
};
use crate::features::research::domain::gray_rhino_candidate::GrayRhinoCandidate;
use anyhow::Result;
use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum GrayRhinoSourceProvider {
    Sec,
    Finnhub,
    Fred,
}

impl GrayRhinoSourceProvider {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        match raw.to_ascii_lowercase().as_str() {
            "sec" => Some(Self::Sec),
            "finnhub" => Some(Self::Finnhub),
            "fred" => Some(Self::Fred),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Sec => "sec",
            Self::Finnhub => "finnhub",
            Self::Fred => "fred",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GrayRhinoSourceCollectionRequest {
    pub provider: GrayRhinoSourceProvider,
    pub symbols: Vec<String>,
    pub as_of_date: NaiveDate,
    pub lookback_days: usize,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GrayRhinoSourceCollectionOutcome {
    pub provider: GrayRhinoSourceProvider,
    pub subject: String,
    pub planned: bool,
    pub accepted: bool,
    pub source_url: Option<String>,
    pub repository_path: Option<String>,
    pub content_sha256: Option<String>,
    pub candidate_count: usize,
    pub failure_taxonomy: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub(crate) struct GrayRhinoFetchedSource {
    pub subject: String,
    pub source_title: String,
    pub source_published_at: NaiveDate,
    pub content: String,
    pub source_url: Option<String>,
    pub repository_path: Option<String>,
    pub content_sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum GrayRhinoFetchOutcome {
    Planned {
        subject: String,
        message: String,
    },
    Accepted(GrayRhinoFetchedSource),
    Rejected {
        subject: String,
        taxonomy: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GrayRhinoDiscoveryRunRecord {
    pub run_id: String,
    pub provider: GrayRhinoSourceProvider,
    pub as_of_date: NaiveDate,
    pub dry_run: bool,
    pub source_count: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub candidate_count: usize,
    pub outcomes: Vec<GrayRhinoSourceCollectionOutcome>,
}

pub(crate) trait GrayRhinoSourceFetcherPort {
    async fn fetch_sources(
        &self,
        request: &GrayRhinoSourceCollectionRequest,
    ) -> Result<Vec<GrayRhinoFetchOutcome>>;
}

pub(crate) trait GrayRhinoCandidateRepositoryPort {
    fn save_candidates(&self, candidates: &[GrayRhinoCandidate]) -> Result<usize>;
}

pub(crate) trait GrayRhinoDiscoveryRunRepositoryPort {
    async fn append_discovery_run(
        &self,
        request: &GrayRhinoSourceCollectionRequest,
        outcomes: &[GrayRhinoSourceCollectionOutcome],
    ) -> Result<()>;
}

pub(crate) struct CollectGrayRhinoSourcesUseCase<'a, F, C, R>
where
    F: GrayRhinoSourceFetcherPort,
    C: GrayRhinoCandidateRepositoryPort,
    R: GrayRhinoDiscoveryRunRepositoryPort,
{
    fetcher: &'a F,
    candidate_repository: &'a C,
    run_repository: &'a R,
}

impl<'a, F, C, R> CollectGrayRhinoSourcesUseCase<'a, F, C, R>
where
    F: GrayRhinoSourceFetcherPort,
    C: GrayRhinoCandidateRepositoryPort,
    R: GrayRhinoDiscoveryRunRepositoryPort,
{
    pub(crate) fn new(fetcher: &'a F, candidate_repository: &'a C, run_repository: &'a R) -> Self {
        Self {
            fetcher,
            candidate_repository,
            run_repository,
        }
    }

    pub(crate) async fn collect(
        &self,
        request: &GrayRhinoSourceCollectionRequest,
    ) -> Result<Vec<GrayRhinoSourceCollectionOutcome>> {
        let fetched = self.fetcher.fetch_sources(request).await?;
        let mut outcomes = Vec::new();
        for fetch in fetched {
            match fetch {
                GrayRhinoFetchOutcome::Planned { subject, message } => {
                    outcomes.push(GrayRhinoSourceCollectionOutcome {
                        provider: request.provider,
                        subject,
                        planned: true,
                        accepted: true,
                        source_url: None,
                        repository_path: None,
                        content_sha256: None,
                        candidate_count: 0,
                        failure_taxonomy: None,
                        message,
                    });
                }
                GrayRhinoFetchOutcome::Accepted(source) => {
                    let mut candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
                        subject: source.subject.clone(),
                        source_title: source.source_title.clone(),
                        observed_at: source.source_published_at,
                        text: source.content,
                    });
                    for candidate in &mut candidates {
                        candidate.source_published_at = Some(source.source_published_at);
                        candidate.last_confirmed_at = Some(request.as_of_date);
                    }
                    self.candidate_repository.save_candidates(&candidates)?;
                    outcomes.push(GrayRhinoSourceCollectionOutcome {
                        provider: request.provider,
                        subject: source.subject,
                        planned: false,
                        accepted: true,
                        source_url: source.source_url,
                        repository_path: source.repository_path,
                        content_sha256: source.content_sha256,
                        candidate_count: candidates.len(),
                        failure_taxonomy: None,
                        message: "source cached for Gray Rhino discovery".to_string(),
                    });
                }
                GrayRhinoFetchOutcome::Rejected {
                    subject,
                    taxonomy,
                    message,
                } => outcomes.push(GrayRhinoSourceCollectionOutcome {
                    provider: request.provider,
                    subject,
                    planned: false,
                    accepted: false,
                    source_url: None,
                    repository_path: None,
                    content_sha256: None,
                    candidate_count: 0,
                    failure_taxonomy: Some(taxonomy),
                    message,
                }),
            }
        }
        outcomes.sort_by(|a, b| a.subject.cmp(&b.subject));
        self.run_repository
            .append_discovery_run(request, &outcomes)
            .await?;
        Ok(outcomes)
    }
}

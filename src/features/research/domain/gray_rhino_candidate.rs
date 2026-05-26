use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrayRhinoCandidateScope {
    Company,
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrayRhinoCandidateKind {
    GovernanceConcentration,
    DependencyConcentration,
    InstitutionalMaturityGap,
    RedundancyGap,
    MarketConcentration,
    NarrativeCrowding,
    LiquidityFragility,
    CapexPaybackFragility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrayRhinoCandidateState {
    Background,
    Visible,
    Expanding,
    Critical,
    Cooling,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrayRhinoCandidate {
    pub scope: GrayRhinoCandidateScope,
    pub kind: GrayRhinoCandidateKind,
    pub subject: String,
    pub state: GrayRhinoCandidateState,
    pub evidence: Vec<String>,
    pub watch_triggers: Vec<String>,
    pub source_title: String,
    pub observed_at: NaiveDate,
    #[serde(default)]
    pub source_published_at: Option<NaiveDate>,
    #[serde(default)]
    pub last_confirmed_at: Option<NaiveDate>,
    #[serde(default)]
    pub resolved_at: Option<NaiveDate>,
}

impl GrayRhinoCandidate {
    pub(crate) fn last_confirmed_at(&self) -> NaiveDate {
        self.last_confirmed_at.unwrap_or(self.observed_at)
    }

    pub(crate) fn resolved_at(&self) -> Option<NaiveDate> {
        self.resolved_at
    }
}

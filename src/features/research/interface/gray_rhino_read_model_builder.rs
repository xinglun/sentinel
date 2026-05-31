use crate::features::research::application::gray_rhino_monitoring_state::GrayRhinoMonitoringStatus;
use crate::features::research::domain::gray_rhino_candidate::{
    GrayRhinoCandidate, GrayRhinoCandidateScope,
};
use std::collections::BTreeMap;

/// 会社単位の灰犀牛候補を subject ごとにまとめる。
pub(crate) fn group_company_candidates(
    candidates: &[GrayRhinoCandidate],
) -> BTreeMap<String, Vec<&GrayRhinoCandidate>> {
    let mut by_subject: BTreeMap<String, Vec<&GrayRhinoCandidate>> = BTreeMap::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.scope == GrayRhinoCandidateScope::Company)
    {
        by_subject
            .entry(candidate.subject.to_uppercase())
            .or_default()
            .push(candidate);
    }
    by_subject
}

/// 会社単位の灰犀牛監視状態を subject ごとにまとめる。
pub(crate) fn group_company_statuses(
    statuses: &[GrayRhinoMonitoringStatus],
) -> BTreeMap<String, Vec<&GrayRhinoMonitoringStatus>> {
    let mut by_subject: BTreeMap<String, Vec<&GrayRhinoMonitoringStatus>> = BTreeMap::new();
    for status in statuses
        .iter()
        .filter(|status| status.scope == GrayRhinoCandidateScope::Company)
    {
        by_subject
            .entry(status.subject.to_uppercase())
            .or_default()
            .push(status);
    }
    by_subject
}

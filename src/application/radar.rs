/// Radar run の data acquisition 結果概要。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataAcquisitionSummary {
    pub successful_fetches: usize,
    pub failed_fetches: usize,
}

impl DataAcquisitionSummary {
    pub fn new(successful_fetches: usize, failed_fetches: usize) -> Self {
        Self {
            successful_fetches,
            failed_fetches,
        }
    }

    /// run history へ正式な decision packet を保存してよいかを判定する。
    pub fn should_persist_decision_history(self) -> bool {
        should_persist_decision_history(self.successful_fetches, self.failed_fetches)
    }

    /// すべての取得対象が失敗したかを返す。
    pub fn is_full_failure(self) -> bool {
        self.successful_fetches == 0 && self.failed_fetches > 0
    }
}

/// データ取得結果に基づく decision history persistence policy。
pub fn should_persist_decision_history(successful_fetches: usize, failed_fetches: usize) -> bool {
    successful_fetches > 0 || failed_fetches == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radar_application_boundary_persists_when_at_least_one_fetch_succeeds() {
        let summary = DataAcquisitionSummary::new(1, 8);
        assert!(summary.should_persist_decision_history());
        assert!(!summary.is_full_failure());
    }

    #[test]
    fn radar_application_boundary_does_not_persist_full_fetch_failure() {
        let summary = DataAcquisitionSummary::new(0, 9);
        assert!(!summary.should_persist_decision_history());
        assert!(summary.is_full_failure());
    }

    #[test]
    fn radar_application_boundary_empty_fetch_set_is_non_failure() {
        let summary = DataAcquisitionSummary::new(0, 0);
        assert!(summary.should_persist_decision_history());
        assert!(!summary.is_full_failure());
    }
}

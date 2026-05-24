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

/// Radar run の data acquisition 成功・失敗結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataAcquisitionResult<T> {
    pub successful_items: Vec<T>,
    pub failed_symbols: Vec<String>,
}

impl<T> DataAcquisitionResult<T> {
    pub fn new(successful_items: Vec<T>, failed_symbols: Vec<String>) -> Self {
        Self {
            successful_items,
            failed_symbols,
        }
    }

    /// 成功・失敗件数を application policy 用の summary へ変換する。
    pub fn summary(&self) -> DataAcquisitionSummary {
        DataAcquisitionSummary::new(self.successful_items.len(), self.failed_symbols.len())
    }

    /// run history へ正式な decision packet を保存してよいかを判定する。
    pub fn should_persist_decision_history(&self) -> bool {
        self.summary().should_persist_decision_history()
    }

    /// すべての取得対象が失敗したかを返す。
    pub fn is_full_failure(&self) -> bool {
        self.summary().is_full_failure()
    }

    pub fn into_parts(self) -> (Vec<T>, Vec<String>) {
        (self.successful_items, self.failed_symbols)
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

    #[test]
    fn radar_application_boundary_result_summarizes_fetch_outcome() {
        let result = DataAcquisitionResult::new(vec!["AAPL"], vec!["MSFT".to_string()]);
        let summary = result.summary();

        assert_eq!(summary.successful_fetches, 1);
        assert_eq!(summary.failed_fetches, 1);
        assert!(result.should_persist_decision_history());
        assert!(!result.is_full_failure());
    }

    #[test]
    fn radar_application_boundary_result_preserves_parts() {
        let result = DataAcquisitionResult::new(vec!["AAPL"], vec!["MSFT".to_string()]);
        let (successful_items, failed_symbols) = result.into_parts();

        assert_eq!(successful_items, vec!["AAPL"]);
        assert_eq!(failed_symbols, vec!["MSFT".to_string()]);
    }
}

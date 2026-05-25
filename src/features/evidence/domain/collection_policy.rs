/// Evidence collection の保存判断を表す domain policy。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceCollectionPolicy {
    persist: bool,
    retention_days: Option<i64>,
}

impl EvidenceCollectionPolicy {
    pub fn new(persist: bool, retention_days: Option<i64>) -> Self {
        Self {
            persist,
            retention_days: retention_days.filter(|days| *days > 0),
        }
    }

    pub fn requires_repository(self) -> bool {
        self.persist
    }

    pub fn retention_days(self) -> Option<i64> {
        self.retention_days
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_non_positive_retention_days() {
        let policy = EvidenceCollectionPolicy::new(true, Some(0));
        assert_eq!(policy.retention_days(), None);
    }
}

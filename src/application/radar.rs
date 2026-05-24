/// Radar run の data acquisition 結果概要。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataAcquisitionSummary {
    pub successful_fetches: usize,
    pub failed_fetches: usize,
}

/// Radar run の data quality status。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataQualityStatus {
    Ok,
    Warning,
    Critical,
}

impl DataQualityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warning => "WARNING",
            Self::Critical => "CRITICAL",
        }
    }
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

    /// data quality log 用の status を返す。
    pub fn data_quality_status(self) -> DataQualityStatus {
        if self.is_full_failure() {
            DataQualityStatus::Critical
        } else if self.failed_fetches > 0 {
            DataQualityStatus::Warning
        } else {
            DataQualityStatus::Ok
        }
    }
}

/// Radar run の data acquisition 成功・失敗結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataAcquisitionResult<T> {
    pub successful_items: Vec<T>,
    pub failed_symbols: Vec<String>,
}

/// account snapshot 保存用の入力値。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccountSnapshotInput<'a> {
    pub date: &'a str,
    pub global_budget: f64,
    pub max_daily_budget: Option<f64>,
    pub daily_traded: f64,
    pub buying_power: f64,
    pub current_exposure: f64,
    pub realized_pl: f64,
    pub failed_fetch_count: usize,
}

/// Radar decisioning の application-level outcome。
#[derive(Debug, Clone)]
pub struct RadarDecisionOutcome {
    pub packet: crate::core::decision::DecisionPacket,
    pub decisioning: crate::core::run_status::DeliveryStatus,
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

    /// data quality log 用の status を返す。
    pub fn data_quality_status(&self) -> DataQualityStatus {
        self.summary().data_quality_status()
    }

    pub fn into_parts(self) -> (Vec<T>, Vec<String>) {
        (self.successful_items, self.failed_symbols)
    }
}

/// データ取得結果に基づく decision history persistence policy。
pub fn should_persist_decision_history(successful_fetches: usize, failed_fetches: usize) -> bool {
    successful_fetches > 0 || failed_fetches == 0
}

/// 正常に生成された decision packet を application outcome へ変換する。
pub fn build_successful_decision_outcome(
    packet: crate::core::decision::DecisionPacket,
) -> RadarDecisionOutcome {
    RadarDecisionOutcome {
        packet,
        decisioning: crate::core::run_status::DeliveryStatus::Succeeded,
    }
}

/// decisioning failure 用の status を構築する。
pub fn build_decisioning_failure_status(
    reason: impl Into<String>,
) -> crate::core::run_status::DeliveryStatus {
    crate::core::run_status::DeliveryStatus::Failed {
        reason: reason.into(),
    }
}

/// 100% data acquisition failure 用の diagnostic packet を構築する。
pub fn build_diagnostic_packet(date: chrono::NaiveDate) -> crate::core::decision::DecisionPacket {
    crate::core::decision::DecisionPacket {
        date,
        ..Default::default()
    }
}

/// 100% data acquisition failure 用の decisioning status を構築する。
pub fn build_full_fetch_failure_status(
    failed_fetch_count: usize,
) -> crate::core::run_status::DeliveryStatus {
    build_decisioning_failure_status(format!(
        "100% data acquisition failure: {} symbols failed",
        failed_fetch_count
    ))
}

/// portfolio snapshot 保存用 payload を構築する。
pub fn build_portfolio_snapshot(
    date: &str,
    realized_pl: f64,
    current_exposure: f64,
    positions: &std::collections::HashMap<String, (f64, f64)>,
) -> serde_json::Value {
    serde_json::json!({
        "date": date,
        "realized_pl": realized_pl,
        "current_exposure": current_exposure,
        "position_count": positions.len(),
        "positions": positions.iter().map(|(symbol, (qty, avg_price))| {
            serde_json::json!({
                "symbol": symbol,
                "qty": qty,
                "avg_price": avg_price,
                "market_value_estimate": qty * avg_price,
            })
        }).collect::<Vec<_>>()
    })
}

/// account snapshot 保存用 payload を構築する。
pub fn build_account_snapshot(input: AccountSnapshotInput<'_>) -> serde_json::Value {
    serde_json::json!({
        "date": input.date,
        "global_budget": input.global_budget,
        "max_daily_budget": input.max_daily_budget,
        "daily_traded": input.daily_traded,
        "buying_power_estimate": input.buying_power,
        "current_exposure": input.current_exposure,
        "realized_pl": input.realized_pl,
        "failed_fetch_count": input.failed_fetch_count,
    })
}

/// data quality log 保存用 payload を構築する。
pub fn build_data_quality_log(
    timestamp: &str,
    date: &str,
    summary: DataAcquisitionSummary,
    failed_symbols: &[String],
) -> serde_json::Value {
    serde_json::json!({
        "timestamp": timestamp,
        "date": date,
        "successful_fetches": summary.successful_fetches,
        "failed_fetches": summary.failed_fetches,
        "failed_symbols": failed_symbols,
        "status": summary.data_quality_status().as_str()
    })
}

/// run status 用の state machine summary を構築する。
pub fn build_state_machine_summary(
    prev_market_state: Option<crate::core::market_regime::MarketState>,
    current_market_state: crate::core::market_regime::MarketState,
    transition_audit: Option<&crate::core::market_regime::MarketTransitionAudit>,
    should_persist_history: bool,
) -> crate::core::run_status::StateMachineSummary {
    let mut summary = crate::core::run_status::StateMachineSummary {
        from_state: format!(
            "{:?}",
            prev_market_state.unwrap_or(crate::core::market_regime::MarketState::IGNITION)
        ),
        to_state: if should_persist_history {
            format!("{:?}", current_market_state)
        } else {
            "DATA_UNAVAILABLE".to_string()
        },
        ..Default::default()
    };

    if let Some(audit) = transition_audit {
        summary.reset_confirmed = audit.reset_gate_passed;
        summary.reset_blocked = audit.is_reset_blocked;
        summary.soft_reset_applied = audit.soft_reset_applied;
        summary.duration_locked = audit.duration_locked;
        summary.defensive_override = audit.defensive_override;
        summary.core_breakdown = audit.core_breakdown;
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::market_regime::{LifecycleState, MarketState, MarketTransitionAudit};
    use std::collections::HashMap;

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
        assert_eq!(summary.data_quality_status(), DataQualityStatus::Ok);
    }

    #[test]
    fn radar_application_boundary_result_summarizes_fetch_outcome() {
        let result = DataAcquisitionResult::new(vec!["AAPL"], vec!["MSFT".to_string()]);
        let summary = result.summary();

        assert_eq!(summary.successful_fetches, 1);
        assert_eq!(summary.failed_fetches, 1);
        assert!(result.should_persist_decision_history());
        assert!(!result.is_full_failure());
        assert_eq!(result.data_quality_status(), DataQualityStatus::Warning);
    }

    #[test]
    fn radar_application_boundary_result_preserves_parts() {
        let result = DataAcquisitionResult::new(vec!["AAPL"], vec!["MSFT".to_string()]);
        let (successful_items, failed_symbols) = result.into_parts();

        assert_eq!(successful_items, vec!["AAPL"]);
        assert_eq!(failed_symbols, vec!["MSFT".to_string()]);
    }

    #[test]
    fn radar_application_boundary_status_matches_existing_policy() {
        assert_eq!(
            DataAcquisitionSummary::new(1, 0)
                .data_quality_status()
                .as_str(),
            "OK"
        );
        assert_eq!(
            DataAcquisitionSummary::new(1, 8)
                .data_quality_status()
                .as_str(),
            "WARNING"
        );
        assert_eq!(
            DataAcquisitionSummary::new(0, 9)
                .data_quality_status()
                .as_str(),
            "CRITICAL"
        );
    }

    #[test]
    fn radar_application_boundary_builds_persistence_payloads() {
        let mut positions = HashMap::new();
        positions.insert("NVDA".to_string(), (2.0, 100.0));
        let portfolio = build_portfolio_snapshot("2026-05-24", 12.5, 200.0, &positions);
        let account = build_account_snapshot(AccountSnapshotInput {
            date: "2026-05-24",
            global_budget: 1000.0,
            max_daily_budget: Some(250.0),
            daily_traded: 50.0,
            buying_power: 800.0,
            current_exposure: 200.0,
            realized_pl: 12.5,
            failed_fetch_count: 1,
        });
        let failed_symbols = vec!["MSFT".to_string()];
        let data_quality = build_data_quality_log(
            "2026-05-24T00:00:00+09:00",
            "2026-05-24",
            DataAcquisitionSummary::new(1, 1),
            &failed_symbols,
        );

        assert_eq!(portfolio["date"], "2026-05-24");
        assert_eq!(portfolio["position_count"], 1);
        assert_eq!(portfolio["positions"][0]["market_value_estimate"], 200.0);
        assert_eq!(account["failed_fetch_count"], 1);
        assert_eq!(account["buying_power_estimate"], 800.0);
        assert_eq!(data_quality["status"], "WARNING");
        assert_eq!(data_quality["failed_symbols"][0], "MSFT");
    }

    #[test]
    fn radar_application_boundary_builds_full_failure_diagnostic_output() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        let packet = build_diagnostic_packet(date);
        let status = build_full_fetch_failure_status(9);

        assert_eq!(packet.date, date);
        assert!(packet.assets.is_empty());
        assert_eq!(
            status,
            crate::core::run_status::DeliveryStatus::Failed {
                reason: "100% data acquisition failure: 9 symbols failed".to_string()
            }
        );
    }

    #[test]
    fn radar_application_boundary_builds_decision_outcome_status() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 24).unwrap();
        let packet = build_diagnostic_packet(date);
        let outcome = build_successful_decision_outcome(packet.clone());
        let failed = build_decisioning_failure_status("engine failed");

        assert_eq!(outcome.packet.date, packet.date);
        assert!(outcome.packet.assets.is_empty());
        assert_eq!(
            outcome.decisioning,
            crate::core::run_status::DeliveryStatus::Succeeded
        );
        assert_eq!(
            failed,
            crate::core::run_status::DeliveryStatus::Failed {
                reason: "engine failed".to_string()
            }
        );
    }

    #[test]
    fn radar_application_boundary_builds_state_machine_summary() {
        let audit = MarketTransitionAudit {
            from: LifecycleState::IGNITION,
            to: LifecycleState::NEWBORN,
            is_reset_blocked: true,
            is_downgrade_clamped: false,
            core_breakdown: true,
            duration_locked: true,
            trend_dominant: false,
            reset_gate_passed: true,
            indicator_cap: LifecycleState::NEWBORN,
            soft_reset_applied: true,
            defensive_override: true,
        };

        let summary = build_state_machine_summary(
            Some(MarketState::IGNITION),
            MarketState::DEFENSIVE,
            Some(&audit),
            true,
        );

        assert_eq!(summary.from_state, "IGNITION");
        assert_eq!(summary.to_state, "DEFENSIVE");
        assert!(summary.reset_confirmed);
        assert!(summary.reset_blocked);
        assert!(summary.soft_reset_applied);
        assert!(summary.duration_locked);
        assert!(summary.defensive_override);
        assert!(summary.core_breakdown);

        let unavailable = build_state_machine_summary(None, MarketState::ESTABLISHED, None, false);
        assert_eq!(unavailable.from_state, "IGNITION");
        assert_eq!(unavailable.to_state, "DATA_UNAVAILABLE");
    }
}

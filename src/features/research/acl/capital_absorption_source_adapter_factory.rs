use crate::config;
use crate::features::research::application::capital_absorption::CapitalAbsorptionAutoSnapshot;
use crate::features::research::infrastructure::capital_absorption_ipo_queue_store::load_ipo_queue_weekly_summary;
use crate::features::research::infrastructure::capital_absorption_source_adapter::build_automatic_capital_absorption_snapshot;
use chrono::NaiveDate;

/// Capital Absorption source adapter の infrastructure 実装を interface から隠蔽する。
pub(crate) async fn build_capital_absorption_auto_snapshot(
    app_config: &config::AppConfig,
    as_of_date: NaiveDate,
    lookback_days: usize,
) -> CapitalAbsorptionAutoSnapshot {
    build_automatic_capital_absorption_snapshot(app_config, as_of_date, lookback_days).await
}

/// IPO Queue ledger の weekly summary 読み出しを interface から隠蔽する。
pub(crate) fn build_capital_absorption_ipo_queue_weekly_summary(
    save_dir: &std::path::Path,
    as_of_date: NaiveDate,
) -> serde_json::Value {
    load_ipo_queue_weekly_summary(save_dir, as_of_date)
}

#[cfg(test)]
mod tests {
    use super::build_capital_absorption_ipo_queue_weekly_summary;
    use chrono::NaiveDate;
    use tempfile::tempdir;

    #[test]
    fn weekly_capital_absorption_context_replays_ledger_without_future_records() {
        let tmp = tempdir().unwrap();
        std::fs::write(
            tmp.path().join("capital_absorption_ipo_queue_history.jsonl"),
            r#"{"date":"2026-06-05","queue_count":1,"reported_count":1,"confirmed_count":0,"pressure":"NORMAL","items":[]}
{"date":"2026-06-08","queue_count":3,"reported_count":2,"confirmed_count":1,"pressure":"ELEVATED","items":[]}
{"date":"2026-06-12","queue_count":6,"reported_count":6,"confirmed_count":2,"pressure":"ELEVATED","items":[]}
"#,
        )
        .unwrap();

        let summary = build_capital_absorption_ipo_queue_weekly_summary(
            tmp.path(),
            NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
        );

        assert_eq!(summary["configured"], serde_json::Value::Bool(true));
        assert_eq!(
            summary["latest_date"],
            serde_json::Value::String("2026-06-08".to_string())
        );
        assert_eq!(summary["queue_count_latest"], serde_json::Value::from(3));
        assert_eq!(summary["queue_count_min_7d"], serde_json::Value::from(1));
        assert_eq!(summary["queue_count_max_7d"], serde_json::Value::from(3));
        assert_eq!(summary["reported_count_latest"], serde_json::Value::from(2));
        assert_eq!(
            summary["confirmed_count_latest"],
            serde_json::Value::from(1)
        );
    }
}

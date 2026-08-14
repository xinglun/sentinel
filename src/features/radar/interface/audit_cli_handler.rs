use crate::features::radar::interface::audit_daily_report::{
    audit_empty_log_message, audit_error_parse_date, build_audit_daily_report_with_formal_baseline,
    load_transition_audit_days, resolve_audit_daily_formal_baseline, resolve_target_index,
};
use crate::features::shared::acl::notification_factory::load_run_evidence_collection_status;
use crate::features::shared::interface::i18n::Language;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::path::Path;

/// audit-daily command の既存 orchestration を保持したまま実行する。
pub(crate) fn run_audit_daily(
    save_dir: &Path,
    target_date_arg: Option<&str>,
    window_days: usize,
    language: Language,
) -> Result<()> {
    let path = save_dir.join("state_transitions.jsonl");
    let days = load_transition_audit_days(&path, language)?;
    if days.is_empty() {
        println!("{}", audit_empty_log_message(language));
        return Ok(());
    }

    let target_date = match target_date_arg {
        Some(raw) => Some(
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .with_context(|| format!("{}: {}", audit_error_parse_date(language), raw))?,
        ),
        None => None,
    };

    let target_idx = resolve_target_index(&days, target_date, language)?;
    let evidence_collection_status =
        load_run_evidence_collection_status(save_dir, days[target_idx].date)
            .unwrap_or(crate::features::shared::application::run_status::DeliveryStatus::Skipped);
    let formal_baseline =
        resolve_audit_daily_formal_baseline(save_dir, days[target_idx].date).unwrap_or(None);
    let report = build_audit_daily_report_with_formal_baseline(
        &days,
        target_idx,
        window_days.max(1),
        language,
        Some(&evidence_collection_status),
        Some(formal_baseline.as_ref()),
    );
    println!("{}", report);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run_audit_daily;
    use crate::features::shared::interface::i18n::Language;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    fn write_transition_log(directory: &std::path::Path) {
        let transition = json!({
            "timestamp": "2026-04-22T15:00:00+00:00",
            "date": "2026-04-22",
            "transition": serde_json::to_value(
                crate::features::radar::domain::transition_log::StateTransitionLog::default()
            )
            .unwrap(),
        });
        fs::write(
            directory.join("state_transitions.jsonl"),
            format!("{}\n", transition),
        )
        .unwrap();
    }

    #[test]
    fn audit_daily_handler_keeps_empty_log_behavior() {
        let directory = tempdir().unwrap();

        run_audit_daily(directory.path(), None, 7, Language::ZhCn).unwrap();
    }

    #[test]
    fn audit_daily_handler_keeps_date_validation_behavior() {
        let directory = tempdir().unwrap();
        write_transition_log(directory.path());

        let error = run_audit_daily(directory.path(), Some("invalid"), 7, Language::EnUs)
            .expect_err("invalid date must remain an error");

        assert!(!error.to_string().is_empty());
    }

    #[test]
    fn audit_daily_handler_keeps_report_execution_behavior() {
        let directory = tempdir().unwrap();
        write_transition_log(directory.path());

        run_audit_daily(directory.path(), None, 0, Language::JaJp).unwrap();
    }
}

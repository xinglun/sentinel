use crate::features::research::domain::governance_source::GovernanceExtractionAuditRecord;
use crate::features::research::interface::gray_rhino_i18n_adapter::{
    governance_sensor_accepted_label, governance_sensor_boundary_label,
    governance_sensor_coverage_label, governance_sensor_health_heading,
    governance_sensor_latest_label, governance_sensor_rejected_label,
    governance_sensor_source_count_label,
};
use crate::features::shared::interface::i18n::Language;

/// ガバナンス証拠の収集健全性を表示用 Markdown に変換する。
pub(crate) fn render_governance_sensor_health(
    audits: &[GovernanceExtractionAuditRecord],
    language: Language,
) -> String {
    if audits.is_empty() {
        return String::new();
    }
    let source_count = audits.len();
    let accepted_count = audits.iter().filter(|audit| audit.accepted).count();
    let rejected_count = source_count.saturating_sub(accepted_count);
    let latest_observed = audits.iter().map(|audit| audit.observed_at).max();
    let coverage_ratio = accepted_count as f64 / source_count as f64;

    let mut out = String::new();
    out.push_str(governance_sensor_health_heading(language));
    out.push('\n');
    out.push_str(&format!(
        "- {}: {}\n",
        governance_sensor_source_count_label(language),
        source_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        governance_sensor_accepted_label(language),
        accepted_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        governance_sensor_rejected_label(language),
        rejected_count
    ));
    out.push_str(&format!(
        "- {}: {:.1}%\n",
        governance_sensor_coverage_label(language),
        coverage_ratio * 100.0
    ));
    if let Some(latest) = latest_observed {
        out.push_str(&format!(
            "- {}: {}\n",
            governance_sensor_latest_label(language),
            latest
        ));
    }
    out.push_str(governance_sensor_boundary_label(language));
    out
}

use crate::features::research::application::gray_rhino_daily_report::{
    BackfillOpsSummary, DiscoveryOpsSummary, GrayRhinoRefreshStatus,
};
use crate::features::research::interface::gray_rhino_i18n_adapter::{
    auto_discovery_ops_title, backfill_ops_title, candidate_count_label, drift_sources_label,
    failed_providers_label, failed_sources_label, latest_run_label, refresh_coverage_label,
    refresh_date_label, refresh_overall_status_label, refresh_reason_label,
    refresh_status_boundary, refresh_status_title, refresh_status_value_label, source_count_label,
    stale_sources_label,
};

/// 灰犀牛 backfill 運用状態を report fragment に変換する。
pub(crate) fn render_backfill_ops_view(
    value: Option<&BackfillOpsSummary>,
    language: Language,
) -> Option<String> {
    let value = value?;
    let mut out = String::new();
    out.push_str(backfill_ops_title(language));
    out.push_str(&format!(
        "- {}: {}\n",
        latest_run_label(language),
        value.run_id
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        source_count_label(language),
        value.source_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        failed_sources_label(language),
        value.rejected
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        stale_sources_label(language),
        value.stale_sources
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        drift_sources_label(language),
        value.drift_sources
    ));
    Some(out)
}

/// 灰犀牛自動発見の運用状態を report fragment に変換する。
pub(crate) fn render_discovery_ops_view(
    value: Option<&DiscoveryOpsSummary>,
    language: Language,
) -> Option<String> {
    let value = value?;
    let mut out = String::new();
    out.push_str(auto_discovery_ops_title(language));
    out.push_str(&format!(
        "- {}: {}\n",
        latest_run_label(language),
        value.run_id
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        source_count_label(language),
        value.source_count
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        candidate_count_label(language),
        value.candidate_count
    ));
    Some(out)
}

/// 灰犀牛収集状態を report fragment に変換する。
pub(crate) fn render_refresh_status(
    value: Option<&GrayRhinoRefreshStatus>,
    language: Language,
) -> Option<String> {
    let value = value?;
    let mut out = String::new();
    out.push_str(refresh_status_title(language));
    out.push_str(&format!(
        "- {}: {}\n",
        refresh_overall_status_label(language),
        refresh_status_value_label(&value.status, language)
    ));
    out.push_str(&format!(
        "- SEC: {} / Finnhub: {} / FRED: {}\n",
        refresh_status_value_label(&value.sec, language),
        refresh_status_value_label(&value.finnhub, language),
        refresh_status_value_label(&value.fred, language)
    ));
    out.push_str(&format!(
        "- {}: SEC {}/{} / Finnhub {}/{} / FRED {}/{}\n",
        refresh_coverage_label(language),
        value.sec_accepted,
        value.sec_accepted + value.sec_rejected,
        value.finnhub_accepted,
        value.finnhub_accepted + value.finnhub_rejected,
        value.fred_accepted,
        value.fred_accepted + value.fred_rejected
    ));
    if !value.failed_providers.trim().is_empty() {
        out.push_str(&format!(
            "- {}: {}\n",
            failed_providers_label(language),
            value.failed_providers.trim()
        ));
    }
    if let Some(date) = &value.date {
        out.push_str(&format!("- {}: {}\n", refresh_date_label(language), date));
    }
    if let Some(reason) = &value.reason {
        out.push_str(&format!(
            "- {}: {}\n",
            refresh_reason_label(language),
            reason
        ));
    }
    out.push_str(refresh_status_boundary(language));
    Some(out)
}

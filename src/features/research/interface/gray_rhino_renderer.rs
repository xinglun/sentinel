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

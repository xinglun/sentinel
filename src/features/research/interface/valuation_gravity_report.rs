use crate::features::research::application::valuation_gravity::{
    ValuationGravityObservation, ValuationPersistenceHealth,
};
use crate::features::research::interface::valuation_gravity_i18n as text;
use crate::features::shared::interface::i18n::Language;

pub(crate) fn build_valuation_gravity_report(
    observation: &ValuationGravityObservation,
    language: Language,
) -> String {
    let mut out = String::new();
    out.push_str(text::title(language));
    out.push_str("\n\n");
    out.push_str(text::observation_notice(language));
    out.push_str(&format!(
        "\n- {}: {}",
        text::persistence_label(language),
        text::persistence(
            observation.persistence_health,
            observation.persistence_reason,
            language
        )
    ));
    if observation.persistence_health == ValuationPersistenceHealth::Failed
        && !observation.persistence_detail.is_empty()
    {
        out.push_str(&format!(
            "\n- {}: {}",
            text::persistence_detail_label(language),
            text::persistence_failure_detail(observation.persistence_reason, language)
        ));
    }
    for asset in &observation.snapshot.assets {
        out.push_str(&format!("\n\n### {}\n", asset.symbol));
        match (asset.gravity, asset.confidence, asset.source) {
            (Some(gravity), Some(confidence), Some(source)) => {
                out.push_str(&format!(
                    "- {}: {}\n- {}: {}\n- {}: {}\n- {}: {}\n- {}: {}",
                    text::gravity_label(language),
                    text::gravity(gravity, language),
                    text::confidence_label(language),
                    text::confidence(confidence, language),
                    text::source_label(language),
                    text::source(source, language),
                    text::provider_label(language),
                    asset.provider,
                    text::as_of_label(language),
                    asset.as_of_date,
                ));
            }
            _ => {
                out.push_str(&format!(
                    "- {}\n- {}: {}\n- {}: {}",
                    text::unavailable(language),
                    text::provider_label(language),
                    asset.provider,
                    text::as_of_label(language),
                    asset.as_of_date,
                ));
            }
        }
        out.push_str(&format!(
            "\n- {}: {}\n- {}: {}\n- {}: {}",
            text::source_health_label(language),
            text::source_health(asset.source_health, language),
            text::evidence_count_label(language),
            asset.evidence_count,
            text::quality_reason_label(language),
            text::quality_reason(asset.quality_reason, language),
        ));
    }
    out.push_str("\n\n");
    out.push_str(text::boundary(language));
    out
}

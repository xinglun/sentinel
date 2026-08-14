use crate::features::radar::domain::transition_log::StateTransitionLog;
use crate::features::radar::domain::trend_cohesion::{
    AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType, SubstantiveEvidence,
    TrendContinuationState,
};
use crate::features::shared::interface::i18n::DisplayDictionary;

pub(crate) struct TrendRecognitionReadModel {
    pub state: Option<String>,
    pub diffusion_score: Option<f64>,
    pub conviction_score: Option<f64>,
    pub lag_state: Option<String>,
    pub single_asset_decay: Option<String>,
    pub substantive_signals: Vec<String>,
    pub substantive_details: Vec<String>,
    pub price_confirmation_record_count: usize,
    pub evidence_quality_summary: Option<String>,
}

const MAX_DISPLAYED_SUBSTANTIVE_DETAILS: usize = 20;

fn select_display_records(records: &[AutomatedEvidenceRecord]) -> Vec<&AutomatedEvidenceRecord> {
    let mut selected: Vec<&AutomatedEvidenceRecord> = records.iter().collect();
    selected.sort_by(|left, right| {
        evidence_source_priority(left.source)
            .cmp(&evidence_source_priority(right.source))
            .then_with(|| right.event_date.cmp(&left.event_date))
    });
    selected.truncate(MAX_DISPLAYED_SUBSTANTIVE_DETAILS);
    selected
}

fn evidence_source_priority(source: EvidenceSourceType) -> u8 {
    match source {
        EvidenceSourceType::OfficialIR => 0,
        EvidenceSourceType::Manual => 1,
        EvidenceSourceType::PriceAction => 2,
        EvidenceSourceType::NewsMedia => 3,
    }
}

pub(crate) fn build_trend_recognition_read_model(
    log: &StateTransitionLog,
    dict: &DisplayDictionary,
) -> TrendRecognitionReadModel {
    let state = log.trend_recognition.as_ref().map(|tr| match tr.state {
        TrendContinuationState::None => dict.trend_recognition.state_none.clone(),
        TrendContinuationState::StructuralPersistence => {
            dict.trend_recognition.state_structural_persistence.clone()
        }
        TrendContinuationState::EarlyLeader => dict.trend_recognition.state_early_leader.clone(),
        TrendContinuationState::LeaderConfirmedFollowersLagging => dict
            .trend_recognition
            .state_leader_confirmed_followers_lagging
            .clone(),
        TrendContinuationState::Broadening => dict.trend_recognition.state_broadening.clone(),
        TrendContinuationState::Mature => dict.trend_recognition.state_mature.clone(),
    });
    let diffusion_score = log.trend_recognition.as_ref().map(|tr| tr.diffusion_score);
    let conviction_score = log.trend_recognition.as_ref().map(|tr| tr.conviction_score);
    let lag_state = log.trend_recognition.as_ref().and_then(|tr| {
        if tr.lag_state {
            Some(dict.trend_recognition.lag_alert.clone())
        } else {
            None
        }
    });
    let single_asset_decay = log
        .trend_recognition
        .as_ref()
        .and_then(|tr| match tr.state {
            TrendContinuationState::Broadening | TrendContinuationState::Mature => None,
            _ => Some(format!(
                "{}/{}",
                tr.single_asset_decay_day,
                tr.single_asset_decay_max.max(1)
            )),
        });

    let mut substantive_signals = Vec::new();
    let mut substantive_details = Vec::new();
    let mut price_confirmation_record_count = 0;
    let mut evidence_quality_summary = None;
    if let Some(sub) = log
        .trend_recognition
        .as_ref()
        .and_then(|tr| tr.substantive.as_ref())
    {
        let has_capex_payoff = sub.capex_payoff_signal
            || sub
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::CapexPayoff);
        let has_earnings_validation = sub.earnings_validation
            || sub
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::EarningsValidation);
        let has_order_visibility = sub.order_visibility
            || sub
                .records
                .iter()
                .any(|record| record.evidence_type == EvidenceType::OrderVisibility);

        if has_capex_payoff {
            substantive_signals.push(dict.trend_recognition.capex_payoff.clone());
        }
        if has_earnings_validation {
            substantive_signals.push(dict.trend_recognition.earnings_validation.clone());
        }
        if has_order_visibility {
            substantive_signals.push(dict.trend_recognition.order_visibility.clone());
        }
        price_confirmation_record_count = sub
            .records
            .iter()
            .filter(|record| record.evidence_type == EvidenceType::FollowThrough)
            .count();
        evidence_quality_summary = build_evidence_quality_summary(sub, dict);
        substantive_details = build_substantive_details(sub, dict);
    }

    TrendRecognitionReadModel {
        state,
        diffusion_score,
        conviction_score,
        lag_state,
        single_asset_decay,
        substantive_signals,
        substantive_details,
        price_confirmation_record_count,
        evidence_quality_summary,
    }
}

fn build_evidence_quality_summary(
    sub: &SubstantiveEvidence,
    dict: &DisplayDictionary,
) -> Option<String> {
    if sub.records.is_empty() {
        return None;
    }

    let mut high_quality = 0;
    let mut medium_quality = 0;
    let mut price_confirmation = 0;

    for record in &sub.records {
        match record.source {
            EvidenceSourceType::OfficialIR => high_quality += 1,
            EvidenceSourceType::Manual => medium_quality += 1,
            EvidenceSourceType::PriceAction => price_confirmation += 1,
            EvidenceSourceType::NewsMedia => {}
        }
    }

    let tr = &dict.trend_recognition;
    let mut parts = Vec::new();
    if high_quality > 0 {
        parts.push(format!("{} {}", tr.evidence_quality_high, high_quality));
    }
    if medium_quality > 0 {
        parts.push(format!("{} {}", tr.evidence_quality_medium, medium_quality));
    }
    if price_confirmation > 0 {
        parts.push(format!(
            "{} {}",
            tr.evidence_quality_price, price_confirmation
        ));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" / "))
    }
}

fn build_substantive_details(sub: &SubstantiveEvidence, dict: &DisplayDictionary) -> Vec<String> {
    let records = select_display_records(&sub.records);
    records
        .iter()
        .map(|record| {
            let source_label = match record.source {
                EvidenceSourceType::Manual => &dict.trend_recognition.source_manual,
                EvidenceSourceType::OfficialIR => &dict.trend_recognition.source_official_ir,
                EvidenceSourceType::NewsMedia => &dict.trend_recognition.source_news_media,
                EvidenceSourceType::PriceAction => &dict.trend_recognition.source_price_action,
            };
            let symbol_part = record
                .symbol
                .as_ref()
                .map(|s| format!("[{}] ", s))
                .unwrap_or_default();
            let url_part = record
                .source_url
                .as_ref()
                .map(|u| format!(" ({})", u))
                .unwrap_or_default();
            format!(
                "{} {}[{}] [{:?}] {} (Conf: {:.1}){}",
                source_label,
                symbol_part,
                record.event_date,
                record.evidence_type,
                record.description,
                record.confidence,
                url_part
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::select_display_records;
    use crate::features::shared::domain::evidence::{
        AutomatedEvidenceRecord, EvidenceSourceType, EvidenceType,
    };

    fn record(source: EvidenceSourceType, event_date: &str) -> AutomatedEvidenceRecord {
        AutomatedEvidenceRecord::new(
            source,
            EvidenceType::EarningsValidation,
            0.7,
            "evidence".to_string(),
            event_date.to_string(),
            Some("TSLA".to_string()),
            None,
            format!("{}:{}", event_date, event_date),
        )
    }

    #[test]
    fn display_selection_caps_details_and_prioritizes_formal_sources() {
        let mut records = vec![record(EvidenceSourceType::NewsMedia, "2026-08-10"); 25];
        records.push(record(EvidenceSourceType::OfficialIR, "2026-01-01"));

        let selected = select_display_records(&records);

        assert_eq!(selected.len(), 20);
        assert_eq!(selected[0].source, EvidenceSourceType::OfficialIR);
        assert_eq!(
            selected
                .iter()
                .filter(|item| item.source == EvidenceSourceType::NewsMedia)
                .count(),
            19
        );
    }
}

use super::presentation::{SignalContextInformationContent, SignalContextV1};
use crate::features::research::interface::macro_event_observation::{
    EvidenceRecord, MarketReaction, ObservationTimePrecision, TemporalBinding,
};
use crate::features::shared::interface::i18n::Language;
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 分類・ランキングから独立した観測専用の HIGH 証拠契約。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalInformationEvidence {
    pub primary_event_id: Option<String>,
    pub reaction_bindings: Vec<TemporalBinding>,
    pub information_level: SignalContextInformationContent,
    pub event_information_character: String,
    pub event_importance: String,
    pub temporal_association: String,
    pub causal_attribution: String,
    pub reaction_evidence_count: usize,
    pub reaction_dimensions: Vec<String>,
    pub high_information_route: String,
    pub high_information_evidence: Vec<String>,
    pub reaction_evidence: Vec<MarketReaction>,
    pub event_evidence: Vec<EvidenceRecord>,
    pub downgrade_reason: Option<String>,
    pub decision_weight: u8,
    pub trade_signal: bool,
}

fn time(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value.trim()).ok()
}

fn character(value: &str) -> Option<&str> {
    match value.trim() {
        "NEW_FACT"
        | "FORMAL_ACTION"
        | "ESCALATION"
        | "SCHEDULED_RELEASE"
        | "COMMENTARY"
        | "ANALYSIS"
        | "PROFILE"
        | "POLITICAL_POSITIONING" => Some(value.trim()),
        _ => None,
    }
}

fn within_window(v: &SignalContextV1, observed: DateTime<FixedOffset>) -> bool {
    let Some(run) = v.report_run_at.as_deref().and_then(time) else {
        return false;
    };
    if observed > run {
        return false;
    }
    let mut lower = None;
    let mut upper = run;
    if let Some(start) = &v.observation_window_start {
        let Some(start) = time(start) else {
            return false;
        };
        lower = Some(start);
    }
    if let Some(end) = &v.observation_window_end {
        let Some(end) = time(end) else {
            return false;
        };
        if end > run {
            return false;
        }
        upper = end;
    }
    lower.is_none_or(|lower| lower <= observed && lower <= upper) && observed <= upper
}

fn binding_valid(v: &SignalContextV1, r: &MarketReaction, b: &TemporalBinding) -> bool {
    v.primary_context.as_ref().is_some_and(|p| {
        b.temporal_eligible
            && b.event_id == p.event_id
            && b.observation_id == r.observation_id
            && b.observation_time_precision == ObservationTimePrecision::Timestamp
            && time(&b.source_published_at).is_some()
            && time(&b.source_published_at) == time(&p.source_published_at)
            && time(&b.observed_at).is_some()
            && time(&b.observed_at) == time(&r.observed_at)
    })
}

fn explicit_proof_valid(v: &SignalContextV1, e: &EvidenceRecord, event_character: &str) -> bool {
    let Some(primary) = &v.primary_context else {
        return false;
    };
    let Some(published) = time(&e.source_published_at) else {
        return false;
    };
    !primary.event_id.trim().is_empty()
        && matches!(
            event_character,
            "NEW_FACT" | "FORMAL_ACTION" | "ESCALATION" | "SCHEDULED_RELEASE"
        )
        && character(&e.event_type) == Some(event_character)
        && matches!(
            e.importance.as_str(),
            "UNEXPECTED" | "MATERIAL_NEW_ACTION" | "MATERIAL_ESCALATION"
        )
        && !e.source.trim().is_empty()
        && !e.source_url.trim().is_empty()
        && !e.subject.trim().is_empty()
        && time(&primary.source_published_at) == Some(published)
        && within_window(v, published)
}

fn reaction_valid(v: &SignalContextV1, r: &MarketReaction) -> bool {
    let Some(primary) = &v.primary_context else {
        return false;
    };
    let Some(proof) = &r.structured_reaction else {
        return false;
    };
    if v.report_run_at.as_deref().and_then(time).is_none() {
        return false;
    }
    let (Some(published), Some(observed)) =
        (time(&primary.source_published_at), time(&r.observed_at))
    else {
        return false;
    };
    if primary.event_id.trim().is_empty()
        || r.observation_id.trim().is_empty()
        || r.instrument.trim().is_empty()
        || r.observation_time_precision != ObservationTimePrecision::Timestamp
        || published > observed
        || !matches!(
            proof.reaction_dimension.as_str(),
            "rates" | "commodity" | "credit" | "equity/index" | "volatility" | "FX"
        )
        || proof.source.trim().is_empty()
        || proof.source_url.trim().is_empty()
        || proof.measurement_method.trim().is_empty()
        || proof.unit.trim().is_empty()
    {
        return false;
    }
    if !within_window(v, observed) {
        return false;
    }
    if !r.observation_date.is_empty() && r.observation_date != observed.date_naive().to_string() {
        return false;
    }
    let numbers = (
        proof.magnitude.parse::<f64>(),
        proof.baseline.parse::<f64>(),
        proof.significant_magnitude.parse::<f64>(),
    );
    let (Ok(magnitude), Ok(baseline), Ok(floor)) = numbers else {
        return false;
    };
    if !magnitude.is_finite()
        || !baseline.is_finite()
        || !floor.is_finite()
        || floor <= 0.0
        || magnitude.abs() < floor
        || !((magnitude > 0.0 && proof.direction == "UP")
            || (magnitude < 0.0 && proof.direction == "DOWN"))
    {
        return false;
    }
    v.temporal_bindings.iter().any(|b| binding_valid(v, r, b))
}

pub(crate) fn evaluate(
    v: &SignalContextV1,
    proposed: SignalContextInformationContent,
) -> SignalInformationEvidence {
    let primary = v.primary_context.as_ref();
    let event_evidence = primary.map(|p| p.evidence.clone()).unwrap_or_default();
    let characters = event_evidence
        .iter()
        .filter_map(|e| character(&e.event_type))
        .collect::<BTreeSet<_>>();
    let event_character = if characters.len() == 1 {
        *characters.first().unwrap()
    } else {
        "UNKNOWN"
    };
    let commentary = matches!(
        event_character,
        "COMMENTARY" | "ANALYSIS" | "PROFILE" | "POLITICAL_POSITIONING"
    );
    let mut ids = BTreeSet::new();
    let mut instruments = BTreeSet::new();
    let mut reactions = Vec::new();
    let mut candidates = v.observed_market_reactions.iter().collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        (&b.observed_at, &a.observation_id).cmp(&(&a.observed_at, &b.observation_id))
    });
    for r in candidates {
        let dimension = r
            .structured_reaction
            .as_ref()
            .map(|proof| &proof.reaction_dimension);
        if v.observed_market_reactions.iter().any(|other| {
            other.instrument == r.instrument
                && other
                    .structured_reaction
                    .as_ref()
                    .map(|proof| &proof.reaction_dimension)
                    != dimension
        }) {
            continue;
        }
        // 重複 identity の矛盾は入力順によらず除外する。
        if v.observed_market_reactions
            .iter()
            .filter(|other| other.observation_id == r.observation_id)
            .any(|other| other != r)
        {
            continue;
        }
        if reaction_valid(v, r)
            && ids.insert(r.observation_id.clone())
            && instruments.insert(r.instrument.clone())
        {
            reactions.push(r.clone());
        }
    }
    let dimensions = reactions
        .iter()
        .filter_map(|r| {
            r.structured_reaction
                .as_ref()
                .map(|p| p.reaction_dimension.clone())
        })
        .collect::<BTreeSet<_>>();
    let explicit_proofs = event_evidence
        .iter()
        .filter(|e| !commentary && explicit_proof_valid(v, e, event_character))
        .collect::<Vec<_>>();
    let explicit = !explicit_proofs.is_empty();
    let route = if dimensions.len() >= 2 {
        "STRUCTURED_MARKET_REACTION"
    } else if explicit {
        "EXPLICIT_SURPRISE_EVENT"
    } else if primary.is_none() {
        "UNAVAILABLE"
    } else {
        "NONE"
    };
    let proven = matches!(
        route,
        "STRUCTURED_MARKET_REACTION" | "EXPLICIT_SURPRISE_EVENT"
    );
    let downgraded = proposed == SignalContextInformationContent::High && !proven;
    let information_level = if downgraded {
        if primary.is_some() {
            SignalContextInformationContent::Medium
        } else {
            SignalContextInformationContent::Unknown
        }
    } else {
        proposed
    };
    let high_information_evidence = match route {
        "STRUCTURED_MARKET_REACTION" => {
            reactions.iter().map(|r| r.observation_id.clone()).collect()
        }
        "EXPLICIT_SURPRISE_EVENT" => explicit_proofs
            .iter()
            .map(|e| e.source_url.clone())
            .collect(),
        _ => Vec::new(),
    };
    let reaction_bindings = v
        .temporal_bindings
        .iter()
        .filter(|b| reactions.iter().any(|r| binding_valid(v, r, b)))
        .cloned()
        .collect();
    SignalInformationEvidence {
        primary_event_id: primary.map(|p| p.event_id.clone()),
        reaction_bindings,
        information_level,
        event_information_character: event_character.into(),
        event_importance: primary
            .map(|p| format!("{:?}", p.market_relevance).to_uppercase())
            .unwrap_or_else(|| "UNKNOWN".into()),
        temporal_association: if reactions.is_empty() {
            "UNAVAILABLE"
        } else {
            "SUPPORTED"
        }
        .into(),
        causal_attribution: "NOT_ESTABLISHED".into(),
        reaction_evidence_count: reactions.len(),
        reaction_dimensions: dimensions.into_iter().collect(),
        high_information_route: route.into(),
        high_information_evidence,
        reaction_evidence: reactions,
        event_evidence,
        downgrade_reason: downgraded.then(|| "HIGH_POSITIVE_EVIDENCE_MISSING".into()),
        decision_weight: 0,
        trade_signal: false,
    }
}

pub(crate) fn render(e: &SignalInformationEvidence, language: Language, archive: bool) -> String {
    let labels = match language {
        Language::ZhCn => [
            "信息证据",
            "事件特征",
            "事件重要性",
            "市场反应证据",
            "反应维度",
            "高信息量路径",
            "时间关联",
            "因果归因",
            "降级原因",
        ],
        Language::EnUs => [
            "Information Evidence",
            "Event Character",
            "Event Importance",
            "Market Reaction Evidence",
            "Reaction Dimensions",
            "High-information route",
            "Temporal Association",
            "Causal Attribution",
            "Downgrade Reason",
        ],
        Language::JaJp => [
            "情報証拠",
            "イベント特性",
            "イベント重要性",
            "市場反応証拠",
            "反応次元",
            "高情報量経路",
            "時間的関連",
            "因果帰属",
            "降格理由",
        ],
    };
    let mut output = format!("    - {}:\n      - {}: {}\n      - {}: {}\n      - {}: {}\n      - {}: {}\n      - {}: {}\n      - {}: {}\n      - {}: {}\n      - {}: {}\n      - decision_weight=0; trade_signal=false\n", labels[0], labels[1],e.event_information_character,labels[2],e.event_importance,labels[3],e.reaction_evidence_count,labels[4],if e.reaction_dimensions.is_empty() { "NONE".into() } else { e.reaction_dimensions.join(" / ") },labels[5],e.high_information_route,labels[6],e.temporal_association,labels[7],e.causal_attribution,labels[8],e.downgrade_reason.as_deref().unwrap_or("NONE"));
    if archive {
        if let Ok(json) = serde_json::to_string_pretty(e) {
            output.push_str(&format!("\n```json\n{json}\n```\n"));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::interface::presentation::{SignalContextItem, SignalContextType};
    use crate::features::research::interface::macro_event_observation::{
        build_temporal_binding, StructuredMarketReaction,
    };

    pub(super) fn context(character: &str, importance: &str) -> SignalContextV1 {
        SignalContextV1 {
            primary_context: Some(SignalContextItem {
                context_type: SignalContextType::Geopolitical,
                event_id: "event-1".into(),
                title: "An event".into(),
                source_published_at: "2026-09-30T12:00:00Z".into(),
                evidence: vec![EvidenceRecord {
                    source: "official source".into(),
                    source_url: "https://example.org/event".into(),
                    timestamp: "2026-09-30T12:00:00Z".into(),
                    source_published_at: "2026-09-30T12:00:00Z".into(),
                    event_type: character.into(),
                    importance: importance.into(),
                    subject: "event fact".into(),
                }],
                ..Default::default()
            }),
            report_run_at: Some("2026-09-30T20:00:00Z".into()),
            observation_window_start: Some("2026-09-30T12:00:00Z".into()),
            observation_window_end: Some("2026-09-30T20:00:00Z".into()),
            ..Default::default()
        }
    }
    fn reaction(id: &str, dimension: &str) -> MarketReaction {
        MarketReaction {
            observation_id: id.into(),
            instrument: id.into(),
            observed_at: "2026-09-30T14:00:00Z".into(),
            observation_date: "2026-09-30".into(),
            observation_time_precision: ObservationTimePrecision::Timestamp,
            structured_reaction: Some(StructuredMarketReaction {
                reaction_dimension: dimension.into(),
                magnitude: "3".into(),
                baseline: "100".into(),
                direction: "UP".into(),
                significant_magnitude: "2".into(),
                unit: "percent_change".into(),
                measurement_method: "session change versus prior close".into(),
                source: "market feed".into(),
                source_url: "https://example.org/series".into(),
            }),
            ..Default::default()
        }
    }
    fn add(v: &mut SignalContextV1, r: MarketReaction) {
        v.temporal_bindings.push(build_temporal_binding(
            "event-1",
            "2026-09-30T12:00:00Z",
            &r,
        ));
        v.observed_market_reactions.push(r);
    }
    #[test]
    fn information_evidence_commentary_and_coverage_cannot_prove_high() {
        for character in [
            "COMMENTARY",
            "ANALYSIS",
            "PROFILE",
            "POLITICAL_POSITIONING",
            "GEOPOLITICAL_ESCALATION",
            "",
        ] {
            let mut v = context(character, "HIGH");
            v.coverage.overall = super::super::presentation::SignalContextSourceStatus::Healthy;
            v.primary_context.as_mut().unwrap().title =
                "Using old election playbook, Netanyahu projects image as Israel's protector"
                    .into();
            let e = evaluate(&v, SignalContextInformationContent::High);
            assert_eq!(e.information_level, SignalContextInformationContent::Medium);
            assert_eq!(e.high_information_route, "NONE");
            assert_eq!(e.reaction_evidence_count, 0);
            assert_eq!(
                v.primary_context.unwrap().context_type,
                SignalContextType::Geopolitical
            );
        }
    }
    #[test]
    fn information_evidence_two_dimensions_prove_high_even_for_commentary() {
        for character in ["ESCALATION", "COMMENTARY", "UNKNOWN"] {
            let mut v = context(character, "HIGH");
            add(&mut v, reaction("oil", "commodity"));
            add(&mut v, reaction("yield", "rates"));
            let e = evaluate(&v, SignalContextInformationContent::High);
            assert_eq!(e.information_level, SignalContextInformationContent::High);
            assert_eq!(e.high_information_route, "STRUCTURED_MARKET_REACTION");
            assert_eq!(e.reaction_dimensions, vec!["commodity", "rates"]);
            assert_eq!(e.temporal_association, "SUPPORTED");
            assert_eq!(e.causal_attribution, "NOT_ESTABLISHED");
            assert_eq!(e.decision_weight, 0);
            assert!(!e.trade_signal);
            assert_eq!(e.high_information_evidence.len(), 2);
        }
    }
    #[test]
    fn information_evidence_formal_action_requires_material_new_source_proof() {
        let v = context("FORMAL_ACTION", "MATERIAL_NEW_ACTION");
        let e = evaluate(&v, SignalContextInformationContent::High);
        assert_eq!(e.high_information_route, "EXPLICIT_SURPRISE_EVENT");
        assert_eq!(e.reaction_evidence_count, 0);
        assert_eq!(e.temporal_association, "UNAVAILABLE");
        assert_eq!(e.causal_attribution, "NOT_ESTABLISHED");
        for (character, importance) in [
            ("FORMAL_ACTION", "HIGH"),
            ("UNKNOWN", "UNEXPECTED"),
            ("COMMENTARY", "UNEXPECTED"),
        ] {
            assert_eq!(
                evaluate(
                    &context(character, importance),
                    SignalContextInformationContent::High
                )
                .information_level,
                SignalContextInformationContent::Medium
            );
        }
        for field in ["source", "source_url", "source_published_at"] {
            let mut v = v.clone();
            let record = &mut v.primary_context.as_mut().unwrap().evidence[0];
            match field {
                "source" => record.source.clear(),
                "source_url" => record.source_url.clear(),
                _ => record.source_published_at = "2026-09-30".into(),
            }
            assert_eq!(
                evaluate(&v, SignalContextInformationContent::High).information_level,
                SignalContextInformationContent::Medium
            );
        }
    }
    #[test]
    fn information_evidence_invalid_reactions_fail_closed() {
        let mut good = context("ESCALATION", "HIGH");
        add(&mut good, reaction("oil", "commodity"));
        add(&mut good, reaction("yield", "rates"));
        for fault in [
            "day_only",
            "before_event",
            "future",
            "unrelated",
            "binding_time",
            "missing_binding",
            "missing_source",
            "nan",
            "baseline",
            "insignificant",
            "direction",
            "duplicate_dimension",
            "duplicate_instrument",
            "duplicate_id",
            "missing_method",
        ] {
            let mut v = good.clone();
            match fault {
                "day_only" => {
                    v.observed_market_reactions[1].observation_time_precision =
                        ObservationTimePrecision::DayOnly
                }
                "before_event" => {
                    v.observed_market_reactions[1].observed_at = "2026-09-30T11:00:00Z".into()
                }
                "future" => {
                    v.observed_market_reactions[1].observed_at = "2026-10-01T14:00:00Z".into()
                }
                "unrelated" => v.temporal_bindings[1].event_id = "other-event".into(),
                "binding_time" => {
                    v.temporal_bindings[1].observed_at = "2026-09-30T15:00:00Z".into()
                }
                "missing_binding" => {
                    v.temporal_bindings.pop();
                }
                "duplicate_instrument" => v.observed_market_reactions[1].instrument = "oil".into(),
                "duplicate_id" => v.observed_market_reactions[1].observation_id = "oil".into(),
                field => {
                    let p = v.observed_market_reactions[1]
                        .structured_reaction
                        .as_mut()
                        .unwrap();
                    match field {
                        "missing_source" => p.source.clear(),
                        "nan" => p.magnitude = "NaN".into(),
                        "baseline" => p.baseline.clear(),
                        "insignificant" => p.magnitude = "0.1".into(),
                        "direction" => p.direction = "DOWN".into(),
                        "duplicate_dimension" => p.reaction_dimension = "commodity".into(),
                        _ => p.measurement_method.clear(),
                    }
                }
            }
            assert_eq!(
                evaluate(&v, SignalContextInformationContent::High).information_level,
                SignalContextInformationContent::Medium,
                "{fault}"
            );
        }
    }
    #[test]
    fn information_evidence_unknown_never_upgrades_and_legacy_json_has_no_proof() {
        let v = context("UNKNOWN", "HIGH");
        assert_eq!(
            evaluate(&v, SignalContextInformationContent::Unknown).information_level,
            SignalContextInformationContent::Unknown
        );
        let legacy: MarketReaction = serde_json::from_str(
            r#"{"observed_at":"2026-09-30","subject":"oil","observation":"up","evidence":[]}"#,
        )
        .unwrap();
        assert!(legacy.structured_reaction.is_none());
    }
    #[test]
    fn information_evidence_missing_identity_or_run_time_cannot_prove_high() {
        for fault in ["identity", "run", "future"] {
            let mut v = context("FORMAL_ACTION", "MATERIAL_NEW_ACTION");
            match fault {
                "identity" => v.primary_context.as_mut().unwrap().event_id.clear(),
                "run" => v.report_run_at = None,
                _ => v.report_run_at = Some("2026-09-30T11:00:00Z".into()),
            }
            assert_eq!(
                evaluate(&v, SignalContextInformationContent::High).information_level,
                SignalContextInformationContent::Medium,
                "{fault}"
            );
        }
    }
    #[test]
    fn information_evidence_instrument_conflicts_are_order_independent() {
        let mut x_commodity = reaction("a", "commodity");
        x_commodity.instrument = "X".into();
        let mut x_rates = reaction("b", "rates");
        x_rates.instrument = "X".into();
        let y = reaction("Y", "commodity");
        let records = [x_commodity, x_rates, y];
        for order in [
            [0, 1, 2],
            [1, 0, 2],
            [2, 0, 1],
            [2, 1, 0],
            [0, 2, 1],
            [1, 2, 0],
        ] {
            let mut v = context("ESCALATION", "HIGH");
            for index in order {
                add(&mut v, records[index].clone());
            }
            let e = evaluate(&v, SignalContextInformationContent::High);
            assert_eq!(
                e.information_level,
                SignalContextInformationContent::Medium,
                "{order:?}"
            );
            assert_eq!(e.reaction_evidence_count, 1);
        }
    }

    #[test]
    fn information_evidence_explicit_source_must_be_in_valid_window() {
        for (start, end) in [
            ("2026-09-30T13:00:00Z", "2026-09-30T20:00:00Z"),
            ("bad", "2026-09-30T20:00:00Z"),
            ("2026-09-30T13:00:00Z", "2026-09-30T11:00:00Z"),
            ("2026-09-30T11:00:00Z", "bad"),
            ("2026-09-30T11:00:00Z", "2026-10-01T20:00:00Z"),
        ] {
            let mut v = context("FORMAL_ACTION", "MATERIAL_NEW_ACTION");
            v.observation_window_start = Some(start.into());
            v.observation_window_end = Some(end.into());
            assert_eq!(
                evaluate(&v, SignalContextInformationContent::High).information_level,
                SignalContextInformationContent::Medium,
                "{start}/{end}"
            );
        }
    }

    #[test]
    fn information_evidence_archive_only_marks_valid_proofs_as_accepted() {
        let mut v = context("FORMAL_ACTION", "MATERIAL_NEW_ACTION");
        let mut invalid = v.primary_context.as_ref().unwrap().evidence[0].clone();
        invalid.source.clear();
        invalid.source_url = "https://invalid.test/proof".into();
        v.primary_context.as_mut().unwrap().evidence.push(invalid);
        let e = evaluate(&v, SignalContextInformationContent::High);
        assert_eq!(
            e.high_information_evidence,
            vec!["https://example.org/event"]
        );
        assert_eq!(e.event_evidence.len(), 2);
        add(&mut v, reaction("oil", "commodity"));
        add(&mut v, reaction("yield", "rates"));
        let mut binding = v.temporal_bindings[0].clone();
        binding.observation_time_precision = ObservationTimePrecision::DayOnly;
        v.temporal_bindings.push(binding);
        let mut binding = v.temporal_bindings[0].clone();
        binding.source_published_at = "2026-09-30T13:00:00Z".into();
        v.temporal_bindings.push(binding);
        let e = evaluate(&v, SignalContextInformationContent::High);
        assert_eq!(e.reaction_bindings.len(), 2);
    }
}

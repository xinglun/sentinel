#![allow(dead_code)]

use crate::features::radar::domain::decision::DecisionPacket;
use crate::features::radar::domain::leader_persistence::{
    build_leader_persistence, LeaderObservation, LeaderState,
};
use crate::features::radar::interface::presentation::{
    LeaderPersistenceViewModel, LeadershipSnapshotViewModel, MarketCyclePosition,
    MarketInterpretationViewModel, PresentationPacket, TrendBreadthMode,
};
use crate::features::shared::interface::i18n::Language;

fn rotation_delta(previous: &[String], current: &[String]) -> (Vec<String>, Vec<String>) {
    let exited = previous
        .iter()
        .filter(|symbol| !current.contains(symbol))
        .cloned()
        .collect();
    let entered = current
        .iter()
        .filter(|symbol| !previous.contains(symbol))
        .cloned()
        .collect();
    (exited, entered)
}

pub(crate) fn build_market_interpretation_view_model(
    packet: &DecisionPacket,
    pres_packet: &PresentationPacket,
    leadership_snapshot: &LeadershipSnapshotViewModel,
    language: Language,
) -> Option<MarketInterpretationViewModel> {
    build_market_interpretation_view_model_with_baseline(
        packet,
        pres_packet,
        leadership_snapshot,
        None,
        None,
        language,
    )
}

pub(crate) fn build_market_interpretation_view_model_with_baseline(
    packet: &DecisionPacket,
    pres_packet: &PresentationPacket,
    leadership_snapshot: &LeadershipSnapshotViewModel,
    previous_interpretation: Option<&MarketInterpretationViewModel>,
    previous_formal_snapshot: Option<
        &crate::features::radar::infrastructure::persistence::TradingDaySnapshot,
    >,
    language: Language,
) -> Option<MarketInterpretationViewModel> {
    let interpretation_layer = pres_packet.interpretation_layer.as_ref()?;
    let transition_evidence = pres_packet.transition_evidence.as_ref();
    let trend_breadth_mode = transition_evidence
        .map(|evidence| evidence.trend_breadth_mode)
        .unwrap_or_default();
    let market_cycle_position = transition_evidence
        .map(|evidence| evidence.market_cycle_position)
        .unwrap_or_default();
    let flow_acceleration = packet.market_features.flow_acceleration.unwrap_or(0.0);

    let primary_context = interpretation_layer
        .signal_context_primary_context_value
        .as_str();
    let exceptional_factors = exceptional_factors(
        primary_context,
        trend_breadth_mode,
        market_cycle_position,
        flow_acceleration,
        language,
    );
    let day_type = if exceptional_factors.is_empty() {
        day_type_normal(language)
    } else {
        day_type_exceptional(language)
    };

    let primary_count =
        usize::from(leadership_snapshot.primary_leader_value != leadership_missing_value(language));
    let leadership_breadth_value = leadership_breadth(
        trend_breadth_mode,
        primary_count,
        leadership_snapshot.secondary_leaders_values.len(),
        leadership_snapshot.watchlist_leaders_values.len(),
        primary_context,
        language,
    );

    let (breadth_score, concentration_score, rotation_score, concentration_label_text) =
        concentration_scores(trend_breadth_mode, market_cycle_position, language);
    let mut rotation_type = rotation_type(&RotationTypeInput {
        primary_context,
        trend_breadth_mode,
        market_cycle_position,
        primary: std::slice::from_ref(&leadership_snapshot.primary_leader_value),
        supporting: &leadership_snapshot.secondary_leaders_values,
        weakening: &leadership_snapshot.watchlist_leaders_values,
        flow_acceleration,
        language,
    });

    let previous_rotation_values = previous_formal_snapshot
        .map(|snapshot| {
            snapshot
                .primary_leader
                .iter()
                .chain(snapshot.secondary_leaders.iter())
                .cloned()
                .collect::<Vec<_>>()
        })
        .or_else(|| {
            previous_interpretation.map(|previous| {
                previous
                    .primary_values
                    .iter()
                    .chain(previous.supporting_values.iter())
                    .cloned()
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    let mut current_rotation_values = vec![leadership_snapshot.primary_leader_value.clone()];
    let additional_supporting: Vec<String> = leadership_snapshot
        .secondary_leaders_values
        .iter()
        .filter(|symbol| !current_rotation_values.contains(symbol))
        .cloned()
        .collect();
    current_rotation_values.extend(additional_supporting);
    let (rotation_from_values, rotation_to_values) =
        rotation_delta(&previous_rotation_values, &current_rotation_values);
    if rotation_from_values.is_empty() && rotation_to_values.is_empty() {
        rotation_type = "no_rotation".to_string();
    }

    let rotation_interpretation_value = rotation_interpretation(
        &rotation_type,
        trend_breadth_mode,
        market_cycle_position,
        flow_acceleration,
        language,
    );
    let current_leaders = std::iter::once(leadership_snapshot.primary_leader_value.clone())
        .chain(leadership_snapshot.secondary_leaders_values.iter().cloned())
        .filter(|leader| !leader.is_empty() && leader != "none")
        .collect::<Vec<_>>();
    let breakout_leaders = pres_packet
        .breakout_summary
        .items
        .iter()
        .filter(|item| {
            item.status
                == crate::features::radar::interface::presentation::BreakoutDisplayStatus::EmergingBreakout
        })
        .map(|item| item.symbol.clone())
        .collect::<Vec<_>>();
    let narrative_values = market_interpretation_narrative_values(
        day_type,
        pres_packet
            .interpretation_layer
            .as_ref()
            .map(|layer| layer.signal_context_next_observation_value.as_str())
            .unwrap_or_default(),
        &current_leaders,
        &breakout_leaders,
        language,
    );

    let interpretation_priority_values = interpretation_priority(&InterpretationPriorityInput {
        trend_confidence: interpretation_layer.trend_confidence_value.as_str(),
        supply_confidence: interpretation_layer.supply_confidence_value.as_str(),
        macro_confidence: interpretation_layer.signal_context_quality_value.as_str(),
        flow_confidence: interpretation_layer.flow_confidence_value.as_str(),
        expectation_confidence: interpretation_layer.expectation_confidence_value.as_str(),
        trend_breadth_mode,
        market_cycle_position,
        exceptional_factors: &exceptional_factors,
        language,
    });

    Some(MarketInterpretationViewModel {
        title: market_interpretation_title(language).to_string(),
        notice: market_interpretation_notice(language).to_string(),
        current_decision_weight_label: current_decision_weight_label(language).to_string(),
        current_decision_weight_value: "0%".to_string(),
        narrative_label: narrative_label(language).to_string(),
        narrative_values,
        day_type_label: day_type_label(language).to_string(),
        day_type_value: day_type.to_string(),
        day_type_reason_label: day_type_reason_label(language).to_string(),
        day_type_reason_value: day_type_reason(
            primary_context,
            trend_breadth_mode,
            market_cycle_position,
            flow_acceleration,
            language,
        )
        .to_string(),
        exceptional_factors_label: exceptional_factors_label(language).to_string(),
        exceptional_factors_values: exceptional_factors,
        leadership_label: leadership_snapshot.title.clone(),
        leadership_classification_label: leadership_snapshot.leadership_confidence_label.clone(),
        leadership_classification_value: leadership_snapshot.leadership_confidence_value.clone(),
        primary_label: leadership_snapshot.primary_leader_label.clone(),
        primary_values: vec![leadership_snapshot.primary_leader_value.clone()],
        supporting_label: leadership_snapshot.secondary_leaders_label.clone(),
        supporting_values: leadership_snapshot.secondary_leaders_values.clone(),
        weakening_label: leadership_snapshot.watchlist_leaders_label.clone(),
        weakening_values: leadership_snapshot.watchlist_leaders_values.clone(),
        leadership_metrics_label: leadership_metrics_label(language).to_string(),
        leadership_breadth_label: leadership_breadth_label(language).to_string(),
        leadership_breadth_value,
        concentration_label: concentration_label_text,
        breadth_score_label: breadth_score_label(language).to_string(),
        breadth_score_value: breadth_score.to_string(),
        concentration_score_label: concentration_score_label(language).to_string(),
        concentration_score_value: concentration_score.to_string(),
        rotation_score_label: rotation_score_label(language).to_string(),
        rotation_score_value: rotation_score.to_string(),
        rotation_label: rotation_label(language).to_string(),
        rotation_type_value: rotation_type,
        rotation_from_label: rotation_from_label(language).to_string(),
        rotation_from_values,
        rotation_to_label: rotation_to_label(language).to_string(),
        rotation_to_values,
        rotation_interpretation_label: rotation_interpretation_label(language).to_string(),
        rotation_interpretation_value,
        confidence_label: confidence_label(language).to_string(),
        trend_confidence_label: trend_confidence_label(language).to_string(),
        trend_confidence_value: interpretation_layer.trend_confidence_value.clone(),
        macro_confidence_label: macro_confidence_label(language).to_string(),
        macro_confidence_value: interpretation_layer.signal_context_quality_value.clone(),
        supply_confidence_label: supply_confidence_label(language).to_string(),
        supply_confidence_value: interpretation_layer.supply_confidence_value.clone(),
        expectation_confidence_label: expectation_confidence_label(language).to_string(),
        expectation_confidence_value: interpretation_layer.expectation_confidence_value.clone(),
        gravity_confidence_label: gravity_confidence_label(language).to_string(),
        gravity_confidence_value: interpretation_layer.gravity_confidence_value.clone(),
        flow_confidence_label: flow_confidence_label(language).to_string(),
        flow_confidence_value: interpretation_layer.flow_confidence_value.clone(),
        overall_confidence_label: overall_confidence_label(language).to_string(),
        overall_confidence_value: interpretation_layer.interpretation_quality_value.clone(),
        interpretation_priority_label: interpretation_priority_label(language).to_string(),
        interpretation_priority_values,
        observation_only_label: observation_only_label(language).to_string(),
        observation_only_value: "true".to_string(),
        boundary: market_interpretation_boundary(language).to_string(),
    })
}

pub(crate) fn build_leadership_snapshot_view_model(
    pres_packet: &PresentationPacket,
    language: Language,
) -> LeadershipSnapshotViewModel {
    let top_actions = &pres_packet.top_actions;
    let primary_values = select_primary_symbols(top_actions);
    let secondary_values = select_supporting_symbols(top_actions, &primary_values);
    let watchlist_values =
        select_watchlist_symbols(pres_packet, &primary_values, &secondary_values);
    build_leadership_snapshot_view_model_from_components(
        primary_values,
        secondary_values,
        watchlist_values,
        !pres_packet.top_actions.is_empty(),
        language,
    )
}

pub(crate) fn build_leadership_snapshot_view_model_from_transition_log(
    log: &crate::features::radar::domain::transition_log::StateTransitionLog,
    language: Language,
) -> LeadershipSnapshotViewModel {
    build_leadership_snapshot_view_model_from_components(
        log.observed_leader.iter().cloned().collect(),
        Vec::new(),
        Vec::new(),
        true,
        language,
    )
}

pub(crate) fn build_leadership_snapshot_view_model_from_components(
    primary_values: Vec<String>,
    secondary_values: Vec<String>,
    watchlist_values: Vec<String>,
    observation_available: bool,
    language: Language,
) -> LeadershipSnapshotViewModel {
    let secondary_count = secondary_values.len();
    let primary_count = primary_values.len();
    let primary_value = primary_values
        .first()
        .cloned()
        .unwrap_or_else(|| leadership_missing_value(language).to_string());
    let conflict_value = leadership_conflict_value(
        &primary_values,
        &secondary_values,
        &watchlist_values,
        observation_available,
        language,
    );
    LeadershipSnapshotViewModel {
        title: leadership_snapshot_title(language).to_string(),
        primary_leader_label: primary_leader_label(language).to_string(),
        primary_leader_value: primary_value,
        secondary_leaders_label: secondary_leaders_label(language).to_string(),
        secondary_leaders_values: secondary_values,
        watchlist_leaders_label: watchlist_leaders_label(language).to_string(),
        watchlist_leaders_reasons: watchlist_values
            .iter()
            .map(|symbol| leadership_watch_reason(symbol, language).to_string())
            .collect(),
        watchlist_leaders_values: watchlist_values,
        leadership_confidence_label: leadership_confidence_label(language).to_string(),
        leadership_confidence_value: leadership_confidence_value(
            primary_count,
            secondary_count,
            conflict_value.is_empty(),
            language,
        )
        .to_string(),
        leadership_conflict_label: leadership_conflict_label(language).to_string(),
        leadership_conflict_value: if conflict_value.is_empty() {
            leadership_conflict_none(language).to_string()
        } else {
            conflict_value
        },
        boundary: leadership_snapshot_boundary(language).to_string(),
    }
}

pub(crate) struct LeaderPersistenceReadModelInput<'a> {
    pub persisted_observations:
        &'a [crate::features::radar::domain::leader_persistence::LeaderObservation],
    pub current_packet: &'a DecisionPacket,
    pub current_presentation: &'a PresentationPacket,
    pub language: Language,
    pub baseline_date: Option<chrono::NaiveDate>,
    pub baseline_status: &'a str,
    pub formal_baseline:
        Option<&'a crate::features::radar::infrastructure::persistence::TradingDaySnapshot>,
}

pub(crate) fn build_leader_persistence_view_model(
    input: LeaderPersistenceReadModelInput<'_>,
) -> Option<LeaderPersistenceViewModel> {
    let current_snapshot = input.current_presentation.leadership_snapshot.as_ref()?;
    let current_leader = current_snapshot.primary_leader_value.trim();
    if current_leader.is_empty() {
        return None;
    }

    let lookback_start = input.current_packet.date
        - chrono::Duration::days(
            (crate::features::radar::domain::leader_persistence::LEADERSHIP_LOOKBACK_DAYS - 1)
                as i64,
        );
    let mut observations = input
        .persisted_observations
        .iter()
        .filter(|observation| {
            observation.date >= lookback_start && observation.date <= input.current_packet.date
        })
        .cloned()
        .collect::<Vec<_>>();
    if let Some(snapshot) = input.formal_baseline {
        if let Some(leader) = snapshot
            .primary_leader
            .as_ref()
            .filter(|leader| !leader.is_empty())
        {
            observations.push(LeaderObservation {
                date: snapshot.market_date,
                leader: leader.clone(),
                confidence: Some(snapshot.confidence),
                breadth: Some(snapshot.breadth),
                relative_strength: None,
                rotation_stability: Some(snapshot.stability),
                sector_or_index_rotation: Some(snapshot.risk_state.clone()),
                supply_state: Some(snapshot.supply_phase.clone()),
            });
        }
    }
    let baseline_available = input.baseline_status == "AVAILABLE"
        && match input.formal_baseline {
            Some(snapshot) => input.baseline_date == Some(snapshot.market_date),
            None => input.baseline_date.is_some_and(|date| {
                observations
                    .iter()
                    .any(|observation| observation.date == date)
            }),
        };
    let has_prior_history = baseline_available;

    let current_observation = build_current_observation(
        input.current_packet,
        input.current_presentation,
        current_snapshot,
    );
    observations.push(current_observation);

    let mut result = build_leader_persistence(&observations)?;
    if !has_prior_history {
        result.history_coverage_complete = false;
        result.history_coverage = if input.baseline_date.is_some() {
            "BASELINE_UNAVAILABLE"
        } else {
            "UNAVAILABLE"
        };
        result.first_observed_at = None;
        result.leader_state = LeaderState::Unavailable;
    }

    Some(LeaderPersistenceViewModel {
        title: leader_persistence_title(input.language).to_string(),
        primary_leader_label: leader_persistence_primary_label(input.language).to_string(),
        primary_leader_value: result.current_leader.clone(),
        persistence_label: leader_persistence_persistence_label(input.language).to_string(),
        persistence_value: leader_persistence_value(result.persistence_days, input.language),
        persistence_days: result.persistence_days,
        leader_absence_duration: result.leader_absence_duration,
        observed_days_label: leader_persistence_observed_days_label(input.language).to_string(),
        observed_days_value: leader_persistence_value(
            result.observed_leadership_days,
            input.language,
        ),
        breakout_continuity_label: leader_persistence_breakout_continuity_label(input.language)
            .to_string(),
        breakout_continuity_value: input
            .current_packet
            .assets
            .iter()
            .find(|asset| asset.symbol == result.current_leader)
            .map(|asset| leader_persistence_value(asset.breakout.breakout_age, input.language))
            .unwrap_or_else(|| leader_persistence_history_unavailable(input.language).to_string()),
        history_coverage_label: leader_persistence_history_coverage_label(input.language)
            .to_string(),
        history_coverage_value: result.history_coverage.to_string(),
        first_observed_at_value: (result.history_coverage == "COMPLETE")
            .then_some(result.first_observed_at)
            .flatten()
            .map(|date| date.to_string()),
        previous_leader_value: result.previous_leader.clone(),
        history_note: (!result.history_coverage_complete)
            .then(|| leader_persistence_history_unavailable(input.language).to_string()),
        leadership_score_label: leader_persistence_score_label(input.language).to_string(),
        leadership_score_value: format!("{:.1}", result.leadership_score),
        leadership_score: result.leadership_score,
        leader_state_label: leader_persistence_state_label(input.language).to_string(),
        leader_state_value: result.leader_state.as_str().to_string(),
        change_from_yesterday_label: leader_persistence_change_label(input.language).to_string(),
        change_from_yesterday_value: leader_persistence_change_value(&result, input.language),
        persistence_change_days: if result.same_leader_as_previous { 1 } else { 0 },
        score_change: result.leadership_score - result.previous_score,
        switch_history_label: leader_persistence_history_label(input.language).to_string(),
        switch_history_values: result.switch_history.clone(),
        boundary: leader_persistence_boundary(input.language, result_has_missing_metrics(&result)),
    })
}

fn build_current_observation(
    packet: &DecisionPacket,
    presentation: &PresentationPacket,
    snapshot: &LeadershipSnapshotViewModel,
) -> LeaderObservation {
    build_presentation_observation(packet, presentation).unwrap_or(LeaderObservation {
        date: packet.date,
        leader: snapshot.primary_leader_value.clone(),
        confidence: leadership_confidence_score(snapshot),
        breadth: None,
        relative_strength: packet
            .assets
            .iter()
            .find(|asset| asset.symbol == snapshot.primary_leader_value)
            .and_then(|asset| asset.relative_strength),
        rotation_stability: None,
        sector_or_index_rotation: None,
        supply_state: None,
    })
}

pub(crate) fn build_leader_observation(
    packet: &DecisionPacket,
    presentation: &PresentationPacket,
) -> Option<LeaderObservation> {
    let snapshot = presentation.leadership_snapshot.as_ref()?;
    Some(build_current_observation(packet, presentation, snapshot))
}

fn build_presentation_observation(
    packet: &DecisionPacket,
    presentation: &PresentationPacket,
) -> Option<LeaderObservation> {
    let leadership_snapshot = presentation.leadership_snapshot.as_ref()?;
    let market_interpretation = presentation.market_interpretation.as_ref();
    Some(LeaderObservation {
        date: packet.date,
        leader: leadership_snapshot.primary_leader_value.clone(),
        confidence: leadership_confidence_score(leadership_snapshot),
        breadth: market_interpretation
            .and_then(|value| value.breadth_score_value.parse::<f64>().ok()),
        relative_strength: packet
            .assets
            .iter()
            .find(|asset| asset.symbol == leadership_snapshot.primary_leader_value)
            .and_then(|asset| asset.relative_strength),
        rotation_stability: market_interpretation
            .and_then(|value| value.rotation_score_value.parse::<f64>().ok())
            .map(|value| (100.0 - value).clamp(0.0, 100.0)),
        sector_or_index_rotation: market_interpretation
            .map(|value| value.rotation_type_value.clone()),
        supply_state: Some(presentation.signal_summary.supply_phase_value.clone())
            .filter(|value| !value.trim().is_empty()),
    })
}

fn leader_persistence_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leader Persistence",
        Language::EnUs => "Leader Persistence",
        Language::JaJp => "Leader Persistence",
    }
}

fn leader_persistence_primary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "综合主导者",
        Language::EnUs => "Composite Leader",
        Language::JaJp => "総合 Leader",
    }
}

fn leader_persistence_persistence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "连续领导天数",
        Language::EnUs => "Leader Persistence",
        Language::JaJp => "連続リーダー日数",
    }
}

fn leader_persistence_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "领导评分",
        Language::EnUs => "Leadership Score",
        Language::JaJp => "リーダースコア",
    }
}

fn leader_persistence_observed_days_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "回看期内观察到的领导天数",
        Language::EnUs => "Observed Leadership Days in Lookback",
        Language::JaJp => "ルックバック内の観測リーダー日数",
    }
}

fn leader_persistence_breakout_continuity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "突破连续性",
        Language::EnUs => "Breakout Continuity",
        Language::JaJp => "ブレイクアウト継続性",
    }
}

fn leader_persistence_history_coverage_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "历史覆盖",
        Language::EnUs => "History Coverage",
        Language::JaJp => "履歴カバレッジ",
    }
}

fn leader_persistence_history_unavailable(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "功能启用前的 Leadership 历史不可用。",
        Language::EnUs => "Leadership history unavailable before feature activation.",
        Language::JaJp => "feature activation 前の Leadership 履歴は利用できません。",
    }
}

fn leader_persistence_state_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "领导状态",
        Language::EnUs => "Leader State",
        Language::JaJp => "リーダー状態",
    }
}

fn leader_persistence_change_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "较昨日变化",
        Language::EnUs => "Change from Yesterday",
        Language::JaJp => "前日比",
    }
}

fn leader_persistence_history_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "切换历史",
        Language::EnUs => "Switch History",
        Language::JaJp => "切替履歴",
    }
}

fn leader_persistence_boundary(language: Language, degraded: bool) -> String {
    let quality = if degraded {
        match language {
            Language::ZhCn => "数据质量：降级，部分历史指标缺失。",
            Language::EnUs => "Data quality: degraded; some historical metrics are unavailable.",
            Language::JaJp => "データ品質：降級、過去の一部指標が欠損しています。",
        }
    } else {
        ""
    };
    let boundary = match language {
        Language::ZhCn => "边界：仅用于观察；本区块不改变 Decision、Gate、Execution、Trader 或 Position Sizing。",
        Language::EnUs => "Boundary: observation only; this block does not change Decision, Gate, Execution, Trader, or Position Sizing.",
        Language::JaJp => "境界：観測専用。このブロックは Decision、Gate、Execution、Trader、Position Sizing を変更しません。",
    };
    if quality.is_empty() {
        boundary.to_string()
    } else {
        format!("{boundary} {quality}")
    }
}

fn leadership_confidence_score(snapshot: &LeadershipSnapshotViewModel) -> Option<f64> {
    match snapshot
        .leadership_confidence_value
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "HIGH" => Some(90.0),
        "MEDIUM" => Some(70.0),
        "LOW" => Some(40.0),
        _ => None,
    }
}

fn result_has_missing_metrics(
    result: &crate::features::radar::domain::leader_persistence::LeaderPersistenceResult,
) -> bool {
    result.current_breadth.is_none()
        || result.current_relative_strength.is_none()
        || result.current_rotation_stability.is_none()
        || result.previous_breadth.is_none() && result.previous_leader.is_some()
        || result.previous_relative_strength.is_none() && result.previous_leader.is_some()
        || result.previous_rotation_stability.is_none() && result.previous_leader.is_some()
}

fn leader_persistence_value(days: usize, language: Language) -> String {
    match language {
        Language::ZhCn => format!("{days} 天"),
        Language::EnUs => format!("{days} days"),
        Language::JaJp => format!("{days} 日"),
    }
}

fn leader_persistence_change_value(
    result: &crate::features::radar::domain::leader_persistence::LeaderPersistenceResult,
    language: Language,
) -> String {
    if !result.same_leader_as_previous {
        return match (&result.previous_leader, language) {
            (Some(previous), Language::ZhCn) => {
                format!("{previous} -> {}，连续天数重置", result.current_leader)
            }
            (Some(previous), Language::JaJp) => {
                format!(
                    "{previous} -> {}、連続日数をリセット",
                    result.current_leader
                )
            }
            (Some(previous), Language::EnUs) => {
                format!("{previous} -> {}, streak reset", result.current_leader)
            }
            (None, Language::ZhCn) if result.history_coverage_complete => {
                format!("{}：首次成为领导者", result.current_leader)
            }
            (None, Language::JaJp) if result.history_coverage_complete => {
                format!("{}：初めてリーダーになりました", result.current_leader)
            }
            (None, Language::EnUs) if result.history_coverage_complete => {
                format!("{}: first became leader", result.current_leader)
            }
            (None, _) => leader_persistence_history_unavailable(language).to_string(),
        };
    }

    if result.leadership_score + 1.0 < result.previous_score {
        return match language {
            Language::ZhCn => "+1 天，评分下降".to_string(),
            Language::EnUs => "+1 day, score down".to_string(),
            Language::JaJp => "+1 日、スコア低下".to_string(),
        };
    }

    if result.leadership_score > result.previous_score + 1.0 {
        return match language {
            Language::ZhCn => "+1 天，评分上升".to_string(),
            Language::EnUs => "+1 day, score up".to_string(),
            Language::JaJp => "+1 日、スコア上昇".to_string(),
        };
    }

    match language {
        Language::ZhCn => "+1 天，评分稳定".to_string(),
        Language::EnUs => "+1 day, score stable".to_string(),
        Language::JaJp => "+1 日、スコア安定".to_string(),
    }
}

fn intersection(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|symbol| right.iter().any(|item| item == *symbol))
        .cloned()
        .collect()
}

fn unique_symbols(symbols: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    for symbol in symbols {
        if !values.contains(symbol) {
            values.push(symbol.clone());
        }
    }
    values
}

fn select_watchlist_symbols(
    pres_packet: &PresentationPacket,
    primary: &[String],
    supporting: &[String],
) -> Vec<String> {
    let mut symbols = Vec::new();
    for symbol in select_weakening_symbols(pres_packet) {
        if !primary.contains(&symbol) && !supporting.contains(&symbol) && !symbols.contains(&symbol)
        {
            symbols.push(symbol);
        }
    }
    for action in &pres_packet.top_actions {
        if !primary.contains(&action.symbol)
            && !supporting.contains(&action.symbol)
            && !symbols.contains(&action.symbol)
        {
            symbols.push(action.symbol.clone());
        }
    }
    symbols.truncate(3);
    symbols
}

fn leadership_conflict_value(
    primary: &[String],
    secondary: &[String],
    watchlist: &[String],
    observation_available: bool,
    language: Language,
) -> String {
    let mut conflicts = Vec::new();
    let primary_secondary = intersection(primary, secondary);
    if !primary_secondary.is_empty() {
        conflicts.push(match language {
            Language::ZhCn => format!("primary / secondary 重叠: {}", primary_secondary.join(", ")),
            Language::EnUs => format!(
                "primary / secondary overlap: {}",
                primary_secondary.join(", ")
            ),
            Language::JaJp => format!(
                "primary / secondary が重複: {}",
                primary_secondary.join(", ")
            ),
        });
    }
    let primary_watchlist = intersection(primary, watchlist);
    if !primary_watchlist.is_empty() {
        conflicts.push(match language {
            Language::ZhCn => format!("primary / watchlist 重叠: {}", primary_watchlist.join(", ")),
            Language::EnUs => format!(
                "primary / watchlist overlap: {}",
                primary_watchlist.join(", ")
            ),
            Language::JaJp => format!(
                "primary / watchlist が重複: {}",
                primary_watchlist.join(", ")
            ),
        });
    }
    if !observation_available {
        conflicts.push(match language {
            Language::ZhCn => "Observation Layer 未提供 leader".to_string(),
            Language::EnUs => "Observation Layer did not provide a leader".to_string(),
            Language::JaJp => "Observation Layer は leader を提供していない".to_string(),
        });
    }
    if conflicts.is_empty() {
        String::new()
    } else {
        conflicts.join(" / ")
    }
}

fn leadership_confidence_value(
    primary_count: usize,
    secondary_count: usize,
    conflict_free: bool,
    language: Language,
) -> &'static str {
    match (primary_count, secondary_count, conflict_free, language) {
        (0, _, _, Language::ZhCn) => "LOW",
        (0, _, _, Language::EnUs) => "LOW",
        (0, _, _, Language::JaJp) => "LOW",
        (_, _, false, _) => {
            if secondary_count > 0 {
                "MEDIUM"
            } else {
                "LOW"
            }
        }
        (1, 0..=1, true, _) => "MEDIUM",
        (_, _, true, _) => "HIGH",
    }
}

fn leadership_missing_value(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "none",
        Language::EnUs => "none",
        Language::JaJp => "none",
    }
}

fn leadership_snapshot_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership Snapshot",
        Language::EnUs => "Leadership Snapshot",
        Language::JaJp => "Leadership Snapshot",
    }
}

fn primary_leader_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "综合主导者",
        Language::EnUs => "Composite Leader",
        Language::JaJp => "総合 Leader",
    }
}

fn secondary_leaders_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Secondary Leaders",
        Language::EnUs => "Secondary Leaders",
        Language::JaJp => "Secondary Leaders",
    }
}

fn watchlist_leaders_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership Watch Candidates",
        Language::EnUs => "Leadership Watch Candidates",
        Language::JaJp => "Leadership Watch Candidates",
    }
}

fn leadership_watch_reason(_symbol: &str, language: Language) -> &'static str {
    match language {
        Language::ZhCn => "战略相关，但当前结构偏弱。",
        Language::EnUs => "strategic relevance, weak current structure",
        Language::JaJp => "戦略的関連性はあるが、現在の構造は弱い。",
    }
}

fn leadership_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership Confidence",
        Language::EnUs => "Leadership Confidence",
        Language::JaJp => "Leadership Confidence",
    }
}

fn leadership_conflict_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership Conflict / Unavailable Reason",
        Language::EnUs => "Leadership Conflict / Unavailable Reason",
        Language::JaJp => "Leadership Conflict / Unavailable Reason",
    }
}

fn leadership_conflict_none(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "none",
        Language::EnUs => "none",
        Language::JaJp => "none",
    }
}

fn leadership_snapshot_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "Boundary: this snapshot is the single leadership facts source for the daily report. It does not compute trade signals or change Gate / Execution / Trader / Position Sizing."
        }
        Language::EnUs => {
            "Boundary: this snapshot is the single leadership facts source for the daily report. It does not compute trade signals or change Gate / Execution / Trader / Position Sizing."
        }
        Language::JaJp => {
            "Boundary: this snapshot is the single leadership facts source for the daily report. It does not compute trade signals or change Gate / Execution / Trader / Position Sizing."
        }
    }
}

fn market_interpretation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Market Interpretation",
        Language::EnUs => "Market Interpretation",
        Language::JaJp => "Market Interpretation",
    }
}

fn market_interpretation_context_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Market Context",
        Language::EnUs => "Market Context",
        Language::JaJp => "Market Context",
    }
}

fn market_interpretation_context_value(
    interpretation_layer: &crate::features::radar::interface::presentation::InterpretationLayerViewModel,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> String {
    let _ = (
        interpretation_layer,
        trend_breadth_mode,
        market_cycle_position,
        flow_acceleration,
    );
    match language {
        Language::ZhCn => "Trend continuation.".to_string(),
        Language::EnUs => "Trend continuation.".to_string(),
        Language::JaJp => "Trend continuation.".to_string(),
    }
}

fn market_interpretation_reason_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Reason",
        Language::EnUs => "Reason",
        Language::JaJp => "Reason",
    }
}

fn market_interpretation_reason_value(
    interpretation_layer: &crate::features::radar::interface::presentation::InterpretationLayerViewModel,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> String {
    let _ = (
        interpretation_layer,
        trend_breadth_mode,
        market_cycle_position,
        flow_acceleration,
    );
    match language {
        Language::ZhCn => "Supply pressure remains manageable.".to_string(),
        Language::EnUs => "Supply pressure remains manageable.".to_string(),
        Language::JaJp => "Supply pressure remains manageable.".to_string(),
    }
}

fn market_interpretation_factors_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Ignored Factors",
        Language::EnUs => "Ignored Factors",
        Language::JaJp => "Ignored Factors",
    }
}

fn market_interpretation_factors_values(
    interpretation_layer: &crate::features::radar::interface::presentation::InterpretationLayerViewModel,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> Vec<String> {
    let _ = (
        interpretation_layer,
        trend_breadth_mode,
        market_cycle_position,
        flow_acceleration,
    );
    vec![match language {
        Language::ZhCn => "Macro / Expectation / Gravity.".to_string(),
        Language::EnUs => "Macro / Expectation / Gravity.".to_string(),
        Language::JaJp => "Macro / Expectation / Gravity.".to_string(),
    }]
}

fn market_interpretation_metrics_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Market Metrics",
        Language::EnUs => "Market Metrics",
        Language::JaJp => "Market Metrics",
    }
}

fn market_interpretation_breadth_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Breadth / Concentration",
        Language::EnUs => "Breadth / Concentration",
        Language::JaJp => "Breadth / Concentration",
    }
}

fn breadth_semantic_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Breadth Label",
        Language::EnUs => "Breadth Label",
        Language::JaJp => "Breadth Label",
    }
}

fn breadth_semantic_value(
    trend_breadth_mode: TrendBreadthMode,
    leadership_snapshot: &LeadershipSnapshotViewModel,
    market_cycle_position: MarketCyclePosition,
    language: Language,
) -> &'static str {
    let _ = (
        trend_breadth_mode,
        leadership_snapshot,
        market_cycle_position,
        language,
    );
    match (trend_breadth_mode, market_cycle_position) {
        (TrendBreadthMode::BroadExpansion, _) => "Broad Participation",
        (TrendBreadthMode::NarrowLeadership, _) => "Very Narrow",
        (TrendBreadthMode::StructuralDefense, _) => "Narrow",
        (TrendBreadthMode::FragileRotation, MarketCyclePosition::DistributionWarning) => "Narrow",
        (TrendBreadthMode::FragileRotation, _) => "Narrow",
    }
}

fn market_breadth_score_value(
    trend_breadth_mode: TrendBreadthMode,
    leadership_snapshot: &LeadershipSnapshotViewModel,
    market_cycle_position: MarketCyclePosition,
) -> u8 {
    let _ = (leadership_snapshot, market_cycle_position);
    match trend_breadth_mode {
        TrendBreadthMode::BroadExpansion => 78,
        TrendBreadthMode::NarrowLeadership => 35,
        TrendBreadthMode::FragileRotation => 48,
        TrendBreadthMode::StructuralDefense => 20,
    }
}

fn market_concentration_score_value(
    trend_breadth_mode: TrendBreadthMode,
    leadership_snapshot: &LeadershipSnapshotViewModel,
    market_cycle_position: MarketCyclePosition,
) -> u8 {
    let _ = (leadership_snapshot, market_cycle_position);
    match trend_breadth_mode {
        TrendBreadthMode::BroadExpansion => 34,
        TrendBreadthMode::NarrowLeadership => 82,
        TrendBreadthMode::FragileRotation => 72,
        TrendBreadthMode::StructuralDefense => 76,
    }
}

fn market_rotation_score_value(
    trend_breadth_mode: TrendBreadthMode,
    leadership_snapshot: &LeadershipSnapshotViewModel,
    market_cycle_position: MarketCyclePosition,
) -> u8 {
    let _ = (leadership_snapshot, market_cycle_position);
    match trend_breadth_mode {
        TrendBreadthMode::BroadExpansion => 14,
        TrendBreadthMode::NarrowLeadership => 18,
        TrendBreadthMode::FragileRotation => 48,
        TrendBreadthMode::StructuralDefense => 30,
    }
}

fn market_interpretation_rotation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Rotation Observation",
        Language::EnUs => "Rotation Observation",
        Language::JaJp => "Rotation Observation",
    }
}

fn market_rotation_type_value(
    trend_breadth_mode: TrendBreadthMode,
    leadership_snapshot: &LeadershipSnapshotViewModel,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> String {
    let _ = (leadership_snapshot, market_cycle_position);
    if matches!(trend_breadth_mode, TrendBreadthMode::StructuralDefense) {
        return rotation_type_defensive(language).to_string();
    }
    if matches!(trend_breadth_mode, TrendBreadthMode::BroadExpansion) {
        return rotation_type_broad(language).to_string();
    }
    if flow_acceleration.abs() >= 0.10 {
        return rotation_type_macro(language).to_string();
    }
    rotation_type_none(language).to_string()
}

fn market_interpretation_rotation_summary(
    trend_breadth_mode: TrendBreadthMode,
    leadership_snapshot: &LeadershipSnapshotViewModel,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> String {
    let _ = (leadership_snapshot, market_cycle_position);
    if matches!(trend_breadth_mode, TrendBreadthMode::BroadExpansion) {
        return match language {
            Language::ZhCn => "Participation is broadening rather than collapsing.".to_string(),
            Language::EnUs => "Participation is broadening rather than collapsing.".to_string(),
            Language::JaJp => "Participation is broadening rather than collapsing.".to_string(),
        };
    }
    if matches!(trend_breadth_mode, TrendBreadthMode::StructuralDefense) {
        return match language {
            Language::ZhCn => "Flow is rotating into defensive groups.".to_string(),
            Language::EnUs => "Flow is rotating into defensive groups.".to_string(),
            Language::JaJp => "Flow is rotating into defensive groups.".to_string(),
        };
    }
    if flow_acceleration.abs() >= 0.10 {
        return match language {
            Language::ZhCn => "Macro repricing is driving the move.".to_string(),
            Language::EnUs => "Macro repricing is driving the move.".to_string(),
            Language::JaJp => "Macro repricing is driving the move.".to_string(),
        };
    }
    match language {
        Language::ZhCn => "No clear rotation regime is observable.".to_string(),
        Language::EnUs => "No clear rotation regime is observable.".to_string(),
        Language::JaJp => "No clear rotation regime is observable.".to_string(),
    }
}

fn leadership_classification_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership Classification",
        Language::EnUs => "Leadership Classification",
        Language::JaJp => "Leadership Classification",
    }
}

fn leadership_metrics_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership Metrics",
        Language::EnUs => "Leadership Metrics",
        Language::JaJp => "Leadership Metrics",
    }
}

fn narrative_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Narrative",
        Language::EnUs => "Narrative",
        Language::JaJp => "Narrative",
    }
}

fn market_interpretation_narrative_values(
    day_type: &str,
    next_observation: &str,
    current_leaders: &[String],
    breakout_leaders: &[String],
    language: Language,
) -> Vec<String> {
    let mut lines = Vec::new();
    let primary = current_leaders
        .first()
        .map(String::as_str)
        .unwrap_or("UNAVAILABLE");
    let supporting = current_leaders
        .iter()
        .skip(1)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    let breakouts = breakout_leaders.join(", ");
    lines.push(match (day_type, language) {
        ("normal", Language::ZhCn) => format!(
            "当前综合排序领先为 {primary}；支持结构为 {}。{}{}，整体属于当前截面观察。",
            if supporting.is_empty() {
                "UNAVAILABLE"
            } else {
                &supporting
            },
            if breakouts.is_empty() {
                "当前没有 Read Model 标记的突破萌芽"
            } else {
                "当前突破萌芽: "
            },
            if breakouts.is_empty() { "" } else { &breakouts }
        ),
        ("normal", Language::EnUs) => format!(
            "Current rank leader: {primary}; supporting leaders: {}. Breakout leaders: {}.",
            if supporting.is_empty() {
                "UNAVAILABLE"
            } else {
                &supporting
            },
            if breakouts.is_empty() {
                "UNAVAILABLE"
            } else {
                &breakouts
            }
        ),
        ("normal", Language::JaJp) => format!(
            "現在の順位リーダーは {primary}、支援リーダーは {}。ブレイクアウト候補は {}。",
            if supporting.is_empty() {
                "UNAVAILABLE"
            } else {
                &supporting
            },
            if breakouts.is_empty() {
                "UNAVAILABLE"
            } else {
                &breakouts
            }
        ),
        ("exceptional", Language::ZhCn) => "今天属于例外驱动日。".to_string(),
        ("exceptional", Language::EnUs) => "Today is an exception-driven day.".to_string(),
        ("exceptional", Language::JaJp) => "今日は例外駆動の日です。".to_string(),
        _ => "Today is a normal trend continuation.".to_string(),
    });
    if !next_observation.is_empty() {
        lines.push(next_observation.to_string());
    }
    lines.push(match language {
        Language::ZhCn => "没有结构性恶化证据。".to_string(),
        Language::EnUs => "No structural deterioration evidence is visible.".to_string(),
        Language::JaJp => "構造的悪化の証拠は見えていません。".to_string(),
    });
    lines
}

fn select_primary_symbols(
    top_actions: &[crate::features::radar::interface::display::TopActionViewModel],
) -> Vec<String> {
    top_actions
        .iter()
        .take(1)
        .map(|action| action.symbol.clone())
        .collect()
}

fn select_supporting_symbols(
    top_actions: &[crate::features::radar::interface::display::TopActionViewModel],
    primary: &[String],
) -> Vec<String> {
    top_actions
        .iter()
        .skip(primary.len())
        .take(2)
        .map(|action| action.symbol.clone())
        .collect()
}

fn select_weakening_symbols(pres_packet: &PresentationPacket) -> Vec<String> {
    let mut weakening = Vec::new();
    for item in &pres_packet.exit_summary.items {
        if !matches!(
            item.intent,
            crate::features::radar::interface::presentation::ExitDisplayIntent::Exit
                | crate::features::radar::interface::presentation::ExitDisplayIntent::Trim
        ) {
            continue;
        }
        weakening.push(item.symbol.clone());
    }
    for item in &pres_packet.risk_opportunities {
        if !weakening.contains(&item.symbol) {
            weakening.push(item.symbol.clone());
        }
    }
    weakening
}

fn exceptional_factors(
    primary_context: &str,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> Vec<String> {
    let mut factors = Vec::new();
    if let Some(value) = exceptional_factor_from_primary_context(primary_context, language) {
        factors.push(value);
    }
    if matches!(trend_breadth_mode, TrendBreadthMode::StructuralDefense) {
        factors.push(exceptional_factor_structural_defense(language));
    }
    if matches!(
        market_cycle_position,
        MarketCyclePosition::DistributionWarning
    ) {
        factors.push(exceptional_factor_distribution(language));
    }
    if flow_acceleration.abs() >= 0.10 {
        factors.push(exceptional_factor_abnormal_flow(language));
    }
    factors.sort();
    factors.dedup();
    factors
}

fn exceptional_factor_from_primary_context(
    primary_context: &str,
    language: Language,
) -> Option<String> {
    let factor = match primary_context {
        "Macro Event" => Some(exceptional_factor_macro_surprise(language)),
        "Index Reconstitution" => Some(exceptional_factor_index_reconstitution(language)),
        "ETF Rebalance" => Some(exceptional_factor_etf_rebalance(language)),
        "Pre-Earnings Waiting" => Some(exceptional_factor_major_earnings(language)),
        "Major Event Waiting" => Some(exceptional_factor_unusual_rotation(language)),
        "Holiday Liquidity" => Some(exceptional_factor_unusual_rotation(language)),
        "Quarter-end Rebalancing" | "Month-end Rebalancing" => {
            Some(exceptional_factor_etf_rebalance(language))
        }
        "None" => None,
        _ => None,
    }?;
    Some(factor)
}

fn leadership_breadth(
    trend_breadth_mode: TrendBreadthMode,
    primary_count: usize,
    supporting_count: usize,
    weakening_count: usize,
    primary_context: &str,
    language: Language,
) -> String {
    if matches!(trend_breadth_mode, TrendBreadthMode::BroadExpansion) && weakening_count == 0 {
        return leadership_breadth_broad(language).to_string();
    }
    if matches!(trend_breadth_mode, TrendBreadthMode::StructuralDefense) {
        return leadership_breadth_defensive(language).to_string();
    }
    if weakening_count > 0
        || primary_context == "Major Event Waiting"
        || primary_context == "Pre-Earnings Waiting"
    {
        return leadership_breadth_rotation(language).to_string();
    }
    if primary_count <= 1 && supporting_count <= 2 {
        return leadership_breadth_narrow(language).to_string();
    }
    leadership_breadth_broad(language).to_string()
}

fn concentration_scores(
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    language: Language,
) -> (u8, u8, u8, String) {
    match (trend_breadth_mode, market_cycle_position) {
        (TrendBreadthMode::BroadExpansion, _) => {
            (78, 34, 14, concentration_label_broad(language).to_string())
        }
        (TrendBreadthMode::NarrowLeadership, MarketCyclePosition::CrowdedExpectation) => (
            35,
            82,
            18,
            concentration_label_very_narrow(language).to_string(),
        ),
        (TrendBreadthMode::NarrowLeadership, _) => {
            (38, 80, 20, concentration_label_narrow(language).to_string())
        }
        (TrendBreadthMode::FragileRotation, MarketCyclePosition::DistributionWarning) => (
            24,
            78,
            56,
            concentration_label_rotation(language).to_string(),
        ),
        (TrendBreadthMode::FragileRotation, _) => (
            30,
            72,
            48,
            concentration_label_rotation(language).to_string(),
        ),
        (TrendBreadthMode::StructuralDefense, _) => (
            20,
            76,
            30,
            concentration_label_defensive(language).to_string(),
        ),
    }
}

struct RotationTypeInput<'a> {
    primary_context: &'a str,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    primary: &'a [String],
    supporting: &'a [String],
    weakening: &'a [String],
    flow_acceleration: f64,
    language: Language,
}

fn rotation_type(input: &RotationTypeInput<'_>) -> String {
    if matches!(
        input.primary_context,
        "Index Reconstitution" | "ETF Rebalance"
    ) {
        return rotation_type_index(input.language).to_string();
    }
    if matches!(input.primary_context, "Macro Event") {
        return rotation_type_macro(input.language).to_string();
    }
    if matches!(
        input.trend_breadth_mode,
        TrendBreadthMode::StructuralDefense
    ) {
        return rotation_type_defensive(input.language).to_string();
    }
    if matches!(input.trend_breadth_mode, TrendBreadthMode::BroadExpansion) {
        return rotation_type_broad(input.language).to_string();
    }
    if input
        .weakening
        .iter()
        .any(|symbol| symbol == "NVDA" || symbol == "PLTR")
        && input.primary.iter().any(|symbol| symbol == "SPY")
        && input.flow_acceleration.abs() < 0.10
    {
        return rotation_type_mega_cap(input.language).to_string();
    }
    if matches!(
        input.market_cycle_position,
        MarketCyclePosition::DistributionWarning
    ) {
        return rotation_type_defensive(input.language).to_string();
    }
    if !input.supporting.is_empty() || !input.weakening.is_empty() {
        return rotation_type_sector(input.language).to_string();
    }
    rotation_type_none(input.language).to_string()
}

fn rotation_interpretation(
    rotation_type: &str,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> String {
    match rotation_type {
        "index_reconstitution" => rotation_interpretation_index(language).to_string(),
        "etf_rebalance" => rotation_interpretation_etf(language).to_string(),
        "macro_repricing" => rotation_interpretation_macro(language).to_string(),
        "mega_cap_internal_rotation" => rotation_interpretation_mega_cap(language).to_string(),
        "defensive_rotation" => rotation_interpretation_defensive(language).to_string(),
        "broad_participation" => rotation_interpretation_broad(language).to_string(),
        "sector_or_index_rotation" => rotation_interpretation_sector(language).to_string(),
        _ => match (trend_breadth_mode, market_cycle_position) {
            (TrendBreadthMode::BroadExpansion, _) => {
                rotation_interpretation_broad(language).to_string()
            }
            (TrendBreadthMode::StructuralDefense, _) => {
                rotation_interpretation_defensive(language).to_string()
            }
            (TrendBreadthMode::FragileRotation, MarketCyclePosition::DistributionWarning) => {
                rotation_interpretation_defensive(language).to_string()
            }
            _ if flow_acceleration < -0.10 => {
                rotation_interpretation_withdrawal(language).to_string()
            }
            _ => rotation_interpretation_none(language).to_string(),
        },
    }
}

struct InterpretationPriorityInput<'a> {
    trend_confidence: &'a str,
    supply_confidence: &'a str,
    macro_confidence: &'a str,
    flow_confidence: &'a str,
    expectation_confidence: &'a str,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    exceptional_factors: &'a [String],
    language: Language,
}

fn interpretation_priority(input: &InterpretationPriorityInput<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "{}: {}",
        interpretation_priority_trend_label(input.language),
        trend_stars(input.trend_confidence)
    ));
    if !input.supply_confidence.eq_ignore_ascii_case("UNAVAILABLE") {
        lines.push(format!(
            "{}: {}",
            interpretation_priority_supply_label(input.language),
            "★★"
        ));
    }
    if !input.macro_confidence.eq_ignore_ascii_case("UNAVAILABLE")
        || !input.exceptional_factors.is_empty()
    {
        lines.push(format!(
            "{}: {}",
            interpretation_priority_macro_label(input.language),
            "★"
        ));
    }
    if !input.flow_confidence.eq_ignore_ascii_case("UNAVAILABLE")
        && !matches!(input.trend_breadth_mode, TrendBreadthMode::BroadExpansion)
    {
        lines.push(format!(
            "{}: {}",
            interpretation_priority_flow_label(input.language),
            "☆"
        ));
    }
    if !input
        .expectation_confidence
        .eq_ignore_ascii_case("UNAVAILABLE")
        || matches!(
            input.market_cycle_position,
            MarketCyclePosition::CrowdedExpectation
        )
    {
        lines.push(format!(
            "{}: {}",
            interpretation_priority_expectation_label(input.language),
            "☆"
        ));
    }
    lines
}

fn trend_stars(value: &str) -> String {
    match value.to_ascii_uppercase().as_str() {
        "HIGH" => "★★★★★".to_string(),
        "MEDIUM" => "★★★".to_string(),
        "LOW" => "★".to_string(),
        _ => "☆".to_string(),
    }
}

fn current_decision_weight_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Current decision weight",
        Language::EnUs => "Current decision weight",
        Language::JaJp => "Current decision weight",
    }
}

fn market_interpretation_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "🧭 市场解释层",
        Language::EnUs => "🧭 Market Interpretation Layer",
        Language::JaJp => "🧭 市場解釈レイヤー",
    }
}

fn market_interpretation_notice(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "仅作解释输出。Decision Weight 固定为 0%，不会进入 Gate / Execution / Trader / Action Matrix / Position Sizing。",
        Language::EnUs => "Observation only. Decision Weight is fixed at 0%, and this layer does not enter Gate / Execution / Trader / Action Matrix / Position Sizing.",
        Language::JaJp => "説明出力のみ。Decision Weight は 0% に固定され、Gate / Execution / Trader / Action Matrix / Position Sizing には入らない。",
    }
}

fn day_type_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "dayType",
        Language::EnUs => "dayType",
        Language::JaJp => "dayType",
    }
}

fn day_type_reason_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "reason",
        Language::EnUs => "reason",
        Language::JaJp => "reason",
    }
}

fn exceptional_factors_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "exceptionalFactors",
        Language::EnUs => "exceptionalFactors",
        Language::JaJp => "exceptionalFactors",
    }
}

fn leadership_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Leadership",
        Language::EnUs => "Leadership",
        Language::JaJp => "Leadership",
    }
}

fn primary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "primary",
        Language::EnUs => "primary",
        Language::JaJp => "primary",
    }
}

fn supporting_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "supporting",
        Language::EnUs => "supporting",
        Language::JaJp => "supporting",
    }
}

fn weakening_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "weakening",
        Language::EnUs => "weakening",
        Language::JaJp => "weakening",
    }
}

fn leadership_breadth_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "leadershipBreadth",
        Language::EnUs => "leadershipBreadth",
        Language::JaJp => "leadershipBreadth",
    }
}

fn breadth_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "breadthScore",
        Language::EnUs => "breadthScore",
        Language::JaJp => "breadthScore",
    }
}

fn concentration_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "concentrationScore",
        Language::EnUs => "concentrationScore",
        Language::JaJp => "concentrationScore",
    }
}

fn rotation_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "rotationScore",
        Language::EnUs => "rotationScore",
        Language::JaJp => "rotationScore",
    }
}

fn rotation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Rotation Observation",
        Language::EnUs => "Rotation Observation",
        Language::JaJp => "Rotation Observation",
    }
}

fn rotation_from_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "from",
        Language::EnUs => "from",
        Language::JaJp => "from",
    }
}

fn rotation_to_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "to",
        Language::EnUs => "to",
        Language::JaJp => "to",
    }
}

fn rotation_interpretation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "interpretation",
        Language::EnUs => "interpretation",
        Language::JaJp => "interpretation",
    }
}

fn confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Observation Confidence",
        Language::EnUs => "Observation Confidence",
        Language::JaJp => "Observation Confidence",
    }
}

fn trend_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "trend",
        Language::EnUs => "trend",
        Language::JaJp => "trend",
    }
}

fn macro_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "macro",
        Language::EnUs => "macro",
        Language::JaJp => "macro",
    }
}

fn supply_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "supply",
        Language::EnUs => "supply",
        Language::JaJp => "supply",
    }
}

fn expectation_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "expectation",
        Language::EnUs => "expectation",
        Language::JaJp => "expectation",
    }
}

fn gravity_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "gravity",
        Language::EnUs => "gravity",
        Language::JaJp => "gravity",
    }
}

fn flow_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "flow",
        Language::EnUs => "flow",
        Language::JaJp => "flow",
    }
}

fn overall_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "overall",
        Language::EnUs => "overall",
        Language::JaJp => "overall",
    }
}

fn interpretation_priority_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Interpretation Priority",
        Language::EnUs => "Interpretation Priority",
        Language::JaJp => "Interpretation Priority",
    }
}

fn interpretation_priority_trend_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Trend",
        Language::EnUs => "Trend",
        Language::JaJp => "Trend",
    }
}

fn interpretation_priority_supply_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Supply",
        Language::EnUs => "Supply",
        Language::JaJp => "Supply",
    }
}

fn interpretation_priority_macro_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Macro",
        Language::EnUs => "Macro",
        Language::JaJp => "Macro",
    }
}

fn interpretation_priority_flow_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Flow",
        Language::EnUs => "Flow",
        Language::JaJp => "Flow",
    }
}

fn interpretation_priority_expectation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Expectation",
        Language::EnUs => "Expectation",
        Language::JaJp => "Expectation",
    }
}

fn observation_only_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "observationOnly",
        Language::EnUs => "observationOnly",
        Language::JaJp => "observationOnly",
    }
}

fn day_type_normal(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "normal",
        Language::EnUs => "normal",
        Language::JaJp => "normal",
    }
}

fn day_type_exceptional(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "exceptional",
        Language::EnUs => "exceptional",
        Language::JaJp => "exceptional",
    }
}

fn day_type_reason(
    primary_context: &str,
    trend_breadth_mode: TrendBreadthMode,
    market_cycle_position: MarketCyclePosition,
    flow_acceleration: f64,
    language: Language,
) -> &'static str {
    match (
        primary_context,
        trend_breadth_mode,
        market_cycle_position,
        flow_acceleration,
    ) {
        ("Macro Event", _, _, _) => match language {
            Language::ZhCn => "macro_surprise",
            Language::EnUs => "macro_surprise",
            Language::JaJp => "macro_surprise",
        },
        ("Index Reconstitution", _, _, _) => match language {
            Language::ZhCn => "index_reconstitution",
            Language::EnUs => "index_reconstitution",
            Language::JaJp => "index_reconstitution",
        },
        ("ETF Rebalance", _, _, _) => match language {
            Language::ZhCn => "etf_rebalance",
            Language::EnUs => "etf_rebalance",
            Language::JaJp => "etf_rebalance",
        },
        ("Pre-Earnings Waiting", _, _, _) => match language {
            Language::ZhCn => "major_earnings_surprise",
            Language::EnUs => "major_earnings_surprise",
            Language::JaJp => "major_earnings_surprise",
        },
        ("Major Event Waiting", _, _, _) => match language {
            Language::ZhCn => "unusual_rotation",
            Language::EnUs => "unusual_rotation",
            Language::JaJp => "unusual_rotation",
        },
        (_, TrendBreadthMode::StructuralDefense, _, _) => match language {
            Language::ZhCn => "defensive_rotation",
            Language::EnUs => "defensive_rotation",
            Language::JaJp => "defensive_rotation",
        },
        (_, _, MarketCyclePosition::DistributionWarning, _) => match language {
            Language::ZhCn => "distribution_warning",
            Language::EnUs => "distribution_warning",
            Language::JaJp => "distribution_warning",
        },
        (_, TrendBreadthMode::FragileRotation, _, x) if x.abs() >= 0.10 => match language {
            Language::ZhCn => "abnormal_flow",
            Language::EnUs => "abnormal_flow",
            Language::JaJp => "abnormal_flow",
        },
        _ => match language {
            Language::ZhCn => "trend_continuation",
            Language::EnUs => "trend_continuation",
            Language::JaJp => "trend_continuation",
        },
    }
}

fn market_interpretation_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Boundary: market interpretation is observation only. It does not enter Gate, Execution, Trader, Action Matrix, Position Sizing, risk sizing, or any decision threshold.",
        Language::EnUs => "Boundary: market interpretation is observation only. It does not enter Gate, Execution, Trader, Action Matrix, Position Sizing, risk sizing, or any decision threshold.",
        Language::JaJp => "境界: market interpretation は観測専用であり、Gate、Execution、Trader、Action Matrix、Position Sizing、risk sizing、いかなる decision threshold にも入らない。",
    }
}

fn concentration_label_broad(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "broad_participation",
        Language::EnUs => "broad_participation",
        Language::JaJp => "broad_participation",
    }
}

fn concentration_label_narrow(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "narrow",
        Language::EnUs => "narrow",
        Language::JaJp => "narrow",
    }
}

fn concentration_label_very_narrow(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "very_narrow",
        Language::EnUs => "very_narrow",
        Language::JaJp => "very_narrow",
    }
}

fn concentration_label_rotation(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "rotation",
        Language::EnUs => "rotation",
        Language::JaJp => "rotation",
    }
}

fn concentration_label_defensive(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "defensive",
        Language::EnUs => "defensive",
        Language::JaJp => "defensive",
    }
}

fn leadership_breadth_broad(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "broad",
        Language::EnUs => "broad",
        Language::JaJp => "broad",
    }
}

fn leadership_breadth_narrow(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "narrow",
        Language::EnUs => "narrow",
        Language::JaJp => "narrow",
    }
}

fn leadership_breadth_rotation(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "rotation",
        Language::EnUs => "rotation",
        Language::JaJp => "rotation",
    }
}

fn leadership_breadth_defensive(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "defensive",
        Language::EnUs => "defensive",
        Language::JaJp => "defensive",
    }
}

fn exceptional_factor_macro_surprise(language: Language) -> String {
    match language {
        Language::ZhCn => "macro surprise".to_string(),
        Language::EnUs => "macro surprise".to_string(),
        Language::JaJp => "macro surprise".to_string(),
    }
}

fn exceptional_factor_index_reconstitution(language: Language) -> String {
    match language {
        Language::ZhCn => "index reconstitution".to_string(),
        Language::EnUs => "index reconstitution".to_string(),
        Language::JaJp => "index reconstitution".to_string(),
    }
}

fn exceptional_factor_etf_rebalance(language: Language) -> String {
    match language {
        Language::ZhCn => "ETF rebalance".to_string(),
        Language::EnUs => "ETF rebalance".to_string(),
        Language::JaJp => "ETF rebalance".to_string(),
    }
}

fn exceptional_factor_major_earnings(language: Language) -> String {
    match language {
        Language::ZhCn => "major earnings surprise".to_string(),
        Language::EnUs => "major earnings surprise".to_string(),
        Language::JaJp => "major earnings surprise".to_string(),
    }
}

fn exceptional_factor_abnormal_flow(language: Language) -> String {
    match language {
        Language::ZhCn => "abnormal volume / flow".to_string(),
        Language::EnUs => "abnormal volume / flow".to_string(),
        Language::JaJp => "abnormal volume / flow".to_string(),
    }
}

fn exceptional_factor_unusual_rotation(language: Language) -> String {
    match language {
        Language::ZhCn => "unusual rotation".to_string(),
        Language::EnUs => "unusual rotation".to_string(),
        Language::JaJp => "unusual rotation".to_string(),
    }
}

fn exceptional_factor_structural_defense(language: Language) -> String {
    match language {
        Language::ZhCn => "structural defense".to_string(),
        Language::EnUs => "structural defense".to_string(),
        Language::JaJp => "structural defense".to_string(),
    }
}

fn exceptional_factor_distribution(language: Language) -> String {
    match language {
        Language::ZhCn => "distribution warning".to_string(),
        Language::EnUs => "distribution warning".to_string(),
        Language::JaJp => "distribution warning".to_string(),
    }
}

fn rotation_type_none(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "none",
        Language::EnUs => "none",
        Language::JaJp => "none",
    }
}

fn rotation_type_index(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "index_rotation",
        Language::EnUs => "index_rotation",
        Language::JaJp => "index_rotation",
    }
}

fn rotation_type_macro(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "macro_repricing",
        Language::EnUs => "macro_repricing",
        Language::JaJp => "macro_repricing",
    }
}

fn rotation_type_mega_cap(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "mega_cap_internal_rotation",
        Language::EnUs => "mega_cap_internal_rotation",
        Language::JaJp => "mega_cap_internal_rotation",
    }
}

fn rotation_type_defensive(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "defensive_rotation",
        Language::EnUs => "defensive_rotation",
        Language::JaJp => "defensive_rotation",
    }
}

fn rotation_type_broad(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "broad_participation",
        Language::EnUs => "broad_participation",
        Language::JaJp => "broad_participation",
    }
}

fn rotation_type_sector(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "sector_or_index_rotation",
        Language::EnUs => "sector_or_index_rotation",
        Language::JaJp => "sector_or_index_rotation",
    }
}

fn rotation_interpretation_none(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资金没有明显撤退，仍视为普通延续观察。",
        Language::EnUs => "No clear withdrawal; observation remains on ordinary continuation.",
        Language::JaJp => "資金は明確に撤退しておらず、通常の継続観測とみなす。",
    }
}

fn rotation_interpretation_broad(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "上涨主要来自广泛参与，而不是少数核心资产。",
        Language::EnUs => {
            "The upside is driven by broad participation rather than a small set of leaders."
        }
        Language::JaJp => "上昇は少数の主役ではなく、広い参加によって支えられている。",
    }
}

fn rotation_interpretation_mega_cap(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资金不是撤退，而是在主导大盘的核心资产之间轮动。",
        Language::EnUs => "Capital is not withdrawing; it is rotating within the mega-cap leaders.",
        Language::JaJp => "資金は撤退ではなく、メガキャップ主導銘柄内でローテーションしている。",
    }
}

fn rotation_interpretation_defensive(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资金更偏向防御与低风险承接，整体解释应视为防御轮动。",
        Language::EnUs => "Capital is tilting toward defense and lower-risk absorption; treat this as defensive rotation.",
        Language::JaJp => "資金は防御と低リスク吸収に寄り、全体は防御ローテーションとみなす。",
    }
}

fn rotation_interpretation_sector(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "不是全面撤退，而是行业 / 资产组内部的轮动。",
        Language::EnUs => {
            "This is not broad withdrawal; it is rotation within sectors or asset groups."
        }
        Language::JaJp => "全面的な撤退ではなく、セクター / 資産 समूहの内部ローテーション。",
    }
}

fn rotation_interpretation_macro(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "宏观信息触发重新定价，属于事件驱动的解释层。",
        Language::EnUs => {
            "Macro information triggered repricing; this is an event-driven explanatory layer."
        }
        Language::JaJp => "マクロ情報が再価格付けを引き起こした。イベント駆動の解釈層。",
    }
}

fn rotation_interpretation_index(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "指数调仓 / 重构解释优先，不应误判为结构趋势转折。",
        Language::EnUs => "Index rebalancing / reconstitution explains the move and should not be mistaken for a structural trend turn.",
        Language::JaJp => "指数リバランス / 再構成が主因であり、構造的なトレンド転換と誤読しない。",
    }
}

fn rotation_interpretation_etf(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "ETF 相关调仓更像技术性轮动，不应直接解读为资金撤退。",
        Language::EnUs => "ETF-related rebalancing looks technical and should not be read as outright capital withdrawal.",
        Language::JaJp => "ETF 関連の調整はテクニカルなローテーションであり、資金撤退と直読しない。",
    }
}

fn rotation_interpretation_withdrawal(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "flow 显示撤退迹象，但仍需结合其他层确认是否只是轮动。",
        Language::EnUs => "Flow shows withdrawal signs, but other layers are still needed to confirm whether this is only rotation.",
        Language::JaJp => "flow は撤退の兆候を示すが、単なるローテーションかどうかは他層の確認が必要。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::radar::domain::trend_cohesion::TrendCohesionTopology;
    use chrono::{Duration, NaiveDate};
    use std::fs;

    #[test]
    fn transition_log_keeps_a_stable_single_leader_without_a_new_breakout() {
        let previous = DecisionPacket {
            top_tier_symbols: vec!["GOOG".to_string()],
            trend_cohesion: crate::features::radar::domain::trend_cohesion::TrendCohesionSnapshot {
                topology: TrendCohesionTopology::SingleLeader,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut current = previous.clone();
        current.assets.clear();

        let log = crate::features::radar::domain::transition_log::StateTransitionLog::compare(
            Some(&previous),
            &current,
        );
        let snapshot =
            build_leadership_snapshot_view_model_from_transition_log(&log, Language::EnUs);

        assert_eq!(snapshot.primary_leader_value, "GOOG");
    }

    #[test]
    fn normal_narrative_mentions_new_goog_breakout_without_promoting_structure() {
        let values = market_interpretation_narrative_values(
            "normal",
            "",
            &["TSLA".to_string()],
            &["GOOG".to_string()],
            Language::ZhCn,
        );

        assert!(values[0].contains("TSLA"));
        assert!(values[0].contains("GOOG"));
        assert!(!values[0].contains("SPY 主导"));
    }

    #[test]
    fn rotation_delta_does_not_report_retained_leader_as_rotation() {
        let previous = vec!["TSLA".to_string()];
        let current = vec!["TSLA".to_string(), "ISRG".to_string(), "SPCX".to_string()];

        let (exited, entered) = rotation_delta(&previous, &current);

        assert!(exited.is_empty());
        assert_eq!(entered, vec!["ISRG", "SPCX"]);
    }

    #[test]
    fn read_model_assembles_persisted_streak_and_marks_missing_history_as_degraded() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let observations = (0..25)
            .map(|offset| LeaderObservation {
                date: start + Duration::days(offset),
                leader: "GOOG".to_string(),
                confidence: Some(90.0),
                breadth: None,
                relative_strength: None,
                rotation_stability: None,
                sector_or_index_rotation: None,
                supply_state: None,
            })
            .collect::<Vec<_>>();
        let temp_dir =
            std::env::temp_dir().join(format!("leader_persistence_e2e_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let persistence =
            crate::features::radar::infrastructure::persistence::PersistenceLayer::new(&temp_dir);
        for observation in &observations {
            persistence.save_leader_observation(observation).unwrap();
        }
        let loaded_observations = persistence.load_leader_observations().unwrap();
        let packet = DecisionPacket {
            date: start + Duration::days(25),
            ..Default::default()
        };
        let presentation = PresentationPacket {
            leadership_snapshot: Some(LeadershipSnapshotViewModel {
                primary_leader_value: "GOOG".to_string(),
                leadership_confidence_value: "HIGH".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let view_model = build_leader_persistence_view_model(LeaderPersistenceReadModelInput {
            persisted_observations: &loaded_observations,
            current_packet: &packet,
            current_presentation: &presentation,
            language: Language::ZhCn,
            baseline_date: Some(packet.date - Duration::days(1)),
            baseline_status: "AVAILABLE",
            formal_baseline: None,
        })
        .unwrap();

        assert_eq!(view_model.persistence_days, 20);
        assert!(view_model.boundary.contains("降级"));
        assert_eq!(view_model.primary_leader_value, "GOOG");
        fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn leader_persistence_switch_and_boundary_are_localized_for_all_languages() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 26).unwrap();
        let previous = LeaderObservation {
            date: date - Duration::days(1),
            leader: "MSFT".to_string(),
            confidence: Some(70.0),
            breadth: None,
            relative_strength: None,
            rotation_stability: None,
            sector_or_index_rotation: None,
            supply_state: None,
        };
        let packet = DecisionPacket {
            date,
            ..Default::default()
        };
        let presentation = PresentationPacket {
            leadership_snapshot: Some(LeadershipSnapshotViewModel {
                primary_leader_value: "GOOG".to_string(),
                leadership_confidence_value: "HIGH".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        for (language, switch_marker, boundary_marker) in [
            (Language::ZhCn, "连续天数重置", "边界：仅用于观察"),
            (Language::EnUs, "streak reset", "Boundary: observation only"),
            (Language::JaJp, "連続日数をリセット", "境界：観測専用"),
        ] {
            let view_model = build_leader_persistence_view_model(LeaderPersistenceReadModelInput {
                persisted_observations: std::slice::from_ref(&previous),
                current_packet: &packet,
                current_presentation: &presentation,
                language,
                baseline_date: Some(previous.date),
                baseline_status: "AVAILABLE",
                formal_baseline: None,
            })
            .unwrap();
            assert!(view_model
                .change_from_yesterday_value
                .contains(switch_marker));
            assert!(view_model.boundary.contains(boundary_marker));
        }
    }

    #[test]
    fn read_model_renders_leader_absence_from_a_prior_observation() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
        let previous = LeaderObservation {
            date: date - Duration::days(1),
            leader: "GOOG".to_string(),
            confidence: Some(90.0),
            breadth: Some(70.0),
            relative_strength: Some(70.0),
            rotation_stability: Some(70.0),
            sector_or_index_rotation: None,
            supply_state: None,
        };
        let packet = DecisionPacket {
            date,
            ..Default::default()
        };
        let presentation = PresentationPacket {
            leadership_snapshot: Some(LeadershipSnapshotViewModel {
                primary_leader_value: "none".to_string(),
                leadership_confidence_value: "LOW".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let view_model = build_leader_persistence_view_model(LeaderPersistenceReadModelInput {
            persisted_observations: std::slice::from_ref(&previous),
            current_packet: &packet,
            current_presentation: &presentation,
            language: Language::EnUs,
            baseline_date: Some(previous.date),
            baseline_status: "AVAILABLE",
            formal_baseline: None,
        })
        .unwrap();

        assert_eq!(view_model.primary_leader_value, "none");
        assert_eq!(view_model.leader_state_value, "ABSENT");
        assert_eq!(view_model.leader_absence_duration, 1);
        assert_eq!(view_model.observed_days_value, "0 days");
        assert!(view_model
            .change_from_yesterday_value
            .contains("GOOG -> none"));
    }

    #[test]
    fn read_model_does_not_count_none_observations_as_leadership_days() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
        let observations = vec![
            LeaderObservation {
                date: date - Duration::days(2),
                leader: "GOOG".to_string(),
                confidence: Some(90.0),
                breadth: Some(70.0),
                relative_strength: Some(70.0),
                rotation_stability: Some(70.0),
                sector_or_index_rotation: None,
                supply_state: None,
            },
            LeaderObservation {
                date: date - Duration::days(1),
                leader: "none".to_string(),
                confidence: Some(0.0),
                breadth: Some(0.0),
                relative_strength: Some(0.0),
                rotation_stability: Some(0.0),
                sector_or_index_rotation: None,
                supply_state: None,
            },
        ];
        let packet = DecisionPacket {
            date,
            ..Default::default()
        };
        let presentation = PresentationPacket {
            leadership_snapshot: Some(LeadershipSnapshotViewModel {
                primary_leader_value: "none".to_string(),
                leadership_confidence_value: "LOW".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let view_model = build_leader_persistence_view_model(LeaderPersistenceReadModelInput {
            persisted_observations: &observations,
            current_packet: &packet,
            current_presentation: &presentation,
            language: Language::EnUs,
            baseline_date: Some(observations[0].date),
            baseline_status: "AVAILABLE",
            formal_baseline: None,
        })
        .unwrap();

        assert_eq!(view_model.primary_leader_value, "none");
        assert_eq!(view_model.observed_days_value, "0 days");
    }

    #[test]
    fn read_model_preserves_unavailable_history_coverage() {
        let packet = DecisionPacket {
            date: NaiveDate::from_ymd_opt(2026, 7, 14).unwrap(),
            ..Default::default()
        };
        let presentation = PresentationPacket {
            leadership_snapshot: Some(LeadershipSnapshotViewModel {
                primary_leader_value: "SPY".to_string(),
                leadership_confidence_value: "HIGH".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let view_model = build_leader_persistence_view_model(LeaderPersistenceReadModelInput {
            persisted_observations: &[],
            current_packet: &packet,
            current_presentation: &presentation,
            language: Language::EnUs,
            baseline_date: None,
            baseline_status: "BASELINE_UNAVAILABLE",
            formal_baseline: None,
        })
        .unwrap();

        assert_eq!(view_model.history_coverage_value, "UNAVAILABLE");
    }

    #[test]
    fn read_model_retains_observation_count_when_baseline_date_is_missing() {
        let current_date = NaiveDate::from_ymd_opt(2026, 7, 14).unwrap();
        let older = LeaderObservation {
            date: current_date - Duration::days(2),
            leader: "GOOG".to_string(),
            confidence: Some(90.0),
            breadth: Some(35.0),
            relative_strength: Some(16.0),
            rotation_stability: Some(82.0),
            sector_or_index_rotation: None,
            supply_state: None,
        };
        let packet = DecisionPacket {
            date: current_date,
            ..Default::default()
        };
        let presentation = PresentationPacket {
            leadership_snapshot: Some(LeadershipSnapshotViewModel {
                primary_leader_value: "SPY".to_string(),
                leadership_confidence_value: "HIGH".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let view_model = build_leader_persistence_view_model(LeaderPersistenceReadModelInput {
            persisted_observations: std::slice::from_ref(&older),
            current_packet: &packet,
            current_presentation: &presentation,
            language: Language::EnUs,
            baseline_date: Some(current_date - Duration::days(1)),
            baseline_status: "BASELINE_UNAVAILABLE",
            formal_baseline: None,
        })
        .unwrap();

        assert_eq!(view_model.history_coverage_value, "BASELINE_UNAVAILABLE");
        assert_eq!(view_model.previous_leader_value, Some("GOOG".to_string()));
        assert_eq!(view_model.observed_days_value, "1 days");
    }

    #[test]
    fn read_model_reports_partial_observation_history_without_formal_baseline() {
        let current_date = NaiveDate::from_ymd_opt(2026, 7, 14).unwrap();
        let older = LeaderObservation {
            date: current_date - Duration::days(1),
            leader: "SPY".to_string(),
            confidence: Some(80.0),
            breadth: Some(40.0),
            relative_strength: Some(12.0),
            rotation_stability: Some(70.0),
            sector_or_index_rotation: None,
            supply_state: None,
        };
        let packet = DecisionPacket {
            date: current_date,
            ..Default::default()
        };
        let presentation = PresentationPacket {
            leadership_snapshot: Some(LeadershipSnapshotViewModel {
                primary_leader_value: "SPY".to_string(),
                leadership_confidence_value: "HIGH".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let view_model = build_leader_persistence_view_model(LeaderPersistenceReadModelInput {
            persisted_observations: std::slice::from_ref(&older),
            current_packet: &packet,
            current_presentation: &presentation,
            language: Language::EnUs,
            baseline_date: Some(current_date - Duration::days(1)),
            baseline_status: "BASELINE_UNAVAILABLE",
            formal_baseline: None,
        })
        .unwrap();

        assert_eq!(view_model.history_coverage_value, "BASELINE_UNAVAILABLE");
        assert_eq!(view_model.observed_days_value, "2 days");
        assert_eq!(view_model.leader_state_value, "UNAVAILABLE");
    }

    #[test]
    fn leader_labels_identify_composite_ranking_semantics() {
        assert_eq!(primary_leader_label(Language::EnUs), "Composite Leader");
        assert_eq!(
            leader_persistence_primary_label(Language::EnUs),
            "Composite Leader"
        );
    }

    #[test]
    fn breadth_semantic_label_matches_fragile_rotation_state() {
        assert_eq!(
            breadth_semantic_value(
                TrendBreadthMode::FragileRotation,
                &LeadershipSnapshotViewModel::default(),
                MarketCyclePosition::EarlyFormation,
                Language::EnUs,
            ),
            "Narrow"
        );
        assert_eq!(
            breadth_semantic_value(
                TrendBreadthMode::BroadExpansion,
                &LeadershipSnapshotViewModel::default(),
                MarketCyclePosition::EarlyFormation,
                Language::EnUs,
            ),
            "Broad Participation"
        );
    }
}

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Weekday};
use std::collections::BTreeMap;
use std::path::Path;

use crate::features::shared::application::run_status::DeliveryStatus;
use crate::features::shared::infrastructure::run_status_reader::load_run_evidence_collection_status;
use crate::features::shared::interface::i18n::Language;

#[derive(Debug, Clone)]
pub(crate) struct TransitionAuditEntry {
    pub(crate) date: NaiveDate,
    pub(crate) timestamp: DateTime<FixedOffset>,
    pub(crate) log: crate::features::radar::domain::transition_log::StateTransitionLog,
}

#[derive(Debug, Clone)]
pub(crate) struct TransitionAuditDay {
    pub(crate) date: NaiveDate,
    pub(crate) events: Vec<TransitionAuditEntry>,
}

pub(crate) fn resolve_audit_daily_formal_baseline(
    save_dir: &Path,
    current_market_date: NaiveDate,
) -> Result<Option<crate::features::radar::infrastructure::persistence::TradingDaySnapshot>> {
    let persistence =
        crate::features::radar::infrastructure::persistence::PersistenceLayer::new(save_dir);
    let cycle_id = persistence
        .load_observation_history_state()?
        .filter(|state| !state.cycle_id.is_empty())
        .map(|state| state.cycle_id);
    Ok(persistence
        .resolve_previous_snapshot_from_history(current_market_date, cycle_id.as_deref())?
        .formal_snapshot)
}

impl TransitionAuditDay {
    pub(crate) fn latest(&self) -> &TransitionAuditEntry {
        self.events
            .last()
            .expect("TransitionAuditDay must include at least one event")
    }
}

pub(crate) fn parse_transition_audit_entry(
    line: &str,
    language: Language,
) -> Result<Option<TransitionAuditEntry>> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    let timestamp = if let Some(ts) = value.get("timestamp").and_then(|v| v.as_str()) {
        DateTime::parse_from_rfc3339(ts)
            .with_context(|| format!("{}: {}", audit_error_invalid_timestamp(language), ts))?
    } else {
        return Ok(None);
    };

    let date = match value.get("date").and_then(|v| v.as_str()) {
        Some(raw_date) => NaiveDate::parse_from_str(raw_date, "%Y-%m-%d")
            .with_context(|| format!("{}: {}", audit_error_invalid_date(language), raw_date))?,
        None => timestamp.date_naive(),
    };

    let log_value = value
        .get("transition")
        .cloned()
        .or_else(|| value.get("log").cloned());
    let Some(log_json) = log_value else {
        return Ok(None);
    };

    let log: crate::features::radar::domain::transition_log::StateTransitionLog =
        serde_json::from_value(log_json)?;
    Ok(Some(TransitionAuditEntry {
        date,
        timestamp,
        log,
    }))
}

pub(crate) fn resolve_target_index(
    days: &[TransitionAuditDay],
    target_date: Option<NaiveDate>,
    language: Language,
) -> Result<usize> {
    if let Some(date) = target_date {
        days.iter()
            .position(|e| e.date == date)
            .with_context(|| format!("{} {}", audit_error_target_date_not_found(language), date))
    } else {
        Ok(days.len() - 1)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DailyCalibrationContext {
    pub(crate) calibration_date: NaiveDate,
    pub(crate) audit_section: String,
    pub(crate) questions_section: String,
}

pub(crate) async fn build_daily_calibration_context(
    save_dir: &std::path::Path,
    target_date_arg: Option<&str>,
    window_days: usize,
    attention_count: usize,
    thesis_count: usize,
    language: Language,
) -> Result<DailyCalibrationContext> {
    let path = save_dir.join("state_transitions.jsonl");
    let days = load_transition_audit_days(&path, language)?;

    let target_date = match target_date_arg {
        Some(raw) => Some(
            chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d").with_context(|| {
                format!(
                    "{}: {}",
                    crate::features::research::interface::valuation_gravity_i18n::future_date_error(
                        language
                    ),
                    raw
                )
            })?,
        ),
        None => None,
    };
    let current_date = chrono::Local::now().date_naive();
    if target_date.is_some_and(|date| date > current_date) {
        return Err(anyhow!(
            "{}: {}",
            crate::features::research::interface::valuation_gravity_i18n::future_date_error(
                language
            ),
            target_date.expect("future target date is present")
        ));
    }

    let mut calibration_date = target_date.unwrap_or(current_date);
    let mut selected_day: Option<&TransitionAuditDay> = None;
    let audit_section = if days.is_empty() {
        audit_empty_log_message(language).to_string()
    } else {
        match resolve_target_index(&days, target_date, language) {
            Ok(target_idx) => {
                calibration_date = days[target_idx].date;
                selected_day = Some(&days[target_idx]);
                let evidence_collection_status =
                    load_run_evidence_collection_status(save_dir, days[target_idx].date)
                        .unwrap_or(DeliveryStatus::Skipped);
                let persistence =
                    crate::features::radar::infrastructure::persistence::PersistenceLayer::new(
                        save_dir,
                    );
                let cycle_id = persistence
                    .load_observation_history_state()
                    .context("failed to load observation_history_state.json for audit daily")?
                    .filter(|state| !state.cycle_id.is_empty())
                    .map(|state| state.cycle_id);
                let formal_baseline = persistence
                    .resolve_previous_snapshot_from_history(
                        days[target_idx].date,
                        cycle_id.as_deref(),
                    )
                    .context("failed to resolve previous snapshot for audit daily")?
                    .formal_snapshot;
                build_audit_daily_report_with_formal_baseline(
                    &days,
                    target_idx,
                    window_days.max(1),
                    language,
                    Some(&evidence_collection_status),
                    Some(formal_baseline.as_ref()),
                )
            }
            Err(_) => audit_empty_log_message(language).to_string(),
        }
    };

    let questions_section =
        build_daily_calibration_questions(attention_count, thesis_count, selected_day, language);

    Ok(DailyCalibrationContext {
        calibration_date,
        audit_section,
        questions_section,
    })
}

#[cfg(test)]
pub(crate) fn build_audit_daily_report_with_evidence_status(
    days: &[TransitionAuditDay],
    target_idx: usize,
    window_days: usize,
    language: Language,
    evidence_collection_status: Option<
        &crate::features::shared::application::run_status::DeliveryStatus,
    >,
) -> String {
    build_audit_daily_report_with_formal_baseline(
        days,
        target_idx,
        window_days,
        language,
        evidence_collection_status,
        None,
    )
}

pub(crate) fn build_audit_daily_report_with_formal_baseline(
    days: &[TransitionAuditDay],
    target_idx: usize,
    window_days: usize,
    language: Language,
    evidence_collection_status: Option<
        &crate::features::shared::application::run_status::DeliveryStatus,
    >,
    formal_baseline: Option<
        Option<&crate::features::radar::infrastructure::persistence::TradingDaySnapshot>,
    >,
) -> String {
    let text = audit_text(language);
    let complete_formal_baseline = formal_baseline
        .flatten()
        .is_some_and(|snapshot| snapshot.source_status == "complete");
    let today = &days[target_idx];
    let today_latest = today.latest();
    let window_start = target_idx.saturating_sub(window_days.saturating_sub(1));
    let window = &days[window_start..=target_idx];
    let window_latest = window.iter().map(|d| d.latest()).collect::<Vec<_>>();

    let gate_is_ready = today_latest.log.trend_cohesion_gate.to;
    let gate_status = if gate_is_ready {
        text.status_ready
    } else {
        text.status_no_trade
    };
    let no_trade_mode = opportunity_mode_label(today_latest.log.opportunity_mode.to, language);
    let scout_days_without_expansion = today_latest.log.scout_days_without_expansion;
    let scout_abort_days = today_latest.log.scout_abort_days.max(1);
    let scout_streak_text = if today_latest.log.opportunity_mode.to
        == crate::features::radar::domain::transition_log::OpportunityMode::NoTradeScout
    {
        format!(
            "{} / {} {}",
            scout_days_without_expansion, scout_abort_days, text.day_unit
        )
    } else {
        text.none.to_string()
    };
    let gate_streak = consecutive_streak(days, target_idx, |log| {
        log.trend_cohesion_gate.to == gate_is_ready
    });

    let blocker_counts = summarize_blockers(&window_latest);
    let top_blockers = blocker_counts.into_iter().take(3).collect::<Vec<_>>();

    let breakout_today = summarize_breakout_changes_from_events(today);
    let no_trade_streak = consecutive_streak(days, target_idx, |log| !log.trend_cohesion_gate.to);
    let mainline_missing_streak = consecutive_streak(days, target_idx, |log| {
        log.trend_cohesion_status.to
            != crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Formed
    });

    let segment_type = if detect_no_trade_resets(window) {
        text.segment_reset
    } else {
        text.segment_continuous
    };

    let transition_state_change = yes_no(
        today.events.iter().any(|e| e.log.market_state.changed),
        language,
    );
    let transition_risk_change = yes_no(
        today.events.iter().any(|e| e.log.risk_overlay.changed),
        language,
    );
    let transition_trend_change = yes_no(
        today
            .events
            .iter()
            .any(|e| e.log.trend_cohesion_status.changed),
        language,
    );
    let transition_mode_change = yes_no(
        today.events.iter().any(|e| e.log.opportunity_mode.changed),
        language,
    );
    let transition_scout_reset = yes_no(
        today.events.iter().any(|e| e.log.scout_reset_triggered),
        language,
    );

    let blocker_text = if gate_is_ready || top_blockers.is_empty() {
        text.none.to_string()
    } else {
        top_blockers
            .iter()
            .map(|(name, _)| blocker_label(name, language))
            .collect::<Vec<_>>()
            .join(" / ")
    };
    let breakout_text = summarize_breakout_sentence(&breakout_today, language);
    let mainline_text = trend_status_label(today_latest.log.trend_cohesion_status.to, language);

    let (substantive_summaries, excluded_non_production_evidence) = {
        let mut summaries = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();
        let mut excluded_count = 0;
        for event in &today.events {
            if let Some(ref rec) = event.log.trend_recognition {
                if let Some(ref substantive) = rec.substantive {
                    for record in &substantive.records {
                        if !record.is_production_eligible() {
                            excluded_count += 1;
                            continue;
                        }
                        let key = if record.dedupe_key().is_empty() {
                            format!(
                                "{:?}:{:?}:{:?}:{}:{}:{}",
                                record.source,
                                record.evidence_type,
                                record.symbol,
                                record.event_date,
                                record.source_url.as_deref().unwrap_or("NO_URL"),
                                record.description
                            )
                        } else {
                            record.dedupe_key().to_string()
                        };
                        if seen_keys.insert(key) {
                            let symbol_part = record
                                .symbol
                                .as_ref()
                                .map(|s| format!("[{}] ", s))
                                .unwrap_or_default();
                            let date_part = format!("[{}] ", record.event_date);
                            let type_part = format!("[{:?}] ", record.evidence_type);
                            let conf_part = format!("(conf:{:.2}) ", record.confidence);
                            let source_part = format!("[{:?}] ", record.source);

                            let url_part = record
                                .source_url
                                .as_deref()
                                .map(|u| format!(" ({})", u))
                                .unwrap_or_default();

                            let description =
                                format_evidence_description(&record.description, language);

                            summaries.push(format!(
                                "- {}{}{}{}{}{}{}",
                                symbol_part,
                                date_part,
                                type_part,
                                conf_part,
                                source_part,
                                description,
                                url_part
                            ));
                        }
                    }
                }
            }
        }
        (summaries, excluded_count)
    };

    let audit_sentence = build_audit_sentence(
        language,
        AuditSentenceContext {
            gate_status,
            gate_streak,
            blocker_text: &blocker_text,
            breakout_text: &breakout_text,
            mainline_text,
            no_trade_mode,
            complete_formal_baseline,
        },
    );

    let mut out = String::new();
    out.push_str(&format!(
        "# {} ({})\n\n",
        text.title,
        today_latest.date.format("%Y-%m-%d")
    ));

    out.push_str(&format!("1. {}\n", text.section_gate));
    out.push_str(&format!("- {}: {}\n", text.label_status, gate_status));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_no_trade_mode, no_trade_mode
    ));
    out.push_str(&format!(
        "- {}: {} {}\n",
        text.label_duration, gate_streak, text.day_unit
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_scout_streak, scout_streak_text
    ));
    out.push_str(&format!("- {}:\n", text.label_top_blockers));
    if top_blockers.is_empty() {
        out.push_str(&format!("- {}\n", text.none));
    } else {
        for (name, count) in &top_blockers {
            out.push_str(&format!(
                "- {} ({})\n",
                blocker_label(name, language),
                count
            ));
        }
    }

    out.push_str(&format!("\n2. {}\n", text.section_transition));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_state_change, transition_state_change
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_risk_change, transition_risk_change
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_trend_change, transition_trend_change
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_mode_change, transition_mode_change
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_scout_reset, transition_scout_reset
    ));

    out.push_str(&format!("\n3. {}\n", text.section_breakout));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_breakout_new,
        format_symbols(&breakout_today.new_symbols, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_breakout_continued,
        format_symbols(&breakout_today.continued_symbols, language)
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_breakout_removed,
        format_symbols(&breakout_today.removed_symbols, language)
    ));

    out.push_str(&format!("\n4. {}\n", text.section_substantive));
    if let Some(status) = evidence_collection_status {
        out.push_str(&format!(
            "- {}: {}\n",
            text.label_evidence_collection,
            format_delivery_status(status, language)
        ));
    }
    out.push_str(&format!("- {}:\n", text.label_evidence_stock));
    if substantive_summaries.is_empty() {
        out.push_str(&format!("- {}\n", text.none));
    } else {
        for summary in substantive_summaries {
            out.push_str(&format!("{}\n", summary));
        }
    }
    if excluded_non_production_evidence > 0 {
        out.push_str(&format!(
            "- {}: {}. {}\n",
            text.label_evidence_excluded,
            excluded_non_production_evidence,
            text.note_evidence_excluded
        ));
    }

    out.push_str(&format!("\n5. {}\n", text.section_streaks));
    out.push_str(&format!(
        "- {}: {} {}\n",
        text.label_no_trade_streak, no_trade_streak, text.day_unit
    ));
    out.push_str(&format!(
        "- {}: {} {}\n",
        text.label_mainline_missing_streak, mainline_missing_streak, text.day_unit
    ));
    out.push_str(&format!(
        "- {}: {}\n",
        text.label_recent_shape, segment_type
    ));
    out.push_str(&format!("- {}\n", text.methodology_note));

    out.push_str(&format!("\n6. {}\n", text.section_one_liner));
    out.push_str(&format!("- {}\n", audit_sentence));

    out.push_str(&build_market_interpretation_audit_snapshot(
        today,
        language,
        &text,
        formal_baseline,
    ));

    if let Some(evidence) = &today_latest.log.trend_recognition {
        let dict = crate::features::shared::interface::i18n::get_dictionary(language);
        let tr_dict = &dict.trend_recognition;

        let state_label = match evidence.state {
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::None => &tr_dict.state_none,
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::StructuralPersistence => {
                &tr_dict.state_structural_persistence
            }
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::EarlyLeader => &tr_dict.state_early_leader,
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::LeaderConfirmedFollowersLagging => &tr_dict.state_leader_confirmed_followers_lagging,
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::Broadening => &tr_dict.state_broadening,
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::Mature => &tr_dict.state_mature,
        };

        let lag_label = if evidence.lag_state {
            &tr_dict.lag_alert
        } else {
            text.none
        };

        let formatted = text
            .template_trend_recognition
            .replace("{state}", state_label)
            .replace("{score}", &format!("{:.2}", evidence.diffusion_score))
            .replace("{conviction}", &format!("{:.2}", evidence.conviction_score))
            .replace("{lag_state}", lag_label);

        out.push_str(&format!("{}\n", formatted));
    }

    out
}

fn build_market_interpretation_audit_snapshot(
    day: &TransitionAuditDay,
    language: Language,
    text: &AuditDailyText,
    formal_baseline: Option<
        Option<&crate::features::radar::infrastructure::persistence::TradingDaySnapshot>,
    >,
) -> String {
    let latest = day.latest();
    let formal_snapshot = formal_baseline
        .flatten()
        .filter(|snapshot| snapshot.source_status == "complete");
    let trend_recognition = latest.log.trend_recognition.as_ref();
    let leadership_snapshot =
        crate::features::radar::interface::market_interpretation_read_model::build_leadership_snapshot_view_model_from_transition_log(&latest.log, language);

    let exceptional_factors = audit_exceptional_factors(latest, trend_recognition);
    let day_type = if exceptional_factors.is_empty() {
        "normal"
    } else {
        "exceptional"
    };
    let day_type_reason = audit_day_type_reason(latest, trend_recognition);
    let primary_count = usize::from(leadership_snapshot.primary_leader_value != "none");
    let concentration = audit_concentration_scores(
        primary_count,
        leadership_snapshot.secondary_leaders_values.len(),
        leadership_snapshot.watchlist_leaders_values.len(),
        latest,
    );
    let leadership_consistency_valid = leadership_snapshot.leadership_conflict_value == "none";
    let classification_value = if leadership_consistency_valid {
        concentration.3.clone()
    } else {
        match language {
            Language::ZhCn => "检测到 leadership 冲突".to_string(),
            Language::EnUs => "Leadership conflict detected".to_string(),
            Language::JaJp => "Leadership conflict detected".to_string(),
        }
    };
    let mut rotation_type = audit_rotation_type(latest, trend_recognition);
    let previous_leadership = formal_snapshot.map(|snapshot| {
        (
            snapshot.primary_leader.clone(),
            snapshot.secondary_leaders.clone(),
        )
    });
    let rotation_from = previous_leadership
        .as_ref()
        .map(|(primary, supporting)| {
            primary
                .iter()
                .chain(supporting.iter())
                .filter(|symbol| !symbol.is_empty() && *symbol != "none")
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut rotation_to = vec![leadership_snapshot.primary_leader_value.clone()];
    let additional_supporting = leadership_snapshot
        .secondary_leaders_values
        .iter()
        .filter(|symbol| !rotation_to.contains(symbol))
        .cloned()
        .collect::<Vec<_>>();
    rotation_to.extend(additional_supporting);
    if formal_snapshot.is_none() {
        rotation_type = "BASELINE_UNAVAILABLE".to_string();
        rotation_to.clear();
    }

    let confidence = audit_confidence_labels(latest, trend_recognition, exceptional_factors.len());
    let priority = audit_interpretation_priority(&confidence);
    let current_leaders = std::iter::once(leadership_snapshot.primary_leader_value.clone())
        .chain(leadership_snapshot.secondary_leaders_values.iter().cloned())
        .filter(|symbol| !symbol.is_empty() && symbol != "none")
        .collect::<Vec<_>>();
    let breakout_leaders = latest
        .log
        .breakout_changes
        .iter()
        .filter(|change| {
            change.to_status
                != crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
        })
        .map(|change| change.symbol.clone())
        .collect::<Vec<_>>();
    let narrative_values = audit_market_interpretation_narrative_values(
        day_type,
        &current_leaders,
        &breakout_leaders,
        language,
    );

    let mut out = String::new();
    out.push_str(&format!("\n7. {}\n", text.market_interpretation_snapshot));
    out.push_str("- decision_weight: 0%\n");
    out.push_str(&format!("- dayType: {}\n", day_type));
    out.push_str(&format!("- reason: {}\n", day_type_reason));
    out.push_str(&format!(
        "- exceptionalFactors: {}\n",
        format_string_list(&exceptional_factors)
    ));
    out.push_str("- Market Interpretation:\n");
    for line in &narrative_values {
        out.push_str(&format!("  - {}\n", line));
    }
    out.push_str(&format!(
        "- Leadership Classification: {}\n",
        classification_value
    ));
    out.push_str("- Leadership Metrics:\n");
    if !leadership_consistency_valid {
        out.push_str("  - leadership detail suppressed because the leadership sets conflict.\n");
    }
    out.push_str(&format!(
        "  - Breadth label: {}\n",
        audit_breadth_label(primary_count, language)
    ));
    out.push_str(&format!("  - Breadth score: {}\n", concentration.0));
    out.push_str(&format!("  - Concentration score: {}\n", concentration.1));
    out.push_str(&format!("  - Rotation score: {}\n", concentration.2));
    out.push_str("- Rotation Observation:\n");
    out.push_str(&format!("  - rotationType: {}\n", rotation_type));
    out.push_str(&format!(
        "  - from: {}\n",
        format_string_list(&rotation_from)
    ));
    out.push_str(&format!("  - to: {}\n", format_string_list(&rotation_to)));
    out.push_str(&format!(
        "  - interpretation: {}\n",
        audit_rotation_interpretation(language, &rotation_type)
    ));
    out.push_str("  - observationOnly: true\n");
    out.push_str("- Observation Confidence:\n");
    out.push_str(&format!("  - trend: {}\n", confidence.0));
    out.push_str(&format!("  - macro: {}\n", confidence.1));
    out.push_str(&format!("  - supply: {}\n", confidence.2));
    out.push_str(&format!("  - expectation: {}\n", confidence.3));
    out.push_str(&format!("  - gravity: {}\n", confidence.4));
    out.push_str(&format!("  - flow: {}\n", confidence.5));
    out.push_str(&format!("  - overall: {}\n", confidence.6));
    out.push_str("- Interpretation Priority:\n");
    for item in priority {
        out.push_str(&format!("  - {}\n", item));
    }
    out.push_str(&format!("- {}\n", text.market_interpretation_boundary));
    out
}

fn audit_breadth_label(primary_count: usize, language: Language) -> &'static str {
    match (primary_count, language) {
        (0, Language::ZhCn) => "Very Narrow",
        (1, Language::ZhCn) => "Narrow",
        (2, Language::ZhCn) => "Healthy Expansion",
        (_, Language::ZhCn) => "Broad Participation",
        (0, Language::EnUs) => "Very Narrow",
        (1, Language::EnUs) => "Narrow",
        (2, Language::EnUs) => "Healthy Expansion",
        (_, Language::EnUs) => "Broad Participation",
        (0, Language::JaJp) => "Very Narrow",
        (1, Language::JaJp) => "Narrow",
        (2, Language::JaJp) => "Healthy Expansion",
        (_, Language::JaJp) => "Broad Participation",
    }
}

fn format_string_list(values: &[String]) -> String {
    if values.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", values.join(", "))
    }
}

fn audit_exceptional_factors(
    latest: &TransitionAuditEntry,
    _trend_recognition: Option<
        &crate::features::radar::domain::trend_cohesion::TrendRecognitionEvidence,
    >,
) -> Vec<String> {
    let mut factors = Vec::new();
    if latest.log.market_state.changed {
        factors.push("macro_surprise".to_string());
    }
    if latest.log.risk_overlay.changed {
        factors.push("abnormal_volume_or_flow".to_string());
    }
    if !latest.log.breakout_changes.is_empty() {
        factors.push("unusual_rotation".to_string());
    }
    factors.sort();
    factors.dedup();
    factors
}

fn audit_day_type_reason(
    latest: &TransitionAuditEntry,
    trend_recognition: Option<
        &crate::features::radar::domain::trend_cohesion::TrendRecognitionEvidence,
    >,
) -> String {
    if latest.log.market_state.changed {
        return "macro_surprise".to_string();
    }
    if !latest.log.breakout_changes.is_empty() {
        return "unusual_rotation".to_string();
    }
    if latest.log.risk_overlay.changed {
        return "abnormal_flow".to_string();
    }
    if trend_recognition.is_some() {
        return "trend_continuation".to_string();
    }
    "trend_continuation".to_string()
}

fn audit_market_interpretation_narrative_values(
    day_type: &str,
    current_leaders: &[String],
    breakout_leaders: &[String],
    language: Language,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(match (day_type, language) {
        ("normal", Language::ZhCn) => "今天是正常趋势延续。".to_string(),
        ("normal", Language::EnUs) => "Today is a normal trend continuation.".to_string(),
        ("normal", Language::JaJp) => "今日は通常のトレンド継続です。".to_string(),
        ("exceptional", Language::ZhCn) => "今天属于例外驱动日。".to_string(),
        ("exceptional", Language::EnUs) => "Today is an exception-driven day.".to_string(),
        ("exceptional", Language::JaJp) => "今日は例外駆動の日です。".to_string(),
        _ => "Today is a normal trend continuation.".to_string(),
    });
    let leaders = if current_leaders.is_empty() {
        "UNAVAILABLE".to_string()
    } else {
        current_leaders.join(", ")
    };
    let breakouts = if breakout_leaders.is_empty() {
        "UNAVAILABLE".to_string()
    } else {
        breakout_leaders.join(", ")
    };
    lines.push(match language {
        Language::ZhCn => format!("当前主导资产：{}；突破观察：{}。", leaders, breakouts),
        Language::EnUs => format!(
            "Current leaders: {}; breakout watch: {}.",
            leaders, breakouts
        ),
        Language::JaJp => format!(
            "現在の主導銘柄：{}；ブレイクアウト観察：{}。",
            leaders, breakouts
        ),
    });
    lines
}

fn audit_concentration_scores(
    primary_count: usize,
    supporting_count: usize,
    weakening_count: usize,
    latest: &TransitionAuditEntry,
) -> (usize, usize, usize, String) {
    let breadth_score = match primary_count + supporting_count + weakening_count {
        0 => 0,
        1 => 18,
        2 => 32,
        3 => 45,
        4 => 58,
        5 => 68,
        _ => 76,
    };
    let concentration_score = 100usize.saturating_sub(breadth_score);
    let rotation_score = if latest.log.breakout_changes.is_empty() {
        18
    } else {
        66
    };
    let label = if concentration_score >= 80 {
        "very_narrow"
    } else if concentration_score >= 60 {
        "narrow"
    } else if concentration_score >= 40 {
        "mixed"
    } else {
        "broad"
    };
    (
        breadth_score,
        concentration_score,
        rotation_score,
        label.to_string(),
    )
}

fn audit_rotation_type(
    latest: &TransitionAuditEntry,
    trend_recognition: Option<
        &crate::features::radar::domain::trend_cohesion::TrendRecognitionEvidence,
    >,
) -> String {
    if latest.log.market_state.changed
        && matches!(
            latest.log.market_state.to,
            crate::features::radar::domain::market_regime::MarketState::DEFENSIVE
        )
    {
        return "defensive_rotation".to_string();
    }
    if latest.log.market_state.changed {
        return "index_rotation".to_string();
    }
    if !latest.log.breakout_changes.is_empty() {
        return "mega_cap_internal_rotation".to_string();
    }
    if matches!(
        trend_recognition.map(|e| e.state),
        Some(
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::LeaderConfirmedFollowersLagging
                | crate::features::radar::domain::trend_cohesion::TrendContinuationState::Broadening
                | crate::features::radar::domain::trend_cohesion::TrendContinuationState::Mature
        )
    ) {
        return "broad_participation".to_string();
    }
    "none".to_string()
}

fn audit_rotation_interpretation(language: Language, rotation_type: &str) -> String {
    match language {
        Language::ZhCn => match rotation_type {
            "defensive_rotation" => "资金更偏向防御板块内切换。".to_string(),
            "index_rotation" => "更像指数内部轮动，而不是系统性撤退。".to_string(),
            "mega_cap_internal_rotation" => "资金在主导资产内部轮动，而非整体流出。".to_string(),
            "broad_participation" => "参与面扩展，轮动更接近广度扩散。".to_string(),
            _ => "当前未观察到明确的轮动切换。".to_string(),
        },
        Language::EnUs => match rotation_type {
            "defensive_rotation" => "Flow is rotating into defensive groups.".to_string(),
            "index_rotation" => {
                "This looks like index-level rotation rather than broad retreat.".to_string()
            }
            "mega_cap_internal_rotation" => {
                "Flow is rotating within the leading mega caps rather than exiting.".to_string()
            }
            "broad_participation" => {
                "Participation is broadening rather than collapsing.".to_string()
            }
            _ => "No clear rotation regime is observable.".to_string(),
        },
        Language::JaJp => match rotation_type {
            "defensive_rotation" => "資金は防御グループ内へ回転している。".to_string(),
            "index_rotation" => {
                "広範な撤退ではなく、指数内部のローテーションとして観測される。".to_string()
            }
            "mega_cap_internal_rotation" => {
                "資金は主導大型株の内部で回っており、全面流出ではない。".to_string()
            }
            "broad_participation" => {
                "参加銘柄が広がっており、広がりを伴うローテーションとして観測される。".to_string()
            }
            _ => "明確なローテーションは観測されない。".to_string(),
        },
    }
}

fn audit_confidence_labels(
    latest: &TransitionAuditEntry,
    trend_recognition: Option<
        &crate::features::radar::domain::trend_cohesion::TrendRecognitionEvidence,
    >,
    exceptional_count: usize,
) -> (String, String, String, String, String, String, String) {
    let trend = match trend_recognition.map(|e| e.state) {
        Some(
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::LeaderConfirmedFollowersLagging
                | crate::features::radar::domain::trend_cohesion::TrendContinuationState::Broadening
                | crate::features::radar::domain::trend_cohesion::TrendContinuationState::Mature,
        ) => "HIGH",
        Some(
            crate::features::radar::domain::trend_cohesion::TrendContinuationState::StructuralPersistence
                | crate::features::radar::domain::trend_cohesion::TrendContinuationState::EarlyLeader,
        ) => "MEDIUM",
        Some(crate::features::radar::domain::trend_cohesion::TrendContinuationState::None) | None => "LOW",
    };
    let macro_confidence = if latest.log.market_state.changed {
        "MEDIUM"
    } else {
        "LOW"
    };
    let supply = if latest.log.breakout_changes.is_empty() {
        "LOW"
    } else {
        "MEDIUM"
    };
    let expectation = "NONE";
    let gravity = "NONE";
    let flow = if latest.log.risk_overlay.changed || exceptional_count > 1 {
        "MEDIUM"
    } else {
        "LOW"
    };
    let overall = if trend == "HIGH" && flow == "MEDIUM" {
        "MEDIUM"
    } else if trend == "HIGH" {
        "HIGH"
    } else if trend == "MEDIUM" || flow == "MEDIUM" {
        "MEDIUM"
    } else {
        "LOW"
    };
    (
        trend.to_string(),
        macro_confidence.to_string(),
        supply.to_string(),
        expectation.to_string(),
        gravity.to_string(),
        flow.to_string(),
        overall.to_string(),
    )
}

fn audit_interpretation_priority(
    confidence: &(String, String, String, String, String, String, String),
) -> Vec<String> {
    let priority = [
        ("Trend", &confidence.0),
        ("Supply", &confidence.2),
        ("Macro", &confidence.1),
        ("Flow", &confidence.5),
        ("Expectation", &confidence.3),
    ];
    priority
        .iter()
        .map(|(label, confidence)| match confidence.as_str() {
            "HIGH" => format!("{}: ★★★★★", label),
            "MEDIUM" => format!("{}: ★★", label),
            "LOW" => format!("{}: ★", label),
            _ => format!("{}: ☆", label),
        })
        .collect()
}

struct AuditDailyText {
    title: &'static str,
    section_gate: &'static str,
    section_transition: &'static str,
    section_breakout: &'static str,
    section_streaks: &'static str,
    section_substantive: &'static str,
    section_one_liner: &'static str,
    market_interpretation_snapshot: &'static str,
    market_interpretation_boundary: &'static str,
    label_status: &'static str,
    label_duration: &'static str,
    label_no_trade_mode: &'static str,
    label_scout_streak: &'static str,
    label_top_blockers: &'static str,
    label_state_change: &'static str,
    label_risk_change: &'static str,
    label_trend_change: &'static str,
    label_mode_change: &'static str,
    label_scout_reset: &'static str,
    label_breakout_new: &'static str,
    label_breakout_continued: &'static str,
    label_breakout_removed: &'static str,
    label_evidence_collection: &'static str,
    label_evidence_stock: &'static str,
    label_evidence_excluded: &'static str,
    note_evidence_excluded: &'static str,
    label_no_trade_streak: &'static str,
    label_mainline_missing_streak: &'static str,
    label_recent_shape: &'static str,
    methodology_note: &'static str,
    none: &'static str,
    yes: &'static str,
    no: &'static str,
    segment_reset: &'static str,
    segment_continuous: &'static str,
    status_no_trade: &'static str,
    status_ready: &'static str,
    mode_cold: &'static str,
    mode_scout: &'static str,
    mode_ready: &'static str,
    day_unit: &'static str,
    template_trend_recognition: &'static str,
}

fn audit_text(language: Language) -> AuditDailyText {
    match language {
        Language::ZhCn => AuditDailyText {
            title: "每日审计",
            section_gate: "门槛摘要",
            section_transition: "状态变化摘要",
            section_breakout: "突破摘要",
            section_streaks: "连续段统计",
            section_substantive: "实体证据摘要",
            section_one_liner: "审计一句话",
            market_interpretation_snapshot: "市场解释快照",
            market_interpretation_boundary: "说明: 这是审计侧的解释快照，仅用于 report / review，不接入 Gate、Execution、Trader、Action Matrix 或 Position Sizing。",
            label_status: "状态",
            label_duration: "持续天数",
            label_no_trade_mode: "NO TRADE 分层",
            label_scout_streak: "侦察未扩散计数",
            label_top_blockers: "最主要阻碍因子前三项",
            label_state_change: "今天是否有状态变化",
            label_risk_change: "今天是否有风险叠加变化",
            label_trend_change: "今天是否有主线状态变化",
            label_mode_change: "今天是否有 NO TRADE 分层变化",
            label_scout_reset: "今天是否触发侦察重置",
            label_breakout_new: "新增突破",
            label_breakout_continued: "延续突破",
            label_breakout_removed: "消失突破",
            label_evidence_collection: "今日证据采集状态",
            label_evidence_stock: "历史证据存量",
            label_evidence_excluded: "已排除非生产来源证据",
            note_evidence_excluded: "历史确信度快照可能包含该来源，请以重新运行后的记录为准",
            label_no_trade_streak: "当前 NO TRADE 连续段长度",
            label_mainline_missing_streak: "当前主线缺失连续段长度",
            label_recent_shape: "最近一段 NO TRADE 形态",
            methodology_note: "口径: 连续段按日志连续计算（周末自动衔接）",
            none: "无",
            yes: "有",
            no: "无",
            segment_reset: "反复 reset",
            segment_continuous: "连续段",
            status_no_trade: "NO TRADE",
            status_ready: "READY",
            mode_cold: "初级（无信号）",
            mode_scout: "侦察态（有信号未验证）",
            mode_ready: "READY（可执行）",
            day_unit: "天",
            template_trend_recognition: "- 趋势识别质量: {state}; 扩散评分 {score}; 确信度 {conviction}; 滞后状态 {lag_state}",
        },
        Language::EnUs => AuditDailyText {
            title: "Audit Daily",
            section_gate: "Gate Summary",
            section_transition: "Transition Summary",
            section_breakout: "Breakout Summary",
            section_streaks: "Streak Metrics",
            section_substantive: "Substantive Evidence",
            section_one_liner: "Audit One-liner",
            market_interpretation_snapshot: "Market Interpretation Snapshot",
            market_interpretation_boundary: "Boundary: this is an audit-side explanation snapshot only; it does not connect to Gate, Execution, Trader, Action Matrix, or Position Sizing.",
            label_status: "Status",
            label_duration: "Duration",
            label_no_trade_mode: "NO TRADE tier",
            label_scout_streak: "Scout non-expansion counter",
            label_top_blockers: "Top 3 blockers",
            label_state_change: "State changed today",
            label_risk_change: "Risk overlay changed today",
            label_trend_change: "Mainline status changed today",
            label_mode_change: "NO TRADE tier changed today",
            label_scout_reset: "Scout reset triggered today",
            label_breakout_new: "New breakout",
            label_breakout_continued: "Continued breakout",
            label_breakout_removed: "Removed breakout",
            label_evidence_collection: "Today's evidence collection status",
            label_evidence_stock: "Historical evidence stock",
            label_evidence_excluded: "Excluded non-production evidence",
            note_evidence_excluded:
                "Stored historical conviction snapshots may contain this source; rely on newly generated records",
            label_no_trade_streak: "Current NO TRADE streak",
            label_mainline_missing_streak: "Current missing-mainline streak",
            label_recent_shape: "Recent NO TRADE segment type",
            methodology_note:
                "Methodology: streaks are calculated by log continuity (weekends auto-bridged)",
            none: "None",
            yes: "Yes",
            no: "No",
            segment_reset: "Repeated resets",
            segment_continuous: "Continuous segment",
            status_no_trade: "NO TRADE",
            status_ready: "READY",
            mode_cold: "Cold (no signal)",
            mode_scout: "Scout (signal unverified)",
            mode_ready: "READY (executable)",
            day_unit: "days",
            template_trend_recognition: "- Trend Recognition Quality: {state}; Diffusion Score {score}; Conviction Score {conviction}; Lag State {lag_state}",
        },
        Language::JaJp => AuditDailyText {
            title: "日次監査",
            section_gate: "ゲートサマリー",
            section_transition: "状態遷移サマリー",
            section_breakout: "ブレイクアウトサマリー",
            section_streaks: "連続区間統計",
            section_substantive: "実体的な証拠サマリー",
            section_one_liner: "監査ワンライン要約",
            market_interpretation_snapshot: "市場解釈スナップショット",
            market_interpretation_boundary: "境界: これは監査側の説明スナップショットであり、Gate / Execution / Trader / Action Matrix / Position Sizing には接続しません。",
            label_status: "状態",
            label_duration: "継続日数",
            label_no_trade_mode: "NO TRADE レイヤー",
            label_scout_streak: "偵察未拡散カウント",
            label_top_blockers: "主要阻害要因 Top 3",
            label_state_change: "本日の状態変化",
            label_risk_change: "本日のリスクオーバーレイ変化",
            label_trend_change: "本日の主線状態変化",
            label_mode_change: "本日の NO TRADE レイヤー変化",
            label_scout_reset: "本日の偵察リセット発生",
            label_breakout_new: "新規ブレイクアウト",
            label_breakout_continued: "継続ブレイクアウト",
            label_breakout_removed: "消失ブレイクアウト",
            label_evidence_collection: "本日の証拠収集状態",
            label_evidence_stock: "履歴証拠ストック",
            label_evidence_excluded: "除外した非本番証拠",
            note_evidence_excluded:
                "履歴の確信度スナップショットには当該ソースが含まれる可能性があるため、再実行後の記録を基準とする",
            label_no_trade_streak: "現在の NO TRADE 連続日数",
            label_mainline_missing_streak: "現在の主線欠如連続日数",
            label_recent_shape: "直近 NO TRADE 区間の形態",
            methodology_note: "口径: 連続区間はログ連続で計算（週末は自動連結）",
            none: "なし",
            yes: "あり",
            no: "なし",
            segment_reset: "反復 reset",
            segment_continuous: "連続区間",
            status_no_trade: "NO TRADE",
            status_ready: "READY",
            mode_cold: "初級（シグナルなし）",
            mode_scout: "偵察（シグナル未検証）",
            mode_ready: "READY（実行可）",
            day_unit: "日",
            template_trend_recognition: "- トレンド認識品質: {state}; 拡散スコア {score}; 確信度 {conviction}; 遅行状態 {lag_state}",
        },
    }
}

pub(crate) fn load_transition_audit_days(
    path: &std::path::Path,
    language: Language,
) -> Result<Vec<TransitionAuditDay>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("{}: {}", audit_error_read_file(language), path.display()))?;

    let mut raw_entries = Vec::<TransitionAuditEntry>::new();
    for (idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(entry) = parse_transition_audit_entry(line, language)
            .with_context(|| format!("{} {}", audit_error_parse_line(language), idx + 1))?
        {
            raw_entries.push(entry);
        }
    }

    raw_entries.sort_by_key(|a| a.timestamp);
    Ok(group_audit_days(raw_entries))
}

pub(crate) fn build_daily_calibration_questions(
    attention_count: usize,
    thesis_count: usize,
    selected_entry: Option<&TransitionAuditDay>,
    language: Language,
) -> String {
    let gate_state = selected_entry
        .map(|entry| {
            if entry.latest().log.trend_cohesion_gate.to {
                "READY"
            } else {
                "NO TRADE"
            }
        })
        .unwrap_or("NO AUDIT");
    let evidence_state = selected_entry
        .and_then(|entry| entry.latest().log.trend_recognition.as_ref())
        .map(|tr| {
            if tr.conviction_score >= 3.0 {
                crate::features::research::interface::cognitive_reports::daily_calibration_evidence_strong(
                    language,
                )
            } else if tr.conviction_score > 0.0 {
                crate::features::research::interface::cognitive_reports::daily_calibration_evidence_observed(
                    language,
                )
            } else {
                crate::features::research::interface::cognitive_reports::daily_calibration_evidence_none(
                    language,
                )
            }
        })
        .unwrap_or(
            crate::features::research::interface::cognitive_reports::daily_calibration_evidence_none(
                language,
            ),
        );

    format!(
        "{}\n{} {}\n{} {}\n{} {}\n{} {}\n{}",
        crate::features::research::interface::cognitive_reports::daily_calibration_question_market(language),
        crate::features::research::interface::cognitive_reports::daily_calibration_question_gate(language),
        gate_state,
        crate::features::research::interface::cognitive_reports::daily_calibration_question_evidence(language),
        evidence_state,
        crate::features::research::interface::cognitive_reports::daily_calibration_question_attention(language),
        attention_count,
        crate::features::research::interface::cognitive_reports::daily_calibration_question_thesis(language),
        thesis_count,
        crate::features::research::interface::cognitive_reports::daily_calibration_question_boundary(language),
    )
}

#[cfg(test)]
pub(crate) fn build_audit_daily_report(
    days: &[TransitionAuditDay],
    target_idx: usize,
    window_days: usize,
    language: Language,
) -> String {
    build_audit_daily_report_with_evidence_status(days, target_idx, window_days, language, None)
}

fn opportunity_mode_label(
    mode: crate::features::radar::domain::transition_log::OpportunityMode,
    language: Language,
) -> &'static str {
    let text = audit_text(language);
    match mode {
        crate::features::radar::domain::transition_log::OpportunityMode::NoTradeCold => {
            text.mode_cold
        }
        crate::features::radar::domain::transition_log::OpportunityMode::NoTradeScout => {
            text.mode_scout
        }
        crate::features::radar::domain::transition_log::OpportunityMode::Ready => text.mode_ready,
    }
}

fn yes_no(flag: bool, language: Language) -> &'static str {
    let text = audit_text(language);
    if flag {
        text.yes
    } else {
        text.no
    }
}

fn format_delivery_status(
    status: &crate::features::shared::application::run_status::DeliveryStatus,
    language: Language,
) -> String {
    match status {
        crate::features::shared::application::run_status::DeliveryStatus::Succeeded => {
            match language {
                Language::ZhCn => "成功".to_string(),
                Language::EnUs => "succeeded".to_string(),
                Language::JaJp => "成功".to_string(),
            }
        }
        crate::features::shared::application::run_status::DeliveryStatus::Skipped => match language
        {
            Language::ZhCn => "跳过".to_string(),
            Language::EnUs => "skipped".to_string(),
            Language::JaJp => "スキップ".to_string(),
        },
        crate::features::shared::application::run_status::DeliveryStatus::Failed { reason } => {
            match language {
                Language::ZhCn => format!("失败 ({})", reason),
                Language::EnUs => format!("failed ({})", reason),
                Language::JaJp => format!("失敗 ({})", reason),
            }
        }
    }
}

fn format_symbols(symbols: &[String], language: Language) -> String {
    if symbols.is_empty() {
        audit_text(language).none.to_string()
    } else {
        symbols.join(", ")
    }
}

fn format_evidence_description(description: &str, language: Language) -> String {
    if description == "Manual ingestion via CLI" {
        return match language {
            Language::ZhCn => "通过 CLI 手动录入".to_string(),
            Language::EnUs => "Manual ingestion via CLI".to_string(),
            Language::JaJp => "CLI から手動入力".to_string(),
        };
    }
    match language {
        Language::ZhCn => "原始证据说明未提供中文版本".to_string(),
        Language::EnUs => description.to_string(),
        Language::JaJp => "元の証拠説明は日本語で未提供".to_string(),
    }
}

fn blocker_label(raw: &str, language: Language) -> String {
    match language {
        Language::ZhCn => match raw {
            "StabilityThreshold" => "稳定性不足".to_string(),
            "ContinuityThreshold" => "连续性不足".to_string(),
            "DirectionalCohesion" => "无主线".to_string(),
            "HighCandidateDispersion" => "候选过散".to_string(),
            "UnstableRotation" => "轮动不稳".to_string(),
            "WeakLeadership" => "领涨不足".to_string(),
            _ => raw.to_string(),
        },
        Language::EnUs => match raw {
            "StabilityThreshold" => "Low stability".to_string(),
            "ContinuityThreshold" => "Low continuity".to_string(),
            "DirectionalCohesion" => "No mainline".to_string(),
            "HighCandidateDispersion" => "Candidates too dispersed".to_string(),
            "UnstableRotation" => "Unstable rotation".to_string(),
            "WeakLeadership" => "Weak leadership".to_string(),
            _ => raw.to_string(),
        },
        Language::JaJp => match raw {
            "StabilityThreshold" => "安定性不足".to_string(),
            "ContinuityThreshold" => "連続性不足".to_string(),
            "DirectionalCohesion" => "主線未形成".to_string(),
            "HighCandidateDispersion" => "候補が分散しすぎ".to_string(),
            "UnstableRotation" => "ローテーション不安定".to_string(),
            "WeakLeadership" => "リーダーシップ不足".to_string(),
            _ => raw.to_string(),
        },
    }
}

fn summarize_blockers(window: &[&TransitionAuditEntry]) -> Vec<(String, usize)> {
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for entry in window {
        if entry.log.trend_cohesion_gate.to {
            continue;
        }
        let mut day_set = std::collections::HashSet::<String>::new();
        for item in &entry.log.trend_cohesion_gate.added {
            day_set.insert(item.clone());
        }
        for item in &entry.log.trend_cohesion_gate.persisting {
            day_set.insert(item.clone());
        }
        for key in day_set {
            *counts.entry(key).or_insert(0) += 1;
        }
    }

    let mut sorted = counts.into_iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted
}

pub(crate) fn group_audit_days(entries: Vec<TransitionAuditEntry>) -> Vec<TransitionAuditDay> {
    let mut grouped = BTreeMap::<NaiveDate, Vec<TransitionAuditEntry>>::new();
    for entry in entries {
        grouped.entry(entry.date).or_default().push(entry);
    }

    let mut days = grouped
        .into_iter()
        .map(|(date, mut events)| {
            events.sort_by_key(|a| a.timestamp);
            TransitionAuditDay { date, events }
        })
        .collect::<Vec<_>>();
    days.sort_by_key(|a| a.date);
    days
}

struct BreakoutDailySummary {
    new_symbols: Vec<String>,
    continued_symbols: Vec<String>,
    removed_symbols: Vec<String>,
}

fn summarize_breakout_changes(
    changes: &[crate::features::radar::domain::transition_log::BreakoutTransition],
) -> BreakoutDailySummary {
    let mut new_symbols = Vec::new();
    let mut continued_symbols = Vec::new();
    let mut removed_symbols = Vec::new();
    for item in changes {
        let from = item.from_status;
        let to = item.to_status;
        if from == crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
            && to != crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
        {
            new_symbols.push(item.symbol.clone());
        } else if from
            != crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
            && to == crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
        {
            removed_symbols.push(item.symbol.clone());
        } else if from
            != crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
            && to != crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout
        {
            continued_symbols.push(item.symbol.clone());
        }
    }
    new_symbols.sort();
    new_symbols.dedup();
    continued_symbols.sort();
    continued_symbols.dedup();
    removed_symbols.sort();
    removed_symbols.dedup();
    BreakoutDailySummary {
        new_symbols,
        continued_symbols,
        removed_symbols,
    }
}

fn summarize_breakout_changes_from_events(day: &TransitionAuditDay) -> BreakoutDailySummary {
    let mut merged = BreakoutDailySummary {
        new_symbols: Vec::new(),
        continued_symbols: Vec::new(),
        removed_symbols: Vec::new(),
    };
    for event in &day.events {
        let once = summarize_breakout_changes(&event.log.breakout_changes);
        merged.new_symbols.extend(once.new_symbols);
        merged.continued_symbols.extend(once.continued_symbols);
        merged.removed_symbols.extend(once.removed_symbols);
    }
    merged.new_symbols.sort();
    merged.new_symbols.dedup();
    merged.continued_symbols.sort();
    merged.continued_symbols.dedup();
    merged.removed_symbols.sort();
    merged.removed_symbols.dedup();
    merged
}

fn detect_no_trade_resets(window: &[TransitionAuditDay]) -> bool {
    window
        .iter()
        .flat_map(|day| day.events.iter())
        .any(|entry| entry.log.trend_cohesion_gate.from != entry.log.trend_cohesion_gate.to)
}

fn trend_status_label(
    status: crate::features::radar::domain::trend_cohesion::TrendCohesionStatus,
    language: Language,
) -> &'static str {
    match language {
        Language::ZhCn => match status {
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed => {
                "未形成"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Forming => {
                "形成中"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Formed => "已形成",
        },
        Language::EnUs => match status {
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed => {
                "Not formed"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Forming => {
                "Forming"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Formed => "Formed",
        },
        Language::JaJp => match status {
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Dispersed => {
                "未形成"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Forming => {
                "形成中"
            }
            crate::features::radar::domain::trend_cohesion::TrendCohesionStatus::Formed => {
                "形成済み"
            }
        },
    }
}

fn summarize_breakout_sentence(summary: &BreakoutDailySummary, language: Language) -> String {
    let mut items = Vec::new();
    for symbol in &summary.new_symbols {
        items.push(format_breakout_item(symbol, language, "new"));
    }
    for symbol in &summary.continued_symbols {
        items.push(format_breakout_item(symbol, language, "continued"));
    }
    for symbol in &summary.removed_symbols {
        items.push(format_breakout_item(symbol, language, "removed"));
    }
    if items.is_empty() {
        audit_text(language).none.to_string()
    } else {
        items.join(", ")
    }
}

fn format_breakout_item(symbol: &str, language: Language, kind: &str) -> String {
    match language {
        Language::ZhCn => match kind {
            "new" => format!("{}（新增）", symbol),
            "continued" => format!("{}（延续）", symbol),
            _ => format!("{}（消失）", symbol),
        },
        Language::EnUs => match kind {
            "new" => format!("{} (new)", symbol),
            "continued" => format!("{} (continued)", symbol),
            _ => format!("{} (removed)", symbol),
        },
        Language::JaJp => match kind {
            "new" => format!("{}（新規）", symbol),
            "continued" => format!("{}（継続）", symbol),
            _ => format!("{}（消失）", symbol),
        },
    }
}

pub(crate) fn consecutive_streak<F>(
    days: &[TransitionAuditDay],
    target_idx: usize,
    predicate: F,
) -> usize
where
    F: Fn(&crate::features::radar::domain::transition_log::StateTransitionLog) -> bool,
{
    if !predicate(&days[target_idx].latest().log) {
        return 0;
    }

    let mut streak = 1usize;
    let mut idx = target_idx;
    while idx > 0 {
        let prev_idx = idx - 1;
        if !is_consecutive_trading_day(days[prev_idx].date, days[idx].date) {
            break;
        }
        if !predicate(&days[prev_idx].latest().log) {
            break;
        }
        streak += 1;
        idx = prev_idx;
    }
    streak
}

fn is_consecutive_trading_day(prev: NaiveDate, curr: NaiveDate) -> bool {
    if curr <= prev {
        return false;
    }

    let mut day = prev.succ_opt().unwrap_or(prev);
    while day < curr {
        match day.weekday() {
            Weekday::Sat | Weekday::Sun => {}
            _ => return false,
        }
        day = day.succ_opt().unwrap_or(day);
    }
    true
}

struct AuditSentenceContext<'a> {
    gate_status: &'a str,
    gate_streak: usize,
    blocker_text: &'a str,
    breakout_text: &'a str,
    mainline_text: &'a str,
    no_trade_mode: &'a str,
    complete_formal_baseline: bool,
}

fn build_audit_sentence(language: Language, context: AuditSentenceContext<'_>) -> String {
    let AuditSentenceContext {
        gate_status,
        gate_streak,
        blocker_text,
        breakout_text,
        mainline_text,
        no_trade_mode,
        complete_formal_baseline,
    } = context;
    if !complete_formal_baseline {
        return match language {
            Language::ZhCn => format!(
                "当前状态：{}；主因：{}；今日突破：{}；主线状态：{}。",
                gate_status, blocker_text, breakout_text, mainline_text
            ),
            Language::EnUs => format!(
                "Current state: {}; primary blockers: {}; today's breakout: {}; mainline status: {}.",
                gate_status, blocker_text, breakout_text, mainline_text
            ),
            Language::JaJp => format!(
                "現在の状態：{}；主因：{}；本日のブレイクアウト：{}；主線状態：{}。",
                gate_status, blocker_text, breakout_text, mainline_text
            ),
        };
    }
    match language {
        Language::ZhCn => format!(
            "{} 连续第 {} 天；主因：{}；NO TRADE 分层：{}；今日突破：{}；主线状态：{}。",
            gate_status, gate_streak, blocker_text, no_trade_mode, breakout_text, mainline_text
        ),
        Language::EnUs => format!(
            "{} day {} in a row; primary blockers: {}; NO TRADE tier: {}; today's breakout: {}; mainline status: {}.",
            gate_status, gate_streak, blocker_text, no_trade_mode, breakout_text, mainline_text
        ),
        Language::JaJp => format!(
            "{} 連続 {} 日目；主因：{}；NO TRADE レイヤー：{}；本日のブレイクアウト：{}；主線状態：{}。",
            gate_status, gate_streak, blocker_text, no_trade_mode, breakout_text, mainline_text
        ),
    }
}

pub(crate) fn audit_empty_log_message(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未找到可用的 state_transitions.jsonl 记录。",
        Language::EnUs => "No usable records found in state_transitions.jsonl.",
        Language::JaJp => "state_transitions.jsonl に有効な記録がありません。",
    }
}

pub(crate) fn audit_error_parse_date(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无法解析 --date",
        Language::EnUs => "Unable to parse --date",
        Language::JaJp => "--date を解析できません",
    }
}

pub(crate) fn audit_error_read_file(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无法读取文件",
        Language::EnUs => "Unable to read file",
        Language::JaJp => "ファイルを読み込めません",
    }
}

pub(crate) fn audit_error_parse_line(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "解析 state_transitions.jsonl 第",
        Language::EnUs => "Failed to parse state_transitions.jsonl line",
        Language::JaJp => "state_transitions.jsonl の行解析に失敗:",
    }
}

pub(crate) fn audit_error_invalid_timestamp(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无效 timestamp",
        Language::EnUs => "Invalid timestamp",
        Language::JaJp => "無効な timestamp",
    }
}

pub(crate) fn audit_error_invalid_date(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无效 date",
        Language::EnUs => "Invalid date",
        Language::JaJp => "無効な date",
    }
}

pub(crate) fn audit_error_target_date_not_found(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未找到目标日期的审计记录:",
        Language::EnUs => "No audit record found for target date:",
        Language::JaJp => "対象日の監査記録が見つかりません:",
    }
}

pub(crate) fn audit_daily_usage(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "用法:\n  cargo run -- audit_daily [--date YYYY-MM-DD] [--days N]\n  cargo run -- transition_audit_summary [--date YYYY-MM-DD] [--days N]"
        }
        Language::EnUs => {
            "Usage:\n  cargo run -- audit_daily [--date YYYY-MM-DD] [--days N]\n  cargo run -- transition_audit_summary [--date YYYY-MM-DD] [--days N]"
        }
        Language::JaJp => {
            "使い方:\n  cargo run -- audit_daily [--date YYYY-MM-DD] [--days N]\n  cargo run -- transition_audit_summary [--date YYYY-MM-DD] [--days N]"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::shared::application::run_status::DeliveryStatus;
    use std::fs;
    use tempfile::{tempdir, NamedTempFile};

    fn sample_transition_json(
        timestamp: &str,
        gate_to: bool,
        status_to: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "timestamp": timestamp,
            "date": "2026-04-21",
            "transition": {
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": gate_to,
                    "unmet_conditions_changed": false,
                    "added": [],
                    "removed": [],
                    "persisting": []
                },
                "trend_cohesion_status": {"from":"Dispersed","to": status_to,"changed": true},
                "trend_cohesion_topology": {"from":"NoLeader","to":"SingleLeader","changed": true},
                "breakout_changes": [],
                "opportunity_mode": {"from":"NoTradeCold","to":"Ready","changed": true},
                "scout_days_without_expansion": 0,
                "scout_abort_days": 0,
                "scout_reset_triggered": false,
                "breakout_active_count": 0,
                "trend_recognition": {
                    "state":"EarlyLeader",
                    "diffusion_score": 0.45,
                    "lag_state": true,
                    "single_asset_decay_day": 1,
                    "single_asset_decay_max": 2,
                    "conviction_score": 0.45,
                    "substantive": {
                        "capex_payoff_signal": false,
                        "earnings_validation": false,
                        "order_visibility": false,
                        "event_days_since": 0,
                        "records": []
                    }
                }
            }
        })
    }

    #[tokio::test]
    async fn daily_calibration_propagates_corrupt_history_state() {
        let directory = tempdir().unwrap();
        let transition = sample_transition_json("2026-04-22T15:00:00+00:00", false, "Dispersed");
        fs::write(
            directory.path().join("state_transitions.jsonl"),
            format!("{}\n", serde_json::to_string(&transition).unwrap()),
        )
        .unwrap();
        fs::write(
            directory.path().join("observation_history_state.json"),
            "{not valid json",
        )
        .unwrap();

        let error =
            build_daily_calibration_context(directory.path(), None, 7, 3, 3, Language::ZhCn)
                .await
                .expect_err("损坏的历史状态必须传播给审计调用方");

        assert!(error.to_string().contains("observation_history_state"));
    }

    fn sample_audit_days() -> Vec<TransitionAuditDay> {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
            timestamp: DateTime::parse_from_rfc3339("2026-04-21T09:00:00+00:00").unwrap(),
            log: serde_json::from_value(
                sample_transition_json("2026-04-21T09:00:00+00:00", false, "Dispersed")
                    .get("transition")
                    .cloned()
                    .unwrap(),
            )
            .unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: DateTime::parse_from_rfc3339("2026-04-22T15:00:00+00:00").unwrap(),
            log: serde_json::from_value(
                sample_transition_json("2026-04-22T15:00:00+00:00", true, "Forming")
                    .get("transition")
                    .cloned()
                    .unwrap(),
            )
            .unwrap(),
        };
        vec![
            TransitionAuditDay {
                date: entry_day1.date,
                events: vec![entry_day1],
            },
            TransitionAuditDay {
                date: entry_day2.date,
                events: vec![entry_day2],
            },
        ]
    }

    #[test]
    fn market_interpretation_audit_keeps_trend_continuation_normal() {
        let mut value = sample_transition_json("2026-04-22T15:00:00+00:00", true, "Forming");
        value["transition"]["trend_recognition"]["state"] = serde_json::json!("Broadening");
        let entry = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: DateTime::parse_from_rfc3339("2026-04-22T15:00:00+00:00").unwrap(),
            log: serde_json::from_value(value["transition"].clone()).unwrap(),
        };
        let day = TransitionAuditDay {
            date: entry.date,
            events: vec![entry],
        };

        let snapshot = build_market_interpretation_audit_snapshot(
            &day,
            Language::EnUs,
            &audit_text(Language::EnUs),
            None,
        );

        assert!(snapshot.contains("- dayType: normal"));
        assert!(snapshot.contains("- reason: trend_continuation"));
        assert!(snapshot.contains("- exceptionalFactors: []"));
        assert!(!snapshot.contains("exceptionalFactors: [trend_continuation]"));
    }

    #[test]
    fn market_interpretation_audit_does_not_promote_fading_breakout_to_primary() {
        let mut value = sample_transition_json("2026-04-22T15:00:00+00:00", true, "Forming");
        value["transition"]["breakout_changes"] = serde_json::json!([
            {
                "symbol": "GOOG",
                "from_status": "ConfirmedBreakout",
                "to_status": "NoBreakout",
                "status_changed": true,
                "risk_changed": false
            }
        ]);
        let entry = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: DateTime::parse_from_rfc3339("2026-04-22T15:00:00+00:00").unwrap(),
            log: serde_json::from_value(value["transition"].clone()).unwrap(),
        };
        let day = TransitionAuditDay {
            date: entry.date,
            events: vec![entry],
        };

        let snapshot = build_market_interpretation_audit_snapshot(
            &day,
            Language::EnUs,
            &audit_text(Language::EnUs),
            None,
        );

        assert!(snapshot.contains("- Leadership Classification:"));
        assert!(snapshot.contains("- Leadership Metrics:"));
        assert!(snapshot.contains("  - Breadth label: "));
        assert!(snapshot.contains("  - Breadth score: "));
        assert!(snapshot.contains("  - Concentration score: "));
        assert!(snapshot.contains("  - Rotation score: "));
        assert!(snapshot.contains("- Rotation Observation:"));
        assert!(!snapshot.contains("  - primary: "));
        assert!(!snapshot.contains("  - supporting: "));
        assert!(!snapshot.contains("  - weakening: "));
    }

    #[test]
    fn audit_daily_does_not_fallback_to_transition_log_without_formal_baseline() {
        let current_entry = sample_audit_days()[1].latest().clone();
        let snapshot = build_market_interpretation_audit_snapshot(
            &TransitionAuditDay {
                date: current_entry.date,
                events: vec![current_entry],
            },
            Language::EnUs,
            &audit_text(Language::EnUs),
            None,
        );

        assert!(snapshot.contains("rotationType: BASELINE_UNAVAILABLE"));
        assert!(snapshot.contains("  - from: []"));
    }

    #[test]
    fn audit_sentence_reports_current_state_without_complete_baseline() {
        let sentence = build_audit_sentence(
            Language::ZhCn,
            AuditSentenceContext {
                gate_status: "NO TRADE",
                gate_streak: 1,
                blocker_text: "无主线",
                breakout_text: "U（新增）",
                mainline_text: "未形成",
                no_trade_mode: "Scout",
                complete_formal_baseline: false,
            },
        );

        assert!(sentence.contains("当前状态：NO TRADE"));
        assert!(!sentence.contains("连续第 1 天"));
    }

    #[test]
    fn load_transition_audit_days_groups_and_sorts_entries() {
        let tmp = NamedTempFile::new().unwrap();
        let content = [
            sample_transition_json("2026-04-21T09:00:00+00:00", false, "Dispersed"),
            sample_transition_json("2026-04-21T15:00:00+00:00", true, "Forming"),
        ]
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        fs::write(tmp.path(), content).unwrap();

        let days = load_transition_audit_days(tmp.path(), Language::EnUs).unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].events.len(), 2);
        assert!(days[0].latest().log.trend_cohesion_gate.to);
    }

    #[test]
    fn daily_calibration_questions_include_selected_gate_and_evidence_state() {
        let entry = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
            timestamp: DateTime::parse_from_rfc3339("2026-04-21T15:00:00+00:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "trend_cohesion_gate": {"from": false, "to": true, "unmet_conditions_changed": false, "added": [], "removed": [], "persisting": []},
                "market_state": {"from": "IGNITION", "to": "IGNITION", "changed": false},
                "risk_overlay": {"from": "NORMAL", "to": "NORMAL", "changed": false},
                "no_trade_persists": true,
                "trend_cohesion_status": {"from": "Dispersed", "to": "Forming", "changed": true},
                "trend_cohesion_topology": {"from": "NoLeader", "to": "SingleLeader", "changed": true},
                "breakout_changes": [],
                "opportunity_mode": {"from": "NoTradeCold", "to": "Ready", "changed": true},
                "scout_days_without_expansion": 0,
                "scout_abort_days": 0,
                "scout_reset_triggered": false,
                "breakout_active_count": 0,
                "trend_recognition": {
                    "state": "EarlyLeader",
                    "diffusion_score": 0.45,
                    "lag_state": true,
                    "single_asset_decay_day": 1,
                    "single_asset_decay_max": 2,
                    "conviction_score": 0.45,
                    "substantive": {
                        "capex_payoff_signal": false,
                        "earnings_validation": false,
                        "order_visibility": false,
                        "event_days_since": 0,
                        "records": []
                    }
                }
            }))
            .unwrap(),
        };
        let day = TransitionAuditDay {
            date: entry.date,
            events: vec![entry],
        };

        let questions = build_daily_calibration_questions(2, 4, Some(&day), Language::ZhCn);
        assert!(questions.contains("固定问题: 今天是市场理解变化，还是只是噪音变化？"));
        assert!(questions.contains("- 战术状态:"));
        assert!(questions.contains("已有结构证据"));
        assert!(questions.contains("- 需校准认知对象数: 2"));
        assert!(questions.contains("- 需复查观察命题数: 4"));
    }

    #[test]
    fn audit_daily_renders_market_interpretation_snapshot() {
        let days = sample_audit_days();
        let report = build_audit_daily_report_with_evidence_status(
            &days,
            1,
            14,
            Language::EnUs,
            Some(&DeliveryStatus::Succeeded),
        );

        assert!(report.contains("Market Interpretation Snapshot"));
        assert!(report.contains("decision_weight: 0%"));
        assert!(report.contains("dayType: normal"));
        assert!(report.contains("Leadership Classification:"));
        assert!(report.contains("Leadership Metrics:"));
        assert!(report.contains("  - Breadth label: "));
        assert!(report.contains("  - Breadth score: "));
        assert!(report.contains("  - Concentration score: "));
        assert!(report.contains("  - Rotation score: "));
        assert!(report.contains("Rotation Observation:"));
        assert!(report.contains("Observation Confidence:"));
        assert!(report.contains("observationOnly: true"));
        assert!(report.contains("Boundary: this is an audit-side explanation snapshot only"));
        assert!(!report.contains("  - primary: "));
        assert!(!report.contains("  - supporting: "));
        assert!(!report.contains("  - weakening: "));
        assert!(!report.contains("Leadership classification: very_narrow."));
        assert!(!report.contains("Rotation reads "));
    }

    #[test]
    fn audit_daily_does_not_repeat_primary_symbol_in_weakening() {
        let mut days = sample_audit_days();
        let day = days
            .last_mut()
            .expect("sample audit days should include a target day");
        let latest = day
            .events
            .last_mut()
            .expect("sample day has at least one event");
        latest.log.breakout_changes.push(
            crate::features::radar::domain::transition_log::BreakoutTransition {
                symbol: "GOOG".to_string(),
                from_status: crate::features::radar::domain::breakout_detection::BreakoutStatus::NoBreakout,
                to_status: crate::features::radar::domain::breakout_detection::BreakoutStatus::ConfirmedBreakout,
                status_changed: true,
                risk_changed: false,
            },
        );
        let report = build_audit_daily_report_with_evidence_status(
            &days,
            1,
            14,
            Language::EnUs,
            Some(&DeliveryStatus::Succeeded),
        );

        assert!(report.contains("  - Breadth label: "));
        assert!(report.contains("  - Breadth score: "));
        assert!(report.contains("  - Concentration score: "));
        assert!(report.contains("  - Rotation score: "));
        assert!(!report.contains("  - primary: [GOOG]"));
        assert!(!report.contains("  - weakening: [GOOG]"));
    }

    #[test]
    fn audit_daily_treats_trend_continuation_as_normal_day_reason() {
        let mut days = sample_audit_days();
        let mut day = days.pop().expect("sample day exists");
        let latest = day.events.last_mut().expect("sample day has event");
        latest.log.breakout_changes.clear();
        latest.log.market_state.changed = false;
        latest.log.risk_overlay.changed = false;

        let report = build_audit_daily_report_with_evidence_status(
            &[day],
            0,
            14,
            Language::EnUs,
            Some(&DeliveryStatus::Succeeded),
        );

        assert!(report.contains("dayType: normal"));
        assert!(report.contains("reason: trend_continuation"));
        assert!(report.contains("exceptionalFactors: []"));
    }
}

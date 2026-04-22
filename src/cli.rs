use anyhow::{anyhow, Context, Result};

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Weekday};
use futures::stream::{self, StreamExt};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use time::OffsetDateTime;

use crate::backtest;
use crate::config;
use crate::core::engine::Engine;
use crate::core::execution_gate::ExecutionGate;
use crate::core::ledger::Ledger;
use crate::core::persistence::PersistenceLayer;
use crate::core::transition_log::TransitionLogger;

use crate::core::i18n::Language;
use crate::core::notify;
use crate::core::presentation_assembler::PresentationAssembler;
use crate::core::report;
use crate::data::provider::MarketDataProvider;

use crate::adapters::futu::client::FutuClient;
use crate::adapters::futu::provider::FutuProvider;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProviderType {
    Yahoo,
    Futu,
}

pub async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let app_config = config::AppConfig::load("config.toml")?;
    let audit_language = app_config.output.language.unwrap_or(Language::ZhCn);

    let mut command = "radar";
    let mut audit_date_arg: Option<String> = None;
    let mut audit_days: usize = 14;
    let mut audit_arg_error: Option<String> = None;
    let mut provider_type = match app_config.provider.as_deref() {
        Some("futu") => ProviderType::Futu,
        _ => ProviderType::Yahoo,
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "backtest" => command = "backtest",
            "daemon" | "trade" => command = "daemon",
            "radar" => command = "radar",
            "review" => command = "review",
            "audit_daily" | "transition_audit_summary" => command = "audit_daily",
            "--provider" if i + 1 < args.len() => {
                let p = args[i + 1].to_lowercase();
                if p == "futu" {
                    provider_type = ProviderType::Futu;
                } else if p == "yahoo" {
                    provider_type = ProviderType::Yahoo;
                }
                i += 1;
            }
            "--date" => {
                if i + 1 >= args.len() || args[i + 1].starts_with("--") {
                    audit_arg_error = Some(audit_error_missing_date(audit_language).to_string());
                } else {
                    audit_date_arg = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--days" => {
                if i + 1 >= args.len() || args[i + 1].starts_with("--") {
                    audit_arg_error = Some(audit_error_missing_days(audit_language).to_string());
                } else {
                    match args[i + 1].parse::<usize>() {
                        Ok(days) if days > 0 => audit_days = days,
                        _ => {
                            audit_arg_error =
                                Some(audit_error_invalid_days(audit_language).to_string())
                        }
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if command == "audit_daily" {
        if let Some(err) = audit_arg_error {
            return Err(anyhow!("{}\n\n{}", err, audit_daily_usage(audit_language)));
        }
    }

    match command {
        "backtest" => {
            let mut from_date = "2024-01-01".to_string();
            let mut to_date = "2024-02-01".to_string();
            let mut iter = args.iter().skip(1);
            while let Some(arg) = iter.next() {
                if arg == "--from" {
                    if let Some(v) = iter.next() {
                        from_date = v.clone();
                    }
                } else if arg == "--to" {
                    if let Some(v) = iter.next() {
                        to_date = v.clone();
                    }
                }
            }
            backtest::run_backtest(&app_config, &from_date, &to_date).await?;
        }
        "daemon" => {
            let is_trading_enabled = app_config
                .trading
                .as_ref()
                .map(|t| t.enabled)
                .unwrap_or(false);
            let mode = if is_trading_enabled {
                crate::core::runtime_mode::ExecutionMode::Live
            } else {
                crate::core::runtime_mode::ExecutionMode::DryRun
            };
            let futu_addr = if let Some(futu_cfg) = &app_config.futu {
                format!("{}:{}", futu_cfg.opend_ip, futu_cfg.opend_port)
            } else {
                "127.0.0.1:11111".to_string()
            };
            let provider = get_provider(provider_type, &futu_addr).await;
            run_pipeline(app_config, provider_type, provider, mode).await?;
        }
        "review" => {
            run_review(&app_config).await?;
        }
        "audit_daily" => {
            run_audit_daily(
                &app_config,
                audit_date_arg.as_deref(),
                audit_days,
                audit_language,
            )?;
        }
        _ => {
            let futu_addr = if let Some(futu_cfg) = &app_config.futu {
                format!("{}:{}", futu_cfg.opend_ip, futu_cfg.opend_port)
            } else {
                "127.0.0.1:11111".to_string()
            };
            let provider = get_provider(provider_type, &futu_addr).await;
            run_pipeline(
                app_config,
                provider_type,
                provider,
                crate::core::runtime_mode::ExecutionMode::Disabled,
            )
            .await?;
        }
    }
    Ok(())
}

async fn get_provider(pt: ProviderType, addr: &str) -> Arc<dyn MarketDataProvider> {
    match pt {
        ProviderType::Futu => match FutuClient::connect(addr).await {
            Ok(client) => Arc::new(FutuProvider::new(Arc::new(client))),
            Err(_) => Arc::new(YahooProviderAdapter),
        },
        ProviderType::Yahoo => Arc::new(YahooProviderAdapter),
    }
}

struct YahooProviderAdapter;
#[async_trait::async_trait]
impl MarketDataProvider for YahooProviderAdapter {
    async fn fetch_history(
        &self,
        s: &str,
        start: Option<OffsetDateTime>,
        end: Option<OffsetDateTime>,
    ) -> Result<crate::data::yahoo_provider::TickerHistory<'static>> {
        crate::data::yahoo_provider::fetch_history(s, start, end).await
    }
}

fn telegram_delivery_precheck(
    config: Option<&crate::config::TelegramConfig>,
) -> Result<&crate::config::TelegramConfig, crate::core::run_status::DeliveryStatus> {
    match config {
        Some(cfg) if !cfg.enabled => Err(crate::core::run_status::DeliveryStatus::Skipped),
        Some(cfg) if cfg.bot_token.is_empty() || cfg.chat_id.is_empty() => {
            Err(crate::core::run_status::DeliveryStatus::Failed {
                reason: "Telegram is enabled but bot_token/chat_id is missing".to_string(),
            })
        }
        Some(cfg) => Ok(cfg),
        None => Err(crate::core::run_status::DeliveryStatus::Skipped),
    }
}

async fn run_pipeline(
    app_config: config::AppConfig,
    _provider_type: ProviderType,
    provider: Arc<dyn MarketDataProvider>,
    _mode: crate::core::runtime_mode::ExecutionMode,
) -> Result<()> {
    let parsed_rules = app_config.get_parsed_rules();
    let config_arc = Arc::new(app_config);
    let rules_arc = Arc::new(parsed_rules);
    let save_dir = std::path::PathBuf::from(&config_arc.output.save_to);
    if !save_dir.exists() {
        std::fs::create_dir_all(&save_dir).context("Failed to create output directory")?;
    }

    let persistence = PersistenceLayer::new(&save_dir);
    let transition_logger = TransitionLogger::new(&save_dir);
    let history = persistence.load_recent_packets(20).unwrap_or_default();
    let prev_packet = history.last();

    let mut ticker_histories = Vec::new();
    let mut failed_symbols = Vec::new();

    let fetches = stream::iter(config_arc.watchlist.iter().filter(|w| w.enable))
        .map(|entry| {
            let provider_ref = Arc::clone(&provider);
            async move {
                (
                    provider_ref.fetch_history(&entry.symbol, None, None).await,
                    entry,
                )
            }
        })
        .buffer_unordered(10);

    let results: Vec<_> = fetches.collect().await;
    for (res, entry) in results {
        match res {
            Ok(h) => ticker_histories.push((h, entry)),
            Err(_) => failed_symbols.push(entry.symbol.clone()),
        }
    }

    let mut outcome = crate::core::run_status::RunOutcome {
        date: chrono::Local::now().date_naive().to_string(),
        timestamp: chrono::Local::now().to_rfc3339(),
        ..Default::default()
    };

    let ledger = Arc::new(Ledger::new(save_dir.clone()));
    let (realized_pl, positions) = ledger.get_portfolio_stats();

    if !ticker_histories.is_empty() || !failed_symbols.is_empty() {
        let should_persist_history =
            should_persist_decision_history(ticker_histories.len(), failed_symbols.len());
        let packet = if !ticker_histories.is_empty() {
            match Engine::run_daily_pipeline(&ticker_histories, &rules_arc, &history, &positions) {
                Ok(p) => {
                    outcome.decisioning = crate::core::run_status::DeliveryStatus::Succeeded;
                    p
                }
                Err(e) => {
                    outcome.decisioning = crate::core::run_status::DeliveryStatus::Failed {
                        reason: e.to_string(),
                    };
                    persistence.save_run_status(&outcome)?;
                    return Err(e);
                }
            }
        } else {
            // 100% Fetch Failure Case: Create a diagnostic packet for presentation/reporting only.
            // This packet must not be treated as a formal market decision.
            outcome.decisioning = crate::core::run_status::DeliveryStatus::Failed {
                reason: format!(
                    "100% data acquisition failure: {} symbols failed",
                    failed_symbols.len()
                ),
            };
            crate::core::decision::DecisionPacket {
                date: chrono::Local::now().date_naive(),
                ..Default::default()
            }
        };

        // 1. Semantic Assembly (Facts -> Presentation Model)
        let lang = config_arc
            .output
            .language
            .unwrap_or(crate::core::i18n::Language::ZhCn);
        let pres_packet =
            PresentationAssembler::assemble(&packet, &rules_arc, &positions, failed_symbols, lang);

        let default_trading_config = crate::config::TradingConfig {
            enabled: false,
            global_budget: 0.0,
            max_daily_budget: None,
        };
        let trading_config = config_arc
            .trading
            .as_ref()
            .unwrap_or(&default_trading_config);
        let daily_traded = ledger.get_daily_traded_amount();
        let current_exposure: f64 = positions
            .values()
            .map(|(qty, avg_price)| qty * avg_price)
            .sum();
        let buying_power = (trading_config.global_budget - current_exposure).max(0.0);
        let execution_result = ExecutionGate::gate_packet(
            &packet,
            trading_config,
            daily_traded,
            buying_power,
            current_exposure,
        );
        persistence.save_execution_gate_result(&packet, &execution_result)?;

        let date_str = packet.date.to_string();
        let portfolio_snapshot = serde_json::json!({
            "date": date_str,
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
        });
        persistence.save_portfolio_snapshot(&portfolio_snapshot, &date_str)?;

        let account_snapshot = serde_json::json!({
            "date": date_str,
            "global_budget": trading_config.global_budget,
            "max_daily_budget": trading_config.max_daily_budget,
            "daily_traded": daily_traded,
            "buying_power_estimate": buying_power,
            "current_exposure": current_exposure,
            "realized_pl": realized_pl,
            "failed_fetch_count": pres_packet.data_alert.as_ref().map(|alert| alert.symbols.len()).unwrap_or(0),
        });
        persistence.save_account_snapshot(&account_snapshot, &date_str)?;

        let data_quality_log = serde_json::json!({
            "timestamp": chrono::Local::now().to_rfc3339(),
            "date": date_str,
            "successful_fetches": ticker_histories.len(),
            "failed_fetches": pres_packet.data_alert.as_ref().map(|alert| alert.symbols.len()).unwrap_or(0),
            "failed_symbols": pres_packet.data_alert.as_ref().map(|alert| alert.symbols.clone()).unwrap_or_default(),
            "status": if ticker_histories.is_empty() {
                "CRITICAL"
            } else if pres_packet.data_alert.is_some() {
                "WARNING"
            } else {
                "OK"
            }
        });
        persistence.save_data_quality_log(&data_quality_log)?;

        let mut sm_summary = crate::core::run_status::StateMachineSummary {
            from_state: format!(
                "{:?}",
                prev_packet
                    .map(|p| p.market_regime.market_state)
                    .unwrap_or(crate::core::market_regime::MarketState::IGNITION)
            ),
            to_state: if should_persist_history {
                format!("{:?}", packet.market_regime.market_state)
            } else {
                "DATA_UNAVAILABLE".to_string()
            },
            ..Default::default()
        };
        if let Some(audit) = &packet.market_regime.transition_audit {
            sm_summary.reset_confirmed = audit.reset_gate_passed;
            sm_summary.reset_blocked = audit.is_reset_blocked;
            sm_summary.soft_reset_applied = audit.soft_reset_applied;
            sm_summary.duration_locked = audit.duration_locked;
            sm_summary.defensive_override = audit.defensive_override;
            sm_summary.core_breakdown = audit.core_breakdown;
        }
        outcome.state_machine = Some(sm_summary);
        outcome.date = packet.date.to_string();

        if should_persist_history {
            persistence.save_packet(&packet)?;
            persistence.save_daily_packet(&packet)?;
            if let Some(log) = &packet.transition_log {
                let _ = transition_logger.log_transition(log);
            }
        }

        // 2. Rendering (Presentation Model -> Final Outputs)
        let prices: std::collections::HashMap<String, f64> = packet
            .assets
            .iter()
            .map(|a| (a.symbol.clone(), a.price))
            .collect();
        let report_result = report::generate_refined_report(
            &config_arc,
            &pres_packet,
            realized_pl,
            &positions,
            &prices,
        )?;

        persistence
            .save_markdown_report(&report_result.archival_markdown, &pres_packet.date_str)?;
        persist_weekly_state_outputs(
            &save_dir,
            &history,
            &packet,
            should_persist_history,
            &pres_packet,
        )?;

        outcome.notification = match telegram_delivery_precheck(config_arc.telegram.as_ref()) {
            Ok(tg_cfg) => {
                match notify::send_telegram_message(tg_cfg, &report_result.telegram_html_body).await
                {
                    Ok(_) => crate::core::run_status::DeliveryStatus::Succeeded,
                    Err(err) => {
                        eprintln!("⚠️ Telegram notification failed: {}", err);
                        crate::core::run_status::DeliveryStatus::Failed {
                            reason: err.to_string(),
                        }
                    }
                }
            }
            Err(crate::core::run_status::DeliveryStatus::Skipped) => {
                if config_arc.telegram.is_some() {
                    eprintln!("ℹ️ Telegram notification skipped: config.telegram.enabled = false");
                } else {
                    eprintln!("ℹ️ Telegram notification skipped: telegram config is missing");
                }
                crate::core::run_status::DeliveryStatus::Skipped
            }
            Err(crate::core::run_status::DeliveryStatus::Failed { reason }) => {
                eprintln!("⚠️ Telegram notification failed precheck: {}", reason);
                crate::core::run_status::DeliveryStatus::Failed { reason }
            }
            Err(other) => other,
        };
        persistence.save_run_status(&outcome)?;
    }
    Ok(())
}

fn should_persist_decision_history(successful_fetches: usize, failed_fetches: usize) -> bool {
    successful_fetches > 0 || failed_fetches == 0
}

fn persist_weekly_state_outputs(
    save_dir: &std::path::Path,
    history: &[crate::core::decision::DecisionPacket],
    current_packet: &crate::core::decision::DecisionPacket,
    include_current_packet: bool,
    pres_packet: &crate::core::presentation::PresentationPacket,
) -> Result<()> {
    let mut recent_packets: Vec<&crate::core::decision::DecisionPacket> =
        history.iter().rev().take(7).collect();
    recent_packets.reverse();
    if include_current_packet {
        recent_packets.push(current_packet);
    }
    if recent_packets.len() > 7 {
        recent_packets = recent_packets[recent_packets.len() - 7..].to_vec();
    }

    let mut market_state_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut risk_overlay_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut total_confidence = 0.0;
    let mut total_stability = 0.0;
    let mut trend_cohesion_ready_days = 0usize;

    for packet in &recent_packets {
        *market_state_counts
            .entry(format!("{:?}", packet.market_regime.market_state))
            .or_insert(0) += 1;
        *risk_overlay_counts
            .entry(format!("{:?}", packet.market_regime.risk_overlay))
            .or_insert(0) += 1;
        total_confidence += packet.market_features.system_confidence;
        total_stability += packet.market_features.stability_score;
        if packet.trend_cohesion.gate_passed {
            trend_cohesion_ready_days += 1;
        }
    }

    let day_count = recent_packets.len();
    let avg_confidence = if day_count > 0 {
        total_confidence / day_count as f64
    } else {
        0.0
    };
    let avg_stability = if day_count > 0 {
        total_stability / day_count as f64
    } else {
        0.0
    };

    let metrics = json!({
        "generated_at": chrono::Local::now().to_rfc3339(),
        "as_of_date": pres_packet.date_str,
        "days_analyzed": day_count,
        "include_current_packet": include_current_packet,
        "data_status": if include_current_packet { "OK" } else { "DATA_UNAVAILABLE" },
        "latest_market_state": format!("{:?}", current_packet.market_regime.market_state),
        "latest_risk_overlay": format!("{:?}", current_packet.market_regime.risk_overlay),
        "avg_confidence": avg_confidence,
        "avg_stability": avg_stability,
        "trend_cohesion_ready_days": trend_cohesion_ready_days,
        "market_state_counts": market_state_counts,
        "risk_overlay_counts": risk_overlay_counts,
    });

    std::fs::write(
        save_dir.join("weekly_state_metrics.json"),
        serde_json::to_string_pretty(&metrics)?,
    )?;

    let mut review = String::new();
    review.push_str("# Weekly State Review (Auto)\n\n");
    review.push_str(&format!("- As of: {}\n", pres_packet.date_str));
    review.push_str(&format!(
        "- Status: {}\n",
        if include_current_packet {
            "using current market decision"
        } else {
            "data unavailable; based on prior persisted history only"
        }
    ));
    review.push_str(&format!(
        "- Latest headline: {} | {}\n",
        pres_packet.macro_display.headline, pres_packet.macro_display.bias_label
    ));
    review.push_str(&format!("- Days analyzed: {}\n", day_count));
    review.push_str(&format!("- Avg confidence: {:.1}\n", avg_confidence));
    review.push_str(&format!("- Avg stability: {:.1}\n", avg_stability));
    review.push_str(&format!(
        "- Trend cohesion ready days: {}\n\n",
        trend_cohesion_ready_days
    ));
    review.push_str("## Market State Counts\n");
    for (state, count) in metrics["market_state_counts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        review.push_str(&format!("- {}: {}\n", state, count));
    }
    review.push_str("\n## Risk Overlay Counts\n");
    for (state, count) in metrics["risk_overlay_counts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        review.push_str(&format!("- {}: {}\n", state, count));
    }

    std::fs::write(save_dir.join("weekly_state_review_auto.md"), review)?;
    Ok(())
}

#[derive(Debug, Clone)]
struct TransitionAuditEntry {
    date: NaiveDate,
    timestamp: DateTime<FixedOffset>,
    log: crate::core::transition_log::StateTransitionLog,
}

#[derive(Debug, Clone)]
struct TransitionAuditDay {
    date: NaiveDate,
    events: Vec<TransitionAuditEntry>,
}

impl TransitionAuditDay {
    fn latest(&self) -> &TransitionAuditEntry {
        self.events
            .last()
            .expect("TransitionAuditDay must include at least one event")
    }
}

fn run_audit_daily(
    app_config: &config::AppConfig,
    target_date_arg: Option<&str>,
    window_days: usize,
    language: Language,
) -> Result<()> {
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
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
    let report = build_audit_daily_report(&days, target_idx, window_days.max(1), language);
    println!("{}", report);
    Ok(())
}

fn load_transition_audit_days(
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

    raw_entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok(group_audit_days(raw_entries))
}

fn parse_transition_audit_entry(
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

    let log: crate::core::transition_log::StateTransitionLog = serde_json::from_value(log_json)?;
    Ok(Some(TransitionAuditEntry {
        date,
        timestamp,
        log,
    }))
}

fn resolve_target_index(
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

fn build_audit_daily_report(
    days: &[TransitionAuditDay],
    target_idx: usize,
    window_days: usize,
    language: Language,
) -> String {
    let text = audit_text(language);
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
    let gate_streak = consecutive_streak(days, target_idx, |log| {
        log.trend_cohesion_gate.to == gate_is_ready
    });

    let blocker_counts = summarize_blockers(&window_latest);
    let top_blockers = blocker_counts.into_iter().take(3).collect::<Vec<_>>();

    let breakout_today = summarize_breakout_changes_from_events(today);
    let no_trade_streak = consecutive_streak(days, target_idx, |log| !log.trend_cohesion_gate.to);
    let mainline_missing_streak = consecutive_streak(days, target_idx, |log| {
        log.trend_cohesion_status.to != crate::core::trend_cohesion::TrendCohesionStatus::Formed
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
    let audit_sentence = build_audit_sentence(
        language,
        gate_status,
        gate_streak,
        &blocker_text,
        &breakout_text,
        mainline_text,
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
        "- {}: {} {}\n",
        text.label_duration, gate_streak, text.day_unit
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

    out.push_str(&format!("\n4. {}\n", text.section_streaks));
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

    out.push_str(&format!("\n5. {}\n", text.section_one_liner));
    out.push_str(&format!("- {}\n", audit_sentence));

    out
}

struct AuditDailyText {
    title: &'static str,
    section_gate: &'static str,
    section_transition: &'static str,
    section_breakout: &'static str,
    section_streaks: &'static str,
    section_one_liner: &'static str,
    label_status: &'static str,
    label_duration: &'static str,
    label_top_blockers: &'static str,
    label_state_change: &'static str,
    label_risk_change: &'static str,
    label_trend_change: &'static str,
    label_breakout_new: &'static str,
    label_breakout_continued: &'static str,
    label_breakout_removed: &'static str,
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
    day_unit: &'static str,
}

fn audit_text(language: Language) -> AuditDailyText {
    match language {
        Language::ZhCn => AuditDailyText {
            title: "Audit Daily",
            section_gate: "Gate 摘要",
            section_transition: "Transition 摘要",
            section_breakout: "Breakout 摘要",
            section_streaks: "连续段统计",
            section_one_liner: "审计一句话",
            label_status: "状态",
            label_duration: "持续天数",
            label_top_blockers: "最主要阻碍因子 Top 3",
            label_state_change: "今天是否有状态变化",
            label_risk_change: "今天是否有 risk overlay 变化",
            label_trend_change: "今天是否有主线状态变化",
            label_breakout_new: "新增 breakout",
            label_breakout_continued: "延续 breakout",
            label_breakout_removed: "消失 breakout",
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
            day_unit: "天",
        },
        Language::EnUs => AuditDailyText {
            title: "Audit Daily",
            section_gate: "Gate Summary",
            section_transition: "Transition Summary",
            section_breakout: "Breakout Summary",
            section_streaks: "Streak Metrics",
            section_one_liner: "Audit One-liner",
            label_status: "Status",
            label_duration: "Duration",
            label_top_blockers: "Top 3 blockers",
            label_state_change: "State changed today",
            label_risk_change: "Risk overlay changed today",
            label_trend_change: "Mainline status changed today",
            label_breakout_new: "New breakout",
            label_breakout_continued: "Continued breakout",
            label_breakout_removed: "Removed breakout",
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
            day_unit: "days",
        },
        Language::JaJp => AuditDailyText {
            title: "Audit Daily",
            section_gate: "Gate サマリー",
            section_transition: "Transition サマリー",
            section_breakout: "Breakout サマリー",
            section_streaks: "連続区間統計",
            section_one_liner: "監査ワンライン要約",
            label_status: "状態",
            label_duration: "継続日数",
            label_top_blockers: "主要阻害要因 Top 3",
            label_state_change: "本日の状態変化",
            label_risk_change: "本日の risk overlay 変化",
            label_trend_change: "本日の主線状態変化",
            label_breakout_new: "新規 breakout",
            label_breakout_continued: "継続 breakout",
            label_breakout_removed: "消失 breakout",
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
            day_unit: "日",
        },
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

fn format_symbols(symbols: &[String], language: Language) -> String {
    if symbols.is_empty() {
        audit_text(language).none.to_string()
    } else {
        symbols.join(", ")
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

fn group_audit_days(entries: Vec<TransitionAuditEntry>) -> Vec<TransitionAuditDay> {
    let mut grouped = BTreeMap::<NaiveDate, Vec<TransitionAuditEntry>>::new();
    for entry in entries {
        grouped.entry(entry.date).or_default().push(entry);
    }

    let mut days = grouped
        .into_iter()
        .map(|(date, mut events)| {
            events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            TransitionAuditDay { date, events }
        })
        .collect::<Vec<_>>();
    days.sort_by(|a, b| a.date.cmp(&b.date));
    days
}

struct BreakoutDailySummary {
    new_symbols: Vec<String>,
    continued_symbols: Vec<String>,
    removed_symbols: Vec<String>,
}

fn summarize_breakout_changes(
    changes: &[crate::core::transition_log::BreakoutTransition],
) -> BreakoutDailySummary {
    let mut new_symbols = Vec::new();
    let mut continued_symbols = Vec::new();
    let mut removed_symbols = Vec::new();
    for item in changes {
        let from = item.from_status;
        let to = item.to_status;
        if from == crate::core::breakout_detection::BreakoutStatus::NoBreakout
            && to != crate::core::breakout_detection::BreakoutStatus::NoBreakout
        {
            new_symbols.push(item.symbol.clone());
        } else if from != crate::core::breakout_detection::BreakoutStatus::NoBreakout
            && to == crate::core::breakout_detection::BreakoutStatus::NoBreakout
        {
            removed_symbols.push(item.symbol.clone());
        } else if from != crate::core::breakout_detection::BreakoutStatus::NoBreakout
            && to != crate::core::breakout_detection::BreakoutStatus::NoBreakout
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
    status: crate::core::trend_cohesion::TrendCohesionStatus,
    language: Language,
) -> &'static str {
    match language {
        Language::ZhCn => match status {
            crate::core::trend_cohesion::TrendCohesionStatus::Dispersed => "未形成",
            crate::core::trend_cohesion::TrendCohesionStatus::Forming => "形成中",
            crate::core::trend_cohesion::TrendCohesionStatus::Formed => "已形成",
        },
        Language::EnUs => match status {
            crate::core::trend_cohesion::TrendCohesionStatus::Dispersed => "Not formed",
            crate::core::trend_cohesion::TrendCohesionStatus::Forming => "Forming",
            crate::core::trend_cohesion::TrendCohesionStatus::Formed => "Formed",
        },
        Language::JaJp => match status {
            crate::core::trend_cohesion::TrendCohesionStatus::Dispersed => "未形成",
            crate::core::trend_cohesion::TrendCohesionStatus::Forming => "形成中",
            crate::core::trend_cohesion::TrendCohesionStatus::Formed => "形成済み",
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

fn consecutive_streak<F>(days: &[TransitionAuditDay], target_idx: usize, predicate: F) -> usize
where
    F: Fn(&crate::core::transition_log::StateTransitionLog) -> bool,
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

fn build_audit_sentence(
    language: Language,
    gate_status: &str,
    gate_streak: usize,
    blocker_text: &str,
    breakout_text: &str,
    mainline_text: &str,
) -> String {
    match language {
        Language::ZhCn => format!(
            "{} 连续第 {} 天；主因：{}；今日 breakout：{}；主线状态：{}。",
            gate_status, gate_streak, blocker_text, breakout_text, mainline_text
        ),
        Language::EnUs => format!(
            "{} day {} in a row; primary blockers: {}; today's breakout: {}; mainline status: {}.",
            gate_status, gate_streak, blocker_text, breakout_text, mainline_text
        ),
        Language::JaJp => format!(
            "{} 連続 {} 日目；主因：{}；本日の breakout：{}；主線状態：{}。",
            gate_status, gate_streak, blocker_text, breakout_text, mainline_text
        ),
    }
}

fn audit_empty_log_message(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未找到可用的 state_transitions.jsonl 记录。",
        Language::EnUs => "No usable records found in state_transitions.jsonl.",
        Language::JaJp => "state_transitions.jsonl に有効な記録がありません。",
    }
}

fn audit_error_missing_date(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--date 需要 YYYY-MM-DD 参数",
        Language::EnUs => "--date requires a YYYY-MM-DD value",
        Language::JaJp => "--date には YYYY-MM-DD の値が必要です",
    }
}

fn audit_error_missing_days(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--days 需要正整数参数",
        Language::EnUs => "--days requires a positive integer value",
        Language::JaJp => "--days には正の整数値が必要です",
    }
}

fn audit_error_invalid_days(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--days 必须为大于 0 的整数",
        Language::EnUs => "--days must be an integer greater than 0",
        Language::JaJp => "--days は 0 より大きい整数である必要があります",
    }
}

fn audit_error_parse_date(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无法解析 --date",
        Language::EnUs => "Unable to parse --date",
        Language::JaJp => "--date を解析できません",
    }
}

fn audit_error_read_file(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无法读取文件",
        Language::EnUs => "Unable to read file",
        Language::JaJp => "ファイルを読み込めません",
    }
}

fn audit_error_parse_line(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "解析 state_transitions.jsonl 第",
        Language::EnUs => "Failed to parse state_transitions.jsonl line",
        Language::JaJp => "state_transitions.jsonl の行解析に失敗:",
    }
}

fn audit_error_invalid_timestamp(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无效 timestamp",
        Language::EnUs => "Invalid timestamp",
        Language::JaJp => "無効な timestamp",
    }
}

fn audit_error_invalid_date(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无效 date",
        Language::EnUs => "Invalid date",
        Language::JaJp => "無効な date",
    }
}

fn audit_error_target_date_not_found(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未找到目标日期的审计记录:",
        Language::EnUs => "No audit record found for target date:",
        Language::JaJp => "対象日の監査記録が見つかりません:",
    }
}

fn audit_daily_usage(language: Language) -> &'static str {
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

async fn run_review(_config: &crate::config::AppConfig) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_audit_daily_report, parse_transition_audit_entry, run_pipeline,
        should_persist_decision_history, telegram_delivery_precheck, ProviderType,
        TransitionAuditDay, TransitionAuditEntry,
    };
    use crate::config::{
        AppConfig, DeviationBasis, OutputConfig, RulesConfig, TelegramConfig, TrendConfig,
        WatchlistEntry,
    };
    use crate::core::i18n::Language;
    use crate::core::run_status::DeliveryStatus;
    use crate::core::runtime_mode::ExecutionMode;
    use crate::data::provider::MarketDataProvider;
    use crate::data::yahoo_provider::{DailyBar, TickerHistory};
    use anyhow::{anyhow, Result};
    use chrono::{NaiveDate, Utc};
    use std::borrow::Cow;
    use std::collections::{BTreeMap, HashMap};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use tempfile::tempdir;
    use time::OffsetDateTime;

    struct AlwaysFailProvider;

    struct PartialSuccessProvider;

    #[async_trait::async_trait]
    impl MarketDataProvider for AlwaysFailProvider {
        async fn fetch_history(
            &self,
            _symbol: &str,
            _start_date: Option<OffsetDateTime>,
            _end_date: Option<OffsetDateTime>,
        ) -> Result<crate::data::yahoo_provider::TickerHistory<'static>> {
            Err(anyhow!("synthetic fetch failure"))
        }
    }

    #[async_trait::async_trait]
    impl MarketDataProvider for PartialSuccessProvider {
        async fn fetch_history(
            &self,
            symbol: &str,
            _start_date: Option<OffsetDateTime>,
            _end_date: Option<OffsetDateTime>,
        ) -> Result<crate::data::yahoo_provider::TickerHistory<'static>> {
            match symbol {
                "AAA" => Ok(create_mock_history(symbol, 100.0, 60, 0.002)),
                _ => Err(anyhow!("synthetic partial fetch failure")),
            }
        }
    }

    fn create_mock_history(
        symbol: &str,
        start_price: f64,
        count: usize,
        daily_change: f64,
    ) -> TickerHistory<'static> {
        let start_date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let mut bars = Vec::with_capacity(count);
        let mut current_price = start_price;

        for i in 0..count {
            bars.push(DailyBar {
                date: start_date + chrono::Duration::days(i as i64),
                close: current_price,
                volume: Some(1000.0),
            });
            current_price *= 1.0 + daily_change;
        }

        TickerHistory {
            symbol: symbol.to_string(),
            bars: Cow::Owned(bars),
            total_trading_days: count,
            latest_quote_timestamp: Some(Utc::now().timestamp()),
        }
    }

    fn mock_config(save_to: &Path) -> AppConfig {
        AppConfig {
            version: 1,
            output: OutputConfig {
                timezone: "UTC".to_string(),
                format: "markdown".to_string(),
                save_to: save_to.display().to_string(),
                weight_kind: Some("equal".to_string()),
                language: Some(Language::ZhCn),
                compact_transition_evidence_in_no_trade: true,
            },
            telegram: None,
            futu: None,
            finnhub: None,
            trading: None,
            provider: Some("yahoo".to_string()),
            rules: RulesConfig {
                trend: TrendConfig::default(),
                deviation_bands: BTreeMap::new(),
                actions: HashMap::new(),
                sizing_multipliers: None,
                core_assets: None,
                min_state_duration: None,
                inertia: None,
                trend_cohesion: None,
                breakout: None,
                market_state_engine: None,
            },
            watchlist: ["AAA", "BBB", "CCC", "DDD", "EEE", "FFF"]
                .into_iter()
                .map(|symbol| WatchlistEntry {
                    symbol: symbol.to_string(),
                    weight: None,
                    market: "US".to_string(),
                    owner_ma_days: 20,
                    leash_ma_days: 5,
                    deviation_basis: DeviationBasis::Owner,
                    enable: true,
                    trade_enabled: Some(false),
                    trade_amount: None,
                })
                .collect(),
        }
    }

    fn assert_audit_snapshot(file_name: &str, actual: &str) {
        let snapshot_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("snapshots")
            .join(file_name);
        let expected = fs::read_to_string(&snapshot_path).unwrap_or_else(|_| {
            panic!(
                "Audit snapshot not found. expected path: {}",
                snapshot_path.display()
            )
        });
        assert_eq!(
            expected.trim_end(),
            actual.trim_end(),
            "audit snapshot mismatch: {}",
            snapshot_path.display()
        );
    }

    fn sample_audit_days() -> Vec<TransitionAuditDay> {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-21T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": true,
                    "added": ["DirectionalCohesion"],
                    "removed": [],
                    "persisting": ["StabilityThreshold","ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [
                    {"symbol":"GOOG","from_status":"NoBreakout","to_status":"EmergingBreakout","status_changed":true,"risk_changed":false}
                ]
            }))
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
    fn persists_normal_runs_and_skips_diagnostic_only_runs() {
        assert!(should_persist_decision_history(3, 0));
        assert!(should_persist_decision_history(3, 2));
        assert!(should_persist_decision_history(1, 99));

        assert!(!should_persist_decision_history(0, 5));
    }

    #[test]
    fn empty_fetch_set_does_not_trigger_diagnostic_skip() {
        assert!(should_persist_decision_history(0, 0));
    }

    #[test]
    fn telegram_precheck_skips_when_disabled() {
        let cfg = TelegramConfig {
            enabled: false,
            bot_token: "token".to_string(),
            chat_id: "chat".to_string(),
        };
        assert!(matches!(
            telegram_delivery_precheck(Some(&cfg)),
            Err(DeliveryStatus::Skipped)
        ));
    }

    #[test]
    fn telegram_precheck_fails_when_credentials_missing() {
        let cfg = TelegramConfig {
            enabled: true,
            bot_token: "".to_string(),
            chat_id: "".to_string(),
        };
        assert!(matches!(
            telegram_delivery_precheck(Some(&cfg)),
            Err(DeliveryStatus::Failed { .. })
        ));
    }

    #[tokio::test]
    async fn full_fetch_failure_generates_report_without_persisting_history() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());
        let provider: Arc<dyn MarketDataProvider> = Arc::new(AlwaysFailProvider);

        run_pipeline(
            config,
            ProviderType::Yahoo,
            provider,
            ExecutionMode::Disabled,
        )
        .await
        .unwrap();

        let today = chrono::Local::now().date_naive().to_string();
        let report_path = tmp.path().join(format!("{}.md", today));
        let run_status_path = tmp.path().join(format!("run_status_{}.json", today));
        let history_path = tmp.path().join("decision_history.jsonl");
        let daily_packet_path = tmp.path().join(format!("decision_packet_{}.json", today));
        let execution_gate_log_path = tmp.path().join("execution_gate_log.jsonl");
        let portfolio_snapshot_path = tmp
            .path()
            .join(format!("portfolio_snapshot_{}.json", today));
        let account_snapshot_path = tmp.path().join(format!("account_snapshot_{}.json", today));
        let data_quality_log_path = tmp.path().join("data_quality_log.jsonl");
        let weekly_metrics_path = tmp.path().join("weekly_state_metrics.json");
        let weekly_review_path = tmp.path().join("weekly_state_review_auto.md");

        assert!(
            report_path.exists(),
            "diagnostic markdown report should exist"
        );
        assert!(
            run_status_path.exists(),
            "run status should still be persisted"
        );
        assert!(
            !history_path.exists(),
            "diagnostic-only run must not create decision history"
        );
        assert!(
            !daily_packet_path.exists(),
            "diagnostic-only run must not create a daily decision packet"
        );
        assert!(execution_gate_log_path.exists());
        assert!(portfolio_snapshot_path.exists());
        assert!(account_snapshot_path.exists());
        assert!(data_quality_log_path.exists());
        assert!(weekly_metrics_path.exists());
        assert!(weekly_review_path.exists());

        let report = std::fs::read_to_string(report_path).unwrap();
        assert!(report.contains("数据不可用"));
        assert!(report.contains("严重"));

        let gate_log = std::fs::read_to_string(execution_gate_log_path).unwrap();
        assert!(gate_log.contains("execution_gate_noop"));

        let quality_log = std::fs::read_to_string(data_quality_log_path).unwrap();
        assert!(quality_log.contains("CRITICAL"));
        let weekly_metrics = std::fs::read_to_string(weekly_metrics_path).unwrap();
        assert!(weekly_metrics.contains("DATA_UNAVAILABLE"));

        let run_status: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(run_status_path).unwrap()).unwrap();
        assert_eq!(
            run_status["state_machine"]["to_state"],
            serde_json::Value::String("DATA_UNAVAILABLE".to_string())
        );
    }

    #[tokio::test]
    async fn partial_fetch_failure_preserves_history_and_real_market_state() {
        let tmp = tempdir().unwrap();
        let config = mock_config(tmp.path());
        let provider: Arc<dyn MarketDataProvider> = Arc::new(PartialSuccessProvider);

        run_pipeline(
            config,
            ProviderType::Yahoo,
            provider,
            ExecutionMode::Disabled,
        )
        .await
        .unwrap();

        let history_path = tmp.path().join("decision_history.jsonl");
        let report_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
            .expect("report should exist for partial-failure runs");
        let run_status_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("run_status_") && name.ends_with(".json"))
                    .unwrap_or(false)
            })
            .expect("run status should exist for partial-failure runs");
        let daily_packet_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("decision_packet_") && name.ends_with(".json"))
                    .unwrap_or(false)
            })
            .expect("daily decision packet must still be produced for partial-failure runs");
        let execution_gate_log_path = tmp.path().join("execution_gate_log.jsonl");
        let portfolio_snapshot_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("portfolio_snapshot_") && name.ends_with(".json"))
                    .unwrap_or(false)
            })
            .expect("portfolio snapshot should exist");
        let account_snapshot_path = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("account_snapshot_") && name.ends_with(".json"))
                    .unwrap_or(false)
            })
            .expect("account snapshot should exist");
        let data_quality_log_path = tmp.path().join("data_quality_log.jsonl");
        let weekly_metrics_path = tmp.path().join("weekly_state_metrics.json");
        let weekly_review_path = tmp.path().join("weekly_state_review_auto.md");

        assert!(
            history_path.exists(),
            "real decisions must still persist when at least one symbol succeeded"
        );
        assert!(execution_gate_log_path.exists());
        assert!(portfolio_snapshot_path.exists());
        assert!(account_snapshot_path.exists());
        assert!(data_quality_log_path.exists());
        assert!(weekly_metrics_path.exists());
        assert!(weekly_review_path.exists());

        let report = std::fs::read_to_string(report_path).unwrap();
        assert!(report.contains("⚠️"));
        assert!(report.contains("警告"));
        assert!(report.contains("获取失败"));

        let daily_packet: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(daily_packet_path).unwrap()).unwrap();
        assert_eq!(daily_packet["assets"].as_array().map(Vec::len), Some(1));

        let run_status: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(run_status_path).unwrap()).unwrap();
        assert_ne!(
            run_status["state_machine"]["to_state"],
            serde_json::Value::String("DATA_UNAVAILABLE".to_string())
        );

        let quality_log = std::fs::read_to_string(data_quality_log_path).unwrap();
        assert!(quality_log.contains("WARNING"));
        let weekly_metrics = std::fs::read_to_string(weekly_metrics_path).unwrap();
        assert!(weekly_metrics.contains("\"include_current_packet\": true"));
    }

    #[test]
    fn parse_transition_audit_entry_supports_legacy_and_v2_lines() {
        let legacy = serde_json::json!({
            "timestamp": "2026-04-22T09:00:00+09:00",
            "log": {
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }
        });
        let v2 = serde_json::json!({
            "schema_version": 2,
            "event_type": "state_transition",
            "timestamp": "2026-04-23T09:00:00+09:00",
            "date": "2026-04-23",
            "transition": {
                "no_trade_persists": false,
                "market_state": {"from":"IGNITION","to":"EARLY_CONFIRMATION","changed": true},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": true,
                    "unmet_conditions_changed": true,
                    "added": [],
                    "removed": ["StabilityThreshold","ContinuityThreshold"],
                    "persisting": []
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Forming","changed": true},
                "trend_cohesion_topology": {"from":"NoLeader","to":"SingleLeader","changed": true},
                "breakout_changes": []
            }
        });

        let legacy_entry = parse_transition_audit_entry(&legacy.to_string(), Language::ZhCn)
            .unwrap()
            .expect("legacy entry");
        let v2_entry = parse_transition_audit_entry(&v2.to_string(), Language::ZhCn)
            .unwrap()
            .expect("v2 entry");

        assert_eq!(
            legacy_entry.date,
            NaiveDate::from_ymd_opt(2026, 4, 22).unwrap()
        );
        assert!(!legacy_entry.log.trend_cohesion_gate.to);
        assert_eq!(v2_entry.date, NaiveDate::from_ymd_opt(2026, 4, 23).unwrap());
        assert!(v2_entry.log.trend_cohesion_gate.to);
    }

    #[test]
    fn build_audit_daily_report_emits_five_fixed_sections() {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-21T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": true,
                    "added": ["DirectionalCohesion"],
                    "removed": [],
                    "persisting": ["StabilityThreshold","ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [
                    {"symbol":"GOOG","from_status":"NoBreakout","to_status":"EmergingBreakout","status_changed":true,"risk_changed":false}
                ]
            }))
            .unwrap(),
        };
        let days = vec![
            TransitionAuditDay {
                date: entry_day1.date,
                events: vec![entry_day1],
            },
            TransitionAuditDay {
                date: entry_day2.date,
                events: vec![entry_day2],
            },
        ];
        let report = build_audit_daily_report(&days, 1, 14, Language::ZhCn);
        assert!(report.contains("1. Gate 摘要"));
        assert!(report.contains("2. Transition 摘要"));
        assert!(report.contains("3. Breakout 摘要"));
        assert!(report.contains("4. 连续段统计"));
        assert!(report.contains("5. 审计一句话"));
        assert!(report.contains("口径: 连续段按日志连续计算（周末自动衔接）"));
        assert!(report.contains("NO TRADE 连续第 2 天；主因："));
        assert!(report.contains("；今日 breakout：GOOG（新增）；主线状态：未形成。"));
    }

    #[test]
    fn build_audit_daily_report_localizes_to_en_us() {
        let entry = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": []
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [
                    {"symbol":"GOOG","from_status":"NoBreakout","to_status":"EmergingBreakout","status_changed":true,"risk_changed":false}
                ]
            }))
            .unwrap(),
        };
        let days = vec![TransitionAuditDay {
            date: entry.date,
            events: vec![entry],
        }];
        let report = build_audit_daily_report(&days, 0, 14, Language::EnUs);
        assert!(report.contains("1. Gate Summary"));
        assert!(report.contains("2. Transition Summary"));
        assert!(report.contains("3. Breakout Summary"));
        assert!(report.contains("4. Streak Metrics"));
        assert!(report.contains("5. Audit One-liner"));
        assert!(report.contains("Methodology: streaks are calculated by log continuity"));
        assert!(report.contains("NO TRADE day 1 in a row; primary blockers:"));
        assert!(report.contains("today's breakout: GOOG (new); mainline status: Not formed."));
    }

    #[test]
    fn build_audit_daily_report_localizes_to_ja_jp() {
        let entry = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": []
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [
                    {"symbol":"GOOG","from_status":"NoBreakout","to_status":"EmergingBreakout","status_changed":true,"risk_changed":false}
                ]
            }))
            .unwrap(),
        };
        let days = vec![TransitionAuditDay {
            date: entry.date,
            events: vec![entry],
        }];
        let report = build_audit_daily_report(&days, 0, 14, Language::JaJp);
        assert!(report.contains("1. Gate サマリー"));
        assert!(report.contains("2. Transition サマリー"));
        assert!(report.contains("3. Breakout サマリー"));
        assert!(report.contains("4. 連続区間統計"));
        assert!(report.contains("5. 監査ワンライン要約"));
        assert!(report.contains("口径: 連続区間はログ連続で計算（週末は自動連結）"));
        assert!(report.contains("NO TRADE 連続 1 日目；主因："));
        assert!(report.contains("本日の breakout：GOOG（新規）；主線状態：未形成。"));
    }

    #[test]
    fn audit_daily_snapshot_zh_cn() {
        let report = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::ZhCn);
        assert_audit_snapshot("audit_daily_zh_cn.txt", &report);
    }

    #[test]
    fn audit_daily_snapshot_en_us() {
        let report = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::EnUs);
        assert_audit_snapshot("audit_daily_en_us.txt", &report);
    }

    #[test]
    fn audit_daily_snapshot_ja_jp() {
        let report = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::JaJp);
        assert_audit_snapshot("audit_daily_ja_jp.txt", &report);
    }

    #[test]
    fn audit_daily_contract_contains_one_liner_and_methodology_lines() {
        let report_zh = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::ZhCn);
        assert!(report_zh.contains("5. 审计一句话"));
        assert!(report_zh.contains("NO TRADE 连续第 2 天；主因："));
        assert!(report_zh.contains("口径: 连续段按日志连续计算（周末自动衔接）"));

        let report_en = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::EnUs);
        assert!(report_en.contains("5. Audit One-liner"));
        assert!(report_en.contains("NO TRADE day 2 in a row; primary blockers:"));
        assert!(report_en.contains(
            "Methodology: streaks are calculated by log continuity (weekends auto-bridged)"
        ));

        let report_ja = build_audit_daily_report(&sample_audit_days(), 1, 14, Language::JaJp);
        assert!(report_ja.contains("5. 監査ワンライン要約"));
        assert!(report_ja.contains("NO TRADE 連続 2 日目；主因："));
        assert!(report_ja.contains("口径: 連続区間はログ連続で計算（週末は自動連結）"));
    }

    #[test]
    fn consecutive_streak_treats_trading_day_sequence_as_continuous() {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 17).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-17T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": [],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-20T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": [],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let days = vec![
            TransitionAuditDay {
                date: entry_day1.date,
                events: vec![entry_day1],
            },
            TransitionAuditDay {
                date: entry_day2.date,
                events: vec![entry_day2],
            },
        ];
        let streak = super::consecutive_streak(&days, 1, |log| !log.trend_cohesion_gate.to);
        assert_eq!(streak, 2);
    }

    #[test]
    fn ready_audit_sentence_uses_no_primary_blocker() {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-21T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": false,
                    "unmet_conditions_changed": false,
                    "added": ["StabilityThreshold"],
                    "removed": [],
                    "persisting": ["ContinuityThreshold"]
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": false,
                "market_state": {"from":"IGNITION","to":"EARLY_CONFIRMATION","changed": true},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {
                    "from": false,
                    "to": true,
                    "unmet_conditions_changed": true,
                    "added": [],
                    "removed": ["StabilityThreshold", "ContinuityThreshold"],
                    "persisting": []
                },
                "trend_cohesion_status": {"from":"Dispersed","to":"Forming","changed": true},
                "trend_cohesion_topology": {"from":"NoLeader","to":"SingleLeader","changed": true},
                "breakout_changes": []
            }))
            .unwrap(),
        };
        let days = vec![
            TransitionAuditDay {
                date: entry_day1.date,
                events: vec![entry_day1],
            },
            TransitionAuditDay {
                date: entry_day2.date,
                events: vec![entry_day2],
            },
        ];
        let report = build_audit_daily_report(&days, 1, 14, Language::ZhCn);
        assert!(report.contains("READY 连续第 1 天；主因：无；"));
    }

    #[test]
    fn breakout_summary_keeps_intraday_event_history() {
        let morning = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {"from": false, "to": false, "unmet_conditions_changed": false, "added": [], "removed": [], "persisting": []},
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": [
                    {"symbol":"GOOG","from_status":"NoBreakout","to_status":"EmergingBreakout","status_changed":true,"risk_changed":false}
                ]
            })).unwrap(),
        };
        let close = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T16:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {"from": false, "to": false, "unmet_conditions_changed": false, "added": [], "removed": [], "persisting": []},
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            })).unwrap(),
        };

        let day = TransitionAuditDay {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            events: vec![morning, close],
        };
        let report = build_audit_daily_report(&[day], 0, 14, Language::ZhCn);
        assert!(report.contains("新增 breakout: GOOG"));
        assert!(report.contains("今日 breakout：GOOG（新增）"));
    }

    #[test]
    fn consecutive_streak_breaks_on_missing_weekday_gap() {
        let entry_day1 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-20T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {"from": false, "to": false, "unmet_conditions_changed": false, "added": [], "removed": [], "persisting": []},
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            })).unwrap(),
        };
        let entry_day2 = TransitionAuditEntry {
            date: NaiveDate::from_ymd_opt(2026, 4, 22).unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-22T09:00:00+09:00").unwrap(),
            log: serde_json::from_value(serde_json::json!({
                "no_trade_persists": true,
                "market_state": {"from":"IGNITION","to":"IGNITION","changed": false},
                "risk_overlay": {"from":"NORMAL","to":"NORMAL","changed": false},
                "trend_cohesion_gate": {"from": false, "to": false, "unmet_conditions_changed": false, "added": [], "removed": [], "persisting": []},
                "trend_cohesion_status": {"from":"Dispersed","to":"Dispersed","changed": false},
                "trend_cohesion_topology": {"from":"NoLeader","to":"NoLeader","changed": false},
                "breakout_changes": []
            })).unwrap(),
        };
        let days = vec![
            TransitionAuditDay {
                date: entry_day1.date,
                events: vec![entry_day1],
            },
            TransitionAuditDay {
                date: entry_day2.date,
                events: vec![entry_day2],
            },
        ];
        let streak = super::consecutive_streak(&days, 1, |log| !log.trend_cohesion_gate.to);
        assert_eq!(streak, 1);
    }
}

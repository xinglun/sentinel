use anyhow::{Context, Result};

use futures::stream::{self, StreamExt};
use serde_json::json;
use std::sync::Arc;
use time::OffsetDateTime;

use crate::backtest;
use crate::config;
use crate::core::engine::Engine;
use crate::core::execution_gate::ExecutionGate;
use crate::core::ledger::Ledger;
use crate::core::persistence::PersistenceLayer;
use crate::core::transition_log::TransitionLogger;

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

    let mut command = "radar";
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
            "--provider" => {
                if i + 1 < args.len() {
                    let p = args[i + 1].to_lowercase();
                    if p == "futu" {
                        provider_type = ProviderType::Futu;
                    } else if p == "yahoo" {
                        provider_type = ProviderType::Yahoo;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
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
            let _ = transition_logger
                .log_transition(prev_packet.map(|p| &p.market_regime), &packet.market_regime);
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

        if let Some(ref tg_cfg) = config_arc.telegram {
            if tg_cfg.enabled {
                let _ = notify::send_telegram_message(tg_cfg, &report_result.markdown_body).await;
            }
        }
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
    let mut participation_ready_days = 0usize;

    for packet in &recent_packets {
        *market_state_counts
            .entry(format!("{:?}", packet.market_regime.market_state))
            .or_insert(0) += 1;
        *risk_overlay_counts
            .entry(format!("{:?}", packet.market_regime.risk_overlay))
            .or_insert(0) += 1;
        total_confidence += packet.market_features.system_confidence;
        total_stability += packet.market_features.stability_score;
        if packet.participation.participation_ready {
            participation_ready_days += 1;
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
        "participation_ready_days": participation_ready_days,
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
        "- Participation ready days: {}\n\n",
        participation_ready_days
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

async fn run_review(_config: &crate::config::AppConfig) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{run_pipeline, should_persist_decision_history, ProviderType};
    use crate::config::{
        AppConfig, DeviationBasis, OutputConfig, RulesConfig, TrendConfig, WatchlistEntry,
    };
    use crate::core::i18n::Language;
    use crate::core::runtime_mode::ExecutionMode;
    use crate::data::provider::MarketDataProvider;
    use crate::data::yahoo_provider::{DailyBar, TickerHistory};
    use anyhow::{anyhow, Result};
    use chrono::{NaiveDate, Utc};
    use std::borrow::Cow;
    use std::collections::{BTreeMap, HashMap};
    use std::path::Path;
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
}

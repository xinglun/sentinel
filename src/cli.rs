use anyhow::{Context, Result};

use futures::stream::{self, StreamExt};
use std::sync::Arc;
use time::OffsetDateTime;

use crate::backtest;
use crate::config;
use crate::core::engine::Engine;
use crate::core::execution_gate::ExecutionGate;
use crate::core::ledger::Ledger;
use crate::core::persistence::PersistenceLayer;
use crate::core::trader_agent::TraderAgent;
use crate::core::transition_log::TransitionLogger;

use crate::core::notify;
use crate::core::report;
use crate::data::provider::MarketDataProvider;

use crate::adapters::futu::client::FutuClient;
use crate::adapters::futu::provider::FutuProvider;
use crate::adapters::futu::trader::FutuTrader;
use crate::trade::trader::TradeExecutor;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProviderType {
    Yahoo,
    Futu,
}

pub async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let app_config = config::AppConfig::load("config.toml")?;

    let mut command = "radar";
    // Priority: CLI Argument > Config File > Default (Yahoo)
    let mut provider_type = match app_config.provider.as_deref() {
        Some("futu") => ProviderType::Futu,
        _ => ProviderType::Yahoo,
    };

    let mut futu_addr = if let Some(futu_cfg) = &app_config.futu {
        format!("{}:{}", futu_cfg.opend_ip, futu_cfg.opend_port)
    } else {
        "127.0.0.1:11111".to_string()
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "backtest" => command = "backtest",
            "daemon" | "trade" => command = "daemon",
            "radar" => command = "radar",
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
            "--opend" => {
                if i + 1 < args.len() {
                    futu_addr = args[i + 1].clone();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    match command {
        "backtest" => {
            println!("🔬 资本望远镜：回测模式启动 (Backtest Mode)");
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
            println!("🤖 哨兵守卫：交易守护进程启动 (Daemon Mode)");
            let is_trading_enabled = app_config
                .trading
                .as_ref()
                .map(|t| t.enabled)
                .unwrap_or(false);
            let mode = if is_trading_enabled {
                crate::core::runtime_mode::ExecutionMode::Live
            } else {
                println!("⚠️  Trading is DISABLED in config. Running in DRY-RUN mode.");
                crate::core::runtime_mode::ExecutionMode::DryRun
            };

            let provider = get_provider(provider_type, &futu_addr).await;
            run_pipeline(app_config, provider_type, provider, mode).await?;
        }
        _ => {
            println!("🐕 Stock Sentinel initializing (Radar Mode)...");
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
        ProviderType::Futu => {
            println!("🔌 尝试通过 Moomoo OpenD ({}) 获取行情...", addr);
            match FutuClient::connect(addr).await {
                Ok(client) => Arc::new(FutuProvider::new(Arc::new(client))),
                Err(e) => {
                    println!("❌ 无法连接至 Moomoo OpenD: {}。降级使用 Yahoo。", e);
                    Arc::new(YahooProviderAdapter)
                }
            }
        }
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
    provider_type: ProviderType,
    provider: Arc<dyn MarketDataProvider>,
    mode: crate::core::runtime_mode::ExecutionMode,
) -> Result<()> {
    let parsed_rules = app_config.get_parsed_rules();

    let config_arc = Arc::new(app_config);
    let rules_arc = Arc::new(parsed_rules);

    let save_dir = std::path::PathBuf::from(&config_arc.output.save_to);
    // Hardening: Ensure save_dir exists
    if !save_dir.exists() {
        std::fs::create_dir_all(&save_dir).context("Failed to create output directory")?;
    }

    let persistence = PersistenceLayer::new(&save_dir);
    let transition_logger = TransitionLogger::new(&save_dir);

    let prev_packet = persistence.load_latest_packet().ok().flatten();

    println!("📊 Fetching data for enabled assets...");

    let mut ticker_histories = Vec::new();
    let fetches = stream::iter(config_arc.watchlist.iter().filter(|w| w.enable))
        .map(|entry| {
            let provider_ref = Arc::clone(&provider);
            async move {
                match provider_ref.fetch_history(&entry.symbol, None, None).await {
                    Ok(h) => (Some(h), Some(entry)),
                    Err(_) => (None, None),
                }
            }
        })
        .buffer_unordered(10);

    let results: Vec<_> = fetches.collect().await;
    for (h, e) in results {
        if let Some(entry) = e {
            let quality_log = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "symbol": entry.symbol,
                "provider": format!("{:?}", provider_type),
                "fetch_ok": h.is_some(),
                "bar_count": h.as_ref().map(|x| x.bars.len()).unwrap_or(0),
                "latest_bar_date": h.as_ref().and_then(|x| x.bars.last()).map(|p| p.date.to_string()),
            });
            persistence.save_data_quality_log(&quality_log)?;

            if let Some(history) = h {
                ticker_histories.push((history, entry));
            }
        }
    }

    let mut outcome = crate::core::run_status::RunOutcome {
        date: chrono::Local::now().date_naive().to_string(),
        timestamp: chrono::Local::now().to_rfc3339(),
        decisioning: crate::core::run_status::DeliveryStatus::Skipped,
        archival: crate::core::run_status::DeliveryStatus::Skipped,
        notification: crate::core::run_status::DeliveryStatus::Skipped,
        execution: crate::core::run_status::DeliveryStatus::Skipped,
        data_quality: "PENDING".to_string(),
        execution_details: None,
    };

    if !ticker_histories.is_empty() {
        // Core Decision Pipeline
        let packet =
            match Engine::run_daily_pipeline(&ticker_histories, &rules_arc, prev_packet.as_ref()) {
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
            };

        // Align run-status naming with the market/packet date so all daily assets
        // share the same archival date key.
        outcome.date = packet.date.to_string();

        // Phase 6 Persistence & Logging
        let archival_result = (|| -> Result<()> {
            persistence.save_packet(&packet)?;
            persistence.save_daily_packet(&packet)?;

            // Calculate config hash for telemetry integrity
            let config_content = std::fs::read_to_string("config.toml").unwrap_or_default();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            use std::hash::Hasher;
            hasher.write(config_content.as_bytes());
            let config_hash = format!("{:x}", hasher.finish());

            let total_enabled = config_arc.watchlist.iter().filter(|w| w.enable).count();
            let fetch_success = ticker_histories.len();
            let data_quality_status = if fetch_success == total_enabled {
                "OK".to_string()
            } else if fetch_success > 0 {
                format!("DEGRADED ({}/{})", fetch_success, total_enabled)
            } else {
                "FAILED".to_string()
            };
            outcome.data_quality = data_quality_status.clone();

            // Build TelemetryRow
            let telemetry_row = crate::core::telemetry::TelemetryRow {
                timestamp: chrono::Local::now().to_rfc3339(),
                date: packet.date.to_string(),
                provider: format!("{:?}", provider_type),
                market_state: packet.market_regime.market_state,
                risk_overlay: packet.market_regime.risk_overlay,
                system_confidence: packet.market_features.system_confidence,
                stability_score: packet.market_features.stability_score,
                dominance_margin: packet.market_features.dominance_margin,
                potential_energy: packet.market_features.potential_energy,
                regime_age: packet.market_features.regime_age,
                up_count: packet.market_features.up_count,
                flat_count: packet.market_features.flat_count,
                down_count: packet.market_features.down_count,
                total_count: packet.market_features.total_count,
                up_weight: packet.market_features.up_weight,
                flat_weight: packet.market_features.flat_weight,
                down_weight: packet.market_features.down_weight,
                total_weight: packet.market_features.total_weight,
                config_hash,
                data_quality_status,
            };
            persistence.save_telemetry(&telemetry_row)?;
            transition_logger.log_transition(
                prev_packet.as_ref().map(|p| &p.market_regime),
                &packet.market_regime,
            )?;
            Ok(())
        })();

        match archival_result {
            Ok(_) => outcome.archival = crate::core::run_status::DeliveryStatus::Succeeded,
            Err(e) => {
                outcome.archival = crate::core::run_status::DeliveryStatus::Failed {
                    reason: e.to_string(),
                }
            }
        }

        let ledger = Arc::new(Ledger::new(save_dir.clone()));

        // --- Phase 7 & 8: Archival & Trading (Consolidated for Closure) ---

        // 1. Initialize TradeExecutor
        let trader_executor: Arc<Mutex<dyn TradeExecutor + Send + Sync>> = if provider_type
            == ProviderType::Futu
        {
            if let Some(futu_config) = &config_arc.futu {
                println!("🔌 [Daemon] Initializing context from LIVE FutuTrader adapter...");
                let addr = format!("{}:{}", futu_config.opend_ip, futu_config.opend_port);
                let futu_client = FutuClient::connect(&addr).await?;
                let trader = FutuTrader::new(Arc::new(futu_client), futu_config.clone());

                // Preflight: Ensure trading is unlocked at startup
                if config_arc
                    .trading
                    .as_ref()
                    .map(|t| t.enabled)
                    .unwrap_or(false)
                {
                    println!("🔑 [Preflight] Unlocking trading account...");
                    trader.unlock_trade().await?;
                }

                Arc::new(Mutex::new(trader))
            } else {
                println!("⚠️ [Daemon] Futu config missing, using MockTradeExecutor for archival context.");
                Arc::new(Mutex::new(crate::trade::trader::MockTradeExecutor::new()))
            }
        } else {
            println!("🧪 [Daemon] Initializing MockTradeExecutor for context...");
            Arc::new(Mutex::new(crate::trade::trader::MockTradeExecutor::new()))
        };

        // 2. Fetch context for ExecutionGate and Snapshots
        let funds = {
            let exec = trader_executor.lock().await;
            exec.get_funds().await?
        };
        let daily_traded = ledger.get_daily_traded_amount();
        let (realized_pl, positions) = ledger.get_portfolio_stats();
        let mut current_exposure = 0.0;
        for asset in &packet.assets {
            if let Some((qty, _avg)) = positions.get(&asset.symbol) {
                current_exposure += qty * asset.price;
            }
        }

        // 3. Run ExecutionGate regardless of execute_trades for AUDIT archival
        let default_trading = crate::config::TradingConfig {
            enabled: false,
            global_budget: 0.0,
            max_daily_budget: None,
        };
        let trading_config = config_arc.trading.as_ref().unwrap_or(&default_trading);
        let execution_result = ExecutionGate::gate_packet(
            &packet,
            trading_config,
            daily_traded,
            funds.power,
            current_exposure,
        );

        // 4. Save Execution Gate Audits
        for audit in &execution_result.audits {
            let log_entry = serde_json::to_value(audit)?;
            persistence.save_execution_gate_log(&log_entry)?;
        }

        // 5. Generate Account Snapshot
        let account_snapshot = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "date": packet.date.to_string(),
            "total_assets": funds.total_assets,
            "cash": funds.total_assets - current_exposure,
            "buying_power": funds.power,
            "market_value": current_exposure,
            "realized_pl": realized_pl
        });
        persistence.save_account_snapshot(&account_snapshot, &packet.date.to_string())?;

        // 6. Generate Portfolio Snapshot
        let mut pos_details = Vec::new();
        for asset in &packet.assets {
            if let Some((qty, avg_price)) = positions.get(&asset.symbol) {
                if *qty > 0.0 {
                    pos_details.push(serde_json::json!({
                        "symbol": asset.symbol,
                        "qty": qty,
                        "avg_price": avg_price,
                        "market_price": asset.price,
                        "market_value": qty * asset.price,
                        "unrealized_pl": (asset.price - avg_price) * qty
                    }));
                }
            }
        }
        let portfolio_snapshot = serde_json::json!({
            "date": packet.date.to_string(),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "total_market_value": current_exposure,
            "positions": pos_details
        });
        persistence.save_portfolio_snapshot(&portfolio_snapshot, &packet.date.to_string())?;

        // 7. Execution (Conditional)
        if mode == crate::core::runtime_mode::ExecutionMode::Live {
            let agent = TraderAgent::new(trader_executor.clone(), ledger.clone());

            println!("🚀 [TraderAgent] Dispatching gated signals for execution...");
            match agent.execute_signals(execution_result.trades.clone()).await {
                Ok(summary) => {
                    outcome.execution_details = Some(serde_json::to_value(summary.audits)?);
                    match summary.status {
                        Ok(_) => {
                            outcome.execution = crate::core::run_status::DeliveryStatus::Succeeded
                        }
                        Err(e) => {
                            outcome.execution = crate::core::run_status::DeliveryStatus::Failed {
                                reason: e.to_string(),
                            }
                        }
                    }
                }
                Err(e) => {
                    // preflight failure (unlock etc)
                    outcome.execution = crate::core::run_status::DeliveryStatus::Failed {
                        reason: e.to_string(),
                    };
                }
            }
        } else {
            println!(
                "💡 [{}] Mode: ExecutionGate logic completed for archival (trading bypassed)",
                mode
            );
            outcome.execution = crate::core::run_status::DeliveryStatus::Skipped;
        }

        let fetched_symbols: std::collections::HashSet<String> = ticker_histories
            .iter()
            .map(|(h, _)| h.symbol.to_string())
            .collect();
        let failed_symbols: Vec<String> = config_arc
            .watchlist
            .iter()
            .filter(|w| w.enable && !fetched_symbols.contains(&w.symbol))
            .map(|w| w.symbol.clone())
            .collect();

        let report_result = report::generate_refined_report(
            &config_arc,
            &packet,
            realized_pl,
            &positions,
            &mode,
            failed_symbols,
        )?;
        persistence
            .save_markdown_report(&report_result.archival_markdown, &packet.date.to_string())?;

        if let Some(ref tg_cfg) = config_arc.telegram {
            if tg_cfg.enabled {
                println!("📤 Sending report to Telegram...");
                match notify::send_telegram_message(tg_cfg, &report_result.markdown_body).await {
                    Ok(_) => {
                        outcome.notification = crate::core::run_status::DeliveryStatus::Succeeded
                    }
                    Err(e) => {
                        println!("❌ Telegram notification failed: {}", e);
                        outcome.notification = crate::core::run_status::DeliveryStatus::Failed {
                            reason: e.to_string(),
                        };
                    }
                }
            }
        }

        // Finalize Outcome and Save
        persistence.save_run_status(&outcome)?;

        // P0 Closure: Fail if critical parts failed (Execution, Notification, OR Archival)
        if mode == crate::core::runtime_mode::ExecutionMode::Live {
            if let crate::core::run_status::DeliveryStatus::Failed { reason } = outcome.execution {
                return Err(anyhow::anyhow!("Critical Execution Failure: {}", reason));
            }
        }

        if let crate::core::run_status::DeliveryStatus::Failed { reason } = outcome.notification {
            return Err(anyhow::anyhow!("Critical Notification Failure: {}", reason));
        }

        if let crate::core::run_status::DeliveryStatus::Failed { reason } = outcome.archival {
            return Err(anyhow::anyhow!("Critical Archival Failure: {}", reason));
        }
    }
    Ok(())
}

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
        "review" => {
            println!("🔍 状态机周复盘辅助：正在汇总近 7 日数据...");
            run_review(&app_config).await?;
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

    // Load history for Memory Layer (V1.3)
    let history = persistence.load_recent_packets(20).unwrap_or_default();
    let prev_packet = history.last();

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
        reconciliation: crate::core::run_status::DeliveryStatus::Skipped,
        reconciliation_report: None,
        data_quality: "PENDING".to_string(),
        execution_details: None,
        preflight: None,
        state_machine: None,
    };

    if !ticker_histories.is_empty() {
        // Core Decision Pipeline
        let packet = match Engine::run_daily_pipeline(&ticker_histories, &rules_arc, &history) {
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

        // Initialize StateMachineSummary for V1.3 Observation
        let mut sm_summary = crate::core::run_status::StateMachineSummary {
            from_state: format!(
                "{:?}",
                prev_packet
                    .as_ref()
                    .map(|p| p.market_regime.market_state)
                    .unwrap_or(crate::core::market_regime::MarketState::IGNITION)
            ),
            to_state: format!("{:?}", packet.market_regime.market_state),
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

                    println!("🔍 [Preflight] Checking broker quote permissions & quota...");
                    let res = trader.get_broker_permissions().await;
                    let mut preflight = crate::core::run_status::PreflightResult {
                        status: "Verified".to_string(),
                        sub_quota_used: 0,
                        sub_quota_total: 0,
                        market_rights: std::collections::HashMap::new(),
                    };

                    match res {
                        Ok(perms) => {
                            preflight.sub_quota_used = perms.sub_quota_used;
                            preflight.sub_quota_total = perms.sub_quota_total;
                            println!(
                                "📊 [Broker] Quota: {}/{} used.",
                                perms.sub_quota_used, perms.sub_quota_total
                            );

                            let required_quota =
                                config_arc.watchlist.iter().filter(|w| w.enable).count() as i32;
                            let remaining = perms.sub_quota_total - perms.sub_quota_used;

                            if required_quota > remaining {
                                let msg = format!(
                                    "Watchlist size ({}) exceeds remaining quota ({}).",
                                    required_quota, remaining
                                );
                                println!("⚠️ [Preflight] {}", msg);
                                preflight.status = "Warning".to_string();
                                if mode == crate::core::runtime_mode::ExecutionMode::Live {
                                    outcome.preflight = Some(preflight);
                                    let _ = persistence.save_run_status(&outcome);
                                    anyhow::bail!(
                                        "Insufficient subscription quota for Live trading: {}",
                                        msg
                                    );
                                }
                            }

                            for watch in config_arc.watchlist.iter().filter(|w| w.enable) {
                                let market_key = match watch.market.as_str() {
                                    "US" => "US",
                                    "HK" => "HK",
                                    "SH" => "SH",
                                    "SZ" => "SZ",
                                    _ => "US",
                                };

                                if let Some(right) = perms.market_rights.get(market_key) {
                                    use crate::trade::trader::MarketRight;
                                    preflight
                                        .market_rights
                                        .insert(market_key.to_string(), format!("{:?}", right));

                                    match right {
                                        MarketRight::BMP
                                        | MarketRight::None
                                        | MarketRight::Unknow => {
                                            let msg = format!("Market {} right is {:?}. No real-time subscription.", market_key, right);
                                            println!(
                                                "⚠️ [Preflight] Potential issue for {}: {}",
                                                watch.symbol, msg
                                            );
                                            preflight.status = "Warning".to_string();
                                            if mode
                                                == crate::core::runtime_mode::ExecutionMode::Live
                                            {
                                                outcome.preflight = Some(preflight);
                                                let _ = persistence.save_run_status(&outcome);
                                                anyhow::bail!(
                                                    "Insufficient market permissions for {}: {}",
                                                    watch.symbol,
                                                    msg
                                                );
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("⚠️ [Preflight] Failed to fetch broker permissions: {}", e);
                            preflight.status = "Failed".to_string();
                            if mode == crate::core::runtime_mode::ExecutionMode::Live {
                                outcome.preflight = Some(preflight);
                                let _ = persistence.save_run_status(&outcome);
                                anyhow::bail!("Broker permission check failed in Live mode: {}", e);
                            }
                        }
                    }
                    outcome.preflight = Some(preflight);
                    if let Some(sm) = outcome.state_machine.as_mut() {
                        sm.preflight_failed = outcome
                            .preflight
                            .as_ref()
                            .map(|p| p.status == "Failed")
                            .unwrap_or(false);
                    }
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
            exec.get_account_funds().await?
        };
        outcome.execution_details = None; // Initial state
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
        persistence.save_execution_gate_result(&packet, &execution_result)?;

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

            // 8. Position Reconciliation (Post-flight)
            println!("🔍 [Post-flight] Performing broker-side position reconciliation...");
            match agent.reconcile_positions().await {
                Ok(report) => {
                    if !report.mismatches.is_empty() {
                        println!("❌ [RECONCILIATION] Critical mismatches detected in Live mode!");
                        for m in &report.mismatches {
                            println!(
                                "   - {}: Local={} Broker={} Diff={}",
                                m.symbol, m.local_qty, m.broker_qty, m.diff
                            );
                        }
                        outcome.reconciliation = crate::core::run_status::DeliveryStatus::Failed {
                            reason: "Position mismatch detected between ledger and broker"
                                .to_string(),
                        };
                    } else {
                        println!("✅ [Post-flight] Reconciliation successful. Local ledger matches broker.");
                        outcome.reconciliation = crate::core::run_status::DeliveryStatus::Succeeded;
                    }
                    outcome.reconciliation_report = Some(report.clone());
                    if let Some(sm) = outcome.state_machine.as_mut() {
                        sm.reconciliation_mismatch_count = report.mismatches.len();
                    }
                }
                Err(e) => {
                    println!("❌ [Post-flight] Reconciliation API failure: {}", e);
                    outcome.reconciliation = crate::core::run_status::DeliveryStatus::Failed {
                        reason: format!("Broker API failure during reconciliation: {}", e),
                    };
                }
            }
        } else {
            println!(
                "💡 [{}] Mode: ExecutionGate logic completed for archival (trading bypassed)",
                mode
            );
            outcome.execution = crate::core::run_status::DeliveryStatus::Skipped;

            // In Dry-run, we can still reconcile if it's Futu type
            if provider_type == ProviderType::Futu {
                let agent = TraderAgent::new(trader_executor.clone(), ledger.clone());
                match agent.reconcile_positions().await {
                    Ok(report) => {
                        outcome.reconciliation_report = Some(report.clone());
                        outcome.reconciliation = crate::core::run_status::DeliveryStatus::Succeeded;
                        if let Some(sm) = outcome.state_machine.as_mut() {
                            sm.reconciliation_mismatch_count = report.mismatches.len();
                        }
                    }
                    Err(e) => {
                        outcome.reconciliation = crate::core::run_status::DeliveryStatus::Failed {
                            reason: e.to_string(),
                        };
                    }
                }
            }
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

        // P0 Closure: Fail if critical parts failed (Execution, Notification, Archival, OR Reconciliation)
        if mode == crate::core::runtime_mode::ExecutionMode::Live {
            if let crate::core::run_status::DeliveryStatus::Failed { reason } = outcome.execution {
                return Err(anyhow::anyhow!("Critical Execution Failure: {}", reason));
            }
            if let crate::core::run_status::DeliveryStatus::Failed { reason } =
                outcome.reconciliation
            {
                return Err(anyhow::anyhow!(
                    "Critical Reconciliation Failure: {}",
                    reason
                ));
            }
        }

        if let crate::core::run_status::DeliveryStatus::Failed { reason } = outcome.notification {
            return Err(anyhow::anyhow!("Critical Notification Failure: {}", reason));
        }

        if let crate::core::run_status::DeliveryStatus::Failed { reason } = outcome.archival {
            return Err(anyhow::anyhow!("Critical Archival Failure: {}", reason));
        }

        // Auto-run review summary after each successful pipeline run
        let _ = run_review(&config_arc).await;
    }
    Ok(())
}

async fn run_review(config: &crate::config::AppConfig) -> Result<()> {
    let save_dir = std::path::PathBuf::from(&config.output.save_to);

    // Scan reports directory for run_status_*.json
    let mut entries = std::fs::read_dir(&save_dir)?
        .filter_map(|res| res.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("run_status_") && name.ends_with(".json")
        })
        .collect::<Vec<_>>();

    entries.sort_by_key(|e| e.file_name());
    let last_7 = entries.iter().rev().take(7).rev().collect::<Vec<_>>();

    let mut summaries = Vec::new();
    let mut weekly_totals = serde_json::json!({
        "reset_confirmed_total": 0,
        "reset_blocked_total": 0,
        "soft_reset_total": 0,
        "duration_lock_total": 0,
        "defensive_override_total": 0,
        "core_breakdown_total": 0,
        "reconciliation_mismatch_total": 0,
    });

    let mut daily_items = Vec::new();

    for entry in last_7 {
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            if let Ok(outcome) =
                serde_json::from_str::<crate::core::run_status::RunOutcome>(&content)
            {
                if let Some(sm) = outcome.state_machine {
                    // Accumulate totals
                    if sm.reset_confirmed {
                        weekly_totals["reset_confirmed_total"] =
                            (weekly_totals["reset_confirmed_total"].as_i64().unwrap_or(0) + 1)
                                .into();
                    }
                    if sm.reset_blocked {
                        weekly_totals["reset_blocked_total"] =
                            (weekly_totals["reset_blocked_total"].as_i64().unwrap_or(0) + 1).into();
                    }
                    if sm.soft_reset_applied {
                        weekly_totals["soft_reset_total"] =
                            (weekly_totals["soft_reset_total"].as_i64().unwrap_or(0) + 1).into();
                    }
                    if sm.duration_locked {
                        weekly_totals["duration_lock_total"] =
                            (weekly_totals["duration_lock_total"].as_i64().unwrap_or(0) + 1).into();
                    }
                    if sm.defensive_override {
                        weekly_totals["defensive_override_total"] = (weekly_totals
                            ["defensive_override_total"]
                            .as_i64()
                            .unwrap_or(0)
                            + 1)
                        .into();
                    }
                    if sm.core_breakdown {
                        weekly_totals["core_breakdown_total"] =
                            (weekly_totals["core_breakdown_total"].as_i64().unwrap_or(0) + 1)
                                .into();
                    }
                    weekly_totals["reconciliation_mismatch_total"] = (weekly_totals
                        ["reconciliation_mismatch_total"]
                        .as_i64()
                        .unwrap_or(0)
                        + sm.reconciliation_mismatch_count as i64)
                        .into();

                    summaries.push(serde_json::json!({
                        "date": outcome.date,
                        "summary": sm
                    }));
                    daily_items.push((outcome.date.clone(), sm));
                }
            }
        }
    }

    let today = chrono::Local::now().date_naive().to_string();
    let weekly_summary = serde_json::json!({
        "week_ending": today,
        "weekly_totals": weekly_totals,
        "daily_summaries": summaries
    });

    // 1. Write JSON
    let json_path = save_dir.join("weekly_state_metrics.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&weekly_summary)?)?;

    // 2. Generate Markdown Draft
    let mut md = "# Weekly State Machine Review (Auto-Draft)\n\n".to_string();
    md.push_str(&format!("**Period Ending**: {}\n\n", today));

    md.push_str("## 📊 Weekly Totals\n\n");
    md.push_str("| Metric | Value |\n|---|---|\n");
    md.push_str(&format!(
        "| Reset Confirmed | {} |\n",
        weekly_totals["reset_confirmed_total"]
    ));
    md.push_str(&format!(
        "| Reset Blocked (Sensitive) | {} |\n",
        weekly_totals["reset_blocked_total"]
    ));
    md.push_str(&format!(
        "| Duration Locks | {} |\n",
        weekly_totals["duration_lock_total"]
    ));
    md.push_str(&format!(
        "| Soft Resets | {} |\n",
        weekly_totals["soft_reset_total"]
    ));
    md.push_str(&format!(
        "| Defensive Overrides | {} |\n",
        weekly_totals["defensive_override_total"]
    ));
    md.push_str(&format!(
        "| Core Breakdowns | {} |\n",
        weekly_totals["core_breakdown_total"]
    ));
    md.push_str(&format!(
        "| Recon Mismatches | {} |\n",
        weekly_totals["reconciliation_mismatch_total"]
    ));
    md.push('\n');

    md.push_str("## 🗓️ Daily Timeline\n\n");
    md.push_str("| Date | Transition | Events |\n|---|---|---|\n");
    for (date, sm) in &daily_items {
        let mut events = Vec::new();
        if sm.reset_confirmed {
            events.push("✅ Reset");
        }
        if sm.reset_blocked {
            events.push("🚫 Blocked");
        }
        if sm.soft_reset_applied {
            events.push("🧠 SoftReset");
        }
        if sm.duration_locked {
            events.push("🔒 Locked");
        }
        if sm.defensive_override {
            events.push("🛡️ Override");
        }
        if sm.reconciliation_mismatch_count > 0 {
            events.push("⚠️ Mismatch");
        }

        md.push_str(&format!(
            "| {} | `{:?}` -> `{:?}` | {} |\n",
            date,
            sm.from_state,
            sm.to_state,
            events.join(", ")
        ));
    }
    md.push('\n');

    md.push_str("## 🚩 Auto-flagged Anomalies\n");
    let anomalies = daily_items
        .iter()
        .filter(|(_, sm)| {
            sm.reset_confirmed
                || sm.reset_blocked
                || sm.defensive_override
                || sm.reconciliation_mismatch_count > 0
        })
        .collect::<Vec<_>>();

    if anomalies.is_empty() {
        md.push_str("- No critical anomalies automatically detected this week.\n");
    } else {
        for (date, sm) in anomalies {
            md.push_str(&format!("### {} Anomaly\n", date));
            if sm.reset_confirmed {
                md.push_str("- **Triggered Reset**: Logic decided to clear state.\n");
            }
            if sm.reset_blocked {
                md.push_str("- **Blocked Reset**: Reset condition met but duration/confidence gate intervened.\n");
            }
            if sm.defensive_override {
                md.push_str(
                    "- **Defensive Override**: Forced state downgrade due to safety rules.\n",
                );
            }
            if sm.reconciliation_mismatch_count > 0 {
                md.push_str(&format!(
                    "- **Recon Mismatch**: {} differences between local and broker state.\n",
                    sm.reconciliation_mismatch_count
                ));
            }
            md.push_str("\n> [!NOTE]\n> **Human Explanation Needed**: (Please describe the market context here)\n\n");
        }
    }

    md.push_str("\n## 🧠 Manual Review Needed\n\n");
    md.push_str("### 1. Sensitivity Assessment\n");
    md.push_str("- [ ] System is behaving as expected\n");
    md.push_str("- [ ] System is over-sensitive (Too many resets/locks)\n");
    md.push_str("- [ ] System is sluggish (Missed crucial transitions)\n\n");

    md.push_str("### 2. Logic Feedback\n");
    md.push_str("(Optional: Notes on any specific rules that need tuning)\n\n");

    md.push_str("### 3. V1.4 Recommendation\n");
    md.push_str("- **Stay in V1.3 Observation** (Wait for more data)\n");
    md.push_str("- **Proceed to V1.4 Parameter Convergence** (Refine thresholds)\n");

    let md_path = save_dir.join("weekly_state_review_auto.md");
    std::fs::write(&md_path, md)?;

    println!("✅ 已生成复盘指标汇总: {:?}", json_path);
    println!("📝 已生成复盘底稿草案: {:?}", md_path);

    Ok(())
}

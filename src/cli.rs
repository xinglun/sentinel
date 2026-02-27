use anyhow::{anyhow, Result};
use chrono::{Local, Utc};
use futures::stream::{self, StreamExt};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::Mutex;

use crate::backtest;
use crate::config;
use crate::core::engine::{self, TickerSnapshot};
use crate::core::ledger::Ledger;
use crate::core::notify;
use crate::core::report;
use crate::core::trader_agent::TraderAgent;
use crate::data::provider::MarketDataProvider;
use crate::trade::trader::TradeExecutor;

// Conditionally import Futu adapter (it requires Tokio runtime and TCP stream)
use crate::adapters::futu::client::FutuClient;
use crate::adapters::futu::provider::FutuProvider;
use crate::adapters::futu::trader::FutuTrader;

#[derive(Clone, Copy, PartialEq)]
enum ProviderType {
    Yahoo,
    Futu,
}

pub async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let mut command = "radar";
    let mut provider_type = ProviderType::Yahoo;
    let mut futu_addr = "127.0.0.1:11111".to_string();

    // Simple argument parsing
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "backtest" => command = "backtest",
            "daemon" | "trade" => command = "daemon",
            "radar" => command = "radar",
            "--provider" => {
                if i + 1 < args.len() {
                    if args[i + 1].to_lowercase() == "futu" {
                        provider_type = ProviderType::Futu;
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
            let mut from_date = "2015-01-01".to_string();
            let mut to_date = "2025-01-01".to_string();

            let mut iter = args.iter().skip(1);
            while let Some(arg) = iter.next() {
                if arg == "--from" {
                    if let Some(val) = iter.next() {
                        from_date = val.clone();
                    }
                } else if arg == "--to" {
                    if let Some(val) = iter.next() {
                        to_date = val.clone();
                    }
                }
            }
            let app_config = config::AppConfig::load("config.toml")?;
            // For backtest, regime_age is not directly applicable from a live telemetry.csv
            // It's usually calculated within the backtest simulation itself.
            // The `trend_maturity` line seems to be a misplaced instruction for `run_radar`.
            // Keeping it commented out or removed as it would cause a compile error.
            // let trend_maturity = (regime_age as f64 / 40.0).min(1.0);
            backtest::run_backtest(&app_config, &from_date, &to_date).await?;
        }
        "daemon" => {
            println!("🤖 哨兵守卫：交易守护进程启动 (Daemon Mode)");
            println!(
                "   数据引擎: {:?}",
                if provider_type == ProviderType::Futu {
                    "Moomoo OpenD"
                } else {
                    "Yahoo Finance"
                }
            );

            let app_config = config::AppConfig::load("config.toml")?;

            if provider_type == ProviderType::Futu {
                let futu_cfg = app_config
                    .futu
                    .clone()
                    .ok_or_else(|| anyhow!("Missing [futu] config section"))?;

                println!("🔌 正在建立协议封装和心跳机制...");
                let client = Arc::new(FutuClient::connect(&futu_addr).await?);
                let provider = Arc::new(FutuProvider::new(client.clone()));

                println!("✅ 成功连接至 Moomoo OpenD ({})", futu_addr);

                let trader = FutuTrader::new(client.clone(), futu_cfg);

                println!("🔑 正在尝试鉴权与解锁交易核心...");
                match trader.unlock_trade().await {
                    Ok(_) => println!("✅ 交易授权解锁成功。"),
                    Err(e) => println!("⚠️ 交易授权未解锁或失败 (通常仅支持读取模式): {}", e),
                }

                println!("💰 查询本地网关账户资金情况...");
                match trader.get_funds().await {
                    Ok(funds) => println!(
                        "   -> 现金: ${:.2}, 购买力: ${:.2}, 总资产: ${:.2}",
                        funds.cash, funds.power, funds.total_assets
                    ),
                    Err(e) => println!("   -> 获取账户资金失败: {}", e),
                }

                println!("🛡️ 哨兵自动化交易模块挂载完毕，进入监听循环...");

                let trader_arc: Arc<Mutex<dyn TradeExecutor + Send + Sync>> =
                    Arc::new(Mutex::new(trader));

                // Initialize the persistent ledger to prevent duplicate daily trades
                let ledger = Arc::new(Ledger::new(std::path::PathBuf::from(
                    &app_config.output.save_to,
                )));

                let trader_agent =
                    TraderAgent::new(Arc::new(app_config.clone()), trader_arc.clone(), ledger);
                let rules_arc = Arc::new(app_config.get_parsed_rules());

                // Keep the daemon alive and executing
                loop {
                    println!(
                        "\n▶️ [Daemon] {} - 开始本轮行情拉取与策略评估...",
                        Local::now().format("%Y-%m-%d %H:%M:%S")
                    );

                    let mut current_snapshots = Vec::new();

                    for entry in app_config.watchlist.iter().filter(|w| w.enable) {
                        match provider.fetch_history(&entry.symbol, None, None).await {
                            Ok(history) => {
                                let snapshot =
                                    engine::evaluate_snapshot(&history, entry, &rules_arc, None);
                                current_snapshots.push(snapshot);
                            }
                            Err(e) => {
                                println!("❌ [Daemon] 无法拉取 {} 行情: {}", entry.symbol, e);
                            }
                        }
                    }

                    if !current_snapshots.is_empty() {
                        let prev_context = report::GravityPrevContext {
                            prev_margin: None,
                            prev_exposure: None,
                            prev_up_count: None,
                            prev_ema_accel: None,
                            prev_system_confidence: None,
                            regime_age: 0,
                        };
                        let gravity_health = report::GravityHealth::compute(
                            &current_snapshots,
                            "daemon",
                            &prev_context,
                            0, // Initial macro for daemon doesn't have yesterday's regime age info easily available
                        );

                        if let Err(e) = trader_agent
                            .execute_signals(&current_snapshots, &gravity_health)
                            .await
                        {
                            println!("❌ [Daemon] 交易代理执行信号失败: {}", e);
                        }
                    } else {
                        println!("⚠️ [Daemon] 本轮未获取到任何有效的行情快照。");
                    }

                    println!("⏳ [Daemon] 评估结束。等待下一次心跳周期 (60s)...");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            } else {
                println!("⚠️ Daemon 模式建议使用 --provider futu 配合本地网关运行以获得最新实盘数据和报单支持。");
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    println!("💓 Daemon Heartbeat (Yahoo/Offline Mode)...");
                }
            }
        }
        _ => {
            // default radar
            println!("🐕 Stock Sentinel initializing (Radar Mode)...");

            let provider: Arc<dyn MarketDataProvider> = match provider_type {
                ProviderType::Futu => {
                    println!("🔌 尝试通过 Moomoo OpenD ({}) 获取行情...", futu_addr);
                    match FutuClient::connect(&futu_addr).await {
                        Ok(client) => Arc::new(FutuProvider::new(Arc::new(client))),
                        Err(e) => {
                            println!(
                                "❌ 无法连接至 Moomoo OpenD: {}。将自动降级使用 Yahoo Finance。",
                                e
                            );
                            Arc::new(YahooProviderAdapter)
                        }
                    }
                }
                ProviderType::Yahoo => Arc::new(YahooProviderAdapter),
            };

            run_radar(provider).await?;
        }
    }

    Ok(())
}

// Wrapper to make existing yahoo_provider fit the MarketDataProvider trait
struct YahooProviderAdapter;

#[async_trait::async_trait]
impl MarketDataProvider for YahooProviderAdapter {
    async fn fetch_history(
        &self,
        symbol: &str,
        start_date: Option<OffsetDateTime>,
        end_date: Option<OffsetDateTime>,
    ) -> Result<crate::data::yahoo_provider::TickerHistory> {
        crate::data::yahoo_provider::fetch_history(symbol, start_date, end_date).await
    }
}

async fn run_radar(provider: Arc<dyn MarketDataProvider>) -> Result<()> {
    let app_config = config::AppConfig::load("config.toml")?;
    let parsed_rules = app_config.get_parsed_rules();

    let config_content = std::fs::read_to_string("config.toml").unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(config_content.as_bytes());
    let config_hash = format!("{:x}", hasher.finalize())[..8].to_string();

    let config_arc = Arc::new(app_config);
    let rules_arc = Arc::new(parsed_rules);

    let watchlist = &config_arc.watchlist;
    let enabled_count = watchlist.iter().filter(|w| w.enable).count();

    let mut yesterday_state = "Unknown".to_string();
    let mut prev_margin = None;
    let mut prev_exposure = None;
    let mut prev_up_count = None;
    let mut prev_ema_accel = None;
    let mut prev_system_confidence = None;

    if let Ok(content) = std::fs::read_to_string(
        std::path::Path::new(&config_arc.output.save_to).join("telemetry.csv"),
    ) {
        let mut historical_margins = Vec::new();
        for line in content.lines().skip(1) {
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() > 12 {
                if let Ok(m) = cols[12].parse::<f64>() {
                    historical_margins.push(m);
                }
            }
        }

        if historical_margins.len() > 1 {
            let mut accelerations = Vec::new();
            for i in 1..historical_margins.len() {
                accelerations.push(historical_margins[i] - historical_margins[i - 1]);
            }
            if !accelerations.is_empty() {
                let mut ema = accelerations[0];
                let alpha = 2.0 / (5.0 + 1.0);
                for &acc in accelerations.iter().skip(1) {
                    ema = alpha * acc + (1.0 - alpha) * ema;
                }
                prev_ema_accel = Some(ema);
            }
        }

        if let Some(last_line) = content.lines().last() {
            let cols: Vec<&str> = last_line.split(',').collect();
            if cols.len() > 12 {
                yesterday_state = cols[3].to_string();
                prev_margin = cols[12].parse::<f64>().ok();
                if cols.len() > 13 {
                    prev_exposure = cols[13].parse::<f64>().ok();
                }
                if cols.len() > 15 {
                    if let (Ok(universe), Ok(up_share)) =
                        (cols[14].parse::<f64>(), cols[15].parse::<f64>())
                    {
                        prev_up_count = Some((universe * up_share).round() as usize);
                    }
                }
                if cols.len() > 16 {
                    // Assuming system confidence is at index 16
                    prev_system_confidence = cols[16].parse::<f64>().ok();
                }
            }
        }
    }

    println!("📊 Fetching data for {} enabled assets...", enabled_count);

    let mut snapshots = Vec::new();
    let mut quote_timestamps = Vec::new();
    use crate::data::sentiment::SentimentProvider;
    let sentiment_provider: Arc<dyn SentimentProvider> = if let Some(ref fh) = config_arc.finnhub {
        println!("🧠 Using Finnhub Sentiment API...");
        Arc::new(crate::data::sentiment::FinnhubSentimentProvider::new(
            fh.api_key.clone(),
        ))
    } else {
        println!("🧠 Using Mock Sentiment API (No config found)...");
        Arc::new(crate::data::sentiment::MockSentimentProvider::new())
    };

    let fetches = stream::iter(watchlist.iter().filter(|w| w.enable))
        .map(|entry| {
            let rules_ref = Arc::clone(&rules_arc);
            let provider_ref = Arc::clone(&provider);
            let sentiment_provider_ref = Arc::clone(&sentiment_provider);
            async move {
                let symbol = &entry.symbol;
                let sentiment_score = sentiment_provider_ref.fetch_sentiment(symbol).await.ok();

                match provider_ref.fetch_history(symbol, None, None).await {
                    Ok(history) => {
                        let latest_ts = history.latest_quote_timestamp;
                        let snapshot =
                            engine::evaluate_snapshot(&history, entry, &rules_ref, sentiment_score);
                        (Some(snapshot), latest_ts)
                    }
                    Err(e) => {
                        println!("[ERROR] Error processing {}: {}", symbol, e);
                        let err_snap = TickerSnapshot {
                            symbol: symbol.clone(),
                            name: entry.name.clone().unwrap_or_else(|| symbol.clone()),
                            weight: entry.weight.unwrap_or(1.0),
                            reason_code: Some("[API ERROR]".to_string()),
                            current_date: chrono::Local::now().date_naive(),
                            dog_price: 0.0,
                            owner_ma: None,
                            leash_ma: None,
                            owner_ma_slope_pct: None,
                            dev_z_score: None,
                            curvature: None,
                            confidence_score: 0,
                            trend_status: engine::TrendStatus::Unknown,
                            deviation_pct: None,
                            deviation_basis_used: format!("{:?}", entry.deviation_basis)
                                .to_lowercase(),
                            state_code: "ERROR".to_string(),
                            action_text: format!("Fetch failed: {}", e),
                            is_bear_mode_active: false,
                            is_caution_mode_active: false,
                            trend_age: 0,
                            owner_deviation_pct: None,
                            deviation_percentile: None,
                            validity: engine::RegimeValidity::Invalid,
                            history_days: 0,
                            sentiment: None,
                        };
                        (Some(err_snap), None)
                    }
                }
            }
        })
        .buffer_unordered(10);

    let mut results_map = std::collections::HashMap::new();
    let results: Vec<(Option<TickerSnapshot>, Option<i64>)> = fetches.collect().await;

    for (res_opt, ts_opt) in results {
        if let Some(res) = res_opt {
            results_map.insert(res.symbol.clone(), res);
        }
        if let Some(ts) = ts_opt {
            quote_timestamps.push(ts);
        }
    }

    for entry in watchlist.iter().filter(|w| w.enable) {
        if let Some(snap) = results_map.remove(&entry.symbol) {
            snapshots.push(snap);
        }
    }

    if !snapshots.is_empty() {
        let temp_prev = report::GravityPrevContext {
            prev_margin,
            prev_exposure,
            prev_up_count,
            prev_ema_accel,
            prev_system_confidence,
            regime_age: 0, // This will be updated after first pass
        };

        // Pass 1: Determine current state to calculate regime age
        let temp_gravity = report::GravityHealth::compute(&snapshots, &config_hash, &temp_prev, 0);
        let posture = temp_gravity.compute_capital_posture();
        let regime_age = calculate_regime_age(
            std::path::Path::new(&config_arc.output.save_to),
            &posture.state_code,
        );

        // Pass 2: Final calculation with correct regime age
        let prev_context = report::GravityPrevContext {
            prev_margin,
            prev_exposure,
            prev_up_count,
            prev_ema_accel,
            prev_system_confidence: None,
            regime_age,
        };
        let gravity_health =
            report::GravityHealth::compute(&snapshots, &config_hash, &prev_context, regime_age);

        let ledger = Ledger::new(std::path::PathBuf::from(&config_arc.output.save_to));
        let (realized_pl, positions) = ledger.get_portfolio_stats();

        let report_result = report::generate_reports(
            &config_arc,
            &snapshots,
            &gravity_health,
            &yesterday_state,
            realized_pl,
            &positions,
        )?;

        if let Some(ref tg_cfg) = config_arc.telegram {
            if !tg_cfg.bot_token.is_empty() {
                let combined_output =
                    format!("{}{}", report_result.markdown, report_result.telegram_html);
                if combined_output.contains(&tg_cfg.bot_token) {
                    panic!("FATAL SECURITY ERROR: bot_token leak detected in reports!");
                }
            }
        }

        let mut max_age_minutes: Option<i64> = None;
        if !quote_timestamps.is_empty() {
            let now = Utc::now().timestamp();
            let mut min_ts = quote_timestamps[0];
            for &ts in &quote_timestamps {
                if ts < min_ts {
                    min_ts = ts;
                }
            }
            let age_sec = now - min_ts;
            max_age_minutes = Some(age_sec / 60);
        }

        let freshness_data = serde_json::json!({
            "max_age_minutes": max_age_minutes,
            "stale": max_age_minutes.is_none_or(|age| age > 15),
            "timestamp_utc": Utc::now().to_rfc3339(),
        });

        if let Ok(freshness_content) = serde_json::to_string_pretty(&freshness_data) {
            let freshness_path =
                std::path::Path::new(&config_arc.output.save_to).join("freshness.json");
            let _ = std::fs::write(freshness_path, freshness_content);
        }

        println!("✅ Report generated: {}", config_arc.output.save_to);

        if let Some(ref tg_cfg) = config_arc.telegram {
            if tg_cfg.enabled {
                println!("📤 Sending report to Telegram...");
                if let Err(e) =
                    notify::send_telegram_message(tg_cfg, &report_result.telegram_html).await
                {
                    println!("❌ Failed to send Telegram message: {}", e);
                }
            }
        }
    } else {
        println!("⚠️ No valid data found. Report generation skipped.");
    }

    Ok(())
}

fn calculate_regime_age(save_dir: &std::path::Path, current_state: &str) -> usize {
    let telemetry_path = save_dir.join("telemetry.csv");
    if let Ok(content) = std::fs::read_to_string(telemetry_path) {
        let mut lines: Vec<&str> = content.lines().collect();
        if lines.len() <= 1 {
            return 1;
        }

        let mut age = 1;
        let mut last_date = "";

        lines.remove(0);

        for line in lines.iter().rev() {
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() > 3 {
                let date = cols[0];
                let state = cols[3];

                if state == current_state {
                    if date != last_date {
                        age += 1;
                        last_date = date;
                    }
                } else {
                    break;
                }
            }
        }
        age
    } else {
        1
    }
}

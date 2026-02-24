mod config;
mod engine;
mod fetcher;
mod report;
mod notify;
mod backtest;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use crate::engine::TickerSnapshot;
use sha2::{Sha256, Digest};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "backtest" {
        println!("🔬 资本望远镜：回测模式启动 (Backtest Mode)");
        let mut from_date = "2015-01-01".to_string();
        let mut to_date = "2025-01-01".to_string();
        
        let mut iter = args.iter().skip(2);
        while let Some(arg) = iter.next() {
            if arg == "--from" {
                if let Some(val) = iter.next() { from_date = val.clone(); }
            } else if arg == "--to" {
                if let Some(val) = iter.next() { to_date = val.clone(); }
            }
        }
        
        // Pass to backtest logic
        let app_config = config::AppConfig::load("config.toml")?;
        backtest::run_backtest(&app_config, &from_date, &to_date).await?;
        return Ok(());
    }

    println!("🐕 Stock Sentinel 起床中...");
    
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
    
    // --- Phase 28: Deep Telemetry Parsing for Delta Tracking ---
    let mut yesterday_state = "Unknown".to_string();
    let mut prev_gp = None;
    let mut prev_margin = None;
    let mut prev_exposure = None;
    let mut prev_up_count = None;
    let mut prev_ema_accel = None;

    if let Ok(content) = std::fs::read_to_string(std::path::Path::new(&config_arc.output.save_to).join("telemetry.csv")) {
        let mut historical_margins = Vec::new();
        for line in content.lines().skip(1) { // Skip header
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
                accelerations.push(historical_margins[i] - historical_margins[i-1]);
            }
            if !accelerations.is_empty() {
                let mut ema = accelerations[0];
                let alpha = 2.0 / (5.0 + 1.0); // 5-day EMA
                for &acc in accelerations.iter().skip(1) {
                    ema = alpha * acc + (1.0 - alpha) * ema;
                }
                prev_ema_accel = Some(ema);
            }
        }

        if let Some(last_line) = content.lines().last() {
            let cols: Vec<&str> = last_line.split(',').collect();
            if cols.len() > 12 {
                yesterday_state = cols[3].to_string(); // state_code
                prev_gp = cols[6].parse::<f64>().ok();
                prev_margin = cols[12].parse::<f64>().ok();
                // If we added exposure column (index 13), parse it
                if cols.len() > 13 {
                    prev_exposure = cols[13].parse::<f64>().ok();
                }
                // extract prev_up_count from count_up_share if possible (we need the raw count, 
                // but since telemetry in V4 only has share, we might have to reconstruct it or accept it's not there.
                // Wait, if it wasn't recorded, we can just leave it as None for the first run, 
                // but let's parse count_up_share (index 15 in V4.2) and multiply by watchlist_size? 
                // Actually, let's just parse the 15th column if available and use it as an approximation or None.
                // In telemetry.csv, index 14 is watchlist_size, index 15 is count_up_share.
                if cols.len() > 15 {
                    if let (Ok(universe), Ok(up_share)) = (cols[14].parse::<f64>(), cols[15].parse::<f64>()) {
                        prev_up_count = Some((universe * up_share).round() as usize);
                    }
                }
            }
        }
    }

    println!("📊 {} 個の有効な銘柄のデータを取得しています...", enabled_count);
    
    let mut snapshots = Vec::new();
    
    let fetches = stream::iter(watchlist.iter().filter(|w| w.enable))
        .map(|entry| {
            let rules_ref = Arc::clone(&rules_arc);
            async move {
                let symbol = &entry.symbol;
                match fetcher::fetch_history(symbol, None, None).await {
                    Ok(history) => {
                        let snapshot = engine::evaluate_snapshot(&history, entry, &rules_ref);
                        Some(snapshot)
                    },
                    Err(e) => {
                        println!("[エラー] {} の処理中にエラーが発生しました: {}", symbol, e);
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
                            deviation_basis_used: format!("{:?}", entry.deviation_basis).to_lowercase(),
                            state_code: "ERROR".to_string(),
                            action_text: format!("取得失敗: {}", e),
                            is_bear_mode_active: false,
                            is_caution_mode_active: false,
                            trend_age: 0,
                            owner_deviation_pct: None,
                            deviation_percentile: None,
                            validity: engine::RegimeValidity::Invalid,
                            history_days: 0,
                        };
                        Some(err_snap)
                    }
                }
            }
        })
        .buffer_unordered(10);
        
    let mut results_map = std::collections::HashMap::new();
    let results: Vec<Option<TickerSnapshot>> = fetches.collect().await;
    
    for res in results.into_iter().flatten() {
        results_map.insert(res.symbol.clone(), res);
    }
    
    // config.toml で定義された元の順序を維持します
    for entry in watchlist.iter().filter(|w| w.enable) {
        if let Some(snap) = results_map.remove(&entry.symbol) {
            snapshots.push(snap);
        }
    }
    
    if !snapshots.is_empty() {
        let mut up_count = 0;
        let mut flat_count = 0;
        let mut down_count = 0;
        let mut up_weight = 0.0;
        let mut flat_weight = 0.0;
        let mut down_weight = 0.0;
        let mut forming_early_count = 0;
        let mut forming_late_count = 0;
        let mut forming_early_weight = 0.0;
        let mut forming_late_weight = 0.0;

        for s in &snapshots {
            if s.validity == engine::RegimeValidity::FormingEarly || s.validity == engine::RegimeValidity::FormingLate {
                if s.validity == engine::RegimeValidity::FormingEarly {
                    forming_early_count += 1;
                    forming_early_weight += s.weight;
                } else {
                    forming_late_count += 1;
                    forming_late_weight += s.weight;
                }
                continue; // --- MACRO CLEANSE: Strictly exclude all forming assets from global identifiers ---
            }

            match s.trend_status {
                engine::TrendStatus::Up => {
                    up_count += 1;
                    up_weight += s.weight;
                },
                engine::TrendStatus::Flat => {
                    flat_count += 1;
                    flat_weight += s.weight;
                },
                engine::TrendStatus::Down | engine::TrendStatus::Unknown => {
                    down_count += 1;
                    down_weight += s.weight;
                },
            }
        }
        
        let total_count = up_count + flat_count + down_count;
        let total_weight = up_weight + flat_weight + down_weight;
        
        let mut total_strength_sum = 0.0;
        let mut weight_for_strength = 0.0;
        
        // Potential Energy
        let mut total_potential_sum = 0.0;
        let mut weight_for_potential = 0.0;
        
        // Capital State Allocations
        let mut trend_alloc_weight = 0.0;
        let mut reversion_alloc_weight = 0.0;
        
        for s in &snapshots {
            // --- MACRO CLEANSE: Exclude Forming assets from Physics logic ---
            if s.validity == engine::RegimeValidity::FormingEarly || s.validity == engine::RegimeValidity::FormingLate {
                continue;
            }

            // Only aggregate strength if valid
            if let Some(strength) = s.owner_ma_slope_pct {
                total_strength_sum += strength * s.weight;
                weight_for_strength += s.weight;
            }
            if let Some(z) = s.dev_z_score {
                total_potential_sum += z.abs() * s.weight;
                weight_for_potential += s.weight;
                
                // Capital State Allocation
                if s.confidence_score >= 80 {
                    trend_alloc_weight += s.weight * (s.confidence_score as f64 / 100.0);
                } else if s.confidence_score <= 60 {
                    reversion_alloc_weight += s.weight * ((100.0 - s.confidence_score as f64) / 100.0) * z.abs();
                }
            }
        }
        
        // global_gravity_strength 
        let global_gravity_strength = if weight_for_strength > 0.0 {
            total_strength_sum / weight_for_strength
        } else {
            0.0
        };
        
        // global_potential_energy
        let global_potential_energy = if weight_for_potential > 0.0 {
            total_potential_sum / weight_for_potential
        } else {
            0.0
        };

        // --- Phase 25: Composite Macro Metrics ---
        // --- Phase 25: Composite Macro Metrics ---
        let dominance_margin = (trend_alloc_weight - reversion_alloc_weight) / total_weight; // Normalized margin
        let conf_trend_alloc = (trend_alloc_weight / total_weight * 50.0).clamp(0.0, 50.0);
        let conf_inverse_potential = (1.0 / (1.0 + global_potential_energy) * 50.0).clamp(0.0, 50.0);
        let confidence_score = conf_trend_alloc + conf_inverse_potential;
        let system_confidence = (confidence_score.clamp(0.0, 100.0) * 100.0).round() / 100.0;

        let market_phase = if dominance_margin > 0.5 {
            if global_gravity_strength > 0.5 { "Mid Bull" } else { "Late Bull" }
        } else if dominance_margin > 0.2 {
            "Early Bull"
        } else if dominance_margin < -0.5 {
            "Bear Market"
        } else if dominance_margin < -0.1 {
            "Correction"
        } else {
            "Neutral / Transition"
        };

        // 1. Calculate true finalized margin from energy adjustment
        let t_share_raw = trend_alloc_weight / total_weight;
        let r_share_raw = reversion_alloc_weight / total_weight;
        let r_share_adj = r_share_raw * (1.0 + global_potential_energy);
        let total_adjusted = t_share_raw + r_share_adj;
        let t_ratio_final = t_share_raw / total_adjusted;
        let r_ratio_final = r_share_adj / total_adjusted;
        let energy_adjusted_margin = t_ratio_final - r_ratio_final;

        // 2. Compute 5-day EMA of Acceleration
        let mut capital_flow_acceleration = None;
        if let Some(pm) = prev_margin {
            let today_accel = energy_adjusted_margin - pm;
            let ema_accel = match prev_ema_accel {
                Some(prev_ema) => {
                    let alpha = 2.0 / (5.0 + 1.0);
                    alpha * today_accel + (1.0 - alpha) * prev_ema
                },
                None => today_accel
            };
            capital_flow_acceleration = Some(ema_accel);
        }

        // --- Phase 43: Semantic Calibration of Flow Vectors (Momentum State) ---
        // Velocity = energy_adjusted_margin
        // Acceleration = capital_flow_acceleration (5-day EMA)
        let capital_flow_vector = if energy_adjusted_margin > 0.0 {
            let acc = capital_flow_acceleration.unwrap_or(0.0);
            if acc.abs() < 0.02 { "Stable Uptrend ↗️" }
            else if acc >= 0.02 { "Accelerating Uptrend 🚀" }
            else { "Decelerating Uptrend ⚠️" }
        } else {
            let acc = capital_flow_acceleration.unwrap_or(0.0);
            if acc.abs() < 0.02 { "Stable Downtrend ↘️" }
            else if acc <= -0.02 { "Accelerating Downtrend 🩸" }
            else { "Decelerating Downtrend (Bottoming) ⏳" }
        };

        // Base exposure relies solely on trend direction (dominance margin)
        let base_exposure = (0.5 + (dominance_margin * 0.5)).clamp(0.0, 1.0);
        let mut adjusted_exposure = base_exposure;

        let mut temp_health = report::GravityHealth {
            up_count,
            flat_count,
            down_count,
            total_count,
            up_weight,
            flat_weight,
            down_weight,
            total_weight,
            global_gravity_strength,
            global_potential_energy,
            trend_alloc_weight,
            reversion_alloc_weight,
            config_hash,
            system_confidence,
            market_phase: market_phase.to_string(),
            capital_flow_vector: capital_flow_vector.to_string(),
            recommended_exposure: 0.0, // Placeholder
            forming_early_count,
            forming_late_count,
            forming_early_weight,
            forming_late_weight,
            universe_count: snapshots.len(),
            prev_potential_energy: prev_gp,
            prev_system_confidence: None, 
            prev_dominance_margin: prev_margin,
            prev_recommended_exposure: prev_exposure,
            prev_up_count,
            regime_age: 0,
            stability_score: 0.0,
            base_exposure,
            adjusted_exposure,
            conf_trend_alloc,
            conf_inverse_potential,
            capital_flow_acceleration,
            universe_integrity: if snapshots.len() > 0 { total_count as f64 / snapshots.len() as f64 } else { 0.0 },
            trend_maturity: 0.0,
            stability_structural: 0.0,
            stability_temporal: 0.0,
            temporal_modifier: 1.0,
            integrity_multiplier: 1.0,
        };
        let posture = temp_health.compute_capital_posture();
        let regime_age = calculate_regime_age(std::path::Path::new(&config_arc.output.save_to), &posture.state_code);
        temp_health.regime_age = regime_age;
        
        let trend_maturity = (regime_age as f64 / 40.0).min(1.0);
        temp_health.trend_maturity = trend_maturity;
        
        let stability_structural = conf_inverse_potential / 50.0;
        let stability_temporal = trend_maturity;
        
        temp_health.stability_structural = conf_inverse_potential; // Raw percentage
        temp_health.stability_temporal = stability_temporal * 100.0; // Raw percentage
        
        let stability_score = stability_structural * stability_temporal;
        temp_health.stability_score = stability_score; // 0.0 to 1.0 range

        let temporal_modifier = 0.85 + (trend_maturity * 0.15).min(0.15);
        temp_health.temporal_modifier = temporal_modifier;
        
        let integrity_multiplier = temp_health.universe_integrity;
        temp_health.integrity_multiplier = integrity_multiplier;

        // Final Exposure Calculation
        let conf_multiplier = (system_confidence / 100.0) * integrity_multiplier;
        let mut final_exposure = base_exposure * conf_multiplier * temporal_modifier;
        final_exposure = final_exposure.max(0.0).min(1.0);

        adjusted_exposure = final_exposure;
        
        temp_health.adjusted_exposure = adjusted_exposure;
        // Also update recommended_exposure to match adjusted_exposure for legacy compat
        temp_health.recommended_exposure = adjusted_exposure;

        let gravity_health = temp_health;

        let report_result = report::generate_reports(&config_arc, &snapshots, &gravity_health, &yesterday_state)?;

        // Phase 8: Hard block on Telegram Token leaks
        if let Some(ref tg_cfg) = config_arc.telegram {
            let combined_output = format!("{}{}", report_result.markdown, report_result.telegram_html);
            if combined_output.contains(&tg_cfg.bot_token) {
                panic!("FATAL SECURITY ERROR: bot_token leak detected in reports!");
            }
        }

        println!("✅ レポートが提供されました: {}", config_arc.output.save_to);
        
        if let Some(ref tg_cfg) = config_arc.telegram {
            if tg_cfg.enabled {
                println!("📤 Telegramにレポートを送信中...");
                if let Err(e) = notify::send_telegram_message(tg_cfg, &report_result.telegram_html).await {
                    println!("❌ Telegramメッセージの送信に失敗しました: {}", e);
                }
            }
        }
        
    } else {
        println!("⚠️ 有効なデータが見つからなかったため、レポートは生成されませんでした。");
    }
    
    Ok(())
}

fn calculate_regime_age(save_dir: &std::path::Path, current_state: &str) -> usize {
    let telemetry_path = save_dir.join("telemetry.csv");
    if let Ok(content) = std::fs::read_to_string(telemetry_path) {
        let mut lines: Vec<&str> = content.lines().collect();
        if lines.len() <= 1 { return 1; }
        
        let mut age = 1;
        let mut last_date = "";
        
        // Skip header
        lines.remove(0);
        
        // Scan backwards to find consecutive days with the same state_code
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

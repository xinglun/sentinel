use crate::config::AppConfig;
use crate::engine::{TickerSnapshot, TrendStatus};
use anyhow::Result;
use chrono::Local;
use std::fs;
use std::path::Path;
use tabled::{Table, Tabled};
use tabled::settings::Style;

#[derive(Tabled)]
struct TerminalRow {
    #[tabled(rename = "銘柄")]
    symbol: String,
    #[tabled(rename = "趋势 (天数)")]
    trend: String,
    #[tabled(rename = "Owner (乖离)")]
    owner_dev: String,
    #[tabled(rename = "强度 (Z)")]
    strength_z: String,
    #[tabled(rename = "状态 (置信度)")]
    state: String,
    #[tabled(rename = "行动建议")]
    action: String,
}

pub struct ReportResult {
    #[allow(dead_code)]
    pub markdown: String,
    pub telegram_html: String,
}


pub struct GravityHealth {
    pub up_count: usize,
    pub flat_count: usize,
    pub down_count: usize,
    pub total_count: usize, // Also watchlist_size
    pub up_weight: f64,
    pub flat_weight: f64,
    pub down_weight: f64,
    pub total_weight: f64,
    pub global_gravity_strength: f64,
    pub global_potential_energy: f64,
    pub trend_alloc_weight: f64,
    pub reversion_alloc_weight: f64,
    pub config_hash: String,
    pub system_confidence: f64,
    pub market_phase: String,
    pub capital_flow_vector: String,
    pub recommended_exposure: f64,
    // Delta trackers (Change vs Yesterday)
    pub prev_potential_energy: Option<f64>,
    pub prev_system_confidence: Option<f64>,
    pub prev_dominance_margin: Option<f64>,
}

pub struct CapitalPosture {
    pub state_code: String,
    pub display_text: String,
    pub t_share_raw: f64,
    pub r_share_raw: f64,
    pub r_share_adj: f64,
    pub t_ratio_final: f64,
    pub r_ratio_final: f64,
    pub dominance_label: String,
}

impl GravityHealth {
    pub fn format_count_health(&self) -> String {
        if self.total_count == 0 {
            return "0% UP / 0% FLAT / 0% DOWN".to_string();
        }
        let up_pct = (self.up_count as f64 / self.total_count as f64 * 100.0).round() as usize;
        let flat_pct = (self.flat_count as f64 / self.total_count as f64 * 100.0).round() as usize;
        let down_pct = 100 - up_pct - flat_pct; // Avoid 101% or 99% logic with simple subtraction
        format!("{}% UP / {}% FLAT / {}% DOWN", up_pct, flat_pct, down_pct)
    }

    pub fn format_weight_health(&self) -> String {
        if self.total_weight <= 0.0 {
            return "0% UP / 0% FLAT / 0% DOWN".to_string();
        }
        let up_pct = (self.up_weight / self.total_weight * 100.0).round() as usize;
        let flat_pct = (self.flat_weight / self.total_weight * 100.0).round() as usize;
        let down_pct = 100 - up_pct - flat_pct;
        format!("{}% UP / {}% FLAT / {}% DOWN", up_pct, flat_pct, down_pct)
    }

    pub fn format_potential_energy(&self) -> String {
        let (label, intensity) = if self.global_potential_energy < 1.0 {
            ("Cold", "(安定 / 波动极低)")
        } else if self.global_potential_energy < 1.5 {
            ("Warm", "(蓄力 / 趋势健康但警惕波动)")
        } else if self.global_potential_energy < 2.0 {
            ("Hot", "(高张力 / 极端背离，波动即将来临)")
        } else {
            ("Critical", "(极端 / 风险极高，严防剧烈修正)")
        };
        format!("{:.2} {} {}", self.global_potential_energy, label, intensity)
    }

    /// Linear interpolation for smooth Potential Modifier
    /// 1.0 -> 0.7 (Discount)
    /// 1.5 -> 1.0 (Neutral)
    /// 2.0 -> 1.3 (Amplify)
    fn get_potential_mod(&self) -> f64 {
        let p = self.global_potential_energy;
        if p <= 1.0 { 0.7 }
        else if p >= 2.0 { 1.3 }
        else if p < 1.5 {
            0.7 + (p - 1.0) * (1.0 - 0.7) / (1.5 - 1.0)
        } else {
            1.0 + (p - 1.5) * (1.3 - 1.0) / (2.0 - 1.5)
        }
    }
    
    pub fn compute_capital_posture(&self) -> CapitalPosture {
        if self.trend_alloc_weight == 0.0 && self.reversion_alloc_weight == 0.0 {
            return CapitalPosture {
                state_code: "NULL".to_string(),
                display_text: "Transitional (Null)".to_string(),
                t_share_raw: 0.0,
                r_share_raw: 0.0,
                r_share_adj: 0.0,
                t_ratio_final: 0.0,
                r_ratio_final: 0.0,
                dominance_label: "Null".to_string(),
            };
        }
        
        let t_raw = self.trend_alloc_weight;
        let r_raw = self.reversion_alloc_weight;
        let total_raw = t_raw + r_raw;
        
        let t_share_raw = t_raw / total_raw;
        let r_share_raw = r_raw / total_raw;
        
        // 2. Continuous Potential Modifier
        let mod_factor = self.get_potential_mod();
        let r_share_adj = r_share_raw * mod_factor;
        
        // 3. Re-normalization
        let mod_total = t_share_raw + r_share_adj;
        let final_trend_ratio = t_share_raw / mod_total;
        let final_reversion_ratio = 1.0 - final_trend_ratio;
        
        let (state_code, display_text) = if final_trend_ratio >= 0.6 {
            ("TREND_DOMINANT", "Trend Dominant (趋势主导 / 复利优先，谨慎加速)")
        } else if final_trend_ratio <= 0.4 {
            ("REVERSION_DOMINANT", "Reversion Dominant (回归主导 / 分批部署，控制仓位)")
        } else {
            ("TRANSITIONAL", "Transitional (结构转换期 / 防御优先，等待确认)")
        };

        let margin = (final_trend_ratio - final_reversion_ratio).abs();
        let dominance_label = if margin < 0.2 {
            "Neutral"
        } else if margin < 0.5 {
            "Weak"
        } else if margin < 0.8 {
            "Strong"
        } else {
            "Absolute"
        };

        CapitalPosture {
            state_code: state_code.to_string(),
            display_text: display_text.to_string(),
            t_share_raw,
            r_share_raw,
            r_share_adj,
            t_ratio_final: final_trend_ratio,
            r_ratio_final: final_reversion_ratio,
            dominance_label: dominance_label.to_string(),
        }
    }

    pub fn get_interpretation(&self, posture: &CapitalPosture) -> String {
        match posture.state_code.as_str() {
            "TREND_DOMINANT" => {
                if self.global_gravity_strength > 0.0 {
                    "📡 Interpretation: 趋势强劲主导。强者继续复利，避免频繁调仓，回撤即机会。".to_string()
                } else {
                    "📡 Interpretation: 趋势仍主导但引力减速。保持仓位但由于动能衰减，严禁追高。".to_string()
                }
            },
            "REVERSION_DOMINANT" => {
                if self.global_potential_energy > 1.8 {
                    "📡 Interpretation: 极端背离导向。结构性超卖/超买严重，分批部署/防御而非追跌杀涨。".to_string()
                } else {
                    "📡 Interpretation: 均值回归主导。震荡格局，避免趋势交易逻辑，关注边缘突破。".to_string()
                }
            },
            "TRANSITIONAL" => {
                "📡 Interpretation: 结构转换期。引力方向不明联，防御优先，等待新体制确立。".to_string()
            },
            _ => "系统状态观测中。".to_string()
        }
    }
}

fn get_z_label(z: f64) -> &'static str {
    let abs_z = z.abs();
    if abs_z < 1.0 { "Neutral" }
    else if abs_z < 2.0 { "Strong" }
    else if abs_z < 3.0 { "Extreme" }
    else { "Panic" }
}

fn get_action_category(state: &str) -> &'static str {
    if state.contains("fear") && !state.contains("down") { "加仓区 (Buy)" }
    else if state.contains("pullback") { "加仓区 (Buy)" }
    else if state.contains("optimal") || state.contains("cruise") { "持有区 (Hold)" }
    else if state.contains("overheat") || state.contains("DEFEND") || state.contains("caution") || (state.contains("fear") && state.contains("down")) { "防御区 (Defend)" }
    else { "观察区 (Watch)" }
}

fn get_rank_priority(state: &str) -> usize {
    if state == "optimal" || state == "cruise" { 3 }
    else if state == "pullback" { 2 }
    else if state.contains("fear") && !state.contains("down") { 1 } // Opportunity fear
    else if state.contains("fear") && state.contains("down") { 5 } // Risk fear
    else if state.contains("overheat") || state.contains("DEFEND") { 6 }
    else { 4 } 
}

fn format_thermometer(value: f64, max: f64) -> String {
    let width = 10;
    let filled = ((value / max) * width as f64).round() as usize;
    let filled = filled.clamp(0, width);
    let mut bar = String::new();
    for _ in 0..filled { bar.push('█'); }
    for _ in filled..width { bar.push('░'); }
    
    let scale = "LOW      MEDIUM        HIGH";
    let markers = "0.0      1.0           2.0";
    format!("{}\n{}\n{} {:.2} / {:.1}", scale, markers, bar, value, max)
}

fn format_sigma(z: f64) -> String {
    let label = if z >= 0.0 { "above equilibrium" } else { "below equilibrium" };
    format!("{:.1}σ {}", z.abs(), label)
}

fn get_position_guidance(state: &str) -> &'static str {
    if state.contains("fear") && !state.contains("down") { "Allocation: +15% (Load)" }
    else if state == "pullback" { "Allocation: +10% (Buy)" }
    else if state == "optimal" { "Target: 100% (Stay Efficient)" }
    else if state == "cruise" { "Target: 100% (Trend Follow)" }
    else if state.contains("overheat") { "Target: 60-80% (Trim)" }
    else if state.contains("down") || state.contains("DEFEND") { "Target: 0-20% (Avoid/Cash)" }
    else { "Neutral" }
}

fn get_final_order(gravity: &GravityHealth, posture: &CapitalPosture, buy_zone: &[String]) -> String {
    let mut orders = Vec::new();
    
    // Command 1: Exposure
    orders.push(format!("保持 {:.0}% 的权益仓位暴露 (Recommended Exposure).", gravity.recommended_exposure * 100.0));
    
    // Command 2: Addition
    if !buy_zone.is_empty() {
        orders.push(format!("仅在回调中增加头寸 (Deploy into: {}).", buy_zone.join(", ")));
    } else {
        orders.push("目前无高弹性加仓机会点，停止新开仓.".to_string());
    }
    
    // Command 3: Defense
    if posture.state_code.contains("Risk") || posture.state_code.contains("Panic") {
        orders.push("严禁在下降趋势或恐慌情绪中抄底 (No Bottom-fishing).".to_string());
    } else {
        orders.push("严禁在趋势末端追高 (Do not chase strength).".to_string());
    }
    
    orders.join("\n • ")
}

fn get_state_emoji(state: &str) -> &'static str {
    if state.starts_with("optimal") || state.starts_with("cruise") { "🟢" }
    else if state.starts_with("pullback") || state.contains("caution") || state.starts_with("CAUTION") { "🟡" }
    else if state.starts_with("overheat") || state.starts_with("fear") || state.starts_with("DEFEND") { "🔴" }
    else { "⚪" }
}

pub fn generate_reports(config: &AppConfig, snapshots: &[TickerSnapshot], gravity_health: &GravityHealth, yesterday_state: &str) -> Result<ReportResult> {
    let now = Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let posture = gravity_health.compute_capital_posture();
    
    let mut snapshots = snapshots.to_vec();
    // Sorting: Opportunity-First (Fear -> Pullback -> Optimal -> Cruise)
    snapshots.sort_by(|a, b| {
        let pa = get_rank_priority(&a.state_code);
        let pb = get_rank_priority(&b.state_code);
        if pa != pb {
            pa.cmp(&pb)
        } else {
            // Tie-break by |Z-score| descending for similar states
            let az = a.dev_z_score.unwrap_or(0.0).abs();
            let bz = b.dev_z_score.unwrap_or(0.0).abs();
            bz.partial_cmp(&az).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    let mut rows = Vec::new();
    let mut buy_zone = Vec::new();
    let mut watch_zone = Vec::new();
    let mut defend_zone = Vec::new();
    let mut hold_zone = Vec::new();

    for s in &snapshots {
        let cat = get_action_category(&s.state_code);
        match cat {
            "加仓区 (Buy)" => buy_zone.push(s.symbol.clone()),
            "观察区 (Watch)" => watch_zone.push(s.symbol.clone()),
            "防御区 (Defend)" => defend_zone.push(s.symbol.clone()),
            _ => hold_zone.push(s.symbol.clone()),
        }

        let trend_icon = match s.trend_status {
            TrendStatus::Up => "↗",
            TrendStatus::Down => "↘",
            TrendStatus::Flat => "→",
            TrendStatus::Unknown => "?",
        };
        let trend_combined = format!("{} ({}d)", trend_icon, s.trend_age);
        
        let owner_dev_str = if let (Some(om), Some(dev)) = (s.owner_ma, s.owner_deviation_pct) {
            format!("{:.2} ({:+.2}%)", om, dev)
        } else {
            "-".to_string()
        };

        let _strength_str = s.owner_ma_slope_pct.map(|v| format!("{:+.2}%", v)).unwrap_or_else(|| "-".to_string());
        let z_val = s.dev_z_score.unwrap_or(0.0);
        let strength_z_combined = format!("{} ({})", format_sigma(z_val), get_z_label(z_val));
        
        let emoji = get_state_emoji(&s.state_code);
        let state_rc = if let Some(rc) = &s.reason_code {
            format!("{} {} {}", emoji, s.state_code, rc)
        } else {
            format!("{} {}", emoji, s.state_code)
        };
        
        rows.push(TerminalRow {
            symbol: s.symbol.clone(),
            trend: trend_combined,
            owner_dev: owner_dev_str,
            strength_z: strength_z_combined,
            state: state_rc,
            action: get_position_guidance(&s.state_code).to_string(), // Strategic guidance instead of action text
        });
    }

    // Rankings
    let mut opportunities = snapshots.iter()
        .filter(|s| (s.state_code.contains("fear") && !s.state_code.contains("down")) || s.state_code.contains("pullback"))
        .collect::<Vec<_>>();
    opportunities.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    let mut risks = snapshots.iter()
        .filter(|s| s.state_code.contains("overheat") || (s.state_code.contains("fear") && s.state_code.contains("down")) || s.state_code.contains("DEFEND"))
        .collect::<Vec<_>>();
    risks.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    // SPY Regime
    let spy_regime = snapshots.iter().find(|s| s.symbol == "SPY").map(|s| {
        if s.state_code.contains("optimal") || s.state_code.contains("cruise") { "Bull Stable" }
        else if s.state_code.contains("fear") || s.state_code.contains("pullback") { "Correction" }
        else if s.trend_status == TrendStatus::Down { "Bear / Crash" }
        else { "Uncertain" }
    }).unwrap_or("Unknown");

    // Previous state comparison
    let yesterday_state = yesterday_state;
    let _velocity_label = if yesterday_state == posture.state_code { "Stability maintained" } else { "Shift detected" };
    
    let raw_label = config.output.weight_kind.clone().unwrap_or_else(|| "Portfolio".to_string());
    let binding = raw_label.to_lowercase();
    let _weight_kind_label = match binding.as_str() {
        "portfolio" => "Portfolio",
        "cap" | "mktcap" => "MktCap",
        "risk" => "Risk",
        _ => &raw_label, 
    };
    
    let mut table = Table::new(rows);
    table.with(Style::modern());
    
    let dominance_margin = posture.t_ratio_final - posture.r_ratio_final;
    let market_structure = format!("{} ({})", gravity_health.market_phase, spy_regime);

    println!("🌍 Macro Indicators (全域监测)");
    println!(" • CAPITAL STATE: {}", posture.display_text);
    println!(" • System Confidence: {}%", gravity_health.system_confidence);
    println!(" • Capital Flow Vector: {}", gravity_health.capital_flow_vector);
    println!(" • Market Structure: {}", market_structure);
    println!(" • Dominance Margin: {:+.2} ({})", dominance_margin, posture.dominance_label);
    println!(" • GRAVITY POTENTIAL: {} ({})", format_thermometer(gravity_health.global_potential_energy, 3.0), gravity_health.format_potential_energy());
    println!("\n> 📡 Interpretation: {}", gravity_health.get_interpretation(&posture));

    println!("\n🎯 Tactical Summary (今日行动要领)");
    println!(" • 加仓区: {}", buy_zone.join(" / "));
    println!(" • 观察区: {}", watch_zone.join(" / "));
    println!(" • 防御区: {}", defend_zone.join(" / "));
    println!(" • 持有区: {}", hold_zone.join(" / "));

    println!("\n🔥 Highest Opportunity");
    for (i, s) in opportunities.iter().take(3).enumerate() {
        println!(" {}. {} ({} / {})", i+1, s.symbol, s.state_code, format_sigma(s.dev_z_score.unwrap_or(0.0)));
    }
    
    println!("\n☠️ Highest Risk");
    for (i, s) in risks.iter().take(3).enumerate() {
        println!(" {}. {} ({} / {})", i+1, s.symbol, s.state_code, format_sigma(s.dev_z_score.unwrap_or(0.0)));
    }

    println!("\n🎯 Execution Radar (個別銘柄レーダー)");
    println!(" > Sorted by: Extreme Opportunities (Fear) > Pullbacks > Optimal > Cruise > Risks/Stable\n");
    println!("{}", table);

    let md_content = generate_markdown(config, &snapshots, &date_str, gravity_health, &posture);
    let tg_html = generate_telegram_html(config, &snapshots, &date_str, gravity_health, &posture);

    if !config.output.save_to.is_empty() {
        fs::create_dir_all(&config.output.save_to)?;
        
        let json_path = Path::new(&config.output.save_to).join(format!("{}.json", date_str));
        let json_content = serde_json::to_string_pretty(&snapshots)?;
        fs::write(json_path, json_content)?;
        
        let md_path = Path::new(&config.output.save_to).join(format!("{}.md", date_str));
        fs::write(&md_path, &md_content)?;
        
        // --- Phase 26: Data Freshness Guard ---
        // If the data date is significantly older than today (e.g. > 3 days on a weekday),
        // it means the API hasn't updated yet. We still show the report but skip telemetry.
        let data_date = snapshots.first().map(|s| s.current_date).unwrap_or_else(|| Local::now().date_naive());
        let today = Local::now().date_naive();
        let days_diff = (today - data_date).num_days();
        
        let should_write_telemetry = if days_diff > 3 {
            println!("⚠️ [WARNING] Data date ({}) is too old. Skipping telemetry to avoid pollution.", data_date);
            false
        } else {
            true
        };

        if should_write_telemetry {
            // --- 📊 Telemetry System Heartbeat (V3 Ultimate Schema) ---
            let telemetry_path = Path::new(&config.output.save_to).join("telemetry.csv");
            let file_exists = telemetry_path.exists();
            
            let up_share = if gravity_health.total_count == 0 { 0.0 } else { gravity_health.up_count as f64 / gravity_health.total_count as f64 };
            let flat_share = if gravity_health.total_count == 0 { 0.0 } else { gravity_health.flat_count as f64 / gravity_health.total_count as f64 };
            let down_share = 1.0 - up_share - flat_share;
            
            let w_up_share = if gravity_health.total_weight <= 0.0 { 0.0 } else { gravity_health.up_weight / gravity_health.total_weight };
            let w_flat_share = if gravity_health.total_weight <= 0.0 { 0.0 } else { gravity_health.flat_weight / gravity_health.total_weight };
            let w_down_share = if gravity_health.total_weight <= 0.0 { 0.0 } else { gravity_health.down_weight / gravity_health.total_weight };

            let timestamp = Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

            let dominance_margin = posture.t_ratio_final - posture.r_ratio_final;

            // Ultimate Schema (19 Columns): 
            // date,timestamp,config_hash,state_code,state_text,gs,gp,t_raw,r_raw,r_adj,t_final,r_final,margin,size,c_up,c_flat,c_down,w_up,w_flat,w_down
            let telemetry_row = format!("{},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4}\n",
                date_str,
                timestamp,
                gravity_health.config_hash,
                posture.state_code,
                posture.display_text,
                gravity_health.global_gravity_strength,
                gravity_health.global_potential_energy,
                posture.t_share_raw,
                posture.r_share_raw,
                posture.r_share_adj,
                posture.t_ratio_final,
                posture.r_ratio_final,
                dominance_margin,
                gravity_health.total_count, // watchlist_size
                up_share,
                flat_share,
                down_share,
                w_up_share,
                w_flat_share,
                w_down_share
            );

            if !file_exists {
                let header = "date,timestamp,config_hash,state_code,state_text,gravity_strength,gravity_potential,t_share_raw,r_share_raw,r_share_adj,t_ratio_final,r_ratio_final,dominance_margin,watchlist_size,count_up_share,count_flat_share,count_down_share,weight_up_share,weight_flat_share,weight_down_share\n";
                fs::write(&telemetry_path, format!("{}{}", header, telemetry_row))?;
            } else {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new().append(true).open(&telemetry_path)?;
                file.write_all(telemetry_row.as_bytes())?;
            }
        }
    }
    
    Ok(ReportResult {
        markdown: md_content,
        telegram_html: tg_html,
    })
}

fn generate_markdown(_config: &AppConfig, snapshots: &[TickerSnapshot], date_str: &str, gravity: &GravityHealth, posture: &CapitalPosture) -> String {
    let mut snapshots = snapshots.to_vec();
    snapshots.sort_by(|a, b| {
        let pa = get_rank_priority(&a.state_code);
        let pb = get_rank_priority(&b.state_code);
        if pa != pb {
            pa.cmp(&pb)
        } else {
            let az = a.dev_z_score.unwrap_or(0.0).abs();
            let bz = b.dev_z_score.unwrap_or(0.0).abs();
            bz.partial_cmp(&az).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    let mut buy_zone = Vec::new();
    let mut watch_zone = Vec::new();
    let mut defend_zone = Vec::new();
    let mut hold_zone = Vec::new();
    for s in &snapshots {
        let cat = get_action_category(&s.state_code);
        match cat {
            "加仓区 (Buy)" => buy_zone.push(s.symbol.clone()),
            "观察区 (Watch)" => watch_zone.push(s.symbol.clone()),
            "防御区 (Defend)" => defend_zone.push(s.symbol.clone()),
            _ => hold_zone.push(s.symbol.clone()),
        }
    }

    let mut opportunities = snapshots.iter()
        .filter(|s| (s.state_code.contains("fear") && !s.state_code.contains("down")) || s.state_code.contains("pullback"))
        .collect::<Vec<_>>();
    opportunities.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    let mut risks = snapshots.iter()
        .filter(|s| s.state_code.contains("overheat") || (s.state_code.contains("fear") && s.state_code.contains("down")) || s.state_code.contains("DEFEND"))
        .collect::<Vec<_>>();
    risks.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    let spy_regime = snapshots.iter().find(|s| s.symbol == "SPY").map(|s| {
        if s.state_code.contains("optimal") || s.state_code.contains("cruise") { "Bull Stable" }
        else if s.state_code.contains("fear") || s.state_code.contains("pullback") { "Correction" }
        else if s.trend_status == TrendStatus::Down { "Bear / Crash" }
        else { "Uncertain" }
    }).unwrap_or("Unknown");

    let dominance_margin = posture.t_ratio_final - posture.r_ratio_final;
    let market_structure = format!("{} ({})", gravity.market_phase, spy_regime);

    let mut md = format!("# 🐕 Stock Sentinel 每日観測レーダー\n📅 **日付**: {}\n\n", date_str);
    
    md.push_str("## 🌍 Macro Indicators (全域状态监测)\n");
    md.push_str(&format!("- **CAPITAL STATE**: {}\n", posture.display_text));
    
    // Delta for System Confidence
    let conf_delta = gravity.prev_system_confidence.map(|p| gravity.system_confidence - p);
    let conf_str = if let Some(d) = conf_delta {
        format!("{}% (Δ {:+.2}%)", gravity.system_confidence, d)
    } else {
        format!("{}%", gravity.system_confidence)
    };
    md.push_str(&format!("- **System Confidence**: {}\n", conf_str));
    
    md.push_str(&format!("- **Capital Flow Vector**: {}\n", gravity.capital_flow_vector));
    md.push_str(&format!("- **Market Structure**: {}\n", market_structure));
    
    // Delta for Dominance Margin
    let margin_delta = gravity.prev_dominance_margin.map(|p| dominance_margin - p);
    let margin_str = if let Some(d) = margin_delta {
        format!("{:+.2} (Δ {:+.2})", dominance_margin, d)
    } else {
        format!("{:+.2}", dominance_margin)
    };
    md.push_str(&format!("- **Dominance Margin**: {} ({} / 趋势统治力)\n", margin_str, posture.dominance_label));
    md.push_str(&format!("- **Recommended Exposure**: **{:.0}%** (Range: 70–100%)\n", gravity.recommended_exposure * 100.0));
    md.push_str(&format!("- **GRAVITY POTENTIAL**:\n```\n{}\n```\n({})\n\n", format_thermometer(gravity.global_potential_energy, 2.0), gravity.format_potential_energy()));

    md.push_str(&format!("> 📡 Interpretation: {}\n\n", gravity.get_interpretation(posture)));

    md.push_str("## 🧭 今日执行指令 (Final Order)\n");
    md.push_str(&format!("- **Command**:\n • {}\n\n", get_final_order(gravity, posture, &buy_zone)));

    md.push_str("## 🎯 Tactical Summary (今日行动要领)\n");
    md.push_str("### 今日行动摘要\n");
    md.push_str(&format!("- **加仓区 (Buy)**: {}\n", buy_zone.join(" / ")));
    md.push_str(&format!("- **观察区 (Watch)**: {}\n", watch_zone.join(" / ")));
    md.push_str(&format!("- **防御区 (Defend)**: {}\n", defend_zone.join(" / ")));
    md.push_str(&format!("- **持有区 (Hold)**: {}\n\n", hold_zone.join(" / ")));

    md.push_str("### 🔥 Highest Opportunity (均值回归弹性)\n");
    for (i, s) in opportunities.iter().take(3).enumerate() {
        let z = s.dev_z_score.unwrap_or(0.0);
        let elasticity = if z.abs() > 2.0 { "High" } else if z.abs() > 1.0 { "Medium" } else { "Normal" };
        let bias = if z < 0.0 { "Mean Reversion ↑" } else { "Mean Reversion ↓" };
        md.push_str(&format!("{}. **{}** ({} / {} / Elasticity: {} / Bias: {})\n", i+1, s.symbol, s.state_code, format_sigma(z), elasticity, bias));
    }
    md.push_str("\n### ☠️ Highest Risk (系统性偏离风险)\n");
    for (i, s) in risks.iter().take(3).enumerate() {
        let z = s.dev_z_score.unwrap_or(0.0);
        let bias = if z > 2.0 { "Overheat Correction ↓" } else { "Trend Breakdown ↓" };
        md.push_str(&format!("{}. **{}** ({} / {} / Bias: {})\n", i+1, s.symbol, s.state_code, format_sigma(z), bias));
    }
    md.push_str("\n");

    md.push_str("## 🎯 個別銘柄レーダー (Execution Radar)\n");
    md.push_str("> ℹ️ *Sorted by: Extreme Opportunities (Fear) > Pullbacks > Optimal > Cruise > Risks/Stable*\n\n");
    md.push_str("| # | 銘柄 | 状态 | Position Size Guidance | 强度 (Sigma) | 趋势 (天数) | 行动建议 |\n");
    md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
    
    for (idx, s) in snapshots.iter().enumerate() {
        let trend_icon = match s.trend_status {
            TrendStatus::Up => "↗️",
            TrendStatus::Down => "↘️",
            TrendStatus::Flat => "➡️",
            TrendStatus::Unknown => "❓",
        };
        let trend_combined = format!("{} ({}d)", trend_icon, s.trend_age);
        
        let z_val = s.dev_z_score.unwrap_or(0.0);
        let z_label = get_z_label(z_val);
        let strength_z_combined = format!("{} ({})", format_sigma(z_val), z_label);
        
        let emoji = get_state_emoji(&s.state_code);
        let state_name = if let Some(rc) = &s.reason_code {
            format!("{} {} {}", emoji, s.state_code, rc)
        } else {
            format!("{} {}", emoji, s.state_code)
        };
        
        md.push_str(&format!("| {} | **{}** | {} | **{}** | {} | {} | {} |\n",
            idx + 1,
            s.symbol,
            state_name,
            get_position_guidance(&s.state_code),
            strength_z_combined,
            trend_combined,
            s.action_text
        ));
    }
    
    md
}

fn generate_telegram_html(_config: &AppConfig, snapshots_raw: &[TickerSnapshot], date_str: &str, gravity: &GravityHealth, posture: &CapitalPosture) -> String {
    let mut snapshots = snapshots_raw.to_vec();
    snapshots.sort_by(|a, b| {
        let pa = get_rank_priority(&a.state_code);
        let pb = get_rank_priority(&b.state_code);
        if pa != pb {
            pa.cmp(&pb)
        } else {
            let az = a.dev_z_score.unwrap_or(0.0).abs();
            let bz = b.dev_z_score.unwrap_or(0.0).abs();
            bz.partial_cmp(&az).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    let mut buy_zone = Vec::new();
    let mut watch_zone = Vec::new();
    let mut defend_zone = Vec::new();
    let mut hold_zone = Vec::new();
    for s in &snapshots {
        let cat = get_action_category(&s.state_code);
        match cat {
            "加仓区 (Buy)" => buy_zone.push(s.symbol.clone()),
            "观察区 (Watch)" => watch_zone.push(s.symbol.clone()),
            "防御区 (Defend)" => defend_zone.push(s.symbol.clone()),
            _ => hold_zone.push(s.symbol.clone()),
        }
    }

    let mut opportunities = snapshots.iter()
        .filter(|s| (s.state_code.contains("fear") && !s.state_code.contains("down")) || s.state_code.contains("pullback"))
        .collect::<Vec<_>>();
    opportunities.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    let mut risks = snapshots.iter()
        .filter(|s| s.state_code.contains("overheat") || (s.state_code.contains("fear") && s.state_code.contains("down")) || s.state_code.contains("DEFEND"))
        .collect::<Vec<_>>();
    risks.sort_by(|a, b| b.dev_z_score.unwrap_or(0.0).abs().partial_cmp(&a.dev_z_score.unwrap_or(0.0).abs()).unwrap());

    let spy_regime = snapshots.iter().find(|s| s.symbol == "SPY").map(|s| {
        if s.state_code.contains("optimal") || s.state_code.contains("cruise") { "Bull Stable" }
        else if s.state_code.contains("fear") || s.state_code.contains("pullback") { "Correction" }
        else if s.trend_status == TrendStatus::Down { "Bear / Crash" }
        else { "Uncertain" }
    }).unwrap_or("Unknown");

    let dominance_margin = posture.t_ratio_final - posture.r_ratio_final;
    let market_structure = format!("{} ({})", gravity.market_phase, spy_regime);

    let mut html = format!("🐕 <b>Stock Sentinel レーダー</b>\n📅 <b>日付:</b> {}\n\n", date_str);

    html.push_str("<b>🌍 Macro Indicators</b>\n");
    html.push_str(&format!(" • CAPITAL STATE: <code>{}</code>\n", posture.display_text));
    
    let conf_delta = gravity.prev_system_confidence.map(|p| gravity.system_confidence - p);
    let conf_str = if let Some(d) = conf_delta {
        format!("<code>{}%</code> (Δ <code>{:+.2}%</code>)", gravity.system_confidence, d)
    } else {
        format!("<code>{}%</code>", gravity.system_confidence)
    };
    html.push_str(&format!(" • System Confidence: {}\n", conf_str));
    html.push_str(&format!(" • Capital Flow Vector: <code>{}</code>\n", gravity.capital_flow_vector));
    html.push_str(&format!(" • Market Structure: <code>{}</code>\n", market_structure));
    
    let margin_delta = gravity.prev_dominance_margin.map(|p| dominance_margin - p);
    let margin_str = if let Some(d) = margin_delta {
        format!("<code>{:+.2}</code> (Δ <code>{:+.2}</code>)", dominance_margin, d)
    } else {
        format!("<code>{:+.2}</code>", dominance_margin)
    };
    html.push_str(&format!(" • Dominance Margin: {} (<i>{}</i>)\n", margin_str, posture.dominance_label));
    html.push_str(&format!(" • Recommended Exposure: <b>{:.0}%</b>\n", gravity.recommended_exposure * 100.0));
    html.push_str(&format!(" • GRAVITY POTENTIAL: <code>{}</code>\n\n", format_thermometer(gravity.global_potential_energy, 2.0).replace("\n", " | ")));

    html.push_str(&format!("<i>📡 Interpretation: {}</i>\n\n", gravity.get_interpretation(posture)));

    html.push_str("<b>🧭 今日执行指令 (Final Order)</b>\n");
    html.push_str(&format!(" • <code>{}</code>\n\n", get_final_order(gravity, posture, &buy_zone).replace("\n", "")));

    html.push_str("<b>🎯 Tactical Summary</b>\n");
    html.push_str(&format!(" • 加仓区: <code>{}</code>\n", buy_zone.join(" / ")));
    html.push_str(&format!(" • 观察区: <code>{}</code>\n", watch_zone.join(" / ")));
    html.push_str(&format!(" • 防御区: <code>{}</code>\n", defend_zone.join(" / ")));
    html.push_str(&format!(" • 持有区: <code>{}</code>\n\n", hold_zone.join(" / ")));

    html.push_str("<b>🔥 Highest Opportunity</b>\n");
    for (i, s) in opportunities.iter().take(3).enumerate() {
        let z = s.dev_z_score.unwrap_or(0.0);
        html.push_str(&format!("{}. <b>{}</b> (<code>{}</code> / <code>{}</code>)\n", i+1, s.symbol, s.state_code, format_sigma(z)));
    }
    html.push_str("\n<b>☠️ Highest Risk</b>\n");
    for (i, s) in risks.iter().take(3).enumerate() {
        let z = s.dev_z_score.unwrap_or(0.0);
        html.push_str(&format!("{}. <b>{}</b> (<code>{}</code> / <code>{}</code>)\n", i+1, s.symbol, s.state_code, format_sigma(z)));
    }
    html.push_str("\n");
    
    html.push_str("<b>🎯 個別銘柄レーダー (Execution Radar)</b>\n");
    html.push_str("<i>ℹ️ Sorted by: Extreme Opportunities (Fear) > Pullbacks > Optimal > Cruise > Risks/Stable</i>\n\n");

    for (idx, s) in snapshots.iter().enumerate() {
        let trend_icon = match s.trend_status {
            TrendStatus::Up => "↗️",
            TrendStatus::Down => "↘️",
            _ => "➡️",
        };
        let z_val = s.dev_z_score.unwrap_or(0.0);
        let emoji = get_state_emoji(&s.state_code);
        
        let state_name = if let Some(rc) = &s.reason_code {
            format!("{} {} {}", emoji, s.state_code, rc)
        } else {
            format!("{} {}", emoji, s.state_code)
        };

        html.push_str(&format!("{}. <b>{}</b> | {} | <code>{}</code>\n", idx+1, s.symbol, state_name, get_position_guidance(&s.state_code)));
        html.push_str(&format!("└ {} | {} | {}\n", format_sigma(z_val), trend_icon, s.action_text));
        html.push_str("\n");
    }
    
    html
}

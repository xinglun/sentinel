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
    #[tabled(rename = "トレンド")]
    trend: String,
    #[tabled(rename = "重力強度")]
    strength_pct: String,
    #[tabled(rename = "Z-Score(曲率)")]
    z_score_curv: String,
    #[tabled(rename = "乖離率 %")]
    dev_pct: String,
    #[tabled(rename = "状態 (置信度)")]
    state: String,
    #[tabled(rename = "行動建議")]
    action: String,
}

pub struct ReportResult {
    #[allow(dead_code)]
    pub markdown: String,
    pub telegram_html: String,
}

const CURVATURE_DEADZONE: f64 = 0.05;

pub struct GravityHealth {
    pub up_count: usize,
    pub flat_count: usize,
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
}

pub struct CapitalPosture {
    pub state_code: String,
    pub display_text: String,
    pub t_share_raw: f64,
    pub r_share_raw: f64,
    pub r_share_adj: f64,
    pub t_ratio_final: f64,
    pub r_ratio_final: f64,
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
        if self.global_potential_energy < 1.0 {
            format!("{:.2} (LOW / 安定)", self.global_potential_energy)
        } else if self.global_potential_energy < 2.0 {
            format!("{:.2} (MEDIUM / 蓄力)", self.global_potential_energy)
        } else {
            format!("{:.2} (HIGH / 高张力)", self.global_potential_energy)
        }
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

        CapitalPosture {
            state_code: state_code.to_string(),
            display_text: display_text.to_string(),
            t_share_raw,
            r_share_raw,
            r_share_adj,
            t_ratio_final: final_trend_ratio,
            r_ratio_final: final_reversion_ratio,
        }
    }
}

pub fn generate_reports(config: &AppConfig, snapshots: &[TickerSnapshot], gravity_health: &GravityHealth) -> Result<ReportResult> {
    let now = Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let posture = gravity_health.compute_capital_posture();
    
    let mut rows = Vec::new();
    for s in snapshots {
        let trend_str = match s.trend_status {
            TrendStatus::Up => "上昇 ↗",
            TrendStatus::Down => "下落 ↘",
            TrendStatus::Flat => "横ばい →",
            TrendStatus::Unknown => "?",
        };
        
        let dev_str = s.deviation_pct.map(|v| format!("{:.2}%", v)).unwrap_or_else(|| "-".to_string());
        let strength_str = s.owner_ma_slope_pct.map(|v| format!("{:+.2}%", v)).unwrap_or_else(|| "-".to_string());
        let z_score_str = s.dev_z_score.map(|v| format!("{:+.1}", v)).unwrap_or_else(|| "-".to_string());
        
        // Curvature dead-zone (centralized constant)
        let curv_str = s.curvature.map(|v| {
            if v >= CURVATURE_DEADZONE { "拐点↗" }
            else if v <= -CURVATURE_DEADZONE { "下沉↘" }
            else { "平坦~" }
        }).unwrap_or("-");
        
        let z_curv_combined = format!("{} ({})", z_score_str, curv_str);
        
        let state_rc = if let Some(rc) = &s.reason_code {
            format!("{} {} ({}%)", s.state_code, rc, s.confidence_score)
        } else {
            format!("{} ({}%)", s.state_code, s.confidence_score)
        };
        
        rows.push(TerminalRow {
            symbol: s.symbol.clone(),
            trend: trend_str.to_string(),
            strength_pct: strength_str,
            z_score_curv: z_curv_combined,
            dev_pct: dev_str,
            state: state_rc,
            action: s.action_text.clone(),
        });
    }
    
    let raw_label = config.output.weight_kind.clone().unwrap_or_else(|| "Portfolio".to_string());
    let binding = raw_label.to_lowercase();
    let weight_kind_label = match binding.as_str() {
        "portfolio" => "Portfolio",
        "cap" | "mktcap" => "MktCap",
        "risk" => "Risk",
        _ => &raw_label, // Use original if not mapped
    };
    
    let mut table = Table::new(rows);
    table.with(Style::modern());
    
    println!("\n🌍 CAPITAL STATE: {}", posture.display_text);
    println!("🌍 GRAVITY (Count): {}", gravity_health.format_count_health());
    println!("🌍 GRAVITY (Weight/{}): {}", weight_kind_label, gravity_health.format_weight_health());
    println!("🌍 GRAVITY (Strength/{}): {:+.2}%", weight_kind_label, gravity_health.global_gravity_strength);
    println!("🌍 GRAVITY POTENTIAL: {}", gravity_health.format_potential_energy());
    println!("{}", table);

    let md_content = generate_markdown(config, snapshots, &date_str, gravity_health, &posture);
    let tg_html = generate_telegram_html(config, snapshots, &date_str, gravity_health, &posture);

    if !config.output.save_to.is_empty() {
        fs::create_dir_all(&config.output.save_to)?;
        
        let json_path = Path::new(&config.output.save_to).join(format!("{}.json", date_str));
        let json_content = serde_json::to_string_pretty(snapshots)?;
        fs::write(json_path, json_content)?;
        
        let md_path = Path::new(&config.output.save_to).join(format!("{}.md", date_str));
        fs::write(md_path, &md_content)?;
        
        // --- 📊 Telemetry System Heartbeat (V3 Ultimate Schema) ---
        let telemetry_path = Path::new(&config.output.save_to).join("telemetry_v3.csv");
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
    
    Ok(ReportResult {
        markdown: md_content,
        telegram_html: tg_html,
    })
}

fn generate_markdown(config: &AppConfig, snapshots: &[TickerSnapshot], date_str: &str, gravity: &GravityHealth, posture: &CapitalPosture) -> String {
    let raw_label = config.output.weight_kind.clone().unwrap_or_else(|| "Portfolio".to_string());
    let binding = raw_label.to_lowercase();
    let weight_kind_label = match binding.as_str() {
        "portfolio" => "Portfolio",
        "cap" | "mktcap" => "MktCap",
        "risk" => "Risk",
        _ => &raw_label,
    };

    let mut md = format!("# 🐕 Stock Sentinel 每日観測レーダー\n📅 **日付**: {}\n🌍 **CAPITAL STATE**: {}\n🌍 **GRAVITY (Count)**: {}\n🌍 **GRAVITY (Weight/{})**: {}\n🌍 **GRAVITY (Strength/{})**: {:+.2}%\n🌍 **GRAVITY POTENTIAL**: {}\n\n", 
        date_str, 
        posture.display_text,
        gravity.format_count_health(),
        weight_kind_label,
        gravity.format_weight_health(),
        weight_kind_label,
        gravity.global_gravity_strength,
        gravity.format_potential_energy()
    );
    
    md.push_str("### 🎯 個別銘柄レーダー\n\n");
    md.push_str("| 銘柄 | トレンド | 強度 | Z-Score (曲率) | 乖離率 | 状態 (置信度) | 行動建議 |\n");
    md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
    
    for s in snapshots {
        let dev_str = s.deviation_pct.map(|v| format!("{:.2}%", v)).unwrap_or_else(|| "-".to_string());
        
        let strength_str = s.owner_ma_slope_pct.map(|v| format!("{:+.2}%", v)).unwrap_or_else(|| "-".to_string());
        let z_score_str = s.dev_z_score.map(|v| format!("{:+.1}", v)).unwrap_or_else(|| "-".to_string());
        
        let curv_str = s.curvature.map(|v| {
            if v >= CURVATURE_DEADZONE { "拐点↗" }
            else if v <= -CURVATURE_DEADZONE { "下沉↘" }
            else { "平坦~" }
        }).unwrap_or("-");
        
        let z_curv_combined = format!("{} ({})", z_score_str, curv_str);
        
        let trend_icon = match s.trend_status {
            TrendStatus::Up => "↗️ 上昇",
            TrendStatus::Down => "↘️ 下落",
            TrendStatus::Flat => "➡️ 横ばい",
            TrendStatus::Unknown => "❓ 未知",
        };
        
        let state_rc = if let Some(rc) = &s.reason_code {
            format!("{} {} ({}%)", s.state_code, rc, s.confidence_score)
        } else {
            format!("{} ({}%)", s.state_code, s.confidence_score)
        };
        
        md.push_str(&format!("| **{}** | {} | {} | {} | **{}** | {} | {} |\n",
            s.symbol,
            trend_icon,
            strength_str,
            z_curv_combined,
            dev_str,
            state_rc,
            s.action_text
        ));
    }
    
    md
}

fn generate_telegram_html(config: &AppConfig, snapshots: &[TickerSnapshot], date_str: &str, gravity: &GravityHealth, posture: &CapitalPosture) -> String {
    let raw_label = config.output.weight_kind.clone().unwrap_or_else(|| "Portfolio".to_string());
    let binding = raw_label.to_lowercase();
    let weight_kind_label = match binding.as_str() {
        "portfolio" => "Portfolio",
        "cap" | "mktcap" => "MktCap",
        "risk" => "Risk",
        _ => &raw_label,
    };

    let mut html = format!("🐕 <b>Stock Sentinel レーダー</b>\n📅 <b>日付:</b> {}\n🌍 <b>CAPITAL STATE:</b> {}\n🌍 <b>GRAVITY (Count):</b> {}\n🌍 <b>GRAVITY (Weight/{}):</b> {}\n🌍 <b>GRAVITY (Strength/{}):</b> {:+.2}%\n🌍 <b>GRAVITY POTENTIAL:</b> {}\n\n", 
        date_str, 
        posture.display_text,
        gravity.format_count_health(),
        weight_kind_label,
        gravity.format_weight_health(),
        weight_kind_label,
        gravity.global_gravity_strength,
        gravity.format_potential_energy()
    );
    
    for s in snapshots {
        let dev_str = s.deviation_pct.map(|v| format!("{:.2}%", v)).unwrap_or_else(|| "-".to_string());
        let owner_ma_str = s.owner_ma.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        let leash_ma_str = s.leash_ma.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "-".to_string());
        
        let trend_icon = match s.trend_status {
            TrendStatus::Up => "↗️ 上昇",
            TrendStatus::Down => "↘️ 下落",
            TrendStatus::Flat => "➡️ 横ばい",
            TrendStatus::Unknown => "❓ 未知",
        };
        
        let state_rc = if let Some(rc) = &s.reason_code {
            format!("{} {} ({}%)", s.state_code, rc, s.confidence_score)
        } else {
            format!("{} ({}%)", s.state_code, s.confidence_score)
        };
        
        let strength_str = s.owner_ma_slope_pct.map(|v| format!("{:+.2}%", v)).unwrap_or_else(|| "-".to_string());
        let z_score_str = s.dev_z_score.map(|v| format!("{:+.1}", v)).unwrap_or_else(|| "-".to_string());
        
        html.push_str(&format!("<b>{}</b> {} <code>${:.2}</code> (Dev: <code>{}</code>)\n", s.symbol, trend_icon, s.dog_price, dev_str));
        html.push_str(&format!(" • <b>状態(置信度):</b> {}\n", state_rc));
        html.push_str(&format!(" • <b>物理:</b> 強度 <code>{}</code> | Z-Score <code>{}</code>\n", strength_str, z_score_str));
        html.push_str(&format!(" • <b>基準:</b> Owner <code>{}</code> | Leash <code>{}</code> | Basis: <code>{}</code>\n", owner_ma_str, leash_ma_str, s.deviation_basis_used));
        html.push_str(&format!(" • <b>建議:</b> {}\n\n", s.action_text));
    }
    
    html
}

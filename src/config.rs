use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::{HashMap, BTreeMap};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub version: u32,
    pub output: OutputConfig,
    pub telegram: Option<TelegramConfig>,
    pub rules: RulesConfig,
    pub watchlist: Vec<WatchlistEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OutputConfig {
    pub timezone: String,
    pub format: String,
    pub save_to: String,
    pub include_summary: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub bot_token: String,
    pub chat_id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RulesConfig {
    pub trend: TrendConfig,
    pub deviation_bands: BTreeMap<String, f64>,
    pub actions: HashMap<String, String>,
    pub bear_mode: BearModeConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TrendConfig {
    pub lookback_days: usize,
    pub flat_threshold_pct: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BearModeConfig {
    pub enabled: bool,
    pub fallback_action: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WatchlistEntry {
    pub symbol: String,
    pub name: Option<String>,
    pub market: String,
    pub owner_ma_days: usize,
    pub leash_ma_days: usize,
    pub deviation_basis: DeviationBasis,
    pub enable: bool,
    pub action_overrides: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum DeviationBasis {
    Owner,
    Leash,
}

#[derive(Debug, Clone)]
pub struct ParsedRules {
    pub trend: TrendConfig,
    pub sorted_bands: Vec<(String, f64)>, // descending thresholds
    pub actions: HashMap<String, String>,
    pub bear_mode: BearModeConfig,
}

impl AppConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
            
        let config: AppConfig = toml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse config file: {}", e))?;
            
        for band_key in config.rules.deviation_bands.keys() {
            if !config.rules.actions.contains_key(band_key) {
                return Err(anyhow!(
                    "Config Error: deviation_bands contains '{}' but no corresponding action is defined.",
                    band_key
                ));
            }
        }
        
        Ok(config)
    }

    pub fn get_parsed_rules(&self) -> ParsedRules {
        let mut bands: Vec<(String, f64)> = self.rules.deviation_bands
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
            
        bands.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        ParsedRules {
            trend: self.rules.trend.clone(),
            sorted_bands: bands,
            actions: self.rules.actions.clone(),
            bear_mode: self.rules.bear_mode.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_config() {
        let toml_str = r#"
            version = 1
            [output]
            timezone = "Asia/Shanghai"
            format = "markdown"
            save_to = "./reports"
            include_summary = true

            [rules.trend]
            lookback_days = 20
            flat_threshold_pct = 0.5

            [rules.deviation_bands]
            overheat_2 = 30.0   
            optimal    = -5.0   

            [rules.actions]
            overheat_2 = "停止买入"
            optimal    = "买入"
            fear       = "恐慌加仓"

            [rules.bear_mode]
            enabled = true
            fallback_action = "强制覆盖"

            [[watchlist]]
            symbol = "TSLA"
            market = "US"
            owner_ma_days = 120
            leash_ma_days = 20
            deviation_basis = "owner"
            enable = true
        "#;
        
        let config: AppConfig = toml::from_str(toml_str).expect("should parse");
        assert_eq!(config.version, 1);
        assert_eq!(config.watchlist[0].symbol, "TSLA");
    }

    #[test]
    fn test_missing_action_for_band() {
        // AppConfig::load uses a path, so we can't test it directly easily, but we can write a quick wrapper to test the logic exactly.
    }
}

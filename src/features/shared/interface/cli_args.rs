use crate::config;
use crate::features::radar::interface::audit_daily_report::{
    audit_error_invalid_days, audit_error_missing_date, audit_error_missing_days,
};
use crate::features::shared::interface::i18n::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliCommand {
    Backtest,
    Daemon,
    Radar,
    Review,
    AuditDaily,
    IngestEvidence,
    IngestEvidenceUrl,
    CollectEvidence,
    ResearchAttention,
    AssetThesis,
    DailyCalibration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliProviderKind {
    Yahoo,
    Futu,
}

#[derive(Debug, Clone)]
pub(crate) struct CliOptions {
    pub command: CliCommand,
    pub provider: CliProviderKind,
    pub audit_date_arg: Option<String>,
    pub audit_days: usize,
    pub audit_arg_error: Option<String>,
    pub evidence_symbol: Option<String>,
    pub evidence_symbols: Vec<String>,
    pub evidence_type_str: String,
    pub evidence_confidence: f64,
    pub evidence_description: String,
    pub evidence_url: Option<String>,
    pub evidence_date_arg: Option<String>,
    pub evidence_source_type_str: String,
    pub evidence_dry_run: bool,
    pub evidence_days: usize,
    pub evidence_source_provider: String,
    pub evidence_arg_error: Option<String>,
    pub research_notify: bool,
    pub backtest_from_date: String,
    pub backtest_to_date: String,
}

impl CliProviderKind {
    fn from_config(provider: Option<&str>) -> Self {
        match provider {
            Some("futu") => Self::Futu,
            _ => Self::Yahoo,
        }
    }

    fn from_arg(provider: &str) -> Option<Self> {
        match provider.to_lowercase().as_str() {
            "futu" => Some(Self::Futu),
            "yahoo" => Some(Self::Yahoo),
            _ => None,
        }
    }
}

impl CliOptions {
    fn new(app_config: &config::AppConfig) -> Self {
        Self {
            command: CliCommand::Radar,
            provider: CliProviderKind::from_config(app_config.provider.as_deref()),
            audit_date_arg: None,
            audit_days: 14,
            audit_arg_error: None,
            evidence_symbol: None,
            evidence_symbols: Vec::new(),
            evidence_type_str: "capex".to_string(),
            evidence_confidence: 1.0,
            evidence_description: "Manual ingestion via CLI".to_string(),
            evidence_url: None,
            evidence_date_arg: None,
            evidence_source_type_str: "official".to_string(),
            evidence_dry_run: false,
            evidence_days: 3,
            evidence_source_provider: "finnhub".to_string(),
            evidence_arg_error: None,
            research_notify: false,
            backtest_from_date: "2024-01-01".to_string(),
            backtest_to_date: "2024-02-01".to_string(),
        }
    }
}

pub(crate) fn parse_cli_options(
    args: &[String],
    app_config: &config::AppConfig,
    audit_language: Language,
) -> CliOptions {
    let mut options = CliOptions::new(app_config);

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "backtest" => options.command = CliCommand::Backtest,
            "daemon" | "trade" => options.command = CliCommand::Daemon,
            "radar" => options.command = CliCommand::Radar,
            "review" => options.command = CliCommand::Review,
            "audit_daily" | "transition_audit_summary" => options.command = CliCommand::AuditDaily,
            "ingest-evidence" => options.command = CliCommand::IngestEvidence,
            "ingest-evidence-url" => options.command = CliCommand::IngestEvidenceUrl,
            "collect-evidence" => options.command = CliCommand::CollectEvidence,
            "research-attention" => options.command = CliCommand::ResearchAttention,
            "asset-thesis" => options.command = CliCommand::AssetThesis,
            "daily-calibration" => options.command = CliCommand::DailyCalibration,
            "--provider" if i + 1 < args.len() => {
                if let Some(provider) = CliProviderKind::from_arg(&args[i + 1]) {
                    options.provider = provider;
                }
                i += 1;
            }
            "--symbol" if i + 1 < args.len() => {
                options.evidence_symbol = Some(args[i + 1].clone());
                i += 1;
            }
            "--symbols" if i + 1 < args.len() => {
                options.evidence_symbols = args[i + 1]
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                i += 1;
            }
            "--type" if i + 1 < args.len() => {
                options.evidence_type_str = args[i + 1].clone();
                i += 1;
            }
            "--confidence" => {
                if i + 1 >= args.len() || args[i + 1].starts_with("--") {
                    options.evidence_arg_error = Some("Missing value for --confidence".to_string());
                } else {
                    match args[i + 1].parse::<f64>() {
                        Ok(value) => options.evidence_confidence = value,
                        Err(_) => {
                            options.evidence_arg_error =
                                Some(format!("Invalid confidence value: {}", args[i + 1]));
                        }
                    }
                    i += 1;
                }
            }
            "--desc" if i + 1 < args.len() => {
                options.evidence_description = args[i + 1].clone();
                i += 1;
            }
            "--url" if i + 1 < args.len() => {
                options.evidence_url = Some(args[i + 1].clone());
                i += 1;
            }
            "--date" => {
                if i + 1 >= args.len() || args[i + 1].starts_with("--") {
                    options.audit_arg_error =
                        Some(audit_error_missing_date(audit_language).to_string());
                    options.evidence_arg_error = Some("Missing value for --date".to_string());
                } else {
                    options.audit_date_arg = Some(args[i + 1].clone());
                    options.evidence_date_arg = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--days" => {
                if i + 1 >= args.len() || args[i + 1].starts_with("--") {
                    options.audit_arg_error =
                        Some(audit_error_missing_days(audit_language).to_string());
                    options.evidence_arg_error = Some("Missing value for --days".to_string());
                } else {
                    match args[i + 1].parse::<usize>() {
                        Ok(days) if days > 0 => {
                            options.audit_days = days;
                            options.evidence_days = days;
                        }
                        _ => {
                            options.audit_arg_error =
                                Some(audit_error_invalid_days(audit_language).to_string());
                            options.evidence_arg_error =
                                Some(format!("Invalid days value: {}", args[i + 1]));
                        }
                    }
                    i += 1;
                }
            }
            "--source_type" if i + 1 < args.len() => {
                options.evidence_source_type_str = args[i + 1].clone();
                i += 1;
            }
            "--dry-run" => {
                options.evidence_dry_run = true;
            }
            "--notify" => {
                options.research_notify = true;
            }
            "--source" if i + 1 < args.len() => {
                options.evidence_source_provider = args[i + 1].to_lowercase();
                i += 1;
            }
            "--from" if i + 1 < args.len() => {
                options.backtest_from_date = args[i + 1].clone();
                i += 1;
            }
            "--to" if i + 1 < args.len() => {
                options.backtest_to_date = args[i + 1].clone();
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    options
}

use crate::config;
use crate::features::shared::interface::i18n::Language;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliCommand {
    Help,
    Backtest,
    ConfigCheck,
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
    OfficialCalendarSmoke,
    GrayRhinoEscalation,
    IngestGrayRhinoGovernance,
    IngestGrayRhinoDependency,
    IngestGrayRhinoInstitutional,
    IngestGrayRhinoRedundancy,
    CollectGrayRhinoGovernance,
    CollectGrayRhinoDependency,
    CollectGrayRhinoInstitutional,
    CollectGrayRhinoRedundancy,
    CollectGrayRhinoBackfill,
    DiscoverGrayRhino,
    CollectGrayRhinoSources,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliProviderKind {
    Yahoo,
    Futu,
}

#[derive(Debug, Clone)]
pub(crate) struct CliOptions {
    pub command: CliCommand,
    pub cli_arg_error: Option<String>,
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
    pub governance_evidence_file: Option<String>,
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
            command: CliCommand::Help,
            cli_arg_error: None,
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
            governance_evidence_file: None,
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
    let mut command_explicit = false;
    let mut help_requested = args.len() <= 1;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "help" | "-h" | "--help" => {
                options.command = CliCommand::Help;
                help_requested = true;
            }
            "backtest" => {
                options.command = CliCommand::Backtest;
                command_explicit = true;
            }
            "config-check" => {
                options.command = CliCommand::ConfigCheck;
                command_explicit = true;
            }
            "daemon" | "trade" => {
                options.command = CliCommand::Daemon;
                command_explicit = true;
            }
            "radar" => {
                options.command = CliCommand::Radar;
                command_explicit = true;
            }
            "review" => {
                options.command = CliCommand::Review;
                command_explicit = true;
            }
            "audit_daily" | "transition_audit_summary" => {
                options.command = CliCommand::AuditDaily;
                command_explicit = true;
            }
            "ingest-evidence" => {
                options.command = CliCommand::IngestEvidence;
                command_explicit = true;
            }
            "ingest-evidence-url" => {
                options.command = CliCommand::IngestEvidenceUrl;
                command_explicit = true;
            }
            "collect-evidence" => {
                options.command = CliCommand::CollectEvidence;
                command_explicit = true;
            }
            "research-attention" => {
                options.command = CliCommand::ResearchAttention;
                command_explicit = true;
            }
            "asset-thesis" => {
                options.command = CliCommand::AssetThesis;
                command_explicit = true;
            }
            "daily-calibration" => {
                options.command = CliCommand::DailyCalibration;
                command_explicit = true;
            }
            "official-calendar-smoke" => {
                options.command = CliCommand::OfficialCalendarSmoke;
                command_explicit = true;
            }
            "gray-rhino" | "gray-rhino-escalation" => {
                options.command = CliCommand::GrayRhinoEscalation;
                command_explicit = true;
            }
            "ingest-gray-rhino-governance" | "ingest-governance-evidence" => {
                options.command = CliCommand::IngestGrayRhinoGovernance;
                command_explicit = true;
            }
            "ingest-gray-rhino-dependency" | "ingest-dependency-evidence" => {
                options.command = CliCommand::IngestGrayRhinoDependency;
                command_explicit = true;
            }
            "ingest-gray-rhino-institutional" | "ingest-institutional-evidence" => {
                options.command = CliCommand::IngestGrayRhinoInstitutional;
                command_explicit = true;
            }
            "ingest-gray-rhino-redundancy" | "ingest-redundancy-evidence" => {
                options.command = CliCommand::IngestGrayRhinoRedundancy;
                command_explicit = true;
            }
            "collect-gray-rhino-governance" => {
                options.command = CliCommand::CollectGrayRhinoGovernance;
                command_explicit = true;
            }
            "collect-gray-rhino-dependency" => {
                options.command = CliCommand::CollectGrayRhinoDependency;
                command_explicit = true;
            }
            "collect-gray-rhino-institutional" => {
                options.command = CliCommand::CollectGrayRhinoInstitutional;
                command_explicit = true;
            }
            "collect-gray-rhino-redundancy" => {
                options.command = CliCommand::CollectGrayRhinoRedundancy;
                command_explicit = true;
            }
            "collect-gray-rhino-backfill" => {
                options.command = CliCommand::CollectGrayRhinoBackfill;
                command_explicit = true;
            }
            "discover-gray-rhino" => {
                options.command = CliCommand::DiscoverGrayRhino;
                command_explicit = true;
            }
            "collect-gray-rhino-sources" => {
                options.command = CliCommand::CollectGrayRhinoSources;
                command_explicit = true;
            }
            "--provider" if i + 1 < args.len() => {
                match CliProviderKind::from_arg(&args[i + 1]) {
                    Some(provider) => options.provider = provider,
                    None => {
                        options.cli_arg_error = Some(format!("Invalid provider: {}", args[i + 1]));
                    }
                }
                i += 1;
            }
            "--provider" => {
                options.cli_arg_error = Some("Missing value for --provider".to_string());
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
            "--file" if i + 1 < args.len() => {
                options.governance_evidence_file = Some(args[i + 1].clone());
                i += 1;
            }
            "--file" => {
                options.evidence_arg_error = Some("Missing value for --file".to_string());
            }
            "--from" if i + 1 < args.len() => {
                options.backtest_from_date = args[i + 1].clone();
                i += 1;
            }
            "--to" if i + 1 < args.len() => {
                options.backtest_to_date = args[i + 1].clone();
                i += 1;
            }
            unknown => {
                options.cli_arg_error = Some(format!("Unknown command or option: {}", unknown));
            }
        }
        i += 1;
    }

    if !command_explicit && !help_requested && options.cli_arg_error.is_none() {
        options.cli_arg_error = Some("No command specified.".to_string());
    }

    options
}

pub(crate) fn cli_usage(_language: Language) -> &'static str {
    "Usage: stock-sentinel <command> [options]\n\nCommands:\n  config-check                  Validate config.toml without running reports\n  radar                         Run the daily radar pipeline\n  daemon | trade                Run the trading daemon mode\n  review                        Render the latest review\n  audit_daily                   Render transition audit summary\n  daily-calibration             Render daily cognitive calibration\n  official-calendar-smoke       Run official calendar live smoke and diagnostics\n  research-attention            Render research attention report\n  asset-thesis                  Render asset thesis registry\n  gray-rhino                    Render Gray Rhino Escalation monitor\n  discover-gray-rhino           Auto-discover Gray Rhino candidates from source text\n  collect-gray-rhino-sources    Collect SEC/Finnhub/FRED sources for Gray Rhino discovery\n  ingest-gray-rhino-governance  Ingest GovernanceConcentration evidence from JSON\n  ingest-gray-rhino-dependency  Ingest DependencyConcentration evidence from JSON\n  ingest-gray-rhino-institutional Ingest InstitutionalMaturity evidence from JSON\n  ingest-gray-rhino-redundancy  Ingest Redundancy evidence from JSON\n  collect-gray-rhino-governance Collect GovernanceConcentration evidence from source\n  collect-gray-rhino-dependency Collect DependencyConcentration evidence from source or URL\n  collect-gray-rhino-institutional Collect InstitutionalMaturity evidence from source\n  collect-gray-rhino-redundancy Collect Redundancy evidence from source\n  collect-gray-rhino-backfill   Run multi-category Gray Rhino dry-run manifest\n  ingest-evidence               Ingest manual evidence\n  ingest-evidence-url           Collect evidence from one URL\n  collect-evidence              Collect evidence from configured sources\n  backtest                      Run backtest\n  help                          Show this help\n\nOptions:\n  --help, -h                    Show this help\n  --notify                      Send supported sidecar report to Telegram\n  --provider <yahoo|futu>       Select market data provider\n  --date <YYYY-MM-DD>           Select audit/evidence date\n  --days <N>                    Select audit/evidence lookback days\n  --source <sec|finnhub|fred>   Select source provider for collection commands\n  --symbol <SYMBOL>             Select evidence collection subject\n  --symbols <A,B,C>             Select batch evidence collection subjects\n  --file <PATH>                 Read structured Gray Rhino evidence JSON or source document\n  --url <URL>                   Read live Gray Rhino dependency source URL\n\nSafety:\n  No command is executed by default. Use `radar` explicitly to run the radar pipeline."
}

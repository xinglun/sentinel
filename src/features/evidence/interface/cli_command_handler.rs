use crate::config::AppConfig;
use crate::features::evidence::acl::evidence_store_factory::{
    build_batch_evidence_fetcher_adapter, build_evidence_extractor_adapter,
    build_evidence_store_adapter, build_url_evidence_fetcher_adapter,
};
use crate::features::evidence::application::evidence::{
    ingest_manual_evidence, ManualEvidenceIngestionRequest,
};
use crate::features::evidence::application::evidence_ingestion::{
    collect_evidence_batch, collect_evidence_from_source, BatchCollectEvidenceRequest,
    BatchEvidenceTarget, CollectEvidenceRequest,
};
use crate::features::evidence::domain::evidence::EvidenceSourceType;
use crate::features::shared::interface::cli_args::{CliCommand, CliOptions};
use crate::features::shared::interface::i18n::Language;
use anyhow::{anyhow, Context, Result};

/// Evidence command の既存 CLI 契約を保ったまま orchestration を実行する。
pub(crate) async fn run_evidence_command(
    app_config: &AppConfig,
    options: &CliOptions,
    language: Language,
) -> Result<()> {
    match options.command {
        CliCommand::IngestEvidence => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }

            let retention_days = app_config
                .get_parsed_rules()
                .market_state_engine
                .evidence_retention_days as i64;
            let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
            let store = build_evidence_store_adapter(&save_dir);
            let outcome = ingest_manual_evidence(
                &store,
                ManualEvidenceIngestionRequest {
                    evidence_type: options.evidence_type_str.clone(),
                    confidence: options.evidence_confidence,
                    description: options.evidence_description.clone(),
                    event_date: options.evidence_date_arg.clone(),
                    symbol: options.evidence_symbol.clone(),
                    source_url: options.evidence_url.clone(),
                    fallback_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
                    retention_days: Some(retention_days),
                },
            )?;
            if outcome.saved_count > 0 {
                println!(
                    "{}",
                    evidence_collection_success_message(language, outcome.saved_count)
                );
            } else {
                println!("{}", evidence_collection_duplicate_message(language));
            }
            Ok(())
        }
        CliCommand::IngestEvidenceUrl => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            let symbol = options
                .evidence_symbol
                .clone()
                .ok_or_else(|| anyhow!("--symbol is required"))?;
            let url = options
                .evidence_url
                .clone()
                .ok_or_else(|| anyhow!("--url is required"))?;

            let st = match options.evidence_source_type_str.as_str() {
                "official" => EvidenceSourceType::OfficialIR,
                "news" => EvidenceSourceType::NewsMedia,
                _ => {
                    return Err(anyhow!(
                        "Invalid source type: {}. Use 'official' or 'news'",
                        options.evidence_source_type_str
                    ))
                }
            };

            let fetcher = build_url_evidence_fetcher_adapter(app_config, &url)?;

            let extractor = build_evidence_extractor_adapter();
            let retention_days = app_config
                .get_parsed_rules()
                .market_state_engine
                .evidence_retention_days as i64;
            let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
            let store = build_evidence_store_adapter(&save_dir);
            let repository = if options.evidence_dry_run {
                None
            } else {
                Some(&store as &dyn crate::features::evidence::application::evidence::EvidenceRepository)
            };
            let outcome = collect_evidence_from_source(
                fetcher.as_ref(),
                extractor.as_ref(),
                repository,
                CollectEvidenceRequest {
                    url: url.clone(),
                    symbol: symbol.clone(),
                    source_type: st,
                    days: options.evidence_days,
                    persist: !options.evidence_dry_run,
                    retention_days: Some(retention_days),
                },
            )
            .await
            .context("Failed to collect evidence from source")?;

            if options.evidence_dry_run {
                println!("{}", evidence_dry_run_title(language));
                println!("{}: {}", evidence_dry_run_source_label(language), url);
                println!("{}: {}", evidence_dry_run_symbol_label(language), symbol);
                if outcome.records.is_empty() {
                    println!("{}", evidence_dry_run_empty_notice(language));
                }
                for (i, r) in outcome.records.iter().enumerate() {
                    println!(
                        "[{}] {}: {:?}, {}: {:.2}, {}: {}",
                        i + 1,
                        evidence_dry_run_type_label(language),
                        r.evidence_type,
                        evidence_dry_run_confidence_label(language),
                        r.confidence,
                        evidence_dry_run_date_label(language),
                        r.event_date
                    );
                    println!(
                        "    {}: {}",
                        evidence_dry_run_desc_label(language),
                        r.description
                    );
                    if let Some(ref url) = r.source_url {
                        println!("    URL:  {}", url);
                    }
                }
                return Ok(());
            }

            if outcome.saved_count > 0 {
                println!(
                    "{}",
                    evidence_collection_success_message(language, outcome.saved_count)
                );
            } else {
                println!("{}", evidence_collection_duplicate_message(language));
            }
            Ok(())
        }
        CliCommand::CollectEvidence => {
            if let Some(err) = &options.evidence_arg_error {
                return Err(anyhow!("{}", err));
            }
            if options.evidence_symbols.is_empty() {
                return Err(anyhow!("--symbols is required (comma separated)"));
            }

            println!("{}", evidence_batch_title(language));
            println!(
                "{}: {:?}",
                evidence_batch_symbols_title(language),
                options.evidence_symbols
            );
            println!(
                "{}:  {} days",
                evidence_batch_window_title(language),
                options.evidence_days
            );

            let fetcher = build_batch_evidence_fetcher_adapter(
                app_config,
                &options.evidence_source_provider,
                options.evidence_dry_run,
            )?;

            let extractor = build_evidence_extractor_adapter();
            let targets = options
                .evidence_symbols
                .iter()
                .map(|symbol| {
                    println!("{} {}...", evidence_batch_fetching_label(language), symbol);
                    // Dry-run で Key がない場合は symbol 自身をファイル名として FixtureFetcher に探させる
                    let url = if app_config.finnhub.is_none() && options.evidence_dry_run {
                        symbol.clone()
                    } else {
                        "finnhub".to_string()
                    };
                    BatchEvidenceTarget {
                        symbol: symbol.clone(),
                        url,
                    }
                })
                .collect::<Vec<_>>();
            let retention_days = app_config
                .get_parsed_rules()
                .market_state_engine
                .evidence_retention_days as i64;
            let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
            let store = build_evidence_store_adapter(&save_dir);
            let repository = if options.evidence_dry_run {
                None
            } else {
                Some(&store as &dyn crate::features::evidence::application::evidence::EvidenceRepository)
            };
            let batch_outcome = collect_evidence_batch(
                fetcher.as_ref(),
                extractor.as_ref(),
                repository,
                BatchCollectEvidenceRequest {
                    targets,
                    source_type: if options.evidence_source_provider == "sec" {
                        EvidenceSourceType::OfficialIR
                    } else {
                        EvidenceSourceType::NewsMedia
                    },
                    days: options.evidence_days,
                    persist: !options.evidence_dry_run,
                    retention_days: Some(retention_days),
                },
            )
            .await?;

            for symbol in &options.evidence_symbols {
                let record_count = batch_outcome
                    .records
                    .iter()
                    .filter(|r| r.symbol.as_deref() == Some(symbol.as_str()))
                    .count();
                if batch_outcome
                    .failures
                    .iter()
                    .any(|failure| failure.symbol == *symbol)
                {
                    continue;
                }
                println!(
                    "  -> {} {}",
                    evidence_batch_extracted_label(language),
                    record_count
                );
            }
            for failure in &batch_outcome.failures {
                eprintln!(
                    "{} for {}: {}",
                    evidence_batch_fetch_error_prefix(language),
                    failure.symbol,
                    failure.error
                );
            }

            println!("{}", evidence_batch_summary_title(language));
            println!(
                "{}: {} {}",
                evidence_batch_processed_label(language),
                options.evidence_symbols.len(),
                evidence_batch_symbols_label(language)
            );
            println!(
                "{}:   {} {}",
                evidence_batch_success_label(language),
                batch_outcome.success_count,
                evidence_batch_symbols_label(language)
            );
            println!(
                "{}:   {} {}",
                evidence_batch_failure_label(language),
                batch_outcome.failure_count,
                evidence_batch_symbols_label(language)
            );

            if options.evidence_dry_run {
                println!("{}", evidence_batch_dry_run_summary_title(language));
                if batch_outcome.records.is_empty() {
                    println!("{}", evidence_batch_no_evidence_notice(language));
                }
                for (i, r) in batch_outcome.records.iter().enumerate() {
                    let date_str = r.event_date.as_str();
                    println!(
                        "[{}] {}: {:?} ({:.2}) | {}: {}",
                        i + 1,
                        r.symbol.as_deref().unwrap_or("GLOBAL"),
                        r.evidence_type,
                        r.confidence,
                        evidence_batch_date_label(language),
                        date_str
                    );
                    println!(
                        "    {}: {}",
                        evidence_batch_desc_label(language),
                        r.description
                    );
                    if let Some(ref url) = r.source_url {
                        println!("    URL:  {}", url);
                    }
                }
                return Ok(());
            }

            println!(
                "{}",
                evidence_batch_success_message(language, batch_outcome.saved_count)
            );
            Ok(())
        }
        _ => unreachable!("evidence command handler called for a non-evidence command"),
    }
}

fn evidence_dry_run_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 干运行：提取到的证据 ---",
        Language::EnUs => "--- Dry Run: Extracted Evidence ---",
        Language::JaJp => "--- ドライラン: 抽出された証拠 ---",
    }
}

fn evidence_dry_run_source_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源",
        Language::EnUs => "Source",
        Language::JaJp => "ソース",
    }
}

fn evidence_dry_run_symbol_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "标的",
        Language::EnUs => "Symbol",
        Language::JaJp => "シンボル",
    }
}

fn evidence_dry_run_empty_notice(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未发现证据。",
        Language::EnUs => "No evidence found.",
        Language::JaJp => "証拠は見つかりませんでした。",
    }
}

fn evidence_dry_run_type_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "类型",
        Language::EnUs => "Type",
        Language::JaJp => "種別",
    }
}

fn evidence_dry_run_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "置信度",
        Language::EnUs => "Confidence",
        Language::JaJp => "確信度",
    }
}

fn evidence_dry_run_date_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "日期",
        Language::EnUs => "Date",
        Language::JaJp => "日付",
    }
}

fn evidence_dry_run_desc_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "说明",
        Language::EnUs => "Desc",
        Language::JaJp => "説明",
    }
}

fn evidence_collection_success_message(language: Language, count: usize) -> String {
    match language {
        Language::ZhCn => format!("成功摄取 {} 条自动证据记录。", count),
        Language::EnUs => format!(
            "Successfully ingested {} automated evidence records.",
            count
        ),
        Language::JaJp => format!("自動証拠レコードを {} 件取り込みました。", count),
    }
}

fn evidence_collection_duplicate_message(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "证据记录已存在（已去重）。",
        Language::EnUs => "Evidence record already exists (deduplicated).",
        Language::JaJp => "証拠レコードは既に存在します（重複除去済み）。",
    }
}

fn evidence_batch_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 批量证据采集 ---",
        Language::EnUs => "--- Batch Evidence Collection ---",
        Language::JaJp => "--- バッチ証拠収集 ---",
    }
}

fn evidence_batch_fetching_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "正在获取",
        Language::EnUs => "Fetching for",
        Language::JaJp => "取得中",
    }
}

fn evidence_batch_symbols_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "标的",
        Language::EnUs => "Symbols",
        Language::JaJp => "銘柄",
    }
}

fn evidence_batch_window_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "窗口",
        Language::EnUs => "Window",
        Language::JaJp => "期間",
    }
}

fn evidence_batch_extracted_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已提取",
        Language::EnUs => "Extracted",
        Language::JaJp => "抽出済み",
    }
}

fn evidence_batch_fetch_error_prefix(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "  [错误] 获取失败",
        Language::EnUs => "  [ERROR] Failed to fetch",
        Language::JaJp => "  [エラー] 取得失敗",
    }
}

fn evidence_batch_processed_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已处理",
        Language::EnUs => "Processed",
        Language::JaJp => "処理済み",
    }
}

fn evidence_batch_success_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "成功",
        Language::EnUs => "Success",
        Language::JaJp => "成功",
    }
}

fn evidence_batch_failure_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "失败",
        Language::EnUs => "Failure",
        Language::JaJp => "失敗",
    }
}

fn evidence_batch_symbols_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "个标的",
        Language::EnUs => "symbols",
        Language::JaJp => "銘柄",
    }
}

fn evidence_batch_no_evidence_notice(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "批次中未发现证据。",
        Language::EnUs => "No evidence found in batch.",
        Language::JaJp => "バッチ内で証拠は見つかりませんでした。",
    }
}

fn evidence_batch_summary_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 批量采集摘要 ---",
        Language::EnUs => "--- Batch Collection Summary ---",
        Language::JaJp => "--- バッチ収集サマリー ---",
    }
}

fn evidence_batch_dry_run_summary_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 干运行：提取到的证据摘要 ---",
        Language::EnUs => "--- Dry Run: Extracted Evidence Summary ---",
        Language::JaJp => "--- ドライラン: 抽出された証拠サマリー ---",
    }
}

fn evidence_batch_date_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "日期",
        Language::EnUs => "Date",
        Language::JaJp => "日付",
    }
}

fn evidence_batch_desc_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "说明",
        Language::EnUs => "Desc",
        Language::JaJp => "説明",
    }
}

fn evidence_batch_success_message(language: Language, count: usize) -> String {
    match language {
        Language::ZhCn => format!("成功摄取 {} 条批量证据记录。", count),
        Language::EnUs => format!(
            "Successfully ingested {} batch evidence records to store.",
            count
        ),
        Language::JaJp => format!("バッチ証拠レコードを {} 件ストアへ取り込みました。", count),
    }
}

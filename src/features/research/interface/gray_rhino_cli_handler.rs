use anyhow::{anyhow, Context, Result};
use chrono::NaiveDate;
use sha2::{Digest, Sha256};

use crate::config;
use crate::features::research::acl::dependency_source_adapter_factory::build_dependency_source_adapter;
use crate::features::research::acl::governance_evidence_store_factory::build_governance_evidence_store_adapter;
use crate::features::research::acl::governance_source_adapter_factory::build_governance_source_adapter;
use crate::features::research::acl::gray_rhino_backfill_runner_factory::{
    append_gray_rhino_backfill_run, collect_gray_rhino_category_source,
};
use crate::features::research::acl::gray_rhino_file_reader::read_gray_rhino_text_file;
use crate::features::research::acl::gray_rhino_source_adapter_factory::{
    collect_gray_rhino_sources, GrayRhinoSourceCollectionRequest, GrayRhinoSourceProvider,
};
use crate::features::research::application::dependency_evidence::ingest_dependency_concentration_evidence;
use crate::features::research::application::dependency_source_pipeline::{
    collect_dependency_concentration_sources, DependencyFieldCoverage,
    DependencySourceCollectionRequest,
};
use crate::features::research::application::governance_evidence::ingest_governance_concentration_evidence;
use crate::features::research::application::governance_source_pipeline::{
    collect_governance_concentration_sources, GovernanceFieldCoverage,
    GovernanceSourceCollectionRequest,
};
use crate::features::research::application::gray_rhino_discovery::{
    discover_gray_rhino_candidates, GrayRhinoDiscoveryInput,
};
use crate::features::research::application::institutional_evidence::ingest_institutional_maturity_evidence;
use crate::features::research::application::redundancy_evidence::ingest_redundancy_evidence;
use crate::features::research::domain::gray_rhino_evidence::{
    DependencyConcentrationEvidence, GovernanceConcentrationEvidence,
    InstitutionalMaturityEvidence, RedundancyEvidence,
};
use crate::features::research::interface::gray_rhino_report::render_gray_rhino_inline_reference;
use crate::features::shared::interface::i18n::Language;

fn gray_rhino_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "边界声明: 仅限证据处理，不更新升级、闸门、执行或交易状态。",
        Language::EnUs => {
            "Boundary: evidence only; no escalation, gate, execution, or trading state updated."
        }
        Language::JaJp => "境界声明: 証拠処理のみで、昇格、ゲート、実行、取引状態は更新しない。",
    }
}

fn gray_rhino_success_message(language: Language, category: &str, saved: bool) -> String {
    match language {
        Language::ZhCn => {
            if saved {
                format!("已摄取 {category} 证据。")
            } else {
                format!("{category} 证据已存在（已去重）。")
            }
        }
        Language::EnUs => {
            if saved {
                format!("Successfully ingested {category} evidence.")
            } else {
                format!("{category} evidence already exists (deduplicated).")
            }
        }
        Language::JaJp => {
            if saved {
                format!("{category} の証拠を取り込みました。")
            } else {
                format!("{category} の証拠は既に存在します（重複除去済み）。")
            }
        }
    }
}

fn gray_rhino_title_auto_discovery(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 灰犀牛自动发现 ---",
        Language::EnUs => "--- Gray Rhino Auto Discovery ---",
        Language::JaJp => "--- グレイリノ自動発見 ---",
    }
}

fn gray_rhino_title_source_collection(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 灰犀牛来源采集 ---",
        Language::EnUs => "--- Gray Rhino Source Collection ---",
        Language::JaJp => "--- グレイリノ由来収集 ---",
    }
}

fn gray_rhino_title_governance_collection(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 灰犀牛治理证据采集 ---",
        Language::EnUs => "--- Gray Rhino Governance Evidence Collection ---",
        Language::JaJp => "--- グレイリノ統治証拠収集 ---",
    }
}

fn gray_rhino_title_dependency_collection(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 灰犀牛依赖证据采集 ---",
        Language::EnUs => "--- Gray Rhino Dependency Evidence Collection ---",
        Language::JaJp => "--- グレイリノ依存証拠収集 ---",
    }
}

fn gray_rhino_title_category_collection(language: Language, category: &str) -> String {
    match language {
        Language::ZhCn => format!("--- 灰犀牛 {category} 证据采集 ---"),
        Language::EnUs => format!("--- Gray Rhino {category} Evidence Collection ---"),
        Language::JaJp => format!("--- グレイリノ {category} 証拠収集 ---"),
    }
}

fn gray_rhino_label_source_count(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源数",
        Language::EnUs => "Sources",
        Language::JaJp => "ソース数",
    }
}

fn gray_rhino_label_formal_persisted(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "正式证据已持久化",
        Language::EnUs => "Formal evidence persisted",
        Language::JaJp => "正式証拠は保存済み",
    }
}

fn gray_rhino_boolean_word(language: Language, value: bool) -> &'static str {
    match language {
        Language::ZhCn => {
            if value {
                "是"
            } else {
                "否"
            }
        }
        Language::EnUs => {
            if value {
                "true"
            } else {
                "false"
            }
        }
        Language::JaJp => {
            if value {
                "はい"
            } else {
                "いいえ"
            }
        }
    }
}

fn gray_rhino_label_coverage(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "覆盖率",
        Language::EnUs => "Coverage",
        Language::JaJp => "カバー率",
    }
}

fn gray_rhino_label_field_coverage(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "字段覆盖率",
        Language::EnUs => "Field coverage",
        Language::JaJp => "項目カバー率",
    }
}

fn gray_rhino_provider_status_value(language: Language, value: &str) -> String {
    match language {
        Language::ZhCn => match value {
            "succeeded" => "成功".to_string(),
            "partial_failure" => "部分失败".to_string(),
            "failed" => "失败".to_string(),
            "skipped" => "跳过".to_string(),
            _ => value.to_string(),
        },
        Language::EnUs => match value {
            "succeeded" => "succeeded".to_string(),
            "partial_failure" => "partial failure".to_string(),
            "failed" => "failed".to_string(),
            "skipped" => "skipped".to_string(),
            _ => value.to_string(),
        },
        Language::JaJp => match value {
            "succeeded" => "成功".to_string(),
            "partial_failure" => "部分失敗".to_string(),
            "failed" => "失敗".to_string(),
            "skipped" => "未実行".to_string(),
            _ => value.to_string(),
        },
    }
}

fn gray_rhino_source_collection_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界声明: 仅限来源采集，不给出交易建议，不覆盖 Gate，不修改趋势凝聚，不触发执行动作。"
        }
        Language::EnUs => {
            "Boundary: source collection only; no trading recommendation, no Gate override, no trend cohesion mutation, no execution action."
        }
        Language::JaJp => {
            "境界声明: 由来収集のみで、取引推奨を出さず、Gate を上書きせず、トレンド凝集を変更せず、実行アクションを起こさない。"
        }
    }
}

fn gray_rhino_category_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "类别",
        Language::EnUs => "Category",
        Language::JaJp => "カテゴリ",
    }
}

fn gray_rhino_source_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源",
        Language::EnUs => "Source",
        Language::JaJp => "ソース",
    }
}

fn gray_rhino_observed_at_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观测时间",
        Language::EnUs => "Observed at",
        Language::JaJp => "観測時刻",
    }
}

fn gray_rhino_provider_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "提供方",
        Language::EnUs => "provider",
        Language::JaJp => "提供元",
    }
}

fn gray_rhino_dry_run_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "干运行",
        Language::EnUs => "dry_run",
        Language::JaJp => "ドライラン",
    }
}

fn gray_rhino_source_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源数",
        Language::EnUs => "source_count",
        Language::JaJp => "ソース数",
    }
}

fn gray_rhino_accepted_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已接受",
        Language::EnUs => "accepted",
        Language::JaJp => "受理済み",
    }
}

fn gray_rhino_rejected_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已拒绝",
        Language::EnUs => "rejected",
        Language::JaJp => "拒否済み",
    }
}

fn gray_rhino_candidate_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "候选数",
        Language::EnUs => "candidate_count",
        Language::JaJp => "候補数",
    }
}

fn gray_rhino_provider_status_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "提供方状态",
        Language::EnUs => "provider_status",
        Language::JaJp => "提供元状態",
    }
}

fn gray_rhino_planned_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已规划",
        Language::EnUs => "planned",
        Language::JaJp => "計画済み",
    }
}

fn gray_rhino_path_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "路径",
        Language::EnUs => "path",
        Language::JaJp => "パス",
    }
}

fn gray_rhino_taxonomy_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "分类",
        Language::EnUs => "taxonomy",
        Language::JaJp => "分類",
    }
}

fn gray_rhino_message_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "消息",
        Language::EnUs => "message",
        Language::JaJp => "メッセージ",
    }
}

fn gray_rhino_saved_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已保存",
        Language::EnUs => "Saved",
        Language::JaJp => "保存済み",
    }
}

fn gray_rhino_manifest_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "清单",
        Language::EnUs => "Manifest",
        Language::JaJp => "マニフェスト",
    }
}

fn gray_rhino_audit_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "审计",
        Language::EnUs => "Audit",
        Language::JaJp => "監査",
    }
}

fn gray_rhino_latest_observed_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "最新观测日",
        Language::EnUs => "Latest observed date",
        Language::JaJp => "最新観測日",
    }
}

fn gray_rhino_backfill_processed_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已处理回填条目",
        Language::EnUs => "Backfill entries processed",
        Language::JaJp => "処理済み回填エントリ",
    }
}

fn gray_rhino_backfill_summary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "回填运行摘要",
        Language::EnUs => "Backfill run summary",
        Language::JaJp => "回填実行サマリー",
    }
}

fn gray_rhino_backfill_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "边界声明: 仅限干运行，不更新升级、闸门、执行或交易状态。",
        Language::EnUs => {
            "Boundary: dry-run only; no escalation, gate, execution, or trading state updated."
        }
        Language::JaJp => "境界声明: ドライランのみで、昇格、ゲート、実行、取引状態は更新しない。",
    }
}

fn gray_rhino_backfill_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "--- 灰犀牛多类别回填干运行 ---",
        Language::EnUs => "--- Gray Rhino Multi-Category Backfill Dry Run ---",
        Language::JaJp => "--- グレイリノ多カテゴリ回填ドライラン ---",
    }
}

pub(crate) fn run_ingest_gray_rhino_governance(
    app_config: &config::AppConfig,
    file_arg: Option<&str>,
    language: Language,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let raw = read_gray_rhino_text_file(file, "Failed to read governance evidence file")?;
    let evidence: GovernanceConcentrationEvidence = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse governance evidence JSON: {}", file))?;
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let outcome = ingest_governance_concentration_evidence(&store, evidence)?;
    println!(
        "{}",
        gray_rhino_success_message(language, "GovernanceConcentration", outcome.saved)
    );
    println!(
        "{}: GovernanceConcentration",
        gray_rhino_category_label(language)
    );
    println!(
        "{}: {}",
        gray_rhino_source_label(language),
        outcome.record.source.source_title
    );
    println!(
        "{}: {}",
        gray_rhino_observed_at_label(language),
        outcome.record.source.observed_at
    );
    println!("{}", gray_rhino_boundary(language));
    Ok(())
}

pub(crate) fn run_discover_gray_rhino(
    symbol: Option<String>,
    file_arg: Option<&str>,
    observed_date_arg: Option<&str>,
    language: Language,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let subject = symbol.unwrap_or_else(|| "UNKNOWN".to_string());
    let observed_at = match observed_date_arg {
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("Invalid Gray Rhino discovery date: {}", raw))?,
        None => chrono::Local::now().date_naive(),
    };
    let text = read_gray_rhino_text_file(file, "Failed to read Gray Rhino discovery source")?;
    let candidates = discover_gray_rhino_candidates(&GrayRhinoDiscoveryInput {
        subject,
        source_title: file.to_string(),
        observed_at,
        text,
    });
    println!("{}", gray_rhino_title_auto_discovery(language));
    println!("{}", render_gray_rhino_inline_reference(&candidates));
    Ok(())
}

pub(crate) async fn run_collect_gray_rhino_sources(
    app_config: &config::AppConfig,
    provider_arg: &str,
    symbols: Vec<String>,
    dry_run: bool,
    observed_date_arg: Option<&str>,
    lookback_days: usize,
    language: Language,
) -> Result<()> {
    let provider = GrayRhinoSourceProvider::parse(provider_arg)
        .ok_or_else(|| anyhow!("Unsupported Gray Rhino source provider: {}", provider_arg))?;
    let as_of_date = match observed_date_arg {
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("Invalid Gray Rhino source collection date: {}", raw))?,
        None => chrono::Local::now().date_naive(),
    };
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let outcomes = collect_gray_rhino_sources(
        app_config,
        &save_dir,
        GrayRhinoSourceCollectionRequest {
            provider,
            symbols,
            as_of_date,
            lookback_days: if provider == GrayRhinoSourceProvider::Sec {
                lookback_days.max(365)
            } else {
                lookback_days
            },
            dry_run,
        },
    )
    .await?;
    println!("{}", gray_rhino_title_source_collection(language));
    println!("{}: {:?}", gray_rhino_provider_label(language), provider);
    println!("{}: {}", gray_rhino_dry_run_label(language), dry_run);
    println!(
        "{}: {}",
        gray_rhino_source_count_label(language),
        outcomes.len()
    );
    let accepted = outcomes.iter().filter(|outcome| outcome.accepted).count();
    let rejected = outcomes.len().saturating_sub(accepted);
    let candidate_count: usize = outcomes.iter().map(|outcome| outcome.candidate_count).sum();
    println!("{}: {}", gray_rhino_accepted_label(language), accepted);
    println!("{}: {}", gray_rhino_rejected_label(language), rejected);
    println!(
        "{}: {}",
        gray_rhino_candidate_count_label(language),
        candidate_count
    );
    let provider_status = if dry_run {
        "skipped"
    } else if accepted == outcomes.len() && accepted > 0 {
        "succeeded"
    } else if accepted > 0 && rejected > 0 {
        "partial_failure"
    } else if accepted == 0 && rejected > 0 {
        "failed"
    } else {
        "skipped"
    };
    println!(
        "{}: {}",
        gray_rhino_provider_status_label(language),
        gray_rhino_provider_status_value(language, provider_status)
    );
    for outcome in &outcomes {
        println!(
            "- {} {}={} {}={} {}={} {}={} {}={} {}={}",
            outcome.subject,
            gray_rhino_accepted_label(language),
            outcome.accepted,
            gray_rhino_planned_label(language),
            outcome.planned,
            gray_rhino_candidate_count_label(language),
            outcome.candidate_count,
            gray_rhino_path_label(language),
            outcome.repository_path.as_deref().unwrap_or("none"),
            gray_rhino_taxonomy_label(language),
            outcome.failure_taxonomy.as_deref().unwrap_or("none"),
            gray_rhino_message_label(language),
            outcome.message
        );
    }
    println!("{}", gray_rhino_source_collection_boundary(language));
    if provider_status == "failed" {
        return Err(anyhow!(
            "Gray Rhino source collection failed for provider {:?}: no accepted source",
            provider
        ));
    }
    Ok(())
}

pub(crate) fn run_ingest_gray_rhino_dependency(
    app_config: &config::AppConfig,
    file_arg: Option<&str>,
    language: Language,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let raw = read_gray_rhino_text_file(file, "Failed to read dependency evidence file")?;
    let evidence: DependencyConcentrationEvidence = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse dependency evidence JSON: {}", file))?;
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let outcome = ingest_dependency_concentration_evidence(&store, evidence)?;
    println!(
        "{}",
        gray_rhino_success_message(language, "DependencyConcentration", outcome.saved)
    );
    println!(
        "{}: DependencyConcentration",
        gray_rhino_category_label(language)
    );
    println!(
        "{}: {}",
        gray_rhino_source_label(language),
        outcome.record.source.source_title
    );
    println!(
        "{}: {}",
        gray_rhino_observed_at_label(language),
        outcome.record.source.observed_at
    );
    println!("{}", gray_rhino_boundary(language));
    Ok(())
}

pub(crate) fn run_ingest_gray_rhino_institutional(
    app_config: &config::AppConfig,
    file_arg: Option<&str>,
    language: Language,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let raw = read_gray_rhino_text_file(file, "Failed to read institutional evidence file")?;
    let evidence: InstitutionalMaturityEvidence = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse institutional evidence JSON: {}", file))?;
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let outcome = ingest_institutional_maturity_evidence(&store, evidence)?;
    println!(
        "{}",
        gray_rhino_success_message(language, "InstitutionalMaturity", outcome.saved)
    );
    println!(
        "{}: InstitutionalMaturity",
        gray_rhino_category_label(language)
    );
    println!(
        "{}: {}",
        gray_rhino_source_label(language),
        outcome.record.source.source_title
    );
    println!(
        "{}: {}",
        gray_rhino_observed_at_label(language),
        outcome.record.source.observed_at
    );
    println!("{}", gray_rhino_boundary(language));
    Ok(())
}

pub(crate) fn run_ingest_gray_rhino_redundancy(
    app_config: &config::AppConfig,
    file_arg: Option<&str>,
    language: Language,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let raw = read_gray_rhino_text_file(file, "Failed to read redundancy evidence file")?;
    let evidence: RedundancyEvidence = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse redundancy evidence JSON: {}", file))?;
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let outcome = ingest_redundancy_evidence(&store, evidence)?;
    println!(
        "{}",
        gray_rhino_success_message(language, "Redundancy", outcome.saved)
    );
    println!("{}: Redundancy", gray_rhino_category_label(language));
    println!(
        "{}: {}",
        gray_rhino_source_label(language),
        outcome.record.source.source_title
    );
    println!(
        "{}: {}",
        gray_rhino_observed_at_label(language),
        outcome.record.source.observed_at
    );
    println!("{}", gray_rhino_boundary(language));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_collect_gray_rhino_governance(
    app_config: &config::AppConfig,
    symbol: Option<String>,
    symbols: Vec<String>,
    source_file: Option<String>,
    dry_run_requested: bool,
    observed_date_arg: Option<&str>,
    lookback_days: usize,
    language: Language,
) -> Result<()> {
    let targets = resolve_governance_collection_targets(app_config, symbol, symbols, &source_file)?;
    let observed_at = match observed_date_arg {
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("Invalid governance evidence date: {}", raw))?,
        None => chrono::Local::now().date_naive(),
    };
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let adapter = build_governance_source_adapter(app_config, &save_dir);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let retrieved_at = chrono::Local::now().date_naive();
    let is_live_sec_path = source_file.is_none();
    let persist_evidence = !dry_run_requested && !is_live_sec_path;
    let mut total_sources = 0;
    let mut total_accepted = 0;
    let mut total_saved = 0;
    let mut total_manifest = 0;
    let mut total_audit = 0;
    let mut rejected = Vec::new();
    let mut latest_observed_at = None;
    let mut metric_coverage = Vec::new();
    for target in targets {
        let summary = collect_governance_concentration_sources(
            &adapter,
            &store,
            GovernanceSourceCollectionRequest {
                symbol: Some(target),
                local_file: source_file.clone(),
                observed_at,
                retrieved_at,
                lookback_days: lookback_days.max(1),
                persist_evidence,
            },
        )
        .await?;
        total_sources += summary.source_count;
        total_accepted += summary.accepted_count;
        total_saved += summary.saved_count;
        total_manifest += summary.manifest_count;
        total_audit += summary.audit_count;
        latest_observed_at = latest_observed_at
            .map(|latest: NaiveDate| {
                summary
                    .latest_observed_at
                    .map(|observed| latest.max(observed))
                    .unwrap_or(latest)
            })
            .or(summary.latest_observed_at);
        metric_coverage.extend(summary.metric_coverage);
        rejected.extend(summary.rejected);
    }
    let coverage_ratio = if total_sources == 0 {
        0.0
    } else {
        total_accepted as f64 / total_sources as f64
    };

    println!("{}", gray_rhino_title_governance_collection(language));
    println!(
        "{}:  {}",
        gray_rhino_source_count_label(language),
        total_sources
    );
    println!(
        "{}: {}",
        gray_rhino_accepted_label(language),
        total_accepted
    );
    println!("{}:    {}", gray_rhino_saved_label(language), total_saved);
    println!(
        "{}: {}",
        gray_rhino_manifest_label(language),
        total_manifest
    );
    println!("{}:    {}", gray_rhino_audit_label(language), total_audit);
    println!(
        "{}:  {}",
        gray_rhino_dry_run_label(language),
        !persist_evidence
    );
    println!(
        "{}: {}",
        gray_rhino_label_formal_persisted(language),
        gray_rhino_boolean_word(language, persist_evidence)
    );
    println!(
        "{}: {:.1}%",
        gray_rhino_label_coverage(language),
        coverage_ratio * 100.0
    );
    render_governance_field_coverage(&metric_coverage, language);
    println!(
        "{}: {}",
        gray_rhino_rejected_label(language),
        rejected.len()
    );
    if let Some(latest) = latest_observed_at {
        println!("{}: {}", gray_rhino_latest_observed_label(language), latest);
    }
    for rejection in &rejected {
        println!(
            "  [REJECTED] {}: {}",
            rejection.source_title, rejection.reason
        );
    }
    println!("{}", gray_rhino_boundary(language));
    Ok(())
}

pub(crate) async fn run_collect_gray_rhino_dependency(
    app_config: &config::AppConfig,
    symbol: Option<String>,
    source_file: Option<String>,
    source_url: Option<String>,
    dry_run_requested: bool,
    observed_date_arg: Option<&str>,
    language: Language,
) -> Result<()> {
    let target = symbol.ok_or_else(|| anyhow!("--symbol is required"))?;
    let observed_at = match observed_date_arg {
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("Invalid dependency evidence date: {}", raw))?,
        None => chrono::Local::now().date_naive(),
    };
    let save_dir = std::path::PathBuf::from(&app_config.output.save_to);
    let store = build_governance_evidence_store_adapter(&save_dir);
    let adapter = build_dependency_source_adapter();
    let summary = collect_dependency_concentration_sources(
        &adapter,
        &store,
        DependencySourceCollectionRequest {
            symbol: Some(target),
            local_file: source_file,
            source_url,
            source_cache_dir: Some(
                save_dir
                    .join("gray_rhino_sources/dependency")
                    .display()
                    .to_string(),
            ),
            observed_at,
            retrieved_at: chrono::Local::now().date_naive(),
            persist_evidence: !dry_run_requested,
        },
    )
    .await?;
    let coverage_ratio = if summary.source_count == 0 {
        0.0
    } else {
        summary.accepted_count as f64 / summary.source_count as f64
    };

    println!("{}", gray_rhino_title_dependency_collection(language));
    println!(
        "{}:  {}",
        gray_rhino_source_count_label(language),
        summary.source_count
    );
    println!(
        "{}: {}",
        gray_rhino_accepted_label(language),
        summary.accepted_count
    );
    println!(
        "{}:    {}",
        gray_rhino_saved_label(language),
        summary.saved_count
    );
    println!(
        "{}: {}",
        gray_rhino_manifest_label(language),
        summary.manifest_count
    );
    println!(
        "{}:    {}",
        gray_rhino_audit_label(language),
        summary.audit_count
    );
    println!(
        "{}:  {}",
        gray_rhino_dry_run_label(language),
        dry_run_requested
    );
    println!(
        "{}: {}",
        gray_rhino_label_formal_persisted(language),
        gray_rhino_boolean_word(language, !dry_run_requested)
    );
    println!(
        "{}: {:.1}%",
        gray_rhino_label_coverage(language),
        coverage_ratio * 100.0
    );
    render_dependency_field_coverage(&summary.metric_coverage, language);
    println!(
        "{}: {}",
        gray_rhino_rejected_label(language),
        summary.rejected.len()
    );
    if let Some(latest) = summary.latest_observed_at {
        println!("{}: {}", gray_rhino_latest_observed_label(language), latest);
    }
    for rejection in &summary.rejected {
        println!(
            "  [REJECTED:{:?}] {}",
            rejection.taxonomy, rejection.source_title
        );
    }
    println!("{}", gray_rhino_boundary(language));
    Ok(())
}

pub(crate) async fn run_collect_gray_rhino_backfill(
    app_config: &config::AppConfig,
    file_arg: Option<&str>,
    observed_date_arg: Option<&str>,
    language: Language,
) -> Result<()> {
    let file = file_arg.ok_or_else(|| anyhow!("--file is required"))?;
    let raw = read_gray_rhino_text_file(file, "Failed to read Gray Rhino backfill manifest")?;
    let entries: Vec<serde_json::Value> = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse Gray Rhino backfill manifest: {}", file))?;
    let started_at = chrono::Local::now().to_rfc3339();
    let run_id = format!(
        "gray-rhino-backfill-{:x}",
        Sha256::digest(format!("{file}:{started_at}").as_bytes())
    );
    let mut processed = 0usize;
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut stale_sources = 0usize;
    let mut drift_sources = 0usize;
    let mut failures = Vec::new();
    let mut categories = Vec::new();
    println!("{}", gray_rhino_backfill_title(language));
    for entry in entries {
        let category = entry
            .get("category")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow!("backfill entry missing category"))?;
        let symbol = entry
            .get("symbol")
            .and_then(|value| value.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();
        let Some(source_file) = entry
            .get("file")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
        else {
            failures.push(serde_json::json!({
                "category": category,
                "symbol": symbol,
                "failure_taxonomy": "unsupported_format",
                "reason": "missing file"
            }));
            rejected += 1;
            processed += 1;
            continue;
        };
        categories.push(category.to_string());
        let source_content = match read_gray_rhino_text_file(
            &source_file,
            "Failed to read Gray Rhino backfill source",
        ) {
            Ok(content) => content,
            Err(err) => {
                failures.push(serde_json::json!({
                    "category": category,
                    "symbol": symbol,
                    "source": source_file,
                    "failure_taxonomy": "fetch_failure",
                    "reason": err.to_string()
                }));
                rejected += 1;
                processed += 1;
                continue;
            }
        };
        let source_hash = format!("{:x}", Sha256::digest(source_content.as_bytes()));
        if let Some(expected) = entry
            .get("expected_sha256")
            .and_then(|value| value.as_str())
        {
            if expected != source_hash {
                drift_sources += 1;
            }
        }
        if let (Some(observed), Some(freshness_days)) = (
            entry.get("observed_at").and_then(|value| value.as_str()),
            entry.get("freshness_days").and_then(|value| value.as_i64()),
        ) {
            if let Ok(observed) = NaiveDate::parse_from_str(observed, "%Y-%m-%d") {
                let as_of = observed_date_arg
                    .and_then(|raw| NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok())
                    .unwrap_or_else(|| chrono::Local::now().date_naive());
                if as_of.signed_duration_since(observed).num_days() > freshness_days {
                    stale_sources += 1;
                }
            }
        }
        match category {
            "DependencyConcentration" => {
                run_collect_gray_rhino_dependency(
                    app_config,
                    Some(symbol),
                    Some(source_file),
                    None,
                    true,
                    observed_date_arg,
                    language,
                )
                .await?;
            }
            "InstitutionalMaturity" => run_collect_gray_rhino_category_source(
                app_config,
                "InstitutionalMaturity",
                Some(symbol),
                Some(source_file),
                true,
                observed_date_arg,
                &[
                    "succession_structure_disclosed",
                    "external_audit_present",
                    "disclosure_quality_score",
                    "oversight_evolution_disclosed",
                    "compliance_maturity_level",
                ],
                language,
            )?,
            "Redundancy" => run_collect_gray_rhino_category_source(
                app_config,
                "Redundancy",
                Some(symbol),
                Some(source_file),
                true,
                observed_date_arg,
                &[
                    "fallback_available",
                    "alternative_supplier_count",
                    "redundancy_ratio",
                    "recovery_path_disclosed",
                    "failover_tested",
                ],
                language,
            )?,
            other => {
                failures.push(serde_json::json!({
                    "category": other,
                    "symbol": symbol,
                    "source": source_file,
                    "failure_taxonomy": "unsupported_format",
                    "reason": "unsupported category"
                }));
                rejected += 1;
                processed += 1;
                continue;
            }
        }
        accepted += 1;
        processed += 1;
    }
    let finished_at = chrono::Local::now().to_rfc3339();
    append_gray_rhino_backfill_run(
        &std::path::PathBuf::from(&app_config.output.save_to),
        &serde_json::json!({
            "run_id": run_id,
            "mode": "dry_run",
            "manifest": file,
            "categories": categories,
            "source_count": processed,
            "accepted": accepted,
            "rejected": rejected,
            "coverage": if processed == 0 { 0.0 } else { accepted as f64 / processed as f64 },
            "stale_sources": stale_sources,
            "drift_sources": drift_sources,
            "failures": failures,
            "started_at": started_at,
            "finished_at": finished_at,
            "boundary": "evidence only; no escalation, gate, execution, or trading state updated"
        }),
    )?;
    println!(
        "{}: {}",
        gray_rhino_backfill_processed_label(language),
        processed
    );
    println!(
        "{}: gray_rhino_backfill_runs.jsonl",
        gray_rhino_backfill_summary_label(language)
    );
    println!("{}", gray_rhino_backfill_boundary(language));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_collect_gray_rhino_category_source(
    app_config: &config::AppConfig,
    category: &str,
    symbol: Option<String>,
    source_file: Option<String>,
    dry_run_requested: bool,
    observed_date_arg: Option<&str>,
    metrics: &[&str],
    language: Language,
) -> Result<()> {
    let summary = collect_gray_rhino_category_source(
        &app_config.output.save_to,
        category,
        symbol,
        source_file,
        dry_run_requested,
        observed_date_arg,
        metrics,
    )?;

    println!(
        "{}",
        gray_rhino_title_category_collection(language, category)
    );
    println!("{}: 1", gray_rhino_label_source_count(language));
    println!(
        "{}: {}",
        gray_rhino_accepted_label(language),
        usize::from(summary.accepted)
    );
    println!("{}: 0", gray_rhino_saved_label(language));
    println!("{}: 1", gray_rhino_manifest_label(language));
    println!("{}: 1", gray_rhino_audit_label(language));
    println!(
        "{}:  {}",
        gray_rhino_dry_run_label(language),
        gray_rhino_boolean_word(language, summary.dry_run_requested)
    );
    println!(
        "{}: {}",
        gray_rhino_label_formal_persisted(language),
        gray_rhino_boolean_word(language, false)
    );
    println!(
        "{}: {:.1}%",
        gray_rhino_label_coverage(language),
        (summary.extracted.len() as f64 / summary.metrics.len() as f64) * 100.0
    );
    println!("{}:", gray_rhino_label_field_coverage(language));
    for metric in &summary.metrics {
        let count = usize::from(summary.extracted.iter().any(|item| item == metric));
        println!(
            "  {}: {:.1}% ({}/1 extracted, {} missing)",
            metric,
            count as f64 * 100.0,
            count,
            summary.missing_count(metric)
        );
    }
    println!(
        "{}: {}",
        gray_rhino_rejected_label(language),
        usize::from(!summary.accepted)
    );
    if !summary.accepted {
        println!("  [REJECTED:{}] {}", summary.taxonomy, summary.source_title);
    }
    println!(
        "{}: {}",
        gray_rhino_latest_observed_label(language),
        summary.observed_at
    );
    println!("{}", gray_rhino_boundary(language));
    Ok(())
}

fn render_dependency_field_coverage(
    metric_coverage: &[DependencyFieldCoverage],
    language: Language,
) {
    if metric_coverage.is_empty() {
        return;
    }
    println!("{}:", gray_rhino_label_field_coverage(language));
    for metric in metric_coverage {
        let total = metric.extracted_count + metric.missing_count;
        println!(
            "  {}: {:.1}% ({}/{} extracted, {} missing)",
            metric.metric,
            metric.coverage_ratio * 100.0,
            metric.extracted_count,
            total,
            metric.missing_count
        );
    }
}

fn render_governance_field_coverage(
    metric_coverage: &[GovernanceFieldCoverage],
    language: Language,
) {
    if metric_coverage.is_empty() {
        return;
    }
    println!("{}:", gray_rhino_label_field_coverage(language));
    for metric in aggregate_governance_field_coverage(metric_coverage) {
        let total = metric.extracted_count + metric.missing_count + metric.invalid_count;
        let coverage_ratio = if total == 0 {
            0.0
        } else {
            metric.extracted_count as f64 / total as f64
        };
        println!(
            "  {}: {:.1}% ({}/{} extracted, {} missing, {} invalid)",
            metric.metric,
            coverage_ratio * 100.0,
            metric.extracted_count,
            total,
            metric.missing_count,
            metric.invalid_count
        );
    }
}

fn aggregate_governance_field_coverage(
    metric_coverage: &[GovernanceFieldCoverage],
) -> Vec<GovernanceFieldCoverage> {
    let mut totals = std::collections::BTreeMap::new();
    for metric in metric_coverage {
        let entry = totals.entry(metric.metric.clone()).or_insert((0, 0, 0));
        entry.0 += metric.extracted_count;
        entry.1 += metric.missing_count;
        entry.2 += metric.invalid_count;
    }
    totals
        .into_iter()
        .map(
            |(metric, (extracted_count, missing_count, invalid_count))| {
                let total = extracted_count + missing_count + invalid_count;
                GovernanceFieldCoverage {
                    metric,
                    extracted_count,
                    missing_count,
                    invalid_count,
                    coverage_ratio: if total == 0 {
                        0.0
                    } else {
                        extracted_count as f64 / total as f64
                    },
                }
            },
        )
        .collect()
}

fn resolve_governance_collection_targets(
    app_config: &config::AppConfig,
    symbol: Option<String>,
    symbols: Vec<String>,
    source_file: &Option<String>,
) -> Result<Vec<String>> {
    if source_file.is_some() {
        return symbol
            .map(|symbol| vec![symbol])
            .or_else(|| symbols.first().cloned().map(|symbol| vec![symbol]))
            .ok_or_else(|| anyhow!("--symbol is required when --file is used"));
    }
    let mut targets = Vec::new();
    if let Some(symbol) = symbol {
        targets.push(symbol);
    }
    targets.extend(
        symbols
            .into_iter()
            .filter(|symbol| !symbol.trim().is_empty()),
    );
    if targets.is_empty() {
        targets = app_config
            .watchlist
            .iter()
            .filter(|entry| entry.enable)
            .map(|entry| entry.symbol.clone())
            .collect();
    }
    if targets.is_empty() {
        return Err(anyhow!("No governance collection targets are configured"));
    }
    targets.sort();
    targets.dedup();
    Ok(targets)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_text_is_stable() {
        assert!(gray_rhino_boundary(Language::ZhCn).contains("边界声明"));
        assert!(gray_rhino_boundary(Language::EnUs).contains("Boundary"));
        assert!(gray_rhino_boundary(Language::JaJp).contains("境界声明"));
    }

    #[test]
    fn success_message_is_localized() {
        assert_eq!(
            gray_rhino_success_message(Language::ZhCn, "GovernanceConcentration", true),
            "已摄取 GovernanceConcentration 证据。"
        );
        assert_eq!(
            gray_rhino_success_message(Language::EnUs, "GovernanceConcentration", false),
            "GovernanceConcentration evidence already exists (deduplicated)."
        );
        assert_eq!(
            gray_rhino_success_message(Language::JaJp, "GovernanceConcentration", true),
            "GovernanceConcentration の証拠を取り込みました。"
        );
    }

    #[test]
    fn provider_status_values_are_localized() {
        assert_eq!(
            gray_rhino_provider_status_value(Language::ZhCn, "succeeded"),
            "成功"
        );
        assert_eq!(
            gray_rhino_provider_status_value(Language::EnUs, "partial_failure"),
            "partial failure"
        );
        assert_eq!(
            gray_rhino_provider_status_value(Language::JaJp, "skipped"),
            "未実行"
        );
    }
}

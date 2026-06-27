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
    let _ = language;
    "Boundary: evidence only; no escalation, gate, execution, or trading state updated."
}

fn gray_rhino_success_message(language: Language, category: &str, saved: bool) -> String {
    let _ = language;
    if saved {
        format!("Successfully ingested {category} evidence.")
    } else {
        format!("{category} evidence already exists (deduplicated).")
    }
}

fn gray_rhino_title_auto_discovery(language: Language) -> &'static str {
    let _ = language;
    "--- Gray Rhino Auto Discovery ---"
}

fn gray_rhino_title_source_collection(language: Language) -> &'static str {
    let _ = language;
    "--- Gray Rhino Source Collection ---"
}

fn gray_rhino_title_governance_collection(language: Language) -> &'static str {
    let _ = language;
    "--- Gray Rhino Governance Evidence Collection ---"
}

fn gray_rhino_title_dependency_collection(language: Language) -> &'static str {
    let _ = language;
    "--- Gray Rhino Dependency Evidence Collection ---"
}

fn gray_rhino_title_category_collection(language: Language, category: &str) -> String {
    let _ = language;
    format!("--- Gray Rhino {category} Evidence Collection ---")
}

fn gray_rhino_label_source_count(language: Language) -> &'static str {
    let _ = language;
    "Sources"
}

fn gray_rhino_label_formal_persisted(language: Language) -> &'static str {
    let _ = language;
    "Formal evidence persisted"
}

fn gray_rhino_boolean_word(language: Language, value: bool) -> &'static str {
    let _ = language;
    if value {
        "true"
    } else {
        "false"
    }
}

fn gray_rhino_label_coverage(language: Language) -> &'static str {
    let _ = language;
    "Coverage"
}

fn gray_rhino_label_field_coverage(language: Language) -> &'static str {
    let _ = language;
    "Field coverage"
}

fn gray_rhino_provider_status_value(language: Language, value: &str) -> String {
    let _ = language;
    match value {
        "succeeded" => "succeeded".to_string(),
        "partial_failure" => "partial failure".to_string(),
        "failed" => "failed".to_string(),
        "skipped" => "skipped".to_string(),
        _ => value.to_string(),
    }
}

fn gray_rhino_source_collection_boundary(language: Language) -> &'static str {
    let _ = language;
    "Boundary: source collection only; no trading recommendation, no Gate override, no trend cohesion mutation, no execution action."
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
    println!("Category: GovernanceConcentration");
    println!("Source: {}", outcome.record.source.source_title);
    println!("Observed at: {}", outcome.record.source.observed_at);
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
    println!("provider: {:?}", provider);
    println!("dry_run: {}", dry_run);
    println!("source_count: {}", outcomes.len());
    let accepted = outcomes.iter().filter(|outcome| outcome.accepted).count();
    let rejected = outcomes.len().saturating_sub(accepted);
    let candidate_count: usize = outcomes.iter().map(|outcome| outcome.candidate_count).sum();
    println!("accepted: {}", accepted);
    println!("rejected: {}", rejected);
    println!("candidate_count: {}", candidate_count);
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
        "provider_status: {}",
        gray_rhino_provider_status_value(language, provider_status)
    );
    for outcome in &outcomes {
        println!(
            "- {} accepted={} planned={} candidates={} path={} taxonomy={} message={}",
            outcome.subject,
            outcome.accepted,
            outcome.planned,
            outcome.candidate_count,
            outcome.repository_path.as_deref().unwrap_or("none"),
            outcome.failure_taxonomy.as_deref().unwrap_or("none"),
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
    println!("Category: DependencyConcentration");
    println!("Source: {}", outcome.record.source.source_title);
    println!("Observed at: {}", outcome.record.source.observed_at);
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
    println!("Category: InstitutionalMaturity");
    println!("Source: {}", outcome.record.source.source_title);
    println!("Observed at: {}", outcome.record.source.observed_at);
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
    println!("Category: Redundancy");
    println!("Source: {}", outcome.record.source.source_title);
    println!("Observed at: {}", outcome.record.source.observed_at);
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
    println!("Sources:  {}", total_sources);
    println!("Accepted: {}", total_accepted);
    println!("Saved:    {}", total_saved);
    println!("Manifest: {}", total_manifest);
    println!("Audit:    {}", total_audit);
    println!("Dry run:  {}", !persist_evidence);
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
    println!("Rejected: {}", rejected.len());
    if let Some(latest) = latest_observed_at {
        println!("Latest observed date: {}", latest);
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
    println!("Sources:  {}", summary.source_count);
    println!("Accepted: {}", summary.accepted_count);
    println!("Saved:    {}", summary.saved_count);
    println!("Manifest: {}", summary.manifest_count);
    println!("Audit:    {}", summary.audit_count);
    println!("Dry run:  {}", dry_run_requested);
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
    println!("Rejected: {}", summary.rejected.len());
    if let Some(latest) = summary.latest_observed_at {
        println!("Latest observed date: {}", latest);
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
    println!("--- Gray Rhino Multi-Category Backfill Dry Run ---");
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
    println!("Backfill entries processed: {}", processed);
    println!("Backfill run summary: gray_rhino_backfill_runs.jsonl");
    println!("Boundary: dry-run only; no escalation, gate, execution, or trading state updated.");
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
    println!("Accepted: {}", usize::from(summary.accepted));
    println!("Saved: 0");
    println!("Manifest: 1");
    println!("Audit: 1");
    println!(
        "Dry run:  {}",
        gray_rhino_boolean_word(language, summary.dry_run_requested)
    );
    println!("{}: false", gray_rhino_label_formal_persisted(language));
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
    println!("Rejected: {}", usize::from(!summary.accepted));
    if !summary.accepted {
        println!("  [REJECTED:{}] {}", summary.taxonomy, summary.source_title);
    }
    println!("Latest observed date: {}", summary.observed_at);
    println!("{}", gray_rhino_boundary(language));
    Ok(())
}

fn render_dependency_field_coverage(
    metric_coverage: &[DependencyFieldCoverage],
    language: Language,
) {
    let _ = language;
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
    let _ = language;
    if metric_coverage.is_empty() {
        return;
    }
    println!("{}:", gray_rhino_label_field_coverage(Language::EnUs));
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
        assert!(gray_rhino_boundary(Language::ZhCn).contains("Boundary"));
        assert!(gray_rhino_boundary(Language::EnUs).contains("Boundary"));
        assert!(gray_rhino_boundary(Language::JaJp).contains("Boundary"));
    }

    #[test]
    fn success_message_is_stable() {
        assert_eq!(
            gray_rhino_success_message(Language::ZhCn, "GovernanceConcentration", true),
            "Successfully ingested GovernanceConcentration evidence."
        );
        assert_eq!(
            gray_rhino_success_message(Language::EnUs, "GovernanceConcentration", false),
            "GovernanceConcentration evidence already exists (deduplicated)."
        );
        assert_eq!(
            gray_rhino_success_message(Language::JaJp, "GovernanceConcentration", true),
            "Successfully ingested GovernanceConcentration evidence."
        );
    }

    #[test]
    fn provider_status_values_are_stable() {
        assert_eq!(
            gray_rhino_provider_status_value(Language::ZhCn, "succeeded"),
            "succeeded"
        );
        assert_eq!(
            gray_rhino_provider_status_value(Language::EnUs, "partial_failure"),
            "partial failure"
        );
        assert_eq!(
            gray_rhino_provider_status_value(Language::JaJp, "skipped"),
            "skipped"
        );
    }
}

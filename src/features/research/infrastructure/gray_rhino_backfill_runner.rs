use anyhow::{Context, Result};
use chrono::NaiveDate;
use sha2::{Digest, Sha256};

pub(crate) fn append_gray_rhino_backfill_run(
    save_dir: &std::path::Path,
    value: &serde_json::Value,
) -> Result<()> {
    std::fs::create_dir_all(save_dir)
        .with_context(|| format!("Failed to create output directory: {}", save_dir.display()))?;
    append_jsonl(&save_dir.join("gray_rhino_backfill_runs.jsonl"), value)
}

pub(crate) fn collect_gray_rhino_category_source(
    save_to: &str,
    category: &str,
    symbol: Option<String>,
    source_file: Option<String>,
    dry_run_requested: bool,
    observed_date_arg: Option<&str>,
    metrics: &[&str],
) -> Result<GrayRhinoCategorySourceSummary> {
    let target = symbol.ok_or_else(|| anyhow::anyhow!("--symbol is required"))?;
    let file = source_file.ok_or_else(|| anyhow::anyhow!("--file is required"))?;
    let observed_at = match observed_date_arg {
        Some(raw) => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("Invalid Gray Rhino source date: {}", raw))?,
        None => chrono::Local::now().date_naive(),
    };
    let retrieved_at = chrono::Local::now().date_naive();
    let content = std::fs::read_to_string(&file)
        .with_context(|| format!("Failed to read Gray Rhino source file: {}", file))?;
    let normalized = content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let extracted: Vec<String> = metrics
        .iter()
        .copied()
        .filter(|metric| metric_found_for_category(category, metric, &normalized))
        .map(str::to_string)
        .collect();
    let missing_count = metrics.len().saturating_sub(extracted.len());
    let accepted = !extracted.is_empty();
    let taxonomy = if accepted {
        "Accepted"
    } else {
        "MetriclessSource"
    };
    let save_dir = std::path::PathBuf::from(save_to);
    std::fs::create_dir_all(&save_dir)
        .with_context(|| format!("Failed to create output directory: {}", save_dir.display()))?;
    let manifest_path = save_dir.join(format!(
        "gray_rhino_{}_source_manifest.jsonl",
        category.to_lowercase()
    ));
    let audit_path = save_dir.join(format!(
        "gray_rhino_{}_extraction_audit.jsonl",
        category.to_lowercase()
    ));
    let manifest = serde_json::json!({
        "subject": target,
        "category": category,
        "source_title": file,
        "repository_path": file,
        "observed_at": observed_at,
        "retrieved_at": retrieved_at,
        "content_sha256": format!("{:x}", Sha256::digest(content.as_bytes()))
    });
    let audit = serde_json::json!({
        "subject": manifest["subject"],
        "category": category,
        "source_title": manifest["source_title"],
        "observed_at": observed_at,
        "retrieved_at": retrieved_at,
        "accepted": accepted,
        "rejection_taxonomy": taxonomy,
        "extracted_metrics": extracted,
        "missing_count": missing_count
    });
    append_jsonl(&manifest_path, &manifest)?;
    append_jsonl(&audit_path, &audit)?;

    Ok(GrayRhinoCategorySourceSummary {
        category: category.to_string(),
        accepted,
        dry_run_requested,
        observed_at,
        taxonomy: taxonomy.to_string(),
        source_title: manifest["source_title"].to_string(),
        metrics: metrics.iter().map(|value| (*value).to_string()).collect(),
        extracted,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GrayRhinoCategorySourceSummary {
    pub category: String,
    pub accepted: bool,
    pub dry_run_requested: bool,
    pub observed_at: NaiveDate,
    pub taxonomy: String,
    pub source_title: String,
    pub metrics: Vec<String>,
    pub extracted: Vec<String>,
}

impl GrayRhinoCategorySourceSummary {
    pub(crate) fn missing_count(&self, metric: &str) -> usize {
        usize::from(!self.extracted.iter().any(|item| item == metric))
    }
}

fn metric_found_for_category(category: &str, metric: &str, normalized: &str) -> bool {
    if normalized.contains(metric) || normalized.contains(&metric.replace('_', " ")) {
        return true;
    }
    metric_aliases(category, metric)
        .iter()
        .any(|alias| normalized.contains(alias))
}

fn metric_aliases(category: &str, metric: &str) -> &'static [&'static str] {
    match (category, metric) {
        ("InstitutionalMaturity", "succession_structure_disclosed") => &[
            "succession plan",
            "succession planning",
            "leadership transition plan",
        ],
        ("InstitutionalMaturity", "external_audit_present") => {
            &["external audit", "independent auditor", "audited by"]
        }
        ("InstitutionalMaturity", "disclosure_quality_score") => &[
            "disclosure quality",
            "comprehensive disclosure",
            "detailed disclosure",
        ],
        ("InstitutionalMaturity", "oversight_evolution_disclosed") => &[
            "oversight evolution",
            "oversight framework evolved",
            "board oversight expanded",
        ],
        ("InstitutionalMaturity", "compliance_maturity_level") => &[
            "compliance maturity",
            "mature compliance",
            "developing compliance",
        ],
        ("Redundancy", "fallback_available") => &[
            "fallback available",
            "fallback provider",
            "backup provider",
            "alternative supplier",
        ],
        ("Redundancy", "alternative_supplier_count") => &[
            "two alternative suppliers",
            "multiple alternative suppliers",
            "alternative suppliers",
        ],
        ("Redundancy", "redundancy_ratio") => &[
            "redundancy ratio",
            "redundant capacity",
            "capacity redundancy",
        ],
        ("Redundancy", "recovery_path_disclosed") => {
            &["recovery path", "recovery plan", "failover path"]
        }
        ("Redundancy", "failover_tested") => &[
            "failover tested",
            "failover test",
            "tested failover",
            "drill completed",
        ],
        _ => &[],
    }
}

fn append_jsonl(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    writeln!(file, "{}", serde_json::to_string(value)?)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

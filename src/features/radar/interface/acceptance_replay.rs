use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::features::radar::application::acceptance_replay::AcceptanceReplayPolicy;
use crate::features::research::interface::macro_event_observation::parse_finnhub_geopolitical_items_for_replay;

const RECEIPT_FILE: &str = "acceptance-replay.receipt.json";

#[derive(Debug, Error)]
pub enum AcceptanceReplayError {
    #[error("acceptance replay input unavailable: {path}: {reason}")]
    InputUnavailable { path: PathBuf, reason: String },
    #[error("acceptance replay input digest mismatch: expected {expected}, got {actual}")]
    InputDigestMismatch { expected: String, actual: String },
    #[error("acceptance replay manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("acceptance replay output must be isolated from canonical state: {output}")]
    IsolatedOutputRequired { output: PathBuf },
    #[error("acceptance replay assertion failed: {0}")]
    AssertionFailed(String),
    #[error("acceptance replay receipt write failed: {0}")]
    ReceiptWrite(#[from] io::Error),
    #[error("acceptance replay receipt serialization failed: {0}")]
    ReceiptSerialization(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct AcceptanceReplayManifest {
    report_date: String,
    input_path: PathBuf,
    input_manifest_digest: String,
    #[serde(default)]
    original_generation_revision: Option<String>,
    #[serde(default)]
    execution_git_commit_sha: Option<String>,
    canonical_state_path: PathBuf,
    decision_projection_digest: String,
    gate_projection_digest: String,
    position_sizing_projection_digest: String,
    #[serde(default)]
    assertions: ReplayAssertions,
}

#[derive(Debug, Default, Deserialize)]
struct ReplayAssertions {
    #[serde(default)]
    forbidden: Vec<ForbiddenAssertion>,
    #[serde(default)]
    required: Vec<RequiredAssertion>,
}

#[derive(Debug, Deserialize)]
struct ForbiddenAssertion {
    context: String,
    evidence_contains: String,
}

#[derive(Debug, Deserialize)]
struct RequiredAssertion {
    context: String,
    evidence_contains: Vec<String>,
}

#[derive(Debug, Clone)]
struct ReplayEvent {
    context: String,
    evidence: String,
}

/// 不変入力を現在の geopolitical parser で再計算し、隔離 receipt を生成する。
pub fn run_acceptance_replay(
    manifest_path: &Path,
    output_dir: &Path,
    notify: bool,
) -> Result<Value, AcceptanceReplayError> {
    run_acceptance_replay_with_expectations(manifest_path, output_dir, notify, None, None)
}

/// CLI から与えられた日付と revision を manifest の provenance に再検証する。
pub fn run_acceptance_replay_with_expectations(
    manifest_path: &Path,
    output_dir: &Path,
    notify: bool,
    expected_report_date: Option<&str>,
    expected_revision: Option<&str>,
) -> Result<Value, AcceptanceReplayError> {
    let manifest_bytes =
        fs::read(manifest_path).map_err(|error| AcceptanceReplayError::InputUnavailable {
            path: manifest_path.to_path_buf(),
            reason: error.to_string(),
        })?;
    let manifest: AcceptanceReplayManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| AcceptanceReplayError::InvalidManifest(error.to_string()))?;
    let report_date = NaiveDate::parse_from_str(&manifest.report_date, "%Y-%m-%d")
        .map_err(|error| AcceptanceReplayError::InvalidManifest(error.to_string()))?;
    if let Some(expected) = expected_report_date {
        if expected != manifest.report_date {
            return Err(AcceptanceReplayError::InvalidManifest(format!(
                "report_date does not match CLI expectation: expected {expected}, manifest {}",
                manifest.report_date
            )));
        }
    }
    let execution_git_commit_sha = expected_revision
        .map(ToOwned::to_owned)
        .or_else(|| manifest.execution_git_commit_sha.clone())
        .filter(|revision| !revision.trim().is_empty())
        .ok_or_else(|| {
            AcceptanceReplayError::InvalidManifest(
                "execution_git_commit_sha is required from the CLI or manifest".to_string(),
            )
        })?;
    if let (Some(expected), Some(manifest_revision)) = (
        expected_revision,
        manifest.execution_git_commit_sha.as_deref(),
    ) {
        if expected != manifest_revision {
            return Err(AcceptanceReplayError::InvalidManifest(format!(
                "execution_git_commit_sha does not match CLI expectation: expected {expected}, manifest {manifest_revision}"
            )));
        }
    }
    if manifest.input_manifest_digest.trim().is_empty() {
        return Err(AcceptanceReplayError::InvalidManifest(
            "input_manifest_digest is empty".to_string(),
        ));
    }

    let input_path = resolve_path(manifest_path, &manifest.input_path);
    let canonical_path = resolve_path(manifest_path, &manifest.canonical_state_path);
    let output_path = resolve_output_dir(manifest_path, output_dir);
    ensure_isolated_output(&output_path, &canonical_path)?;
    let input = fs::read(&input_path).map_err(|error| AcceptanceReplayError::InputUnavailable {
        path: input_path.clone(),
        reason: error.to_string(),
    })?;
    let actual_input_digest = sha256_bytes(&input);
    if actual_input_digest != manifest.input_manifest_digest {
        return Err(AcceptanceReplayError::InputDigestMismatch {
            expected: manifest.input_manifest_digest,
            actual: actual_input_digest,
        });
    }
    let canonical_before =
        digest_path(&canonical_path).map_err(|error| AcceptanceReplayError::InputUnavailable {
            path: canonical_path.clone(),
            reason: error.to_string(),
        })?;

    let raw =
        String::from_utf8(input).map_err(|error| AcceptanceReplayError::InputUnavailable {
            path: input_path.clone(),
            reason: error.to_string(),
        })?;
    let accepted_at = format!("{report_date}T23:59:59Z");
    let (events, malformed_count) =
        parse_finnhub_geopolitical_items_for_replay(&raw, report_date, &accepted_at).map_err(
            |error| AcceptanceReplayError::InputUnavailable {
                path: input_path.clone(),
                reason: error.to_string(),
            },
        )?;
    let replay_events = events
        .iter()
        .map(|event| ReplayEvent {
            context: event.title.clone(),
            evidence: format!(
                "{} {}",
                event.event_fact,
                event
                    .evidence_subjects
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        })
        .collect::<Vec<_>>();
    let warsh_passed = forbidden_assertions_pass(&manifest.assertions.forbidden, &replay_events);
    let real_geopolitical_passed =
        required_assertions_pass(&manifest.assertions.required, &replay_events);
    let lexical_passed = warsh_passed && real_geopolitical_passed;
    let canonical_after =
        digest_path(&canonical_path).map_err(|error| AcceptanceReplayError::InputUnavailable {
            path: canonical_path.clone(),
            reason: error.to_string(),
        })?;
    let canonical_unchanged = canonical_before == canonical_after;
    let policy = AcceptanceReplayPolicy::isolated();
    let receipt = json!({
        "report_date": report_date.to_string(),
        "report_lifecycle": {
            "mode": "GENERATED",
            "run_purpose": policy.run_purpose,
            "publication_scope": policy.publication_scope,
            "canonical_write": policy.canonical_write
        },
        "original_generation_revision": manifest.original_generation_revision,
        "execution_git_commit_sha": execution_git_commit_sha,
        "input_manifest_digest": manifest.input_manifest_digest,
        "canonical_snapshot_before": canonical_before,
        "canonical_snapshot_after": canonical_after,
        "canonical_state_unchanged": canonical_unchanged,
        "lexical_regression": {
            "warsh": if warsh_passed { "PASS" } else { "FAIL" },
            "real_geopolitical": if real_geopolitical_passed { "PASS" } else { "FAIL" }
        },
        "malformed_input_count": malformed_count,
        "decision_projection": {
            "before": manifest.decision_projection_digest,
            "after": manifest.decision_projection_digest
        },
        "gate_projection": {
            "before": manifest.gate_projection_digest,
            "after": manifest.gate_projection_digest
        },
        "position_sizing_projection": {
            "before": manifest.position_sizing_projection_digest,
            "after": manifest.position_sizing_projection_digest
        },
        "decision_weight": policy.decision_weight,
        "trade_signal": policy.trade_signal,
        "gate_effect": policy.gate_effect,
        "notification": {
            "source": if notify { "generated_report_payload" } else { "none" },
            "status": if notify { "requested" } else { "skipped" }
        },
        "publishable": policy.publishable
    });

    fs::create_dir_all(&output_path)?;
    let receipt_path = output_path.join(RECEIPT_FILE);
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    if !canonical_unchanged {
        return Err(AcceptanceReplayError::AssertionFailed(
            "canonical state changed during acceptance replay".to_string(),
        ));
    }
    if !lexical_passed {
        return Err(AcceptanceReplayError::AssertionFailed(
            "lexical regression assertion failed".to_string(),
        ));
    }
    Ok(receipt)
}

/// Optional delivery の結果だけを isolated receipt に反映する。
pub fn record_notification_status(
    output_dir: &Path,
    status: &crate::features::shared::application::run_status::DeliveryStatus,
) -> Result<Value, AcceptanceReplayError> {
    let receipt_path = output_dir.join(RECEIPT_FILE);
    let bytes = fs::read(&receipt_path)?;
    let mut receipt: Value = serde_json::from_slice(&bytes)?;
    let status_value = match status {
        crate::features::shared::application::run_status::DeliveryStatus::Succeeded => "succeeded",
        crate::features::shared::application::run_status::DeliveryStatus::Skipped => "skipped",
        crate::features::shared::application::run_status::DeliveryStatus::Failed { .. } => "failed",
    };
    receipt["notification"] = json!({
        "source": if matches!(status, crate::features::shared::application::run_status::DeliveryStatus::Succeeded) {
            "generated_report_payload"
        } else {
            "none"
        },
        "status": status_value
    });
    if let crate::features::shared::application::run_status::DeliveryStatus::Failed { reason } =
        status
    {
        receipt["notification"]["reason"] = Value::String(reason.clone());
    }
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt)
}

/// manifest-relative output を実行前に確定し、通知 receipt も同じ isolated path を使う。
pub fn resolve_output_dir(manifest_path: &Path, output_dir: &Path) -> PathBuf {
    resolve_path(manifest_path, output_dir)
}

fn resolve_path(manifest_path: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

fn ensure_isolated_output(output: &Path, canonical: &Path) -> Result<(), AcceptanceReplayError> {
    let canonical = absolute_path(canonical);
    let output = absolute_path(output);
    if output == canonical || output.starts_with(&canonical) {
        return Err(AcceptanceReplayError::IsolatedOutputRequired { output });
    }
    if output.exists()
        && fs::read_dir(&output)
            .map_err(AcceptanceReplayError::ReceiptWrite)?
            .next()
            .is_some()
    {
        return Err(AcceptanceReplayError::IsolatedOutputRequired { output });
    }
    Ok(())
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn forbidden_assertions_pass(assertions: &[ForbiddenAssertion], events: &[ReplayEvent]) -> bool {
    assertions.iter().all(|assertion| {
        !events.iter().any(|event| {
            normalize_context(&event.context) == normalize_context(&assertion.context)
                && event
                    .evidence
                    .to_ascii_lowercase()
                    .contains(&assertion.evidence_contains.to_ascii_lowercase())
        })
    })
}

fn required_assertions_pass(assertions: &[RequiredAssertion], events: &[ReplayEvent]) -> bool {
    assertions.iter().all(|assertion| {
        events.iter().any(|event| {
            normalize_context(&event.context) == normalize_context(&assertion.context)
                && assertion.evidence_contains.iter().all(|fragment| {
                    event
                        .evidence
                        .to_ascii_lowercase()
                        .contains(&fragment.to_ascii_lowercase())
                })
        })
    })
}

fn normalize_context(context: &str) -> String {
    context
        .chars()
        .map(|character| {
            if character == '_' {
                ' '
            } else {
                character.to_ascii_lowercase()
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn digest_path(path: &Path) -> io::Result<String> {
    if path.is_file() {
        return Ok(sha256_bytes(&fs::read(path)?));
    }
    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "canonical state path does not exist",
        ));
    }
    let mut files = Vec::new();
    collect_files(path, path, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut digest = Sha256::new();
    for (relative, bytes) in files {
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update(bytes);
        digest.update([0]);
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            files.push((relative, fs::read(path)?));
        }
    }
    Ok(())
}

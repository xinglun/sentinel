// 複数銘柄の証拠収集コマンドの統合テスト。

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn prepare_workspace() -> TempDir {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = root.join("config.toml");
    let mut raw = fs::read_to_string(&config_path).expect("failed to read base config.toml");

    // Ensure save_to points to the temp dir
    let save_to = tmp.path().to_string_lossy().to_string();
    raw = raw.replace(
        "save_to = \"./reports\"",
        &format!("save_to = \"{}\"", save_to),
    );

    // Mock Finnhub API key if not present
    if !raw.contains("[finnhub]") {
        raw.push_str("\n[finnhub]\napi_key = \"mock_key\"\n");
    }

    fs::write(tmp.path().join("config.toml"), raw).expect("failed to write temp config.toml");
    tmp
}

fn run_cli(tmp: &TempDir, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_stock-sentinel"))
        .current_dir(tmp.path())
        .args(args)
        .output()
        .expect("failed to execute stock-sentinel")
}

#[test]
fn test_collect_evidence_missing_symbols() {
    let tmp = prepare_workspace();
    let out = run_cli(&tmp, &["collect-evidence"]);

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--symbols is required"));
}

#[test]
fn test_collect_evidence_invalid_days() {
    let tmp = prepare_workspace();
    let out = run_cli(
        &tmp,
        &["collect-evidence", "--symbols", "AAPL", "--days", "foo"],
    );

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Invalid days value: foo"));
}

#[test]
fn test_collect_evidence_dry_run_fallback_no_key() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = root.join("config.toml");
    let mut raw = fs::read_to_string(&config_path).expect("failed to read base config.toml");

    // Remove finnhub section to simulate missing key
    if let Some(pos) = raw.find("[finnhub]") {
        raw.truncate(pos);
    }

    // Ensure save_to points to the temp dir
    let save_to = tmp.path().to_string_lossy().to_string();
    raw = raw.replace(
        "save_to = \"./reports\"",
        &format!("save_to = \"{}\"", save_to),
    );

    fs::write(tmp.path().join("config.toml"), raw).expect("failed to write temp config.toml");

    // Create a mock fixture file named "AAPL" in the temp dir
    fs::write(
        tmp.path().join("AAPL"),
        "Headline: Positive earnings keywords found for AAPL",
    )
    .unwrap();

    let out = run_cli(
        &tmp,
        &["collect-evidence", "--symbols", "AAPL", "--dry-run"],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Finnhub API key not found. Falling back to Fixture mode"));
    assert!(stdout.contains("Extracted 1 records"));
    assert!(stdout.contains("AAPL: EarningsValidation"));
}

#[test]
fn test_collect_evidence_sec_missing_ua() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = root.join("config.toml");
    let mut raw = fs::read_to_string(&config_path).expect("failed to read base config.toml");

    // Ensure [sec] is absent
    if let Some(pos) = raw.find("[sec]") {
        raw.truncate(pos);
    }

    fs::write(tmp.path().join("config.toml"), raw).expect("failed to write temp config.toml");

    let out = run_cli(
        &tmp,
        &["collect-evidence", "--symbols", "AAPL", "--source", "sec"],
    );

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("SEC user_agent is not configured"));
}

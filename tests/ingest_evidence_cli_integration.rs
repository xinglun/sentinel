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
fn test_ingest_evidence_valid() {
    let tmp = prepare_workspace();
    let out = run_cli(
        &tmp,
        &[
            "ingest-evidence",
            "--symbol",
            "AAPL",
            "--type",
            "capex",
            "--confidence",
            "0.9",
            "--date",
            "2024-05-01",
            "--desc",
            "Test record",
        ],
    );

    if !out.status.success() {
        eprintln!("STDOUT: {}", String::from_utf8_lossy(&out.stdout));
        eprintln!("STDERR: {}", String::from_utf8_lossy(&out.stderr));
    }
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("成功摄取 1 条自动证据记录。"));
    // Evidence interface への dispatch 後も、既存の CLI 出力契約を保持する。
    assert!(!stdout.contains("Dry-Run"));

    // Check if file exists and contains the record
    let record_file = tmp.path().join("evidence_records.jsonl");
    assert!(record_file.exists());
    let content = fs::read_to_string(record_file).unwrap();
    assert!(content.contains("\"symbol\":\"AAPL\""));
    assert!(content.contains("\"confidence\":0.9"));
    assert!(content.contains("\"event_date\":\"2024-05-01\""));
}

#[test]
fn test_ingest_evidence_invalid_date() {
    let tmp = prepare_workspace();
    let out = run_cli(
        &tmp,
        &[
            "ingest-evidence",
            "--symbol",
            "AAPL",
            "--date",
            "invalid-date",
        ],
    );

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Invalid date format"));
}

#[test]
fn test_ingest_evidence_invalid_confidence() {
    let tmp = prepare_workspace();
    let out = run_cli(
        &tmp,
        &["ingest-evidence", "--symbol", "AAPL", "--confidence", "2.0"],
    );

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Confidence must be between 0.0 and 1.0"));
}

#[test]
fn test_ingest_evidence_rejects_non_numeric_confidence() {
    let tmp = prepare_workspace();
    let out = run_cli(
        &tmp,
        &["ingest-evidence", "--symbol", "AAPL", "--confidence", "abc"],
    );

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Invalid confidence value: abc"));
}

#[test]
fn test_ingest_evidence_rejects_missing_confidence_value() {
    let tmp = prepare_workspace();
    let out = run_cli(
        &tmp,
        &["ingest-evidence", "--symbol", "AAPL", "--confidence"],
    );

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Missing value for --confidence"));
}

#[test]
fn test_ingest_evidence_deduplication() {
    let tmp = prepare_workspace();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // First ingest
    run_cli(
        &tmp,
        &[
            "ingest-evidence",
            "--symbol",
            "GOOG",
            "--type",
            "earnings",
            "--date",
            &today,
        ],
    );

    // Second ingest (same everything)
    let out = run_cli(
        &tmp,
        &[
            "ingest-evidence",
            "--symbol",
            "GOOG",
            "--type",
            "earnings",
            "--date",
            &today,
        ],
    );

    if !out.status.success() {
        eprintln!("STDOUT: {}", String::from_utf8_lossy(&out.stdout));
        eprintln!("STDERR: {}", String::from_utf8_lossy(&out.stderr));
    }
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("证据记录已存在（已去重）。"));
}

#[test]
fn test_ingest_evidence_url_sec_missing_ua() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = root.join("config.toml");
    let raw = fs::read_to_string(&config_path).expect("failed to read base config.toml");

    // Comment out [sec] section instead of truncating
    let modified_config = raw
        .replace("[sec]", "# [sec]")
        .replace("user_agent =", "# user_agent =");
    fs::write(tmp.path().join("config.toml"), modified_config)
        .expect("failed to write temp config.toml");

    let out = run_cli(
        &tmp,
        &[
            "ingest-evidence-url",
            "--symbol",
            "AAPL",
            "--url",
            "sec://recent",
        ],
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("SEC user_agent is not configured"));
}

#[test]
fn test_ingest_evidence_url_dry_run_shows_date_and_url() {
    let tmp = prepare_workspace();
    fs::write(
        tmp.path().join("GOOG.fixture"),
        "Headline: GOOG earnings validation\nSummary: revenue growth and margin expansion",
    )
    .expect("failed to write fixture");

    let out = run_cli(
        &tmp,
        &[
            "ingest-evidence-url",
            "--symbol",
            "GOOG",
            "--url",
            "GOOG.fixture",
            "--dry-run",
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert!(stdout.contains("日期:"));
    assert!(stdout.contains(&today));
    assert!(stdout.contains("URL:  file://GOOG.fixture"));
}

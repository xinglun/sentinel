// daily_radar.yml の証拠収集ステップを実行契約として検証する。

use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;

const STEP_NAME: &str = "Collect Evidence (non-blocking)";

fn extract_collect_evidence_script() -> String {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(workflow_path).expect("failed to read daily_radar.yml");
    let lines: Vec<&str> = workflow.lines().collect();
    let step_idx = lines
        .iter()
        .position(|line| line.trim() == format!("- name: {STEP_NAME}"))
        .expect("Collect Evidence step is missing");
    let run_idx = lines[step_idx..]
        .iter()
        .position(|line| line.trim() == "run: |")
        .map(|idx| step_idx + idx)
        .expect("Collect Evidence run block is missing");

    let mut script = String::new();
    for line in lines.iter().skip(run_idx + 1) {
        if line.starts_with("      - name:") {
            break;
        }
        if line.trim().is_empty() {
            script.push('\n');
        } else {
            let stripped = line
                .strip_prefix("          ")
                .expect("run block line must keep workflow indentation");
            script.push_str(stripped);
            script.push('\n');
        }
    }

    assert!(
        script.contains("evidence_collection_status_latest.json"),
        "script must write evidence collection status"
    );
    assert!(
        script.contains("exit 0"),
        "script must keep evidence collection non-blocking"
    );
    script
}

fn write_script(dir: &Path) -> std::path::PathBuf {
    let script_path = dir.join("collect_evidence_step.sh");
    fs::write(&script_path, extract_collect_evidence_script()).expect("failed to write script");
    script_path
}

#[test]
fn daily_radar_collect_evidence_step_has_valid_shell_syntax() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = write_script(tmp.path());

    let output = Command::new("bash")
        .arg("-n")
        .arg(&script_path)
        .output()
        .expect("failed to run bash -n");

    assert!(
        output.status.success(),
        "Collect Evidence shell syntax failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn daily_radar_collect_evidence_bad_config_writes_failed_status_without_blocking() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = write_script(tmp.path());
    fs::write(tmp.path().join("config.toml"), "not = [valid\n").unwrap();

    let output = Command::new("bash")
        .arg(&script_path)
        .current_dir(tmp.path())
        .env("EVIDENCE_DAYS", "7")
        .env("FINNHUB_API_KEY", "")
        .env("SEC_USER_AGENT", "")
        .output()
        .expect("failed to run Collect Evidence step");

    assert!(
        output.status.success(),
        "Collect Evidence must not block radar on config errors: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status_path = tmp
        .path()
        .join("reports")
        .join("evidence_collection_status_latest.json");
    let status: Value = serde_json::from_str(&fs::read_to_string(status_path).unwrap())
        .expect("invalid status JSON");
    assert_eq!(status["status"], "failed");
    assert!(
        status["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("unexpected evidence collection step error"),
        "failed status should record a diagnostic reason"
    );
}

#[test]
fn daily_radar_collect_evidence_missing_key_writes_skipped_status_without_blocking() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = write_script(tmp.path());
    fs::write(
        tmp.path().join("config.toml"),
        r#"
[[watchlist]]
symbol = "GOOG"
enable = true
"#,
    )
    .unwrap();

    let output = Command::new("bash")
        .arg(&script_path)
        .current_dir(tmp.path())
        .env("EVIDENCE_DAYS", "7")
        .env("FINNHUB_API_KEY", "")
        .env("SEC_USER_AGENT", "")
        .output()
        .expect("failed to run Collect Evidence step");

    assert!(
        output.status.success(),
        "Collect Evidence must not block radar when Finnhub key is absent"
    );

    let status_path = tmp
        .path()
        .join("reports")
        .join("evidence_collection_status_latest.json");
    let status: Value = serde_json::from_str(&fs::read_to_string(status_path).unwrap())
        .expect("invalid status JSON");
    assert_eq!(status["status"], "skipped");
    assert_eq!(status["reason"], "FINNHUB_API_KEY is not configured");
    assert_eq!(status["symbols"][0], "GOOG");
}

#[test]
fn daily_radar_restores_and_validates_formal_history_without_reimplementing_migration() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(workflow_path).expect("failed to read daily_radar.yml");

    assert!(workflow.contains("RESTORED_SNAPSHOT_COUNT"));
    assert!(workflow.contains("RESTORED_LEGACY_PACKET_COUNT"));
    assert!(workflow.contains("mkdir -p reports/snapshots"));
    assert!(workflow.contains("Legacy decision history exists but formal trading-day snapshots"));
    assert!(workflow.contains("legacy history was not fully backfilled into formal snapshots"));
    assert!(workflow.contains("make radar-release"));
    assert!(workflow.contains("formal snapshot history did not append across the new market date"));
    assert!(!workflow.contains("packet-to-snapshot"));
    assert!(!workflow.contains("MIGRATED_LEGACY"));
}

#[test]
fn daily_radar_checks_out_the_triggered_ref_and_commit() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(workflow_path).expect("failed to read daily_radar.yml");

    assert!(workflow.contains("ref: ${{ github.ref_name }}"));
    assert!(workflow.contains("CHECKED_OUT_SHA=\"$(git rev-parse HEAD)\""));
    assert!(workflow.contains("test \"${CHECKED_OUT_SHA}\" = \"${GITHUB_SHA}\""));
}

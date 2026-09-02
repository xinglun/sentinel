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

fn extract_step_script(workflow_path: &Path, step_name: &str) -> String {
    let workflow = fs::read_to_string(workflow_path).expect("failed to read workflow");
    let lines: Vec<&str> = workflow.lines().collect();
    let step_idx = lines
        .iter()
        .position(|line| line.trim() == format!("- name: {step_name}"))
        .expect("workflow step is missing");
    let run_idx = lines[step_idx..]
        .iter()
        .position(|line| line.trim() == "run: |")
        .map(|idx| step_idx + idx)
        .expect("workflow step run block is missing");

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
fn daily_radar_failure_notification_has_secrets_and_fails_closed() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(&workflow_path).expect("failed to read daily_radar.yml");
    let script = extract_step_script(&workflow_path, "Notify on Failure");

    assert!(workflow.contains(
        "TELEGRAM_BOT_TOKEN: ${{ secrets.TELEGRAM_BOT_TOKEN }}\n          TELEGRAM_CHAT_ID: ${{ secrets.TELEGRAM_CHAT_ID }}"
    ));
    assert!(script.contains("set -euo pipefail"));
    assert!(script.contains("curl --fail-with-body -sS"));
    assert!(script.contains("jq -e '.ok == true'"));
    assert!(!script.contains("|| echo \"Failed to send Telegram notification\""));
}

#[test]
fn daily_radar_manual_resend_reuses_archived_report_and_has_valid_shell_syntax() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(&workflow_path).expect("failed to read daily_radar.yml");
    let script = extract_step_script(&workflow_path, "Resend Existing Daily Report");
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = tmp.path().join("resend_daily_report.sh");
    fs::write(&script_path, &script).expect("failed to write resend script");

    let output = Command::new("bash")
        .arg("-n")
        .arg(&script_path)
        .output()
        .expect("failed to run bash -n");
    assert!(
        output.status.success(),
        "resend shell syntax failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(workflow.contains("type: choice"));
    assert!(workflow.contains("resend"));
    assert!(workflow.contains("inputs.mode != 'resend'"));
    assert!(script.contains("reports/${DATE_JST}.md"));
    assert!(script.contains("run_status_${DATE_JST}.json"));
    assert!(script.contains("api.telegram.org"));
    assert!(script.contains("\"ok\""));
    assert!(script.contains("notification_resend"));
    assert!(script.contains("report_lifecycle"));
    assert!(script.contains("\"mode\": \"RESENT\""));
    assert!(workflow.contains("SENTINEL_EXECUTION_GIT_SHA: ${{ github.sha }}"));
    assert!(workflow.contains("SENTINEL_EXECUTION_GIT_BRANCH: ${{ github.ref_name }}"));
    assert!(!script.contains("make radar-release"));
}

#[test]
fn data_branch_write_back_steps_have_valid_shell_syntax() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    for (workflow_name, step_name) in [
        ("daily_radar.yml", "Commit and Push to Data Worktree"),
        ("weekly_backtest.yml", "Commit and Push to Data Worktree"),
        (
            "weekly_backtest.yml",
            "Snapshot Backtest Summary (latest + archive)",
        ),
    ] {
        let workflow_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".github/workflows")
            .join(workflow_name);
        let script_path = tmp.path().join(format!("{workflow_name}.sh"));
        fs::write(&script_path, extract_step_script(&workflow_path, step_name))
            .expect("failed to write extracted workflow script");

        let output = Command::new("bash")
            .arg("-n")
            .arg(&script_path)
            .output()
            .expect("failed to run bash -n");
        assert!(
            output.status.success(),
            "{workflow_name} write-back shell syntax failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn weekly_backtest_archives_validation_utility_without_overwriting_date_archive() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/weekly_backtest.yml");
    let workflow = fs::read_to_string(workflow_path).expect("failed to read weekly_backtest.yml");

    assert!(workflow.contains("test -s backtest/enhanced/validation.json"));
    assert!(workflow.contains("backtest/validation_latest.json"));
    assert!(workflow
        .contains("VALIDATION_ARCHIVE_PATH=\"backtest/archive/validation_${DATE_JST}.json\""));
    assert!(workflow.contains("Validation archive already exists"));
    assert!(workflow.contains("keeping existing, not overwriting"));
    assert!(workflow.contains(
        "rsync -a \"${ROOT_DIR}/backtest/validation_latest.json\" \"${DATA_DIR}/backtest/\""
    ));
    assert!(workflow
        .contains("rsync -a \"${ROOT_DIR}/backtest/archive/\" \"${DATA_DIR}/backtest/archive/\""));
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
    assert!(workflow.contains("formal snapshot history did not append across the new report date"));
    assert!(!workflow.contains("packet-to-snapshot"));
    assert!(!workflow.contains("MIGRATED_LEGACY"));
}

#[test]
fn daily_radar_fails_closed_when_existing_data_branch_history_cannot_be_restored() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(workflow_path).expect("failed to read daily_radar.yml");

    assert!(
        workflow.contains("REMOTE_DATA_BRANCH_EXISTS"),
        "restore step must distinguish an absent data branch from a failed restore"
    );
    assert!(
        workflow.contains("refusing to continue with empty reports"),
        "existing data history must not be replaced by an empty reports directory"
    );
    assert!(
        workflow.contains("Fetched data branch has no reports tree"),
        "a branch that appears during restore must also fail closed when it has no reports tree"
    );
    assert!(
        workflow.contains("git ls-remote --exit-code --heads origin data"),
        "restore failure handling must verify whether the remote data branch exists"
    );
    assert!(
        workflow.contains("REMOTE_DATA_BRANCH_LOOKUP_STATUS"),
        "restore step must preserve the remote branch lookup exit status"
    );
    assert!(
        workflow.contains("refusing to bootstrap or overwrite data"),
        "commit step must not bootstrap after an indeterminate remote branch lookup"
    );
    assert!(
        workflow.contains("RESTORED_HISTORY_COUNT"),
        "daily validation must remember the restored observation history count"
    );
    assert!(
        workflow.contains("observation history state count did not append"),
        "daily validation must reject a snapshot-only append without state count growth"
    );
    assert!(
        workflow.contains("type(history_count) is not int"),
        "daily validation must reject boolean or otherwise non-integer history counts"
    );
    assert!(
        workflow.contains("Remote data branch persistence verified"),
        "daily write-back must verify the persisted remote history state"
    );
    assert!(
        workflow.contains("remote observation history count is behind"),
        "daily write-back must reject a remote state that lost observations"
    );
    assert!(
        workflow.contains("2)"),
        "only the explicit no-ref status may be treated as an absent data branch"
    );
}

#[test]
fn weekly_backtest_fails_closed_when_existing_data_branch_history_cannot_be_restored() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/weekly_backtest.yml");
    let workflow = fs::read_to_string(workflow_path).expect("failed to read weekly_backtest.yml");

    assert!(
        workflow.contains("REMOTE_DATA_BRANCH_EXISTS"),
        "weekly restore step must distinguish an absent data branch from a failed restore"
    );
    assert!(
        workflow.contains("refusing to continue with empty reports"),
        "weekly backtest must not replace existing data history with an empty reports directory"
    );
    assert!(
        workflow.contains("Fetched data branch has no reports tree"),
        "weekly restore must fail closed for a branch created during the lookup race"
    );
    assert!(
        workflow.contains("refusing to bootstrap or overwrite data"),
        "weekly backtest must not bootstrap after an indeterminate remote branch lookup"
    );
    assert!(
        workflow.contains("REMOTE_DATA_BRANCH_LOOKUP_STATUS"),
        "weekly restore must preserve the remote branch lookup exit status"
    );
    assert!(
        workflow.contains("2)"),
        "only the explicit no-ref status may be treated as an absent data branch"
    );
    assert!(
        workflow.contains("Remote data branch persistence verified"),
        "weekly write-back must verify the persisted remote history state when present"
    );
    assert!(
        workflow.contains("No observation history state carried by weekly backtest"),
        "weekly bootstrap without history must explicitly record the verification skip"
    );
}

#[test]
fn data_branch_writers_share_one_concurrency_group() {
    let daily_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let weekly_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/weekly_backtest.yml");
    let daily = fs::read_to_string(daily_path).expect("failed to read daily_radar.yml");
    let weekly = fs::read_to_string(weekly_path).expect("failed to read weekly_backtest.yml");

    assert!(
        daily.contains("group: sentinel-data-branch"),
        "daily radar must serialize writes to the shared data branch"
    );
    assert!(
        weekly.contains("group: sentinel-data-branch"),
        "weekly backtest must serialize writes to the shared data branch"
    );
    assert!(
        daily.contains("cancel-in-progress: false") && weekly.contains("cancel-in-progress: false"),
        "data branch writers must finish in order instead of cancelling a history write"
    );
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

#[test]
fn daily_radar_requires_current_report_and_fails_on_decisioning_failure() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(workflow_path).expect("failed to read daily_radar.yml");

    assert!(
        workflow.contains("make radar-release"),
        "daily radar must execute the release runner whose packet date is report_date"
    );
    assert!(
        workflow.contains("REPORT_PACKET_PATH=\"reports/decision_packet_${DATE_JST}.json\""),
        "daily radar must resolve the packet for the current JST date"
    );
    assert!(
        workflow.contains("RUN_STATUS_PATH=\"reports/run_status_${DATE_JST}.json\""),
        "daily radar must validate the run status for the current JST date"
    );
    assert!(
        workflow.contains("decisioning status is not succeeded"),
        "decisioning failures must fail the workflow and activate Notify on Failure"
    );
    assert!(
        workflow.contains("decisioning_failed_reason="),
        "missing packets must expose the persisted decisioning failure reason"
    );
    assert!(
        workflow.contains("REPORT_DATE_JST=\"${DATE_JST}\""),
        "later workflow steps must use the current JST date"
    );
    assert!(
        !workflow.contains("find reports -maxdepth 1 -type f -name 'decision_packet_*.json'"),
        "daily radar must not fall back to a stale packet"
    );
}

#[test]
fn daily_radar_snapshot_gate_uses_report_date_and_preserves_market_date() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(workflow_path).expect("failed to read daily_radar.yml");

    assert!(
        workflow.contains("snapshot_report_date = value.get(\"report_date\")"),
        "snapshot restore must prefer report_date"
    );
    assert!(
        workflow.contains("snapshot_report_date = snapshot.get(\"report_date\")"),
        "freshness validation must inspect the new report_date field"
    );
    assert!(
        workflow.contains("legacy current snapshot market_date does not match report date"),
        "legacy snapshots must keep an explicit market_date fallback"
    );
    assert!(
        workflow.contains("if not isinstance(snapshot.get(\"market_date\"), str)"),
        "new snapshots must still carry the market-date fact"
    );
    assert!(
        !workflow.contains("CURRENT_SNAPSHOT=\"$(find reports/snapshots -maxdepth 1 -type f -name \"*_${DATE_JST}.json\""),
        "freshness validation must not infer the report date from the snapshot filename"
    );
}

#[test]
fn daily_radar_freshness_gate_has_valid_shell_syntax() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = tmp.path().join("freshness_gate.sh");
    fs::write(
        &script_path,
        extract_step_script(&workflow_path, "Freshness Gate and Output Validation"),
    )
    .expect("failed to write extracted workflow script");

    let output = Command::new("bash")
        .arg("-n")
        .arg(&script_path)
        .output()
        .expect("failed to run bash -n");
    assert!(
        output.status.success(),
        "Freshness Gate shell syntax failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn daily_radar_run_step_has_valid_shell_syntax() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = tmp.path().join("run_sentinel_radar.sh");
    fs::write(
        &script_path,
        extract_step_script(&workflow_path, "Run Sentinel Radar"),
    )
    .expect("failed to write extracted workflow script");

    let output = Command::new("bash")
        .arg("-n")
        .arg(&script_path)
        .output()
        .expect("failed to run bash -n");
    assert!(
        output.status.success(),
        "Run Sentinel Radar shell syntax failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

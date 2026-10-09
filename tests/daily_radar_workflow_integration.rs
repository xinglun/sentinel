// daily_radar.yml の証拠収集ステップを実行契約として検証する。

use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

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

fn extract_report_date_resolver_script(workflow_path: &Path) -> String {
    let script = extract_step_script(workflow_path, "Resolve Report Date");
    assert!(
        script.contains("REPORT_DATE_JST"),
        "report date resolver must export REPORT_DATE_JST"
    );
    assert!(
        script.contains("GITHUB_EVENT_NAME"),
        "report date resolver must distinguish scheduled and manual runs"
    );
    assert!(
        !script.contains("make radar-release"),
        "report date resolver must not generate a report"
    );
    assert!(
        !script.contains("api.telegram.org") && !script.contains("TELEGRAM_BOT_TOKEN"),
        "report date resolver must not send Telegram messages"
    );
    script
}

fn run_report_date_resolver(
    script: &str,
    event_name: &str,
    now_jst: &str,
    report_date_input: &str,
    mode_input: &str,
) -> Result<String, String> {
    let tmp = tempfile::tempdir().expect("failed to create report date resolver fixture");
    let script_path = tmp.path().join("resolve_report_date.sh");
    let github_env = tmp.path().join("github_env");
    fs::write(&script_path, script).expect("failed to write report date resolver script");

    let output = Command::new("bash")
        .arg(&script_path)
        .env("GITHUB_ENV", &github_env)
        .env("GITHUB_EVENT_NAME", event_name)
        .env("SENTINEL_NOW_JST", now_jst)
        .env("REPORT_DATE_INPUT", report_date_input)
        .env("MODE_INPUT", mode_input)
        .output()
        .expect("failed to run report date resolver");
    if !output.status.success() {
        return Err(format!(
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    fs::read_to_string(github_env)
        .map_err(|error| format!("report date resolver did not write GITHUB_ENV: {error}"))
}

fn run_market_date_resolver(run_date_jst: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_stock-sentinel"))
        .args(["resolve-market-date", "--date", run_date_jst])
        .output()
        .expect("failed to run market date resolver");
    assert!(
        output.status.success(),
        "market date resolver failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("market date resolver output must be UTF-8")
        .trim()
        .to_string()
}

fn successful_reuse_status(report_date: &str, market_date: &str, run_id: &str) -> Value {
    serde_json::json!({
        "date": report_date,
        "decisioning": "succeeded",
        "notification": "succeeded",
        "runtime_identity": {
            "report_run_id": run_id,
            "report_run_at": "2026-10-06T06:00:00+09:00",
            "data_snapshot_date": market_date
        },
        "runtime_integrity": {
            "status": "HEALTHY",
            "report_artifact_matches_run": true
        }
    })
}

fn reuse_packet(report_date: &str, market_date: &str) -> Value {
    serde_json::json!({
        "date": report_date,
        "market_features": {
            "date": market_date,
            "test_ratio": 0.000001,
            "small_threshold": 0.00001,
            "large_threshold": 1e15,
            "scientific_large": 1e16,
            "precise_ratio": 1.2345678901234567,
            "literal_text": "1e-05"
        }
    })
}

fn decision_packet_digest(packet: &Value) -> String {
    use sha2::{Digest, Sha256};

    format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(packet).expect("failed to serialize packet"))
    )
}

fn canonical_probe_fact(market_date: &str, run_id: &str) -> Value {
    canonical_probe_fact_with_permission(market_date, run_id, "PROBE")
}

fn canonical_probe_fact_with_permission(
    market_date: &str,
    run_id: &str,
    permission: &str,
) -> Value {
    serde_json::json!({
        "market_date": market_date,
        "permission": permission,
        "eligible_asset_count": 0,
        "report_run_id": run_id,
        "observed_at": "2026-10-06T06:00:00+09:00",
        "provenance": "canonical-final-execution-v1"
    })
}

fn probe_observation_window(fact: &Value, sessions: usize) -> Value {
    serde_json::json!({
        "as_of": fact["market_date"],
        "history_window_sessions": sessions,
        "known_permission_days": 1,
        "unknown_permission_days": 0,
        "permission_coverage_rate": 1.0,
        "known_probe_eligibility_days": 1,
        "unknown_probe_eligibility_days": 0,
        "canonical_history_started_at": fact["market_date"],
        "history_quality_reasons": [],
        "canonical_provenance": [{
            "market_date": fact["market_date"],
            "permission": fact["permission"],
            "eligible_asset_count": fact["eligible_asset_count"],
            "provenance": fact["provenance"],
            "report_run_id": fact["report_run_id"],
            "observed_at": fact["observed_at"]
        }],
        "history_coverage": "COMPLETE",
        "quality": "COMPLETE",
        "probe_open_days": 1,
        "probe_days_with_eligible_assets": 0,
        "eligible_known_probe_days": 0,
        "probe_day_conversion_rate": null,
        "completed_probe_episodes": 0,
        "converted_probe_episodes": 0,
        "probe_episode_conversion_rate": null,
        "current_probe_streak_days": 1,
        "current_probe_streak_eligible_days": 0,
        "avg_completed_probe_episode_days": null,
        "median_first_eligible_latency_days": null,
        "fallback_to_no_trade_count": 0,
        "fallback_to_no_trade_rate": null,
        "escalated_to_ready_count": 0,
        "escalated_to_ready_rate": null,
        "unknown_day_count": 0,
        "excluded_unknown_days": 0,
        "excluded_unknown_eligibility_days": 0,
        "episodes": [],
        "decision_weight": 0,
        "trade_signal": false
    })
}

fn successful_trading_day_snapshot(
    report_date: &str,
    market_date: &str,
    run_id: &str,
    packet_digest: &str,
) -> Value {
    serde_json::json!({
        "report_date": report_date,
        "market_date": market_date,
        "report_run_id": run_id,
        "schema_version": "1",
        "generated_at": "2026-10-06T06:00:00+09:00",
        "is_valid_trading_day": true,
        "decision_packet_digest": packet_digest,
        "runtime_integrity": {
            "status": "HEALTHY",
            "report_artifact_matches_run": true
        }
    })
}

fn run_reuse_report_probe(
    script: &str,
    status: &Value,
    packet: &Value,
    expected_market_date: &str,
    canonical: Option<(&str, &str)>,
    include_artifacts: bool,
) -> String {
    let tmp = tempfile::tempdir().expect("failed to create reuse probe fixture");
    let reports = tmp.path().join("reports");
    fs::create_dir_all(&reports).expect("failed to create reports directory");
    let report_date = status["fixture_expected_report_date"]
        .as_str()
        .or_else(|| status["date"].as_str())
        .unwrap_or("2026-09-10");
    let report_run_id = status["runtime_identity"]["report_run_id"]
        .as_str()
        .unwrap_or("run-1");
    let marker_run_id = status["fixture_report_run_id"]
        .as_str()
        .unwrap_or(report_run_id);
    fs::write(
        reports.join(format!("run_status_{report_date}.json")),
        serde_json::to_vec_pretty(status).expect("failed to serialize status fixture"),
    )
    .expect("failed to write status fixture");
    if include_artifacts {
        fs::write(
            reports.join(format!("decision_packet_{report_date}.json")),
            serde_json::to_vec_pretty(packet).expect("failed to serialize packet fixture"),
        )
        .expect("failed to write packet fixture");
        let marker = format!("<!-- report_run_id: {marker_run_id} -->\n");
        fs::write(reports.join(format!("{report_date}.md")), &marker)
            .expect("failed to write Markdown fixture");
        fs::write(
            reports.join(format!("telegram_report_{report_date}.html")),
            &marker,
        )
        .expect("failed to write Telegram fixture");

        let snapshot_run_id = status["fixture_snapshot_run_id"]
            .as_str()
            .unwrap_or(report_run_id);
        let snapshot_market_date = status["runtime_identity"]["data_snapshot_date"]
            .as_str()
            .unwrap_or("");
        let packet_digest = status["fixture_snapshot_packet_digest"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| decision_packet_digest(packet));
        let mut snapshot = successful_trading_day_snapshot(
            report_date,
            snapshot_market_date,
            snapshot_run_id,
            &packet_digest,
        );
        if status["fixture_snapshot_schema_version_missing"] == true {
            snapshot.as_object_mut().unwrap().remove("schema_version");
        }
        let snapshots = reports.join("snapshots");
        fs::create_dir_all(&snapshots).expect("failed to create trading day snapshot directory");
        fs::write(
            snapshots.join(format!("cycle_{report_date}.json")),
            serde_json::to_vec_pretty(&snapshot).expect("failed to serialize snapshot fixture"),
        )
        .expect("failed to write snapshot fixture");
    }
    if let Some((path, contents)) = canonical {
        let probe_dir = reports.join("probe_observations");
        fs::create_dir_all(&probe_dir).expect("failed to create Probe observation directory");
        fs::write(probe_dir.join(path), contents).expect("failed to write Probe fixture");

        let canonical_value: Value = serde_json::from_str(contents)
            .unwrap_or_else(|_| canonical_probe_fact(expected_market_date, report_run_id));
        let archive_run_id = status["fixture_archive_run_id"]
            .as_str()
            .unwrap_or_else(|| {
                canonical_value["report_run_id"]
                    .as_str()
                    .unwrap_or(report_run_id)
            });
        let mut archive_fact = canonical_probe_fact(expected_market_date, archive_run_id);
        if let Some(permission) = canonical_value["permission"].as_str() {
            archive_fact["permission"] = serde_json::json!(permission);
        }
        if let Some(permission) = status["fixture_archive_permission"].as_str() {
            archive_fact["permission"] = serde_json::json!(permission);
        }
        let session_20 = probe_observation_window(&archive_fact, 20);
        let session_60 = probe_observation_window(&archive_fact, 60);
        let all_history = probe_observation_window(&archive_fact, 2);
        let archive = serde_json::json!({
            "session_20": session_20,
            "session_60": session_60,
            "all_history": all_history,
            "history_read_failed": false
        });
        let archives_dir = reports.join("probe_observation_archives");
        if status["fixture_archive_missing"] != true {
            fs::create_dir_all(&archives_dir)
                .expect("failed to create Probe observation archive directory");
            fs::write(
                archives_dir.join(format!("{report_run_id}.json")),
                serde_json::to_vec_pretty(&archive)
                    .expect("failed to serialize Probe archive fixture"),
            )
            .expect("failed to write Probe archive fixture");
        }
    }
    let original_canonical = canonical.map(|(path, contents)| {
        (
            reports.join("probe_observations").join(path),
            contents.as_bytes().to_vec(),
        )
    });

    let script_path = tmp.path().join("reuse_report.sh");
    let github_output = tmp.path().join("github_output");
    fs::write(&script_path, script).expect("failed to write reuse report script");
    let output = Command::new("bash")
        .arg(&script_path)
        .current_dir(tmp.path())
        .env("REPORT_DATE_JST", report_date)
        .env("EXPECTED_MARKET_DATE", expected_market_date)
        .env("GITHUB_OUTPUT", &github_output)
        .output()
        .expect("failed to run reuse report probe");
    assert!(
        output.status.success(),
        "reuse report probe failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output =
        fs::read_to_string(github_output).expect("reuse report probe did not write GITHUB_OUTPUT");
    if output.contains("reuse=true") {
        if let Some((canonical_path, original_bytes)) = original_canonical {
            assert_eq!(
                fs::read(canonical_path).expect("canonical fact should remain available"),
                original_bytes,
                "reuse must not rewrite or duplicate the canonical fact"
            );
        }
    }
    output
}

fn run_failed_radar_generation(script: &str, report_date: &str) -> (bool, String, Vec<u8>) {
    let tmp = tempfile::tempdir().expect("failed to create failed generation fixture");
    let reports = tmp.path().join("reports");
    fs::create_dir_all(&reports).expect("failed to create reports directory");
    let old_status = serde_json::to_vec(&successful_reuse_status(
        report_date,
        "2026-10-05",
        "old-run",
    ))
    .expect("failed to serialize old status");
    let status_path = reports.join(format!("run_status_{report_date}.json"));
    fs::write(&status_path, &old_status).expect("failed to write old status");
    fs::write(
        reports.join(format!("decision_packet_{report_date}.json")),
        serde_json::to_vec(&reuse_packet(report_date, "2026-10-05"))
            .expect("failed to serialize old packet"),
    )
    .expect("failed to write old packet");
    fs::write(reports.join(format!("{report_date}.md")), "old report\n")
        .expect("failed to write old report");
    fs::write(
        reports.join(format!("telegram_report_{report_date}.html")),
        "old notification\n",
    )
    .expect("failed to write old notification");

    let fake_bin = tmp.path().join("bin");
    fs::create_dir_all(&fake_bin).expect("failed to create fake bin directory");
    let make_path = fake_bin.join("make");
    fs::write(
        &make_path,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$MAKE_LOG\"\nexit 23\n",
    )
    .expect("failed to write fake make command");
    let mut permissions = fs::metadata(&make_path)
        .expect("failed to inspect fake make command")
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(0o755);
    fs::set_permissions(&make_path, permissions).expect("failed to make command executable");

    let script_path = tmp.path().join("run_radar.sh");
    fs::write(&script_path, script).expect("failed to write Radar step script");
    let make_log = tmp.path().join("make.log");
    let mut path = vec![fake_bin.to_string_lossy().to_string()];
    path.push(std::env::var("PATH").unwrap_or_default());
    let output = Command::new("bash")
        .arg(&script_path)
        .current_dir(tmp.path())
        .env("PATH", path.join(":"))
        .env("REPORT_DATE_JST", report_date)
        .env("GITHUB_ENV", tmp.path().join("github_env"))
        .env("MAKE_LOG", &make_log)
        .output()
        .expect("failed to run Radar step probe");
    let log = fs::read_to_string(make_log).unwrap_or_default();
    let status_after = fs::read(status_path).expect("old run status should remain available");
    (output.status.success(), log, status_after)
}

fn run_radar_generation_probe(
    script: &str,
    report_date: &str,
    expected_market_date: &str,
    generated_market_date: &str,
) -> (bool, String, String, String, String) {
    let tmp = tempfile::tempdir().expect("failed to create Radar generation fixture");
    let reports = tmp.path().join("reports");
    let generated_artifacts = tmp.path().join("generated_artifacts");
    fs::create_dir_all(&generated_artifacts).expect("failed to create generated artifact fixture");
    let run_id = "run-new";
    let status = successful_reuse_status(report_date, generated_market_date, run_id);
    let packet = reuse_packet(report_date, generated_market_date);
    fs::write(
        generated_artifacts.join(format!("run_status_{report_date}.json")),
        serde_json::to_vec_pretty(&status).expect("failed to serialize run status fixture"),
    )
    .expect("failed to write run status fixture");
    fs::write(
        generated_artifacts.join(format!("decision_packet_{report_date}.json")),
        serde_json::to_vec_pretty(&packet).expect("failed to serialize packet fixture"),
    )
    .expect("failed to write packet fixture");
    let marker = format!("<!-- report_run_id: {run_id} -->\n");
    fs::write(
        generated_artifacts.join(format!("{report_date}.md")),
        &marker,
    )
    .expect("failed to write Markdown fixture");
    fs::write(
        generated_artifacts.join(format!("telegram_report_{report_date}.html")),
        &marker,
    )
    .expect("failed to write Telegram fixture");

    let snapshot = successful_trading_day_snapshot(
        report_date,
        generated_market_date,
        run_id,
        &decision_packet_digest(&packet),
    );
    let snapshots = generated_artifacts.join("snapshots");
    fs::create_dir_all(&snapshots).expect("failed to create snapshot directory");
    fs::write(
        snapshots.join(format!("cycle_{report_date}.json")),
        serde_json::to_vec_pretty(&snapshot).expect("failed to serialize snapshot fixture"),
    )
    .expect("failed to write snapshot fixture");

    let canonical = canonical_probe_fact(generated_market_date, run_id);
    let canonical_path = generated_artifacts.join("probe_observations");
    fs::create_dir_all(&canonical_path).expect("failed to create canonical observation directory");
    fs::write(
        canonical_path.join(format!("{generated_market_date}-{run_id}.json")),
        serde_json::to_vec_pretty(&canonical).expect("failed to serialize canonical fixture"),
    )
    .expect("failed to write canonical fixture");
    let archives = generated_artifacts.join("probe_observation_archives");
    fs::create_dir_all(&archives).expect("failed to create observation archive directory");
    let archive = serde_json::json!({
        "session_20": probe_observation_window(&canonical, 20),
        "session_60": probe_observation_window(&canonical, 60),
        "all_history": probe_observation_window(&canonical, 2),
        "history_read_failed": false
    });
    fs::write(
        archives.join(format!("{run_id}.json")),
        serde_json::to_vec_pretty(&archive).expect("failed to serialize observation archive"),
    )
    .expect("failed to write observation archive");

    let fake_bin = tmp.path().join("bin");
    fs::create_dir_all(&fake_bin).expect("failed to create fake bin directory");
    let make_path = fake_bin.join("make");
    fs::write(
        &make_path,
        "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$*\" >> \"$MAKE_LOG\"\nif [ \"$1\" = radar-release ]; then\n  mkdir -p reports\n  cp -R \"$RADAR_GENERATED_ARTIFACTS\"/. reports/\n  printf '%s\\n' generated-artifacts-at-radar-call >> \"$MAKE_LOG\"\nfi\nexit 0\n",
    )
    .expect("failed to write fake make command");
    let mut permissions = fs::metadata(&make_path)
        .expect("failed to inspect fake make command")
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(0o755);
    fs::set_permissions(&make_path, permissions).expect("failed to make command executable");

    let script_path = tmp.path().join("run_radar.sh");
    fs::write(&script_path, script).expect("failed to write Radar step script");
    let make_log = tmp.path().join("make.log");
    let mut path = vec![fake_bin.to_string_lossy().to_string()];
    path.push(std::env::var("PATH").unwrap_or_default());
    let output = Command::new("bash")
        .arg(&script_path)
        .current_dir(tmp.path())
        .env("PATH", path.join(":"))
        .env("REPORT_DATE_JST", report_date)
        .env("EXPECTED_MARKET_DATE", expected_market_date)
        .env("RADAR_GENERATED_ARTIFACTS", &generated_artifacts)
        .env("GITHUB_ENV", tmp.path().join("github_env"))
        .env("MAKE_LOG", &make_log)
        .output()
        .expect("failed to run Radar step probe");
    let log = fs::read_to_string(make_log).unwrap_or_default();
    let generated_canonical = fs::read_to_string(
        reports
            .join("probe_observations")
            .join(format!("{generated_market_date}-{run_id}.json")),
    )
    .unwrap_or_default();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        log,
        generated_canonical,
    )
}

#[test]
fn daily_radar_protoc_install_ignores_third_party_apt_sources() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(workflow_path).expect("failed to read daily_radar.yml");

    assert!(
        workflow.contains("Dir::Etc::sourcelist=\"/etc/apt/sources.list.d/ubuntu.sources\""),
        "Protobuf installation must update the Ubuntu source list explicitly"
    );
    assert!(
        workflow.contains("Dir::Etc::sourceparts=\"-\""),
        "Protobuf installation must exclude unrelated third-party source parts"
    );
    assert!(
        workflow.contains("sudo apt-get install -y protobuf-compiler"),
        "Protobuf installation must keep installing protobuf-compiler"
    );
    assert!(
        !workflow.contains("run: sudo apt-get update && sudo apt-get install -y protobuf-compiler"),
        "Protobuf installation must not update every runner-provided APT source"
    );
}

#[test]
fn daily_radar_report_date_resolver_rolls_back_delayed_scheduled_runs() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let script = extract_report_date_resolver_script(&workflow_path);

    let delayed =
        run_report_date_resolver(&script, "schedule", "2026-09-05 02:43:39", "", "").unwrap();
    assert!(delayed.contains("REPORT_DATE_JST=2026-09-04"));
    assert!(delayed.contains("RUN_DATE_JST=2026-09-05"));

    let normal =
        run_report_date_resolver(&script, "schedule", "2026-09-04 23:30:00", "", "").unwrap();
    assert!(normal.contains("REPORT_DATE_JST=2026-09-04"));
    assert!(normal.contains("RUN_DATE_JST=2026-09-04"));

    let monday =
        run_report_date_resolver(&script, "schedule", "2026-09-07 02:43:39", "", "").unwrap();
    assert!(monday.contains("REPORT_DATE_JST=2026-09-04"));
    assert!(monday.contains("RUN_DATE_JST=2026-09-07"));
}

#[test]
fn daily_radar_date_resolution_keeps_report_boundary_separate_from_market_session() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let script = extract_report_date_resolver_script(&workflow_path);

    let before_boundary =
        run_report_date_resolver(&script, "schedule", "2026-10-07 05:59:59", "", "").unwrap();
    assert!(before_boundary.contains("REPORT_DATE_JST=2026-10-06"));
    assert!(before_boundary.contains("RUN_DATE_JST=2026-10-07"));

    let at_boundary =
        run_report_date_resolver(&script, "schedule", "2026-10-07 06:00:00", "", "").unwrap();
    assert!(at_boundary.contains("REPORT_DATE_JST=2026-10-07"));
    assert!(at_boundary.contains("RUN_DATE_JST=2026-10-07"));
    assert_eq!(run_market_date_resolver("2026-10-07"), "2026-10-06");
}

#[test]
fn daily_radar_report_date_resolver_limits_generate_and_preserves_historical_resend() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let script = extract_report_date_resolver_script(&workflow_path);

    let rejected_generate = run_report_date_resolver(
        &script,
        "workflow_dispatch",
        "2026-09-05 02:43:39",
        "2026-08-28",
        "generate",
    );
    assert!(
        rejected_generate.is_err(),
        "generate must reject a historical report date"
    );

    let current_generate = run_report_date_resolver(
        &script,
        "workflow_dispatch",
        "2026-09-05 02:43:39",
        "2026-09-05",
        "generate",
    )
    .unwrap();
    assert!(current_generate.contains("REPORT_DATE_JST=2026-09-05"));
    assert!(current_generate.contains("RUN_DATE_JST=2026-09-05"));

    let historical_resend = run_report_date_resolver(
        &script,
        "workflow_dispatch",
        "2026-09-05 02:43:39",
        "2026-08-28",
        "resend",
    )
    .unwrap();
    assert!(historical_resend.contains("REPORT_DATE_JST=2026-08-28"));
    assert!(historical_resend.contains("RUN_DATE_JST=2026-09-05"));
}

#[test]
fn daily_radar_generation_accepts_only_serialized_succeeded_status() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let script = extract_step_script(&workflow_path, "Run Sentinel Radar");
    let classifier = extract_embedded_python_at(&script, 2);
    let tmp = tempfile::tempdir().expect("failed to create generation status fixture");
    let status_path = tmp.path().join("run_status.json");

    for (decisioning, expected) in [
        (serde_json::json!("succeeded"), "succeeded"),
        (serde_json::json!({ "succeeded": false }), "failed"),
        (serde_json::json!({ "succeeded": "yes" }), "failed"),
        (serde_json::json!({ "status": "succeeded" }), "failed"),
        (serde_json::json!("skipped"), "failed"),
        (serde_json::Value::Null, "failed"),
    ] {
        fs::write(
            &status_path,
            serde_json::to_vec(&serde_json::json!({ "decisioning": decisioning }))
                .expect("failed to serialize generation status fixture"),
        )
        .expect("failed to write generation status fixture");
        let output = Command::new("python3")
            .arg("-")
            .env("RUN_STATUS_PATH", &status_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to start generation status classifier");
        let mut child = output;
        child
            .stdin
            .as_mut()
            .expect("classifier stdin must be available")
            .write_all(classifier.as_bytes())
            .expect("failed to send classifier script");
        drop(child.stdin.take());
        let output = child
            .wait_with_output()
            .expect("failed to collect generation classifier output");
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout)
                .expect("classifier output must be UTF-8")
                .trim(),
            expected
        );
    }
}

#[test]
fn daily_radar_expected_market_date_resolver_uses_nyse_sessions() {
    let cases = [
        ("2026-10-07", "2026-10-06"),
        ("2026-10-06", "2026-10-05"),
        ("2026-10-10", "2026-10-09"),
        ("2026-10-11", "2026-10-09"),
        ("2026-10-12", "2026-10-09"),
        ("2026-04-04", "2026-04-02"),
        ("2026-01-20", "2026-01-16"),
    ];
    for (run_date, expected_market_date) in cases {
        assert_eq!(
            run_market_date_resolver(run_date),
            expected_market_date,
            "unexpected expected market date for run date {run_date}"
        );
    }

    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(workflow_path).expect("failed to read daily_radar.yml");
    let makefile = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Makefile"))
        .expect("failed to read Makefile");
    assert!(workflow.contains("name: Resolve Expected Market Date"));
    assert!(workflow.contains("EXPECTED_MARKET_DATE"));
    assert!(makefile.contains("radar-market-date"));
    assert!(makefile.contains("resolve-market-date --date"));
}

#[test]
fn daily_radar_report_date_is_shared_by_generation_and_freshness_validation() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(&workflow_path).expect("failed to read daily_radar.yml");
    let run_step = extract_step_script(&workflow_path, "Run Sentinel Radar");
    let notification_step = extract_step_script(&workflow_path, "Show Notification Outcome");
    let freshness_step =
        extract_step_script(&workflow_path, "Freshness Gate and Output Validation");

    assert!(workflow.contains("name: Resolve Report Date"));
    assert!(workflow.contains("name: Resolve Expected Market Date"));
    assert!(run_step.contains("RADAR_ARGS=\"--date ${REPORT_DATE_JST}\""));
    assert!(!run_step.contains("DATE_JST=\"$(TZ=Asia/Tokyo date +%Y-%m-%d)\""));
    assert!(notification_step.contains("DATE_JST=\"${REPORT_DATE_JST:?"));
    assert!(freshness_step.contains("DATE_JST=\"${REPORT_DATE_JST:?"));
    assert!(workflow.contains("REPORT_DATE_JST=\"${DATE_JST}\""));
    assert!(workflow.contains("RUN_DATE_JST"));
}

#[test]
fn daily_radar_scheduled_reuses_only_complete_successful_reports() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(&workflow_path).expect("failed to read daily_radar.yml");
    let script = extract_step_script(&workflow_path, "Reuse Existing Successful Daily Report");

    assert!(workflow.contains("id: reuse_report"));
    assert!(workflow.contains("if: ${{ github.event_name == 'schedule' }}"));
    assert!(workflow.contains("steps.reuse_report.outputs.reuse != 'true'"));
    assert!(script.contains("reports/run_status_${DATE_JST}.json"));
    assert!(script.contains("reports/decision_packet_${DATE_JST}.json"));
    assert!(script.contains("reports/${DATE_JST}.md"));
    assert!(script.contains("reports/telegram_report_${DATE_JST}.html"));
    assert!(script.contains("decisioning"));
    assert!(script.contains("notification"));

    let report_date = "2026-10-06";
    let market_date = "2026-10-06";
    let run_id = "run-1";
    let successful = successful_reuse_status(report_date, market_date, run_id);
    let packet = reuse_packet(report_date, market_date);
    for permission in ["PROBE", "READY", "NO_TRADE"] {
        let canonical = serde_json::to_string(&canonical_probe_fact_with_permission(
            market_date,
            run_id,
            permission,
        ))
        .unwrap();
        let canonical_path = format!("{market_date}-{run_id}.json");
        let valid = run_reuse_report_probe(
            &script,
            &successful,
            &packet,
            market_date,
            Some((&canonical_path, &canonical)),
            true,
        );
        assert!(
            valid.contains("reuse=true"),
            "permission={permission}: {valid}"
        );
        assert!(
            valid.contains("reason=REUSED"),
            "permission={permission}: {valid}"
        );
    }

    let canonical = serde_json::to_string(&canonical_probe_fact(market_date, run_id)).unwrap();
    let canonical_path = format!("{market_date}-{run_id}.json");

    let mut wrong_packet_digest = successful.clone();
    wrong_packet_digest["fixture_snapshot_packet_digest"] = serde_json::json!(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    );
    let packet_digest_mismatch = run_reuse_report_probe(
        &script,
        &wrong_packet_digest,
        &packet,
        market_date,
        Some((&canonical_path, &canonical)),
        true,
    );
    assert!(
        packet_digest_mismatch.contains("reuse=false"),
        "{packet_digest_mismatch}"
    );
    assert!(
        packet_digest_mismatch.contains("reason=ARTIFACT_INTEGRITY_MISMATCH"),
        "{packet_digest_mismatch}"
    );

    let mut archive_permission_mismatch = successful.clone();
    archive_permission_mismatch["fixture_archive_permission"] = serde_json::json!("READY");
    let archive_permission_mismatch = run_reuse_report_probe(
        &script,
        &archive_permission_mismatch,
        &packet,
        market_date,
        Some((&canonical_path, &canonical)),
        true,
    );
    assert!(
        archive_permission_mismatch.contains("reuse=false"),
        "{archive_permission_mismatch}"
    );
    assert!(
        archive_permission_mismatch.contains("reason=ARTIFACT_INTEGRITY_MISMATCH"),
        "{archive_permission_mismatch}"
    );

    let mut archive_run_mismatch = successful.clone();
    archive_run_mismatch["fixture_archive_run_id"] = serde_json::json!("archive-other-run");
    let archive_mismatch = run_reuse_report_probe(
        &script,
        &archive_run_mismatch,
        &packet,
        market_date,
        Some((&canonical_path, &canonical)),
        true,
    );
    assert!(
        archive_mismatch.contains("reuse=false"),
        "{archive_mismatch}"
    );
    assert!(
        archive_mismatch.contains("reason=RUN_ID_MISMATCH"),
        "{archive_mismatch}"
    );

    let mut archive_missing_status = successful.clone();
    archive_missing_status["fixture_archive_missing"] = serde_json::json!(true);
    let archive_missing = run_reuse_report_probe(
        &script,
        &archive_missing_status,
        &packet,
        market_date,
        Some((&canonical_path, &canonical)),
        true,
    );
    assert!(archive_missing.contains("reuse=false"), "{archive_missing}");
    assert!(
        archive_missing.contains("reason=CANONICAL_MISSING"),
        "{archive_missing}"
    );

    let mut missing_schema_version = successful.clone();
    missing_schema_version["fixture_snapshot_schema_version_missing"] = serde_json::json!(true);
    let missing_schema = run_reuse_report_probe(
        &script,
        &missing_schema_version,
        &packet,
        market_date,
        Some((&canonical_path, &canonical)),
        true,
    );
    assert!(missing_schema.contains("reuse=false"), "{missing_schema}");
    assert!(
        missing_schema.contains("reason=CANONICAL_INCOMPLETE"),
        "{missing_schema}"
    );

    let stale_market_date = "2026-10-05";
    let stale_status = successful_reuse_status(report_date, stale_market_date, run_id);
    let stale_packet = reuse_packet(report_date, stale_market_date);
    let stale_canonical =
        serde_json::to_string(&canonical_probe_fact(stale_market_date, run_id)).unwrap();
    let stale_canonical_path = format!("{stale_market_date}-{run_id}.json");
    let stale = run_reuse_report_probe(
        &script,
        &stale_status,
        &stale_packet,
        market_date,
        Some((&stale_canonical_path, &stale_canonical)),
        true,
    );
    assert!(stale.contains("reuse=false"), "{stale}");
    assert!(
        stale.contains("reason=REUSE_REJECTED_MARKET_DATE_MISMATCH"),
        "{stale}"
    );

    let missing_canonical =
        run_reuse_report_probe(&script, &successful, &packet, market_date, None, true);
    assert!(
        missing_canonical.contains("reuse=false"),
        "{missing_canonical}"
    );
    assert!(missing_canonical.contains("reason=CANONICAL_MISSING"));

    let corrupt = run_reuse_report_probe(
        &script,
        &successful,
        &packet,
        market_date,
        Some((&canonical_path, "{")),
        true,
    );
    assert!(corrupt.contains("reuse=false"), "{corrupt}");
    assert!(corrupt.contains("reason=CANONICAL_CORRUPT"));

    let mut incomplete_fact = canonical_probe_fact(market_date, run_id);
    incomplete_fact
        .as_object_mut()
        .unwrap()
        .remove("eligible_asset_count");
    let incomplete_json = serde_json::to_string(&incomplete_fact).unwrap();
    let incomplete = run_reuse_report_probe(
        &script,
        &successful,
        &packet,
        market_date,
        Some((&canonical_path, &incomplete_json)),
        true,
    );
    assert!(incomplete.contains("reuse=false"), "{incomplete}");
    assert!(incomplete.contains("reason=CANONICAL_INCOMPLETE"));

    for field in [
        "market_date",
        "permission",
        "report_run_id",
        "observed_at",
        "provenance",
    ] {
        let mut incomplete_fact = canonical_probe_fact(market_date, run_id);
        incomplete_fact.as_object_mut().unwrap().remove(field);
        let incomplete_json = serde_json::to_string(&incomplete_fact).unwrap();
        let incomplete = run_reuse_report_probe(
            &script,
            &successful,
            &packet,
            market_date,
            Some((&canonical_path, &incomplete_json)),
            true,
        );
        assert!(
            incomplete.contains("reuse=false"),
            "missing {field} must reject reuse: {incomplete}"
        );
    }

    let other_run_id = "run-2";
    let other_run =
        serde_json::to_string(&canonical_probe_fact(market_date, other_run_id)).unwrap();
    let other_run_path = format!("{market_date}-{other_run_id}.json");
    let run_mismatch = run_reuse_report_probe(
        &script,
        &successful,
        &packet,
        market_date,
        Some((&other_run_path, &other_run)),
        true,
    );
    assert!(run_mismatch.contains("reuse=false"), "{run_mismatch}");
    assert!(run_mismatch.contains("reason=RUN_ID_MISMATCH"));

    let mut failed = successful.clone();
    failed["decisioning"] = serde_json::json!({"failed": {"reason": "snapshot conflict"}});
    assert!(run_reuse_report_probe(
        &script,
        &failed,
        &packet,
        market_date,
        Some((&canonical_path, &canonical)),
        true,
    )
    .contains("reuse=false"));
    for (field, abnormal_status) in [
        ("decisioning", serde_json::json!({ "succeeded": true })),
        ("decisioning", serde_json::json!({ "status": "succeeded" })),
        ("decisioning", serde_json::json!({ "succeeded": false })),
        ("notification", serde_json::json!({ "succeeded": true })),
        ("notification", serde_json::json!({ "status": "succeeded" })),
        ("notification", serde_json::json!({ "succeeded": false })),
    ] {
        let mut abnormal = successful.clone();
        abnormal[field] = abnormal_status;
        let outcome = run_reuse_report_probe(
            &script,
            &abnormal,
            &packet,
            market_date,
            Some((&canonical_path, &canonical)),
            true,
        );
        assert!(
            outcome.contains("reuse=false"),
            "abnormal {field} status must reject reuse: {outcome}"
        );
    }
    assert!(run_reuse_report_probe(
        &script,
        &successful,
        &packet,
        market_date,
        Some((&canonical_path, &canonical)),
        false,
    )
    .contains("reuse=false"));

    let mismatched_packet = reuse_packet(report_date, "2026-09-08");
    let inconsistent = run_reuse_report_probe(
        &script,
        &successful,
        &mismatched_packet,
        market_date,
        Some((&canonical_path, &canonical)),
        true,
    );
    assert!(inconsistent.contains("reuse=false"), "{inconsistent}");
    assert!(inconsistent.contains("reason=ARTIFACT_INTEGRITY_MISMATCH"));

    let mut marker_mismatch = successful.clone();
    marker_mismatch["fixture_report_run_id"] = serde_json::json!("run-other");
    let marker_mismatch = run_reuse_report_probe(
        &script,
        &marker_mismatch,
        &packet,
        market_date,
        Some((&canonical_path, &canonical)),
        true,
    );
    assert!(marker_mismatch.contains("reuse=false"), "{marker_mismatch}");
    assert!(marker_mismatch.contains("reason=RUN_ID_MISMATCH"));

    let mut report_date_mismatch = successful.clone();
    report_date_mismatch["date"] = serde_json::json!("2026-10-05");
    report_date_mismatch["fixture_expected_report_date"] = serde_json::json!(report_date);
    let report_date_mismatch = run_reuse_report_probe(
        &script,
        &report_date_mismatch,
        &packet,
        market_date,
        Some((&canonical_path, &canonical)),
        true,
    );
    assert!(
        report_date_mismatch.contains("reuse=false"),
        "{report_date_mismatch}"
    );
    assert!(report_date_mismatch.contains("reason=REPORT_DATE_MISMATCH"));

    let mut snapshot_mismatch = successful.clone();
    snapshot_mismatch["fixture_snapshot_run_id"] = serde_json::json!("run-new");
    let snapshot_mismatch = run_reuse_report_probe(
        &script,
        &snapshot_mismatch,
        &packet,
        market_date,
        Some((&canonical_path, &canonical)),
        true,
    );
    assert!(
        snapshot_mismatch.contains("reuse=false"),
        "{snapshot_mismatch}"
    );
    assert!(snapshot_mismatch.contains("reason=RUN_ID_MISMATCH"));
}

#[test]
fn daily_radar_recompute_failure_does_not_promote_stale_success() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(&workflow_path).expect("failed to read daily_radar.yml");
    let script = extract_step_script(&workflow_path, "Run Sentinel Radar");
    let run_step_start = workflow
        .find("- name: Run Sentinel Radar")
        .expect("Run Sentinel Radar step is missing");
    let run_step_header = workflow[run_step_start..]
        .lines()
        .take_while(|line| !line.starts_with("      - name:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(run_step_header.contains("steps.reuse_report.outputs.reuse != 'true'"));
    let persistence_step_start = workflow
        .find("- name: Commit and Push to Data Worktree")
        .expect("data persistence step is missing");
    let persistence_step_header = workflow[persistence_step_start..]
        .lines()
        .take_while(|line| !line.starts_with("      - name:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        persistence_step_header
            .contains("if: ${{ success() && steps.reuse_report.outputs.reuse != 'true' }}"),
        "failed recalculation must not publish a mixed data-branch state"
    );

    let (success, make_log, status_after) = run_failed_radar_generation(&script, "2026-10-06");
    assert!(!success, "failed recalculation must fail the workflow step");
    assert!(make_log.contains("radar-release"), "{make_log}");
    assert_eq!(
        status_after,
        serde_json::to_vec(&successful_reuse_status(
            "2026-10-06",
            "2026-10-05",
            "old-run"
        ))
        .unwrap(),
        "failed calculation must not rewrite the old status as the current result"
    );
}

#[test]
fn daily_radar_recalculation_requires_expected_market_date() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let script = extract_step_script(&workflow_path, "Run Sentinel Radar");

    let (success, _, stderr, make_log, canonical) =
        run_radar_generation_probe(&script, "2026-10-06", "2026-10-06", "2026-10-06");
    assert!(success, "correct generated session should pass: {stderr}");
    assert!(make_log.contains("radar-release"), "{make_log}");
    assert!(
        make_log.contains("generated-artifacts-at-radar-call"),
        "the fake Radar command must create its output artifacts when invoked: {make_log}"
    );
    let canonical: Value = serde_json::from_str(&canonical)
        .expect("successful Radar command must have generated a canonical fact");
    assert_eq!(canonical["market_date"], "2026-10-06");

    let (success, stdout, stderr, make_log, canonical) =
        run_radar_generation_probe(&script, "2026-10-06", "2026-10-06", "2026-10-05");
    assert!(
        !success,
        "stale recalculation output must fail the Radar step"
    );
    assert!(make_log.contains("radar-release"), "{make_log}");
    assert!(
        make_log.contains("generated-artifacts-at-radar-call"),
        "the stale fixture must be generated at Radar command time: {make_log}"
    );
    let canonical: Value = serde_json::from_str(&canonical)
        .expect("mismatched Radar command must still have generated a canonical fact");
    assert_eq!(canonical["market_date"], "2026-10-05");
    assert!(
        format!("{stdout}{stderr}").contains("EXPECTED_MARKET_DATE_MISMATCH"),
        "stdout={stdout} stderr={stderr}"
    );
}

fn extract_embedded_python_at(script: &str, occurrence: usize) -> String {
    let start_marker = "python - <<'PY'\n";
    let start = script
        .match_indices(start_marker)
        .nth(occurrence)
        .map(|(index, _)| index + start_marker.len())
        .expect("Python heredoc is missing");
    let end = script[start..]
        .find("\nPY\n")
        .map(|index| start + index)
        .expect("Python heredoc terminator is missing");
    script[start..end].to_string()
}

fn extract_embedded_python(script: &str) -> String {
    extract_embedded_python_at(script, 0)
}

fn read_mock_http_request(stream: &mut TcpStream) -> Option<Value> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("failed to configure mock HTTP read timeout");
    let mut headers = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        stream.read_exact(&mut byte).ok()?;
        headers.push(byte[0]);
        if headers.ends_with(b"\r\n\r\n") {
            break;
        }
        if headers.len() > 8192 {
            return None;
        }
    }
    let headers_text = String::from_utf8_lossy(&headers);
    let content_length = headers_text
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())?;
    let mut body = vec![0_u8; content_length];
    stream.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn serve_mock_telegram_request(stream: &mut TcpStream, messages: &Arc<Mutex<Vec<Value>>>) {
    if let Some(payload) = read_mock_http_request(stream) {
        messages
            .lock()
            .expect("mock Telegram messages mutex was poisoned")
            .push(payload);
        let response_body = b"{\"ok\":true}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response_body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("failed to write mock Telegram response headers");
        stream
            .write_all(response_body)
            .expect("failed to write mock Telegram response body");
    }
}

type MockTelegramServer = (
    String,
    Arc<Mutex<Vec<Value>>>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
);

fn start_mock_telegram_server() -> MockTelegramServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind mock Telegram server");
    listener
        .set_nonblocking(true)
        .expect("failed to configure mock Telegram server");
    let address = format!(
        "http://{}/sendMessage",
        listener
            .local_addr()
            .expect("failed to read mock Telegram server address")
    );
    let messages = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let thread_messages = Arc::clone(&messages);
    let thread_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        while !thread_stop.load(Ordering::Acquire) && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => serve_mock_telegram_request(&mut stream, &thread_messages),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });
    (address, messages, stop, handle)
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
    assert!(script.contains("reports/telegram_report_${DATE_JST}.html"));
    assert!(script.contains("run_status_${DATE_JST}.json"));
    assert!(script.contains("api.telegram.org"));
    assert!(script.contains("\"ok\""));
    assert!(script.contains("notification_resend"));
    assert!(script.contains("report_lifecycle"));
    assert!(script.contains("\"mode\": \"RESENT\""));
    assert!(workflow.contains("SENTINEL_EXECUTION_GIT_SHA: ${{ github.sha }}"));
    assert!(workflow.contains("SENTINEL_EXECUTION_GIT_BRANCH: ${{ github.ref_name }}"));
    assert!(!script.contains("make radar-release"));
    assert!(
        workflow.contains(
            "name: Freshness Gate and Output Validation\n        if: ${{ (github.event_name != 'workflow_dispatch' || inputs.mode != 'resend') && steps.reuse_report.outputs.reuse != 'true' }}"
        ),
        "resend and scheduled reuse must skip freshness validation intended for newly generated reports"
    );
}

#[test]
fn daily_radar_manual_resend_accepts_a_validated_report_date_without_generating() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let workflow = fs::read_to_string(&workflow_path).expect("failed to read daily_radar.yml");
    let script = extract_step_script(&workflow_path, "Resend Existing Daily Report");

    assert!(workflow.contains("report_date:"));
    assert!(workflow.contains("description: \"重发的 JST 报告日期"));
    assert!(workflow.contains("REPORT_DATE_INPUT: ${{ inputs.report_date }}"));
    let resolver = extract_report_date_resolver_script(&workflow_path);
    assert!(resolver.contains("REPORT_DATE_INPUT"));
    assert!(script.contains("REPORT_DATE_JST:?"));
    assert!(script.contains("datetime.strptime(date_jst, \"%Y-%m-%d\")"));
    assert!(script.contains("reports/telegram_report_${DATE_JST}.html"));
    assert!(script.contains("run_status_${DATE_JST}.json"));
    assert!(!script.contains("make radar-release"));
}

#[test]
fn daily_radar_persists_the_final_telegram_payload_before_delivery() {
    let runner_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/features/radar/interface/radar_pipeline_runner.rs");
    let runner = fs::read_to_string(runner_path).expect("failed to read radar pipeline runner");
    let persist = runner
        .find("save_telegram_html_report")
        .expect("final Telegram HTML payload must be persisted");
    let deliver = runner
        .rfind("send_telegram_with_status")
        .expect("final Telegram HTML payload must be delivered");

    assert!(persist < deliver);
    assert!(runner.contains("&report_result.telegram_html_body"));
}

#[test]
fn daily_radar_manual_resend_uses_the_archived_telegram_html_payload() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let script = extract_step_script(&workflow_path, "Resend Existing Daily Report");

    assert!(script.contains("reports/telegram_report_${DATE_JST}.html"));
    assert!(!script.contains("reports/${DATE_JST}.md"));
    assert!(script.contains("parse_mode"));
    assert!(script.contains("HTML"));
    assert!(script.contains("data_branch_telegram_html_payload"));
    assert!(script.contains("payload_path"));
    assert!(script.contains("sanitize_telegram_html"));
    assert!(script.contains("report_run_id"));
    assert!(script.contains("chunk_telegram_html_message"));
    assert!(script.contains("def utf8_len"));
    assert!(script.contains("archived Telegram HTML payload is missing"));
}

#[test]
fn daily_radar_manual_resend_executes_html_payload_safely() {
    let workflow_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/daily_radar.yml");
    let script = extract_step_script(&workflow_path, "Resend Existing Daily Report");
    let python = extract_embedded_python(&script);
    let tmp = tempfile::tempdir().expect("failed to create resend fixture directory");
    let reports = tmp.path().join("reports");
    fs::create_dir_all(&reports).expect("failed to create reports directory");
    let report = format!(
        "<!-- report_run_id: run-2026-09-03 -->\n<!-- report_run_id: keep me -->\n<b>报告开始<&> <u>危险</u> inline <!-- report_run_id: keep-me --> __TG_OPEN_B__ {}</b>\n<i>报告结束</i>\n",
        "中文🧪".repeat(1100)
    );
    fs::write(reports.join("telegram_report_2026-09-03.html"), &report)
        .expect("failed to write HTML payload fixture");
    let status = serde_json::json!({
        "date": "2026-09-03",
        "decisioning": "succeeded",
        "runtime_identity": {
            "report_run_at": "<b>不可信</b>",
            "git_commit_sha": "abc<script>"
        }
    });
    fs::write(
        reports.join("run_status_2026-09-03.json"),
        serde_json::to_vec_pretty(&status).unwrap(),
    )
    .expect("failed to write status fixture");
    let python_path = tmp.path().join("resend.py");
    fs::write(&python_path, python).expect("failed to write resend Python fixture");

    let (api_url, messages, stop, server) = start_mock_telegram_server();
    let output = Command::new("python3")
        .arg(&python_path)
        .current_dir(tmp.path())
        .env("DATE_JST", "2026-09-03")
        .env(
            "TELEGRAM_REPORT_PATH",
            "reports/telegram_report_2026-09-03.html",
        )
        .env("STATUS_PATH", "reports/run_status_2026-09-03.json")
        .env("TELEGRAM_BOT_TOKEN", "test-token")
        .env("TELEGRAM_CHAT_ID", "test-chat")
        .env("TELEGRAM_API_URL", &api_url)
        .env("SENTINEL_EXECUTION_GIT_SHA", "resend-sha")
        .output()
        .expect("failed to execute resend Python fixture");
    assert!(
        output.status.success(),
        "resend fixture failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let first_messages = messages
        .lock()
        .expect("mock Telegram messages mutex was poisoned")
        .clone();
    assert!(
        first_messages.len() > 1,
        "fixture must exercise HTML chunking"
    );
    assert!(first_messages.iter().all(|payload| {
        payload["text"]
            .as_str()
            .is_some_and(|text| text.len() <= 3800)
    }));
    assert!(first_messages.iter().all(|payload| {
        let text = payload["text"].as_str().unwrap_or_default();
        text.matches("<b>").count() == text.matches("</b>").count()
            && text.matches("<i>").count() == text.matches("</i>").count()
    }));
    assert!(first_messages[0]["text"]
        .as_str()
        .unwrap()
        .contains("<b>报告开始"));
    let sent_text = first_messages
        .iter()
        .filter_map(|payload| payload["text"].as_str())
        .collect::<String>();
    assert!(!sent_text.contains("run-2026-09-03"));
    assert!(sent_text.contains("&lt;!-- report_run_id: keep me --&gt;"));
    assert!(sent_text.contains("&lt;!-- report_run_id: keep-me --&gt;"));
    assert!(sent_text.contains("&lt;u&gt;危险&lt;/u&gt;"));
    assert!(sent_text.contains("&lt;b&gt;不可信&lt;/b&gt;"));
    assert!(sent_text.contains("&lt;script&gt;"));
    assert!(sent_text.contains("__TG_OPEN_B__"));
    assert!(first_messages
        .iter()
        .all(|payload| { payload["parse_mode"].as_str() == Some("HTML") }));

    for decisioning in [
        serde_json::json!({ "succeeded": false }),
        serde_json::json!({ "succeeded": true }),
        serde_json::json!({ "status": "succeeded" }),
        serde_json::json!("skipped"),
    ] {
        fs::write(
            reports.join("run_status_2026-09-03.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "date": "2026-09-03",
                "decisioning": decisioning
            }))
            .unwrap(),
        )
        .expect("failed to write failed status fixture");
        let rejected = Command::new("python3")
            .arg(&python_path)
            .current_dir(tmp.path())
            .env("DATE_JST", "2026-09-03")
            .env(
                "TELEGRAM_REPORT_PATH",
                "reports/telegram_report_2026-09-03.html",
            )
            .env("STATUS_PATH", "reports/run_status_2026-09-03.json")
            .env("TELEGRAM_BOT_TOKEN", "test-token")
            .env("TELEGRAM_CHAT_ID", "test-chat")
            .env("TELEGRAM_API_URL", &api_url)
            .output()
            .expect("failed to execute rejected resend fixture");
        assert!(!rejected.status.success());
        assert_eq!(
            messages
                .lock()
                .expect("mock Telegram messages mutex was poisoned")
                .len(),
            first_messages.len(),
            "abnormal decisioning status must not send any Telegram request"
        );
    }
    stop.store(true, Ordering::Release);
    server.join().expect("mock Telegram server panicked");
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
fn snapshot_conflict_stops_before_report_and_telegram_delivery() {
    let runner_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/features/radar/interface/radar_pipeline_runner.rs");
    let runner = fs::read_to_string(runner_path).expect("failed to read radar pipeline runner");
    let conflict_check = runner
        .find("validate_trading_day_snapshot_conflict")
        .expect("snapshot conflict check is missing");
    let report_render = runner
        .find("let mut report_context")
        .expect("report rendering boundary is missing");
    let notification = runner[report_render..]
        .find("send_telegram_with_status")
        .map(|offset| report_render + offset)
        .expect("Telegram notification boundary is missing");
    let conflict_path = &runner[conflict_check..report_render];

    assert!(
        conflict_path.contains("mark_snapshot_persistence_failure"),
        "snapshot conflict must become a failed run"
    );
    assert!(
        conflict_path.contains("save_run_status") && conflict_path.contains("return Ok(())"),
        "snapshot conflict must persist failure and stop before report delivery"
    );
    assert!(
        report_render < notification,
        "Telegram notification must remain after report rendering"
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

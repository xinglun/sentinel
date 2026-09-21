use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

use stock_sentinel::features::radar::interface::acceptance_replay::{
    run_acceptance_replay, run_acceptance_replay_with_expectations, AcceptanceReplayError,
};

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn write_fixture(root: &Path) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let canonical = root.join("canonical");
    let output = root.join("isolated");
    fs::create_dir_all(&canonical).unwrap();
    fs::write(canonical.join("snapshot.json"), br#"{"version":1}"#).unwrap();

    let input = root.join("historical-finnhub.json");
    let raw = br#"[
        {"datetime":1788825600,"headline":"Warsh faces tough battle at the Fed","summary":"Expected interest rate hike","source":"Structured News","url":"https://example.test/warsh"},
        {"datetime":1788825600,"headline":"War in Iran escalates after missile strike","summary":"Regional security risk rises","source":"Structured News","url":"https://example.test/war"},
        {"datetime":1788825600,"headline":"Workers strike over wages","summary":"Labor negotiations continue","source":"Structured News","url":"https://example.test/workers"},
        {"datetime":1788825600,"headline":"Options strike price reset","summary":"Derivatives pricing update","source":"Structured News","url":"https://example.test/options"},
        {"datetime":1788825600,"headline":"Company attacks rising costs","summary":"Operating margin discussion","source":"Structured News","url":"https://example.test/company"},
        {"datetime":1788825600,"headline":"Israel launches air strikes","summary":"Regional security risk rises","source":"Structured News","url":"https://example.test/israel"},
        {"datetime":1788825600,"headline":"Drone attack hits oil facility","summary":"Regional security risk rises","source":"Structured News","url":"https://example.test/drone"},
        {"datetime":1788825600,"headline":"Missile strike hits military base","summary":"Regional security risk rises","source":"Structured News","url":"https://example.test/missile"}
    ]"#;
    fs::write(&input, raw).unwrap();
    let input_digest = sha256_bytes(raw);
    let manifest = root.join("input-manifest.json");
    fs::write(
        &manifest,
        serde_json::to_vec_pretty(&json!({
            "report_date": "2026-09-08",
            "input_path": input,
            "input_manifest_digest": input_digest,
            "execution_git_commit_sha": "ab74a7b9b5a10c6ad967af2b0b3900e18f30a273",
            "canonical_state_path": canonical,
            "decision_projection_digest": "sha256:decision-before",
            "gate_projection_digest": "sha256:gate-before",
            "position_sizing_projection_digest": "sha256:position-before",
            "assertions": {
                "forbidden": [
                    {"context": "GEOPOLITICAL_ESCALATION", "evidence_contains": "Warsh"},
                    {"context": "GEOPOLITICAL_ESCALATION", "evidence_contains": "Workers strike"},
                    {"context": "GEOPOLITICAL_ESCALATION", "evidence_contains": "Options strike"},
                    {"context": "GEOPOLITICAL_ESCALATION", "evidence_contains": "Company attacks"}
                ],
                "required": [
                    {"context": "GEOPOLITICAL_ESCALATION", "evidence_contains": ["War", "Iran", "missile"]},
                    {"context": "GEOPOLITICAL_ESCALATION", "evidence_contains": ["Israel", "air strikes"]},
                    {"context": "GEOPOLITICAL_ESCALATION", "evidence_contains": ["Drone", "attack", "oil facility"]},
                    {"context": "GEOPOLITICAL_ESCALATION", "evidence_contains": ["Missile", "military base"]}
                ]
            }
        }))
        .unwrap(),
    )
    .unwrap();
    (manifest, output, input_digest)
}

#[test]
fn acceptance_replay_proves_warsh_negative_and_real_geopolitical_positive() {
    let tmp = tempdir().unwrap();
    let (manifest, output, input_digest) = write_fixture(tmp.path());
    let canonical_before = fs::read(tmp.path().join("canonical/snapshot.json")).unwrap();

    let receipt = run_acceptance_replay(&manifest, &output, false).unwrap();

    assert_eq!(receipt["report_lifecycle"]["mode"], "GENERATED");
    assert_eq!(
        receipt["report_lifecycle"]["run_purpose"],
        "ACCEPTANCE_REPLAY"
    );
    assert_eq!(receipt["report_lifecycle"]["publication_scope"], "ISOLATED");
    assert_eq!(receipt["report_lifecycle"]["canonical_write"], false);
    assert_eq!(receipt["input_manifest_digest"], input_digest);
    assert_eq!(receipt["lexical_regression"]["warsh"], "PASS");
    assert_eq!(receipt["lexical_regression"]["real_geopolitical"], "PASS");
    assert_eq!(
        receipt["decision_projection"]["before"],
        receipt["decision_projection"]["after"]
    );
    assert_eq!(
        receipt["gate_projection"]["before"],
        receipt["gate_projection"]["after"]
    );
    assert_eq!(
        receipt["position_sizing_projection"]["before"],
        receipt["position_sizing_projection"]["after"]
    );
    assert_eq!(
        receipt["canonical_snapshot_before"],
        receipt["canonical_snapshot_after"]
    );
    assert_eq!(receipt["canonical_state_unchanged"], true);
    assert_eq!(receipt["decision_weight"], 0);
    assert_eq!(receipt["trade_signal"], false);
    assert_eq!(receipt["gate_effect"], "none");
    assert_eq!(receipt["publishable"], false);
    assert_eq!(receipt["notification"]["source"], "none");
    assert_eq!(
        canonical_before,
        fs::read(tmp.path().join("canonical/snapshot.json")).unwrap()
    );
    assert!(output.join("acceptance-replay.receipt.json").is_file());
}

#[test]
fn acceptance_replay_uses_current_cli_revision_and_preserves_input_origin_revision() {
    let tmp = tempdir().unwrap();
    let (manifest, output, _) = write_fixture(tmp.path());
    let mut value: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value["original_generation_revision"] = json!("fixture:structured-news-2026-09-08-v1");
    value
        .as_object_mut()
        .unwrap()
        .remove("execution_git_commit_sha");
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();

    let receipt = run_acceptance_replay_with_expectations(
        &manifest,
        &output,
        false,
        Some("2026-09-08"),
        Some("current-execution-revision"),
    )
    .unwrap();

    assert_eq!(
        receipt["original_generation_revision"],
        "fixture:structured-news-2026-09-08-v1"
    );
    assert_eq!(
        receipt["execution_git_commit_sha"],
        "current-execution-revision"
    );
}

#[test]
fn repository_fixture_is_immutable_and_workflow_ready() {
    let output = tempdir().unwrap();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/acceptance_replay/2026-09-08/input-manifest.json");

    let receipt = run_acceptance_replay_with_expectations(
        &manifest,
        output.path(),
        false,
        Some("2026-09-08"),
        Some("fixture-execution-revision"),
    )
    .unwrap();

    assert_eq!(receipt["lexical_regression"]["warsh"], "PASS");
    assert_eq!(receipt["lexical_regression"]["real_geopolitical"], "PASS");
    assert_eq!(receipt["report_lifecycle"]["canonical_write"], false);
    assert_eq!(receipt["publishable"], false);
}

#[test]
fn acceptance_replay_fails_closed_when_immutable_input_is_unavailable() {
    let tmp = tempdir().unwrap();
    let manifest = tmp.path().join("input-manifest.json");
    fs::write(
        &manifest,
        serde_json::to_vec(&json!({
            "report_date": "2026-09-08",
            "input_path": tmp.path().join("missing.json"),
            "input_manifest_digest": "sha256:missing",
            "execution_git_commit_sha": "ab74a7b9b5a10c6ad967af2b0b3900e18f30a273",
            "canonical_state_path": tmp.path().join("canonical"),
            "decision_projection_digest": "sha256:decision-before",
            "gate_projection_digest": "sha256:gate-before",
            "position_sizing_projection_digest": "sha256:position-before"
        }))
        .unwrap(),
    )
    .unwrap();

    let error = run_acceptance_replay(&manifest, &tmp.path().join("isolated"), false).unwrap_err();
    assert!(matches!(
        error,
        AcceptanceReplayError::InputUnavailable { .. }
    ));
}

#[test]
fn acceptance_replay_fails_closed_for_empty_or_malformed_input() {
    let tmp = tempdir().unwrap();
    let (manifest, output, _) = write_fixture(tmp.path());
    let input = tmp.path().join("historical-finnhub.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();

    fs::write(&input, b"[]").unwrap();
    value["input_manifest_digest"] = json!(sha256_bytes(b"[]"));
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    let error = run_acceptance_replay(&manifest, &output, false).unwrap_err();
    assert!(matches!(error, AcceptanceReplayError::AssertionFailed(_)));
    assert!(output.join("acceptance-replay.receipt.json").is_file());

    let malformed = [0xff, 0xfe, 0xfd];
    fs::write(&input, malformed).unwrap();
    value["input_manifest_digest"] = json!(sha256_bytes(&malformed));
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    let error =
        run_acceptance_replay(&manifest, &tmp.path().join("malformed-output"), false).unwrap_err();
    assert!(matches!(
        error,
        AcceptanceReplayError::InputUnavailable { .. }
    ));
}

#[test]
fn acceptance_replay_rejects_publishable_output_paths_and_digest_mismatch() {
    let tmp = tempdir().unwrap();
    let (manifest, output, _) = write_fixture(tmp.path());
    let canonical = tmp.path().join("canonical");

    let error = run_acceptance_replay(&manifest, &canonical, false).unwrap_err();
    assert!(matches!(
        error,
        AcceptanceReplayError::IsolatedOutputRequired { .. }
    ));

    let mut value: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value["input_manifest_digest"] = json!("sha256:wrong");
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    let error = run_acceptance_replay(&manifest, &output, false).unwrap_err();
    assert!(matches!(
        error,
        AcceptanceReplayError::InputDigestMismatch { .. }
    ));
}

#[test]
fn acceptance_replay_workflow_has_no_canonical_publication_step() {
    let workflow = include_str!("../.github/workflows/acceptance_replay.yml");

    assert!(workflow.contains("acceptance-replay"));
    assert!(workflow.contains("actions/upload-artifact"));
    assert!(!workflow.contains("git push"));
    assert!(!workflow.contains("data branch"));
    assert!(!workflow.contains("--force"));
    assert!(!workflow.contains("ignore-snapshot-conflict"));
}

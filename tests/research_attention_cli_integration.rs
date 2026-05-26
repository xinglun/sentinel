// Research Attention の CLI 境界を固定する統合テスト。

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn prepare_workspace(extra_config: &str) -> TempDir {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = root.join("config.toml");
    let raw = fs::read_to_string(&config_path).expect("failed to read base config.toml");
    let mut raw = strip_optional_calibration_sections(&raw);

    let save_to = tmp.path().to_string_lossy().to_string();
    raw = raw.replace(
        "save_to = \"./reports\"",
        &format!("save_to = \"{}\"", save_to),
    );
    raw.push_str(extra_config);

    fs::write(tmp.path().join("config.toml"), raw).expect("failed to write temp config.toml");
    tmp
}

fn strip_optional_calibration_sections(raw: &str) -> String {
    let mut output = String::new();
    let mut skipping = false;
    for line in raw.lines() {
        if line.starts_with('[') {
            skipping = line.starts_with("[research_attention.")
                || line.starts_with("[asset_thesis.")
                || line == "[macro_gravity]"
                || line == "[gray_rhino_escalation]"
                || line == "[gray_rhino_provider_registry]";
        }
        if !skipping {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

fn strip_gray_rhino_provider_registry_section(raw: &str) -> String {
    let mut output = String::new();
    let mut skipping = false;
    for line in raw.lines() {
        if line.starts_with('[') {
            skipping = line == "[gray_rhino_provider_registry]";
        }
        if !skipping {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

fn prepare_standard_workspace(language: &str) -> TempDir {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let raw =
        fs::read_to_string(root.join("config.toml")).expect("failed to read base config.toml");
    let mut raw = strip_gray_rhino_provider_registry_section(&raw);
    raw = raw.replace(
        "save_to = \"./reports\"",
        &format!("save_to = \"{}\"", tmp.path().to_string_lossy()),
    );
    raw = raw.replace(
        "language = \"zh-cn\"",
        &format!("language = \"{language}\""),
    );
    fs::write(tmp.path().join("config.toml"), raw).expect("failed to write temp config.toml");
    tmp
}

#[test]
fn gray_rhino_escalation_outputs_structural_monitor_without_trade_signal() {
    let tmp = prepare_workspace(
        r#"

[gray_rhino_escalation]
risk_expansion_rate = "ELEVATED"
constraint_growth_rate = "LOW"
dependency_centralization = "HIGH"
awareness_decay = "HIGH"
narrative_overconfidence = "ELEVATED"
single_point_fragility = "MODERATE"
fallback_survivability_risk = "MODERATE"
notes = [
  "基础设施依赖集中仍在扩大。",
  "市场对治理风险的敏感度正在下降。",
  "马上卖出",
  "Musk 非常危险"
]
"#,
    );

    let out = run_cli(&tmp, &["gray-rhino"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("灰犀牛升级监控"));
    assert!(stdout.contains("输入来源: 人工结构基线（配置输入）"));
    assert!(stdout.contains("评估方法: 显式规则判定（可重放）"));
    assert!(stdout.contains("状态: 风险常态化"));
    assert!(stdout.contains("升级趋势: 上升"));
    assert!(stdout.contains("基础设施依赖集中仍在扩大。"));
    assert!(stdout.contains("不生成交易信号。"));
    assert!(stdout.contains("不代表自动事实发现"));
    assert!(stdout.contains("已抑制违反结构性观察边界的 notes: 2"));
    assert!(!stdout.contains("马上卖出"));
    assert!(!stdout.contains("Musk"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("Gate"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn gray_rhino_escalation_output_has_zh_en_ja_boundary_notice() {
    for (language, title, expected, notice, forbidden) in [
        (
            "zh-cn",
            "灰犀牛升级监控",
            "状态: 风险常态化",
            "不生成交易信号。",
            "State:",
        ),
        (
            "en-us",
            "Gray Rhino Escalation",
            "State: Normalized",
            "It does not generate trading signals.",
            "风险扩张速度",
        ),
        (
            "ja-jp",
            "灰色のサイ昇格監視",
            "状態: リスク常態化",
            "取引シグナルを生成しない。",
            "State:",
        ),
    ] {
        let tmp = prepare_workspace(
            r#"

[gray_rhino_escalation]
risk_expansion_rate = "ELEVATED"
constraint_growth_rate = "LOW"
dependency_centralization = "HIGH"
awareness_decay = "HIGH"
narrative_overconfidence = "ELEVATED"
single_point_fragility = "MODERATE"
fallback_survivability_risk = "MODERATE"
"#,
        );
        set_output_language(&tmp, language);

        let out = run_cli(&tmp, &["gray-rhino-escalation"]);

        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains(title));
        assert!(stdout.contains(expected));
        assert!(stdout.contains(notice));
        assert!(!stdout.contains(forbidden));
    }
}

#[test]
fn gray_rhino_governance_evidence_ingest_writes_jsonl_without_escalation() {
    let tmp = prepare_standard_workspace("zh-cn");
    let evidence_path = tmp.path().join("governance_evidence.json");
    fs::write(
        &evidence_path,
        r#"{
  "subject": "Example issuer",
  "source": {
    "source_type": "GovernanceDocument",
    "source_title": "Proxy statement",
    "publisher": "Example issuer",
    "source_url": "https://example.com/proxy",
    "repository_path": null,
    "observed_at": "2026-05-25",
    "retrieved_at": "2026-05-25"
  },
  "confidence": 0.9,
  "extraction_note": "Proxy statement discloses voting rights.",
  "structural_fact": "Dual class shares create unequal voting rights.",
  "metrics": {
    "founder_voting_power": 61.2,
    "independent_board_ratio": 0.42,
    "dual_class_structure": true,
    "super_voting_rights": true,
    "succession_disclosure": false
  }
}"#,
    )
    .expect("failed to write governance evidence fixture");

    let out = run_cli(
        &tmp,
        &[
            "ingest-gray-rhino-governance",
            "--file",
            evidence_path.to_str().unwrap(),
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Successfully ingested GovernanceConcentration evidence."));
    assert!(stdout.contains("Boundary: evidence only"));
    let store = fs::read_to_string(tmp.path().join("gray_rhino_evidence.jsonl"))
        .expect("failed to read gray rhino evidence store");
    assert!(store.contains("\"category\":\"GovernanceConcentration\""));
    assert!(!tmp.path().join("gray_rhino_snapshots.jsonl").exists());
}

#[test]
fn gray_rhino_dependency_evidence_ingest_writes_jsonl_without_escalation() {
    let tmp = prepare_standard_workspace("zh-cn");
    let evidence_path = tmp.path().join("dependency_evidence.json");
    fs::write(
        &evidence_path,
        r#"{
  "subject": "Example issuer",
  "source": {
    "source_type": "SupplierDisclosure",
    "source_title": "Supplier dependency disclosure",
    "publisher": "Example issuer",
    "source_url": "https://example.com/supplier",
    "repository_path": null,
    "observed_at": "2026-05-25",
    "retrieved_at": "2026-05-25"
  },
  "confidence": 0.86,
  "extraction_note": "Supplier disclosure identifies dependency concentration.",
  "structural_fact": "Critical supplier dependency has no disclosed fallback.",
  "metrics": {
    "dependency_kind": "Supplier",
    "dependency_name": "Example supplier",
    "concentration_ratio": 0.7,
    "single_point_of_failure": true,
    "fallback_disclosed": false
  }
}"#,
    )
    .expect("failed to write dependency evidence fixture");

    let out = run_cli(
        &tmp,
        &[
            "ingest-gray-rhino-dependency",
            "--file",
            evidence_path.to_str().unwrap(),
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Successfully ingested DependencyConcentration evidence."));
    assert!(stdout.contains("Boundary: evidence only"));
    let store = fs::read_to_string(tmp.path().join("gray_rhino_evidence.jsonl"))
        .expect("failed to read gray rhino evidence store");
    assert!(store.contains("\"category\":\"DependencyConcentration\""));
    assert!(!tmp.path().join("gray_rhino_snapshots.jsonl").exists());
}

#[test]
fn gray_rhino_institutional_evidence_ingest_writes_jsonl_without_escalation() {
    let tmp = prepare_standard_workspace("zh-cn");
    let evidence_path = tmp.path().join("institutional_evidence.json");
    fs::write(
        &evidence_path,
        r#"{
  "subject": "Example issuer",
  "source": {
    "source_type": "GovernanceDocument",
    "source_title": "Institutional maturity disclosure",
    "publisher": "Example issuer",
    "source_url": "https://example.com/institutional",
    "repository_path": null,
    "observed_at": "2026-05-25",
    "retrieved_at": "2026-05-25"
  },
  "confidence": 0.83,
  "extraction_note": "Annual report discloses governance maturity controls.",
  "structural_fact": "Institutional oversight maturity is supported by disclosures.",
  "metrics": {
    "succession_structure_disclosed": true,
    "external_audit_present": true,
    "disclosure_quality_score": 0.72,
    "oversight_evolution_disclosed": true,
    "compliance_maturity_level": "developing"
  }
}"#,
    )
    .expect("failed to write institutional evidence fixture");

    let out = run_cli(
        &tmp,
        &[
            "ingest-gray-rhino-institutional",
            "--file",
            evidence_path.to_str().unwrap(),
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Successfully ingested InstitutionalMaturity evidence."));
    assert!(stdout.contains("Boundary: evidence only"));
    let store = fs::read_to_string(tmp.path().join("gray_rhino_evidence.jsonl"))
        .expect("failed to read gray rhino evidence store");
    assert!(store.contains("\"category\":\"InstitutionalMaturity\""));
    assert!(!tmp.path().join("gray_rhino_snapshots.jsonl").exists());
}

#[test]
fn gray_rhino_redundancy_evidence_ingest_writes_jsonl_without_escalation() {
    let tmp = prepare_standard_workspace("zh-cn");
    let evidence_path = tmp.path().join("redundancy_evidence.json");
    fs::write(
        &evidence_path,
        r#"{
  "subject": "Example issuer",
  "source": {
    "source_type": "SupplierDisclosure",
    "source_title": "Redundancy disclosure",
    "publisher": "Example issuer",
    "source_url": "https://example.com/redundancy",
    "repository_path": null,
    "observed_at": "2026-05-25",
    "retrieved_at": "2026-05-25"
  },
  "confidence": 0.84,
  "extraction_note": "Supplier disclosure identifies redundancy controls.",
  "structural_fact": "Fallback availability is disclosed.",
  "metrics": {
    "fallback_available": true,
    "alternative_supplier_count": 2,
    "redundancy_ratio": 0.5,
    "recovery_path_disclosed": true,
    "failover_tested": false
  }
}"#,
    )
    .expect("failed to write redundancy evidence fixture");

    let out = run_cli(
        &tmp,
        &[
            "ingest-gray-rhino-redundancy",
            "--file",
            evidence_path.to_str().unwrap(),
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Successfully ingested Redundancy evidence."));
    assert!(stdout.contains("Boundary: evidence only"));
    let store = fs::read_to_string(tmp.path().join("gray_rhino_evidence.jsonl"))
        .expect("failed to read gray rhino evidence store");
    assert!(store.contains("\"category\":\"Redundancy\""));
    assert!(!tmp.path().join("gray_rhino_snapshots.jsonl").exists());
}

#[test]
fn gray_rhino_daily_report_uses_evidence_backed_sensor_health() {
    let tmp = prepare_standard_workspace("en-us");
    let evidence_path = tmp.path().join("dependency_evidence.json");
    fs::write(
        &evidence_path,
        r#"{
  "subject": "Example issuer",
  "source": {
    "source_type": "SupplierDisclosure",
    "source_title": "Supplier dependency disclosure",
    "publisher": "Example issuer",
    "source_url": "https://example.com/supplier",
    "repository_path": null,
    "observed_at": "2026-05-25",
    "retrieved_at": "2026-05-25"
  },
  "confidence": 0.86,
  "extraction_note": "Supplier disclosure identifies dependency concentration.",
  "structural_fact": "Critical supplier dependency has no disclosed fallback.",
  "metrics": {
    "dependency_kind": "Supplier",
    "dependency_name": "Example supplier",
    "concentration_ratio": 0.7,
    "single_point_of_failure": true,
    "fallback_disclosed": false
  }
}"#,
    )
    .expect("failed to write dependency evidence fixture");
    let ingest = run_cli(
        &tmp,
        &[
            "ingest-gray-rhino-dependency",
            "--file",
            evidence_path.to_str().unwrap(),
        ],
    );
    assert!(ingest.status.success());

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Evidence-backed sensor store"));
    assert!(stdout.contains("Gray Rhino Sensor Health"));
    assert!(stdout.contains("Dependency Concentration: 1 evidence record"));
    assert!(stdout.contains("It does not generate trading signals."));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("Gate override"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn gray_rhino_backfill_and_readiness_report_stays_non_signal() {
    let tmp = prepare_standard_workspace("en-us");
    let dependency_path = tmp.path().join("dependency_source.txt");
    let institutional_path = tmp.path().join("institutional_source.txt");
    let manifest_path = tmp.path().join("backfill_manifest.json");
    fs::write(
        &dependency_path,
        "dependency_kind: Supplier; dependency_name: Supplier A; concentration_ratio: 0.7",
    )
    .expect("failed to write dependency source");
    fs::write(
        &institutional_path,
        "succession_structure_disclosed: true; external_audit_present: true; disclosure_quality_score: 0.72",
    )
    .expect("failed to write institutional source");
    fs::write(
        &manifest_path,
        format!(
            r#"[
  {{"category":"DependencyConcentration","symbol":"EXAMPLE","file":"{}"}},
  {{"category":"InstitutionalMaturity","symbol":"EXAMPLE","file":"{}"}}
]"#,
            dependency_path.display(),
            institutional_path.display()
        ),
    )
    .expect("failed to write backfill manifest");

    let backfill = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-backfill",
            "--file",
            manifest_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
        ],
    );
    assert!(backfill.status.success());
    let backfill_stdout = String::from_utf8_lossy(&backfill.stdout);
    assert!(backfill_stdout.contains("Gray Rhino Multi-Category Backfill Dry Run"));
    assert!(backfill_stdout.contains("Backfill entries processed: 2"));
    assert!(backfill_stdout.contains("dry-run only"));
    assert!(!tmp.path().join("gray_rhino_evidence.jsonl").exists());

    let evidence_path = tmp.path().join("dependency_evidence.json");
    fs::write(
        &evidence_path,
        r#"{
  "subject": "Example issuer",
  "source": {
    "source_type": "SupplierDisclosure",
    "source_title": "Supplier dependency disclosure",
    "publisher": "Example issuer",
    "source_url": "https://example.com/supplier",
    "repository_path": null,
    "observed_at": "2026-05-25",
    "retrieved_at": "2026-05-25"
  },
  "confidence": 0.86,
  "extraction_note": "Supplier disclosure identifies dependency concentration.",
  "structural_fact": "Critical supplier dependency has no disclosed fallback.",
  "metrics": {
    "dependency_kind": "Supplier",
    "dependency_name": "Example supplier",
    "concentration_ratio": 0.7,
    "single_point_of_failure": true,
    "fallback_disclosed": false
  }
}"#,
    )
    .expect("failed to write dependency evidence fixture");
    assert!(run_cli(
        &tmp,
        &[
            "ingest-gray-rhino-dependency",
            "--file",
            evidence_path.to_str().unwrap(),
        ],
    )
    .status
    .success());

    let report = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);
    assert!(report.status.success());
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(stdout.contains("Readiness score: 25.0% (1/4)"));
    assert!(stdout.contains("Evidence quality dimensions"));
    assert!(stdout.contains("readiness=insufficient"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("execution signal"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn gray_rhino_auto_discovery_finds_governance_control_and_reports_inline() {
    let tmp = prepare_standard_workspace("en-us");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_text = fs::read_to_string(
        root.join("tests/fixtures/gray_rhino_discovery/spacex_governance_control.txt"),
    )
    .expect("failed to read SpaceX governance fixture");
    let discovery_dir = tmp.path().join("gray_rhino_sources").join("SPACEX");
    fs::create_dir_all(&discovery_dir).expect("failed to create discovery dir");
    let source_path = discovery_dir.join("spacex_governance_control.txt");
    fs::write(&source_path, source_text).expect("failed to write discovery source");

    let cli = run_cli(
        &tmp,
        &[
            "discover-gray-rhino",
            "--symbol",
            "SPACEX",
            "--file",
            source_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
        ],
    );
    assert!(cli.status.success());
    let cli_stdout = String::from_utf8_lossy(&cli.stdout);
    assert!(cli_stdout.contains("Gray Rhino Auto Discovery"));
    assert!(cli_stdout.contains("GovernanceConcentration"));
    assert!(cli_stdout.contains("Trigger watch"));

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Gray Rhino Summary (semantic isolation)"));
    assert!(stdout.contains("Market active candidates: 0"));
    assert!(stdout.contains("Company active candidates: none"));
    assert!(stdout.contains("Gray Rhino Inline Reference (semantic isolation)"));
    assert!(!stdout.contains("SPACEX / Company / Governance Concentration / Expanding"));
    assert!(!stdout.contains("IPO voting terms"));
    assert!(stdout.contains("reference only; no trading"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("execution signal"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn gray_rhino_observation_daily_report_does_not_replay_sec_htm_cache() {
    let tmp = prepare_standard_workspace("en-us");
    let discovery_dir = tmp
        .path()
        .join("gray_rhino_sources")
        .join("governance")
        .join("TSLA");
    fs::create_dir_all(&discovery_dir).expect("failed to create discovery dir");
    fs::write(
        discovery_dir.join("tsla-proxy.htm"),
        "The founder controls majority voting power through class B shares. The board is controlled and no independent directors provide effective checks.",
    )
    .expect("failed to write htm SEC cache fixture");
    let stale_dir = tmp
        .path()
        .join("gray_rhino_sources")
        .join("governance")
        .join("STALE");
    fs::create_dir_all(&stale_dir).expect("failed to create stale discovery dir");
    fs::write(
        stale_dir.join("stale-proxy.htm"),
        "The founder controls majority voting power through class B shares. The board is controlled and no independent directors provide effective checks.",
    )
    .expect("failed to write stale htm SEC cache fixture");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("TSLA / Company / Governance Concentration / Expanding"));
    assert!(!stdout.contains("STALE / Company / Governance Concentration"));
    assert!(stdout.contains("reference only; no trading"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("execution signal"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn gray_rhino_observation_old_cache_does_not_refresh_persisted_candidate_date() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"GOOG","state":"Visible","evidence":["Persisted old founder voting control candidate."],"watch_triggers":["proxy update"],"source_title":"Persisted old SEC proxy","observed_at":"2026-04-24"}
"#,
    )
    .expect("failed to write old candidate store");
    let cache_dir = tmp
        .path()
        .join("gray_rhino_sources")
        .join("governance")
        .join("GOOG");
    fs::create_dir_all(&cache_dir).expect("failed to create source cache dir");
    fs::write(
        cache_dir.join("old-proxy.htm"),
        "The founder controls majority voting power through class B shares. The board is controlled and no independent directors provide effective checks.",
    )
    .expect("failed to write old cache source");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("GOOG / Company / Governance Concentration: Cooling"));
    assert!(stdout.contains("Cooling"));
    assert!(stdout.contains("latest: 2026-04-24"));
    assert!(stdout.contains("stale_days: 31"));
    assert!(!stdout.contains("latest: 2026-05-25"));
    assert!(!stdout.contains("Intensifying"));
}

#[test]
fn gray_rhino_replay_date_is_honored_without_transition_log() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"GOOG","state":"Visible","evidence":["Persisted old founder voting control candidate."],"watch_triggers":["proxy update"],"source_title":"Persisted old SEC proxy","observed_at":"2026-04-24"}
"#,
    )
    .expect("failed to write candidate store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("latest: 2026-04-24"));
    assert!(stdout.contains("stale_days: 31"));
    assert!(!stdout.contains("stale_days: 32"));
}

#[test]
fn gray_rhino_replay_future_candidate_is_excluded_from_historical_report() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Critical","evidence":["Future critical evidence."],"watch_triggers":["future proxy"],"source_title":"Future SEC proxy","observed_at":"2026-05-27"}
"#,
    )
    .expect("failed to write candidate store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("Future critical evidence."));
    assert!(!stdout.contains("TSLA / Company / Governance Concentration / Critical"));
}

#[test]
fn gray_rhino_audit_replay_excludes_future_ops_and_refresh_status() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_discovery_runs.jsonl"),
        r#"{"run_id":"current-run-2026-05-24","provider":"Fred","as_of_date":"2026-05-24","dry_run":false,"source_count":1,"accepted":1,"rejected":0,"candidate_count":1,"outcomes":[]}
{"run_id":"future-run-2026-05-27","provider":"Fred","as_of_date":"2026-05-27","dry_run":false,"source_count":9,"accepted":9,"rejected":0,"candidate_count":9,"outcomes":[]}
"#,
    )
    .expect("failed to write discovery runs");
    fs::write(
        tmp.path().join("gray_rhino_refresh_status_latest.json"),
        r#"{"status":"succeeded","sec":"succeeded","finnhub":"succeeded","fred":"succeeded","sec_accepted":1,"sec_rejected":0,"finnhub_accepted":1,"finnhub_rejected":0,"fred_accepted":1,"fred_rejected":0,"failed_providers":"","date":"2026-05-27"}
"#,
    )
    .expect("failed to write future refresh status");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("latest_run: current-run-2026-05-24"));
    assert!(!stdout.contains("future-run-2026-05-27"));
    assert!(!stdout.contains("refresh_date: 2026-05-27"));
}

#[test]
fn gray_rhino_refresh_status_ledger_replays_as_of_date() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_refresh_status_latest.json"),
        r#"{"status":"failed","sec":"failed","finnhub":"skipped","fred":"skipped","failed_providers":"sec"}
"#,
    )
    .expect("failed to write legacy undated refresh status");

    let legacy_out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-24"]);
    assert!(legacy_out.status.success());
    let legacy_stdout = String::from_utf8_lossy(&legacy_out.stdout);
    assert!(!legacy_stdout.contains("Gray Rhino Refresh Status"));
    assert!(!legacy_stdout.contains("overall_status: failed"));

    fs::write(
        tmp.path().join("gray_rhino_refresh_status.jsonl"),
        r#"{"date":"2026-05-24","status":"succeeded","sec":"succeeded","finnhub":"skipped","fred":"skipped","sec_accepted":1,"sec_rejected":0,"finnhub_accepted":0,"finnhub_rejected":0,"fred_accepted":0,"fred_rejected":0,"failed_providers":""}
{"date":"2026-05-27","status":"failed","sec":"failed","finnhub":"skipped","fred":"skipped","sec_accepted":0,"sec_rejected":1,"finnhub_accepted":0,"finnhub_rejected":0,"fred_accepted":0,"fred_rejected":0,"failed_providers":"sec"}
"#,
    )
    .expect("failed to write refresh status ledger");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("refresh_date: 2026-05-24"));
    assert!(stdout.contains("overall_status: succeeded"));
    assert!(!stdout.contains("refresh_date: 2026-05-27"));
    assert!(!stdout.contains("failed_providers: sec"));
}

#[test]
fn gray_rhino_candidate_store_feeds_daily_inline_reference() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Expanding","evidence":["Persisted founder voting control candidate."],"watch_triggers":["proxy update"],"source_title":"Persisted SEC proxy","observed_at":"2026-05-25"}
{"scope":"Company","kind":"GovernanceConcentration","subject":"STALE","state":"Expanding","evidence":["Persisted stale candidate."],"watch_triggers":["proxy update"],"source_title":"Persisted stale proxy","observed_at":"2026-05-25"}
{"scope":"Market","kind":"LiquidityFragility","subject":"Market","state":"Expanding","evidence":["Persisted liquidity fragility candidate."],"watch_triggers":["credit spread widening"],"source_title":"Persisted FRED macro","observed_at":"2026-05-25"}
"#,
    )
    .expect("failed to write candidate store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("TSLA / Company / Governance Concentration / Expanding"));
    assert!(stdout.contains("Persisted founder voting control candidate."));
    assert!(stdout.contains("Market / Market / Liquidity Fragility / Expanding"));
    assert!(stdout.contains("Persisted liquidity fragility candidate."));
    assert!(stdout.contains("Market Reference"));
    assert!(stdout.contains("Watchlist Inline Reference"));
    assert!(!stdout.contains("STALE / Company / Governance Concentration"));
    assert!(stdout.contains("reference only; no trading"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("execution signal"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn gray_rhino_display_latest_uses_latest_candidate_body() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Visible","evidence":["Old visible evidence."],"watch_triggers":["old proxy"],"source_title":"Prior SEC proxy","observed_at":"2026-05-24"}
{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Critical","evidence":["New critical evidence."],"watch_triggers":["new proxy"],"source_title":"Current SEC proxy","observed_at":"2026-05-25"}
"#,
    )
    .expect("failed to write candidate store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("TSLA / Company / Governance Concentration / Critical"));
    assert!(stdout.contains("New critical evidence."));
    assert!(!stdout.contains("TSLA / Company / Governance Concentration / Visible"));
    assert!(!stdout.contains("Old visible evidence."));
    assert!(stdout.contains("TSLA / Company / Governance Concentration: Critical"));
    assert!(stdout.contains("Intensifying"));
}

#[test]
fn gray_rhino_display_latest_prefers_cooling_after_critical() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Critical","evidence":["Critical evidence."],"watch_triggers":["critical proxy"],"source_title":"Critical SEC proxy","observed_at":"2026-05-24"}
{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Cooling","evidence":["Cooling evidence."],"watch_triggers":["cooling proxy"],"source_title":"Cooling SEC proxy","observed_at":"2026-05-25"}
"#,
    )
    .expect("failed to write candidate store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("TSLA / Company / Governance Concentration / Cooling"));
    assert!(stdout.contains("Cooling evidence."));
    assert!(!stdout.contains("TSLA / Company / Governance Concentration / Critical"));
    assert!(stdout.contains("TSLA / Company / Governance Concentration: Cooling"));
}

#[test]
fn gray_rhino_lifecycle_same_day_resolved_wins_over_critical() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Critical","evidence":["Critical evidence."],"watch_triggers":["critical proxy"],"source_title":"Critical SEC proxy","observed_at":"2026-05-25"}
{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Resolved","evidence":["Resolved evidence."],"watch_triggers":["resolved proxy"],"source_title":"Resolved SEC proxy","observed_at":"2026-05-25","resolved_at":"2026-05-25"}
"#,
    )
    .expect("failed to write candidate store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("TSLA / Company / Governance Concentration / Resolved"));
    assert!(stdout.contains("Resolved evidence."));
    assert!(!stdout.contains("TSLA / Company / Governance Concentration / Critical"));
}

#[test]
fn gray_rhino_watchlist_inline_report_groups_company_and_market_candidates() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Expanding","evidence":["Founder voting control remains visible."],"watch_triggers":["proxy update"],"source_title":"SEC proxy","observed_at":"2026-05-25"}
{"scope":"Market","kind":"LiquidityFragility","subject":"Market","state":"Critical","evidence":["FRED threshold critical."],"watch_triggers":["credit spread widening"],"source_title":"FRED macro","observed_at":"2026-05-25"}
"#,
    )
    .expect("failed to write candidate store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Gray Rhino Inline Reference (semantic isolation)"));
    assert!(stdout.contains("Market Reference"));
    assert!(stdout.contains("Market / Market / Liquidity Fragility / Critical"));
    assert!(stdout.contains("Watchlist Inline Reference"));
    assert!(stdout.contains("- TSLA"));
    assert!(stdout.contains("TSLA / Company / Governance Concentration / Expanding"));
    assert!(!stdout.contains("Other Company Reference"));
    assert!(stdout.contains("Watchlist Inline Monitoring"));
    assert!(stdout.contains("reference only; no trading"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("execution signal"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn gray_rhino_summary_compresses_market_and_company_candidates() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Visible","evidence":["Founder voting control remains visible."],"watch_triggers":["proxy update"],"source_title":"Prior SEC proxy","observed_at":"2026-05-24"}
{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Visible","evidence":["Founder voting control remains visible."],"watch_triggers":["proxy update"],"source_title":"Current SEC proxy","observed_at":"2026-05-25"}
{"scope":"Market","kind":"LiquidityFragility","subject":"Market","state":"Critical","evidence":["FRED threshold critical."],"watch_triggers":["credit spread widening"],"source_title":"FRED macro","observed_at":"2026-05-25"}
"#,
    )
    .expect("failed to write candidate store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Gray Rhino Summary (semantic isolation)"));
    assert!(stdout.contains("Market active candidates: 1"));
    assert!(stdout.contains("Company active candidates: TSLA"));
    assert!(stdout.contains("Company intensifying watch: TSLA"));
    assert!(stdout.contains("summary only; no trading"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("execution signal"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn gray_rhino_summary_github_actions_runs_refresh_before_radar() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workflow = fs::read_to_string(root.join(".github/workflows/daily_radar.yml"))
        .expect("failed to read daily radar workflow");

    assert!(workflow.contains("make gray-rhino-refresh"));
    assert!(
        workflow.find("make gray-rhino-refresh").unwrap()
            < workflow.find("make radar-release").unwrap()
    );
    assert!(workflow.contains("GRAY_RHINO_REFRESH_PROVIDERS=\"${GRAY_RHINO_PROVIDERS}\""));
    assert!(workflow.contains("GRAY_RHINO_REFRESH_DATE=\"${DATE_JST}\""));
    assert!(workflow.contains("GRAY_RHINO_REFRESH_ARGS=\"--date ${DATE_JST}\""));
    assert!(!workflow.contains("GRAY_RHINO_REFRESH_DAILY_ARGS"));
    assert!(workflow.contains("reports/gray_rhino_refresh_status_latest.json"));
    assert!(workflow.contains("gray_rhino_refresh_status.jsonl"));
    assert!(workflow.contains("\"reports/gray_rhino_refresh_status_latest.json\""));
    assert!(workflow.contains("Gray Rhino refresh failed before radar but radar will continue"));
    assert!(!workflow.contains("FINNHUB_API_KEY or FRED_API_KEY is not configured"));
}

#[test]
fn gray_rhino_monitoring_state_reports_candidate_intensification() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Visible","evidence":["Founder voting control remains visible."],"watch_triggers":["proxy update"],"source_title":"Prior SEC proxy","observed_at":"2026-05-24"}
{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Visible","evidence":["Founder voting control remains visible."],"watch_triggers":["proxy update"],"source_title":"Current SEC proxy","observed_at":"2026-05-25"}
"#,
    )
    .expect("failed to write candidate store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Gray Rhino Monitoring State (semantic isolation)"));
    assert!(stdout.contains("TSLA / Company / Governance Concentration: Expanding"));
    assert!(stdout.contains("Intensifying"));
    assert!(stdout.contains("observations: 2"));
    assert!(stdout.contains("reference only; no trading"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("execution signal"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn gray_rhino_completion_legacy_evidence_missing_risk_effect_is_visible() {
    let tmp = prepare_standard_workspace("zh-cn");
    fs::write(
        tmp.path().join("gray_rhino_evidence.jsonl"),
        r#"{"category":"DependencyConcentration","source":{"source_type":"SupplierDisclosure","source_title":"Legacy dependency disclosure","publisher":"Example issuer","source_url":"https://example.com/legacy","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.86,"extraction_note":"Legacy record without risk effect.","structural_fact":"Dependency concentration is disclosed."}
"#,
    )
    .expect("failed to write legacy evidence");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("旧证据记录不可评分"));
    assert!(stdout.contains("缺少风险作用的记录数: 1"));
    assert!(stdout.contains("不参与正式升级评分"));
    assert!(stdout.contains("风险升级评估: 尚无正式证据 / 未启用人工基线。"));
    assert!(!stdout.contains("输入来源: Evidence-backed sensor store"));
    assert!(!stdout.contains("状态: 风险扩张"));
}

#[test]
fn gray_rhino_completion_evidence_store_boundary_does_not_claim_manual_only() {
    let tmp = prepare_standard_workspace("zh-cn");
    let evidence_path = tmp.path().join("institutional_evidence.json");
    fs::write(
        &evidence_path,
        r#"{
  "subject": "Example issuer",
  "source": {
    "source_type": "IndependentAudit",
    "source_title": "Institutional maturity audit",
    "publisher": "Example auditor",
    "source_url": "https://example.com/audit",
    "repository_path": null,
    "observed_at": "2026-05-25",
    "retrieved_at": "2026-05-25"
  },
  "confidence": 0.88,
  "extraction_note": "External audit and succession structure are disclosed.",
  "structural_fact": "Institutional oversight maturity is supported.",
  "metrics": {
    "succession_structure_disclosed": true,
    "external_audit_present": true,
    "disclosure_quality_score": 0.72,
    "oversight_evolution_disclosed": true,
    "compliance_maturity_level": "developing"
  }
}"#,
    )
    .expect("failed to write institutional evidence");
    let ingest = run_cli(
        &tmp,
        &[
            "ingest-gray-rhino-institutional",
            "--file",
            evidence_path.to_str().unwrap(),
        ],
    );
    assert!(ingest.status.success());

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("输入来源: Evidence-backed sensor store"));
    assert!(stdout.contains("当前正式评估来自结构化 EvidenceStore"));
    assert!(stdout.contains("审计链: 结构化 EvidenceStore -> directional risk_effect -> 日次快照"));
    assert!(!stdout.contains("审计链: 人工结构基线 -> 七项观测 -> 日次快照"));
    assert!(!stdout.contains("尚未接入专用灰犀牛外部证据源"));
}

#[test]
fn gray_rhino_final_query_command_does_not_write_snapshot() {
    let tmp = prepare_standard_workspace("en-us");

    let out = run_cli(&tmp, &["gray-rhino"]);

    assert!(out.status.success());
    assert!(!tmp.path().join("gray_rhino_snapshots.jsonl").exists());
}

#[test]
fn gray_rhino_readonly_daily_calibration_does_not_write_snapshot() {
    let tmp = prepare_standard_workspace("en-us");
    fs::write(
        tmp.path().join("gray_rhino_evidence.jsonl"),
        r#"{"category":"GovernanceConcentration","source":{"source_type":"GovernanceDocument","source_title":"Proxy statement","publisher":"Example issuer","source_url":"https://example.com/proxy","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.9,"risk_effect":"Amplifying","extraction_note":"Proxy statement discloses voting rights.","structural_fact":"Dual class shares create unequal voting rights."}
{"category":"DependencyConcentration","source":{"source_type":"SupplierDisclosure","source_title":"Supplier disclosure","publisher":"Example issuer","source_url":"https://example.com/supplier","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.9,"risk_effect":"Amplifying","extraction_note":"Supplier disclosure identifies dependency.","structural_fact":"Critical supplier dependency has no fallback."}
"#,
    )
    .expect("failed to write evidence store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    assert!(!tmp.path().join("gray_rhino_snapshots.jsonl").exists());
}

#[test]
fn gray_rhino_completion_zh_candidate_body_does_not_leak_enum_labels() {
    let tmp = prepare_standard_workspace("zh-cn");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Expanding","evidence":["Founder voting control remains visible. Governance check-and-balance weakness detected."],"watch_triggers":["IPO voting terms","board composition changes"],"source_title":"SEC proxy","observed_at":"2026-05-24"}
{"scope":"Company","kind":"GovernanceConcentration","subject":"TSLA","state":"Critical","evidence":["Founder voting control remains visible. Governance check-and-balance weakness detected."],"watch_triggers":["IPO voting terms","board composition changes"],"source_title":"SEC proxy","observed_at":"2026-05-25"}
{"scope":"Market","kind":"LiquidityFragility","subject":"Market","state":"Expanding","evidence":["Liquidity or rate-pressure fragility detected."],"watch_triggers":["credit spread widening"],"source_title":"FRED macro","observed_at":"2026-05-25"}
"#,
    )
    .expect("failed to write candidate store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("TSLA / 公司 / 治理集中 / 临界"));
    assert!(stdout.contains("Market / 市场 / 流动性脆弱 / 扩张"));
    assert!(stdout.contains("升温"));
    assert!(stdout.contains("检测到创始人或单一主体投票控制。"));
    assert!(stdout.contains("IPO 投票条款"));
    assert!(!stdout.contains("GovernanceConcentration"));
    assert!(!stdout.contains("LiquidityFragility"));
    assert!(!stdout.contains("Intensifying"));
}

#[test]
fn gray_rhino_source_collection_dry_run_reports_boundary() {
    let tmp = prepare_standard_workspace("en-us");

    let out = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-sources",
            "--source",
            "fred",
            "--dry-run",
            "--date",
            "2026-05-25",
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Gray Rhino Source Collection"));
    assert!(stdout.contains("provider: Fred"));
    assert!(stdout.contains("dry_run: true"));
    assert!(stdout.contains("FRED macro series fetch planned"));
    assert!(stdout.contains("source collection only"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("execution signal"));
    assert!(!stdout.contains("trend_cohesion"));

    let run_store = fs::read_to_string(tmp.path().join("gray_rhino_discovery_runs.jsonl"))
        .expect("failed to read discovery run store");
    assert!(run_store.contains("\"provider\":\"Fred\""));
    assert!(run_store.contains("\"dry_run\":true"));

    let report = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);
    assert!(report.status.success());
    let report_stdout = String::from_utf8_lossy(&report.stdout);
    assert!(report_stdout.contains("Auto Discovery Ops View"));
    assert!(report_stdout.contains("latest_run: fred-2026-05-25"));
    assert_eq!(report_stdout.matches("Auto Discovery Ops View").count(), 1);
}

#[test]
fn gray_rhino_refresh_status_is_rendered_in_daily_report() {
    let tmp = prepare_standard_workspace("zh-cn");
    fs::write(
        tmp.path().join("gray_rhino_refresh_status_latest.json"),
        r#"{"status":"partial_failure","sec":"succeeded","finnhub":"skipped","fred":"failed","sec_accepted":2,"sec_rejected":0,"finnhub_accepted":0,"finnhub_rejected":0,"fred_accepted":0,"fred_rejected":1,"failed_providers":"fred","date":"2026-05-25","reason":"FRED returned 403"}
"#,
    )
    .expect("failed to write refresh status sidecar");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("灰犀牛采集状态"));
    assert!(stdout.contains("整体状态: 部分失败"));
    assert!(stdout.contains("SEC: 成功 / Finnhub: 跳过 / FRED: 失败"));
    assert!(stdout.contains("覆盖率: SEC 2/2 / Finnhub 0/0 / FRED 0/1"));
    assert!(stdout.contains("失败来源: fred"));
    assert!(stdout.contains("采集日期: 2026-05-25"));
    assert!(stdout.contains("采集状态仅说明自动情报新鲜度"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
}

#[test]
fn gray_rhino_refresh_status_i18n_renders_zh_en_ja_labels() {
    for (lang, expected_status, expected_providers, unexpected) in [
        (
            "zh-cn",
            "整体状态: 部分失败",
            "SEC: 成功 / Finnhub: 跳过 / FRED: 失败",
            "partial_failure",
        ),
        (
            "en-us",
            "overall_status: partial_failure",
            "SEC: succeeded / Finnhub: skipped / FRED: failed",
            "",
        ),
        (
            "ja-jp",
            "全体状態: 部分失敗",
            "SEC: 成功 / Finnhub: 未実行 / FRED: 失敗",
            "partial_failure|skip|coverage|provider",
        ),
    ] {
        let tmp = prepare_standard_workspace(lang);
        fs::write(
            tmp.path().join("gray_rhino_refresh_status_latest.json"),
            r#"{"status":"partial_failure","sec":"succeeded","finnhub":"skipped","fred":"failed","sec_accepted":2,"sec_rejected":0,"finnhub_accepted":0,"finnhub_rejected":0,"fred_accepted":0,"fred_rejected":1,"failed_providers":"fred","date":"2026-05-25","reason":"FRED returned 403"}
"#,
        )
        .expect("failed to write refresh status sidecar");

        let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains(expected_status));
        assert!(stdout.contains(expected_providers));
        for forbidden in unexpected.split('|').filter(|value| !value.is_empty()) {
            assert!(!stdout.contains(forbidden));
        }
    }
}

#[test]
fn gray_rhino_report_single_language_snapshots() {
    for (language, expected, forbidden) in [
        (
            "zh-cn",
            vec!["证据质量维度", "来源多样性", "证据解释图", "失败来源"],
            vec![
                "evidence 质量",
                "source 数",
                "Evidence 解释图",
                "失败 provider",
            ],
        ),
        (
            "ja-jp",
            vec![
                "灰色のサイセンサー健全性",
                "平均信頼度",
                "由来の多様性",
                "証拠説明グラフ",
            ],
            vec![
                "sensor health",
                "平均 confidence",
                "source 数",
                "Evidence 説明",
            ],
        ),
    ] {
        let tmp = prepare_standard_workspace(language);
        fs::write(
            tmp.path().join("gray_rhino_evidence.jsonl"),
            r#"{"category":"DependencyConcentration","source":{"source_type":"SupplierDisclosure","source_title":"Dependency disclosure","publisher":"Example issuer","source_url":"https://example.com/dependency","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.86,"risk_effect":"Amplifying","extraction_note":"Supplier disclosure identifies dependency concentration.","structural_fact":"Critical supplier dependency has no disclosed fallback."}
{"category":"Redundancy","source":{"source_type":"IndependentAudit","source_title":"Legacy audit","publisher":"Example auditor","source_url":"https://example.com/audit","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.91,"risk_effect":"Unclassified","extraction_note":"Legacy record lacks direction.","structural_fact":"Fallback provider is mentioned."}
"#,
        )
        .expect("failed to write evidence store");
        fs::write(
            tmp.path().join("gray_rhino_refresh_status_latest.json"),
            r#"{"date":"2026-05-25","status":"partial_failure","sec":"succeeded","finnhub":"skipped","fred":"failed","sec_accepted":1,"sec_rejected":0,"finnhub_accepted":0,"finnhub_rejected":0,"fred_accepted":0,"fred_rejected":1,"failed_providers":"fred"}
"#,
        )
        .expect("failed to write refresh status");

        let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        for expected_text in expected {
            assert!(stdout.contains(expected_text), "missing {expected_text}");
        }
        for forbidden_text in forbidden {
            assert!(
                !stdout.contains(forbidden_text),
                "unexpected mixed term {forbidden_text}"
            );
        }
    }
}

#[test]
fn gray_rhino_refresh_make_target_runs_collectors_before_daily_report() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let makefile = fs::read_to_string(root.join("Makefile")).expect("failed to read Makefile");

    assert!(makefile.contains("gray-rhino-refresh:"));
    assert!(makefile.contains("for provider in $$providers"));
    assert!(makefile.contains("collect-gray-rhino-sources --source $$provider"));
    assert!(makefile.contains("partial_failure"));
    assert!(makefile.contains("status=\"failed\""));
    assert!(makefile.contains("status=\"skipped\""));
    assert!(makefile.contains("GRAY_RHINO_REFRESH_PROVIDERS ?= sec finnhub fred"));
    assert!(makefile.contains("gray_rhino_refresh_status_latest.json"));
    assert!(makefile.contains("gray-rhino-refresh-report:"));
    assert!(!makefile.contains("daily_status"));
    assert!(makefile.contains("test \"$$failed_count\" -eq 0"));
}

#[test]
fn gray_rhino_failure_appendix_reports_build_errors() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runner =
        fs::read_to_string(root.join("src/features/radar/interface/radar_pipeline_runner.rs"))
            .expect("failed to read radar pipeline runner");

    assert!(runner.contains("Gray Rhino: FAILED"));
    assert!(runner.contains("does not change trading, Gate, trend, or market state"));
    assert!(!runner.contains("else {\n        return;"));
}

#[test]
fn gray_rhino_renderer_is_interface_owned() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let application =
        fs::read_to_string(root.join("src/features/research/application/gray_rhino_discovery.rs"))
            .expect("failed to read discovery application");
    let interface =
        fs::read_to_string(root.join("src/features/research/interface/gray_rhino_report.rs"))
            .expect("failed to read gray rhino interface");
    let checker = fs::read_to_string(root.join("scripts/check_gray_rhino_evidence_contract.py"))
        .expect("failed to read contract checker");

    assert!(!application.contains("render_gray_rhino_inline_reference"));
    assert!(!application.contains("Boundary: reference only"));
    assert!(interface.contains("render_gray_rhino_inline_reference"));
    assert!(checker.contains("must not contain user-facing output template"));
}

#[test]
fn gray_rhino_refresh_coverage_make_target_records_provider_coverage() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let makefile = fs::read_to_string(root.join("Makefile")).expect("failed to read Makefile");

    assert!(makefile.contains("provider_status:"));
    assert!(makefile.contains("partial_count"));
    assert!(makefile.contains("sec_accepted"));
    assert!(makefile.contains("sec_rejected"));
    assert!(makefile.contains("finnhub_accepted"));
    assert!(makefile.contains("fred_rejected"));
    assert!(makefile.contains("GRAY_RHINO_REFRESH_DATE"));
    assert!(makefile.contains("gray_rhino_refresh_status.jsonl"));
    assert!(makefile.contains("gray_rhino_refresh_status_$$refresh_date.json"));
    assert!(makefile.contains("\"sec_accepted\":%s"));
}

#[test]
fn gray_rhino_quality_report_explains_dimensions_without_signals() {
    let tmp = prepare_standard_workspace("en-us");
    let dependency_path = tmp.path().join("dependency_evidence.json");
    fs::write(
        &dependency_path,
        r#"{
  "subject": "Example issuer",
  "source": {
    "source_type": "SupplierDisclosure",
    "source_title": "Supplier dependency disclosure",
    "publisher": "Example supplier report",
    "source_url": "https://example.com/supplier",
    "repository_path": null,
    "observed_at": "2026-05-25",
    "retrieved_at": "2026-05-25"
  },
  "confidence": 0.86,
  "extraction_note": "Supplier disclosure identifies dependency concentration.",
  "structural_fact": "Critical supplier dependency has no disclosed fallback.",
  "metrics": {
    "dependency_kind": "Supplier",
    "dependency_name": "Example supplier",
    "concentration_ratio": 0.7,
    "single_point_of_failure": true,
    "fallback_disclosed": false
  }
}"#,
    )
    .expect("failed to write dependency evidence");
    assert!(run_cli(
        &tmp,
        &[
            "ingest-gray-rhino-dependency",
            "--file",
            dependency_path.to_str().unwrap(),
        ],
    )
    .status
    .success());

    let report = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(report.status.success());
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(stdout.contains("Quality score: insufficient"));
    assert!(stdout.contains("source diversity 1"));
    assert!(stdout.contains("Evidence Explanation Graph"));
    assert!(stdout.contains("dependency_centralization -> DependencyConcentration"));
    assert!(
        stdout.contains("fallback_survivability_risk -> DependencyConcentration + Redundancy gap")
    );
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("execution signal"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn gray_rhino_provider_registry_backfill_records_ops_status() {
    let tmp = prepare_standard_workspace("en-us");
    let dependency_path = tmp.path().join("dependency_source.txt");
    let missing_path = tmp.path().join("missing_dependency_source.txt");
    let manifest_path = tmp.path().join("provider_registry.json");
    fs::write(
        &dependency_path,
        "dependency_kind: Supplier; dependency_name: Supplier A; concentration_ratio: 0.7; single_point_of_failure: true",
    )
    .expect("failed to write dependency source");
    fs::write(
        &manifest_path,
        format!(
            r#"[
  {{
    "category": "DependencyConcentration",
    "symbol": "EXAMPLE",
    "provider_kind": "supplier",
    "source_type": "SupplierDisclosure",
    "file": "{}",
    "observed_at": "2026-01-01",
    "freshness_days": 30,
    "expected_sha256": "outdated-hash"
  }},
  {{
    "category": "DependencyConcentration",
    "symbol": "EXAMPLE",
    "provider_kind": "cloud",
    "source_type": "InfrastructureStatus",
    "file": "{}"
  }}
]"#,
            dependency_path.display(),
            missing_path.display()
        ),
    )
    .expect("failed to write provider registry");

    let backfill = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-backfill",
            "--file",
            manifest_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
        ],
    );

    assert!(backfill.status.success());
    let summary = fs::read_to_string(tmp.path().join("gray_rhino_backfill_runs.jsonl"))
        .expect("failed to read backfill summary");
    assert!(summary.contains("\"source_count\":2"));
    assert!(summary.contains("\"accepted\":1"));
    assert!(summary.contains("\"rejected\":1"));
    assert!(summary.contains("\"stale_sources\":1"));
    assert!(summary.contains("\"drift_sources\":1"));
    assert!(summary.contains("\"failure_taxonomy\":\"fetch_failure\""));

    let evidence_path = tmp.path().join("dependency_evidence.json");
    fs::write(
        &evidence_path,
        r#"{
  "subject": "Example issuer",
  "source": {
    "source_type": "SupplierDisclosure",
    "source_title": "Supplier dependency disclosure",
    "publisher": "Example supplier report",
    "source_url": "https://example.com/supplier",
    "repository_path": null,
    "observed_at": "2026-05-25",
    "retrieved_at": "2026-05-25"
  },
  "confidence": 0.86,
  "extraction_note": "Supplier disclosure identifies dependency concentration.",
  "structural_fact": "Critical supplier dependency has no disclosed fallback.",
  "metrics": {
    "dependency_kind": "Supplier",
    "dependency_name": "Example supplier",
    "concentration_ratio": 0.7,
    "single_point_of_failure": true,
    "fallback_disclosed": false
  }
}"#,
    )
    .expect("failed to write dependency evidence");
    assert!(run_cli(
        &tmp,
        &[
            "ingest-gray-rhino-dependency",
            "--file",
            evidence_path.to_str().unwrap(),
        ],
    )
    .status
    .success());
    let report = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);
    assert!(report.status.success());
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(stdout.contains("Backfill Ops View"));
    assert!(stdout.contains("failed_sources: 1"));
    assert!(stdout.contains("stale_sources: 1"));
    assert!(stdout.contains("drift_sources: 1"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
    assert!(!stdout.contains("gate signal"));
    assert!(!stdout.contains("execution signal"));
    assert!(!stdout.contains("trend_cohesion"));
}

#[test]
fn dependency_real_backfill_writes_run_summary() {
    let tmp = prepare_standard_workspace("en-us");
    let dependency_path = tmp.path().join("dependency_source.txt");
    let manifest_path = tmp.path().join("dependency_backfill_manifest.json");
    fs::write(
        &dependency_path,
        "dependency_kind: Supplier; dependency_name: Supplier A; concentration_ratio: 0.7; single_point_of_failure: true",
    )
    .expect("failed to write dependency source");
    fs::write(
        &manifest_path,
        format!(
            r#"[{{"category":"DependencyConcentration","symbol":"EXAMPLE","file":"{}"}}]"#,
            dependency_path.display()
        ),
    )
    .expect("failed to write dependency backfill manifest");

    let out = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-backfill",
            "--file",
            manifest_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Backfill run summary: gray_rhino_backfill_runs.jsonl"));
    let summary = fs::read_to_string(tmp.path().join("gray_rhino_backfill_runs.jsonl"))
        .expect("failed to read backfill run summary");
    assert!(summary.contains("\"source_count\":1"));
    assert!(summary.contains("\"accepted\":1"));
    assert!(summary.contains("\"rejected\":0"));
    assert!(summary.contains("\"mode\":\"dry_run\""));
    assert!(!tmp.path().join("gray_rhino_evidence.jsonl").exists());
}

#[test]
fn dependency_local_source_collection_produces_coverage_and_rejections() {
    let tmp = prepare_standard_workspace("zh-cn");
    let source_path = tmp.path().join("dependency_source.txt");
    fs::write(
        &source_path,
        "dependency_kind: Supplier; dependency_name: Example supplier; concentration_ratio: 0.70; single_point_of_failure: true; fallback_disclosed: false",
    )
    .expect("failed to write dependency source fixture");

    let out = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-dependency",
            "--symbol",
            "EXAMPLE",
            "--file",
            source_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
            "--dry-run",
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Gray Rhino Dependency Evidence Collection"));
    assert!(stdout.contains("Sources:  1"));
    assert!(stdout.contains("Accepted: 1"));
    assert!(stdout.contains("Saved:    0"));
    assert!(stdout.contains("Manifest: 1"));
    assert!(stdout.contains("Audit:    1"));
    assert!(stdout.contains("Formal evidence persisted: false"));
    assert!(stdout.contains("Field coverage:"));
    assert!(stdout.contains("concentration_ratio: 100.0% (1/1 extracted"));
    assert!(stdout.contains("Rejected: 0"));
    assert!(stdout.contains("Boundary: evidence only"));
    assert!(tmp
        .path()
        .join("gray_rhino_dependency_source_manifest.jsonl")
        .exists());
    assert!(tmp
        .path()
        .join("gray_rhino_dependency_extraction_audit.jsonl")
        .exists());
    assert!(!tmp.path().join("gray_rhino_evidence.jsonl").exists());
}

#[test]
fn dependency_source_collection_reports_rejection_taxonomy() {
    let tmp = prepare_standard_workspace("zh-cn");
    let source_path = tmp.path().join("dependency_metricless.txt");
    fs::write(
        &source_path,
        "This live dependency disclosure mentions suppliers but omits structured metrics.",
    )
    .expect("failed to write metricless dependency fixture");

    let out = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-dependency",
            "--symbol",
            "EXAMPLE",
            "--file",
            source_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
            "--dry-run",
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Rejected: 1"));
    assert!(stdout.contains("[REJECTED:MetriclessSource]"));
    assert!(stdout.contains("Formal evidence persisted: false"));
    assert!(stdout.contains("Boundary: evidence only"));
    assert!(!tmp.path().join("gray_rhino_evidence.jsonl").exists());
}

#[test]
fn institutional_and_redundancy_source_collection_reports_coverage() {
    let tmp = prepare_standard_workspace("zh-cn");
    let institutional_path = tmp.path().join("institutional_source.txt");
    let redundancy_path = tmp.path().join("redundancy_source.txt");
    fs::write(
        &institutional_path,
        "succession_structure_disclosed: true; external_audit_present: true; disclosure_quality_score: 0.72; oversight_evolution_disclosed: true; compliance_maturity_level: developing",
    )
    .expect("failed to write institutional source fixture");
    fs::write(
        &redundancy_path,
        "fallback_available: true; alternative_supplier_count: 2; redundancy_ratio: 0.5; recovery_path_disclosed: true; failover_tested: false",
    )
    .expect("failed to write redundancy source fixture");

    let institutional = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-institutional",
            "--symbol",
            "EXAMPLE",
            "--file",
            institutional_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
            "--dry-run",
        ],
    );
    let redundancy = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-redundancy",
            "--symbol",
            "EXAMPLE",
            "--file",
            redundancy_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
            "--dry-run",
        ],
    );

    assert!(institutional.status.success());
    assert!(redundancy.status.success());
    let institutional_stdout = String::from_utf8_lossy(&institutional.stdout);
    let redundancy_stdout = String::from_utf8_lossy(&redundancy.stdout);
    assert!(institutional_stdout.contains("Gray Rhino InstitutionalMaturity Evidence Collection"));
    assert!(institutional_stdout.contains("Coverage: 100.0%"));
    assert!(institutional_stdout.contains("Formal evidence persisted: false"));
    assert!(redundancy_stdout.contains("Gray Rhino Redundancy Evidence Collection"));
    assert!(redundancy_stdout.contains("Coverage: 100.0%"));
    assert!(redundancy_stdout.contains("Boundary: evidence only"));
    assert!(tmp
        .path()
        .join("gray_rhino_institutionalmaturity_source_manifest.jsonl")
        .exists());
    assert!(tmp
        .path()
        .join("gray_rhino_redundancy_extraction_audit.jsonl")
        .exists());
    assert!(!tmp.path().join("gray_rhino_evidence.jsonl").exists());
}

#[test]
fn institutional_redundancy_extractors_calibrate_synonyms_and_rejections() {
    let tmp = prepare_standard_workspace("en-us");
    let institutional_path = tmp.path().join("institutional_synonyms.txt");
    let redundancy_path = tmp.path().join("redundancy_synonyms.txt");
    let metricless_path = tmp.path().join("institutional_metricless.txt");
    fs::write(
        &institutional_path,
        "The annual report includes succession planning, an independent auditor, comprehensive disclosure, board oversight expanded, and developing compliance.",
    )
    .expect("failed to write institutional synonym source");
    fs::write(
        &redundancy_path,
        "The supplier disclosure claims a backup provider and two alternative suppliers. A recovery plan exists, but testing evidence is not disclosed.",
    )
    .expect("failed to write redundancy synonym source");
    fs::write(
        &metricless_path,
        "The organization describes maturity in broad narrative terms without structured evidence.",
    )
    .expect("failed to write metricless institutional source");

    let institutional = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-institutional",
            "--symbol",
            "EXAMPLE",
            "--file",
            institutional_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
            "--dry-run",
        ],
    );
    let redundancy = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-redundancy",
            "--symbol",
            "EXAMPLE",
            "--file",
            redundancy_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
            "--dry-run",
        ],
    );
    let metricless = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-institutional",
            "--symbol",
            "EXAMPLE",
            "--file",
            metricless_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
            "--dry-run",
        ],
    );

    assert!(institutional.status.success());
    assert!(redundancy.status.success());
    assert!(metricless.status.success());
    let institutional_stdout = String::from_utf8_lossy(&institutional.stdout);
    let redundancy_stdout = String::from_utf8_lossy(&redundancy.stdout);
    let metricless_stdout = String::from_utf8_lossy(&metricless.stdout);
    assert!(institutional_stdout.contains("succession_structure_disclosed: 100.0%"));
    assert!(institutional_stdout.contains("external_audit_present: 100.0%"));
    assert!(redundancy_stdout.contains("fallback_available: 100.0%"));
    assert!(redundancy_stdout.contains("failover_tested: 0.0%"));
    assert!(metricless_stdout.contains("[REJECTED:MetriclessSource]"));
    assert!(metricless_stdout.contains("Formal evidence persisted: false"));
}

#[test]
fn gray_rhino_governance_source_collection_caches_and_extracts_metrics() {
    let tmp = prepare_standard_workspace("zh-cn");
    let source_path = tmp.path().join("proxy_source.txt");
    fs::write(
        &source_path,
        "founder_voting_power: 61.2%; independent_board_ratio: 0.42; dual_class_structure: true; super_voting_rights: yes; succession_disclosure: false",
    )
    .expect("failed to write governance source fixture");

    let out = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-governance",
            "--symbol",
            "EXAMPLE",
            "--file",
            source_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Gray Rhino Governance Evidence Collection"));
    assert!(stdout.contains("Sources:  1"));
    assert!(stdout.contains("Accepted: 1"));
    assert!(stdout.contains("Manifest: 1"));
    assert!(stdout.contains("Audit:    1"));
    assert!(stdout.contains("Dry run:  false"));
    assert!(stdout.contains("Formal evidence persisted: true"));
    assert!(stdout.contains("Coverage: 100.0%"));
    assert!(stdout.contains("Field coverage:"));
    assert!(stdout.contains("founder_voting_power: 100.0% (1/1 extracted"));
    assert!(stdout.contains("Rejected: 0"));
    assert!(stdout.contains("Boundary: evidence only"));
    let store = fs::read_to_string(tmp.path().join("gray_rhino_evidence.jsonl"))
        .expect("failed to read gray rhino evidence store");
    assert!(store.contains("\"category\":\"GovernanceConcentration\""));
    assert!(tmp
        .path()
        .join("gray_rhino_sources/governance/EXAMPLE/proxy_source.txt")
        .exists());
    let manifest = fs::read_to_string(
        tmp.path()
            .join("gray_rhino_governance_source_manifest.jsonl"),
    )
    .expect("failed to read governance source manifest");
    assert!(manifest.contains("\"content_sha256\""));
    let audit = fs::read_to_string(
        tmp.path()
            .join("gray_rhino_governance_extraction_audit.jsonl"),
    )
    .expect("failed to read governance extraction audit");
    assert!(audit.contains("\"metric\":\"founder_voting_power\""));
    assert!(!tmp.path().join("gray_rhino_snapshots.jsonl").exists());
}

#[test]
fn gray_rhino_governance_source_collection_rejects_metricless_source() {
    let tmp = prepare_standard_workspace("zh-cn");
    let source_path = tmp.path().join("proxy_source.txt");
    fs::write(&source_path, "generic governance prose only")
        .expect("failed to write governance source fixture");

    let out = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-governance",
            "--symbol",
            "EXAMPLE",
            "--file",
            source_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
        ],
    );

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Sources:  1"));
    assert!(stdout.contains("Accepted: 0"));
    assert!(stdout.contains("Rejected: 1"));
    assert!(stdout.contains("Dry run:  false"));
    assert!(stdout.contains("Formal evidence persisted: true"));
    assert!(stdout.contains("Coverage: 0.0%"));
    assert!(stdout.contains("Field coverage:"));
    assert!(stdout.contains("succession_disclosure: 0.0% (0/1 extracted"));
    assert!(stdout.contains("MissingGovernanceMetric"));
    assert!(!tmp.path().join("gray_rhino_evidence.jsonl").exists());
    assert!(tmp
        .path()
        .join("gray_rhino_governance_extraction_audit.jsonl")
        .exists());
    assert!(!tmp.path().join("gray_rhino_snapshots.jsonl").exists());
}

#[test]
fn gray_rhino_daily_report_shows_governance_sensor_health_only() {
    let tmp = prepare_standard_workspace("zh-cn");
    let source_path = tmp.path().join("proxy_source.txt");
    fs::write(
        &source_path,
        "founder_voting_power: 61.2%; independent_board_ratio: 0.42",
    )
    .expect("failed to write governance source fixture");

    let collect = run_cli(
        &tmp,
        &[
            "collect-gray-rhino-governance",
            "--symbol",
            "EXAMPLE",
            "--file",
            source_path.to_str().unwrap(),
            "--date",
            "2026-05-25",
        ],
    );
    assert!(collect.status.success());

    let report = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(report.status.success());
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(stdout.contains("治理传感器健康度"));
    assert!(stdout.contains("覆盖率"));
    assert!(stdout.contains("边界声明: 治理传感器健康度仅用于证据覆盖检查"));
    assert!(!stdout.contains("BUY"));
    assert!(!stdout.contains("SELL"));
}

#[test]
fn governance_sec_replay_pack_produces_coverage_and_rejections() {
    let tmp = prepare_standard_workspace("zh-cn");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures = [
        "example_def14a_full.txt",
        "example_10k_board.txt",
        "example_s1_dual_class.txt",
        "example_20f_succession.txt",
        "example_metricless.txt",
    ];

    for fixture in fixtures {
        let path = root.join("tests/fixtures/governance_sec").join(fixture);
        let out = run_cli(
            &tmp,
            &[
                "collect-gray-rhino-governance",
                "--symbol",
                fixture.trim_end_matches(".txt"),
                "--file",
                path.to_str().unwrap(),
                "--date",
                "2026-05-25",
            ],
        );
        assert!(out.status.success());
    }

    let audit = fs::read_to_string(
        tmp.path()
            .join("gray_rhino_governance_extraction_audit.jsonl"),
    )
    .expect("failed to read governance extraction audit");
    assert_eq!(audit.lines().count(), 5);
    assert!(audit.contains("\"accepted\":true"));
    assert!(audit.contains("\"accepted\":false"));
    assert!(audit.contains("MissingGovernanceMetric"));

    let manifest = fs::read_to_string(
        tmp.path()
            .join("gray_rhino_governance_source_manifest.jsonl"),
    )
    .expect("failed to read governance source manifest");
    assert_eq!(manifest.lines().count(), 5);
    assert!(manifest.contains("\"content_sha256\""));
}

fn run_cli(tmp: &TempDir, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_stock-sentinel"))
        .current_dir(tmp.path())
        .args(args)
        .output()
        .expect("failed to execute stock-sentinel")
}

fn set_output_language(tmp: &TempDir, language: &str) {
    let config_path = tmp.path().join("config.toml");
    let raw = fs::read_to_string(&config_path).expect("failed to read temp config.toml");
    let raw = raw.replace(
        "language = \"zh-cn\"",
        &format!("language = \"{language}\""),
    );
    fs::write(config_path, raw).expect("failed to update temp config.toml");
}

#[test]
fn cli_help_is_explicit_and_does_not_run_radar() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &["--help"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Usage: stock-sentinel <command> [options]"));
    assert!(stdout.contains("No command is executed by default"));
    assert!(!stdout.contains("Telegram notification skipped"));
}

#[test]
fn cli_unknown_command_is_rejected_without_radar_fallback() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &["gray-rhnio"]);

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Unknown command or option: gray-rhnio"));
    assert!(stderr.contains("Usage: stock-sentinel <command> [options]"));
    assert!(!stderr.contains("Telegram notification skipped"));
}

#[test]
fn cli_invalid_provider_is_rejected_without_provider_fallback() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &["radar", "--provider", "typo"]);

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Invalid provider: typo"));
    assert!(stderr.contains("Usage: stock-sentinel <command> [options]"));
}

#[test]
fn cli_missing_provider_value_is_rejected() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &["radar", "--provider"]);

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Missing value for --provider"));
    assert!(stderr.contains("Usage: stock-sentinel <command> [options]"));
}

#[test]
fn cli_without_command_shows_help_without_radar_fallback() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &[]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Usage: stock-sentinel <command> [options]"));
    assert!(!stdout.contains("Telegram notification skipped"));
}

#[test]
fn research_attention_outputs_daily_sidecar_report() {
    let tmp = prepare_workspace(
        r#"

[research_attention.TSLA]
cognitive_yield = "HIGH"
attention_cost = "HIGH"
information_density = "EXPANDING"
reason = "Physical AI / FSD / 製造自動化は高変化率を維持。"

[research_attention.GOOG]
cognitive_yield = "MEDIUM"
attention_cost = "LOW"
information_density = "STABLE"
reason = "AI 収益化の理解が進み、辺際的な情報増分は低下。"
"#,
    );

    let out = run_cli(&tmp, &["research-attention"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("🧠 研究注意力"));
    assert!(stdout.contains("HIGH:"));
    assert!(stdout.contains("TSLA · 信息密度 EXPANDING · 注意力成本 HIGH"));
    assert!(stdout.contains("MEDIUM:"));
    assert!(stdout.contains("GOOG · 信息密度 STABLE · 注意力成本 LOW"));
    assert!(stdout.contains("用户自定义观察说明未提供中文版本。"));
    assert!(!stdout.contains("製造自動化"));
    assert!(stdout.contains("认知收益低 ≠ 股票不好"));
}

#[test]
fn research_attention_empty_config_is_non_blocking() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &["research-attention"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("未配置认知观察对象"));
}

#[test]
fn standard_research_catalog_has_complete_zh_en_ja_content() {
    for (language, expected_reason, expected_thesis, expected_focus, expected_invalidation) in [
        (
            "zh-cn",
            "AI 基础设施需求、供应约束、毛利率与数据中心投资回报的变化率较高。",
            "观察 AI 商业化能否沉淀到搜索、云服务和广告的利润结构中",
            "Azure 成长率与 AI 贡献",
            "支撑高估值的证据不足",
        ),
        (
            "en-us",
            "AI infrastructure demand, supply constraints, gross margin",
            "Observe whether AI commercialization becomes embedded in search",
            "Azure growth and AI contribution",
            "Evidence supporting high valuation becomes insufficient",
        ),
        (
            "ja-jp",
            "AI インフラ需要、供給制約、粗利率",
            "AI 商業化が検索、クラウド、広告の利益構造へ定着",
            "Azure 成長率と AI 寄与",
            "高バリュエーションを支える証拠が不足",
        ),
    ] {
        let tmp = prepare_standard_workspace(language);
        let attention = run_cli(&tmp, &["research-attention"]);
        let thesis = run_cli(&tmp, &["asset-thesis"]);

        assert!(attention.status.success());
        assert!(thesis.status.success());
        let content = format!(
            "{}\n{}",
            String::from_utf8_lossy(&attention.stdout),
            String::from_utf8_lossy(&thesis.stdout)
        );
        assert!(content.contains(expected_reason));
        assert!(content.contains(expected_thesis));
        assert!(content.contains(expected_focus));
        assert!(content.contains(expected_invalidation));
        assert!(!content.contains("User-defined observation text is not provided"));
        assert!(!content.contains("用户自定义观察说明未提供"));
    }
}

#[test]
fn standard_english_catalog_does_not_leak_japanese_body_text() {
    let tmp = prepare_standard_workspace("en-us");
    let attention = run_cli(&tmp, &["research-attention"]);
    let thesis = run_cli(&tmp, &["asset-thesis"]);
    let content = format!(
        "{}\n{}",
        String::from_utf8_lossy(&attention.stdout),
        String::from_utf8_lossy(&thesis.stdout)
    );

    assert!(!content.contains("観測"));
    assert!(!content.contains("収益"));
    assert!(!content.contains("導入"));
    assert!(!content.contains("失効"));
}

#[test]
fn custom_english_research_text_requires_explicit_translation() {
    let tmp = prepare_workspace(
        r#"

[research_attention.CUSTOM]
cognitive_yield = "HIGH"
attention_cost = "LOW"
information_density = "ACTIVE"
reason = "独自の観測理由。"

[asset_thesis.CUSTOM]
thesis = "独自の観測命題。"
observation_focus = ["独自焦点"]
invalidation = ["独自失効"]
"#,
    );
    set_output_language(&tmp, "en-us");

    let attention = run_cli(&tmp, &["research-attention"]);
    let thesis = run_cli(&tmp, &["asset-thesis"]);
    let content = format!(
        "{}\n{}",
        String::from_utf8_lossy(&attention.stdout),
        String::from_utf8_lossy(&thesis.stdout)
    );

    assert!(content.contains("User-defined observation text is not provided in English."));
    assert!(!content.contains("独自の観測理由"));
    assert!(!content.contains("独自焦点"));
}

#[test]
fn research_attention_notify_requires_explicit_telegram_availability() {
    let tmp = prepare_workspace(
        r#"

[research_attention.PLTR]
cognitive_yield = "HIGH"
attention_cost = "MODERATE"
information_density = "EXPANDING"
reason = "Ontology と企業 AI 組織化の変化率が高い。"
"#,
    );

    let out = run_cli(&tmp, &["research-attention", "--notify"]);

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Telegram notification is not available for research-attention"));
}

#[test]
fn asset_thesis_outputs_observation_contract() {
    let tmp = prepare_workspace(
        r#"

[asset_thesis.NVDA]
thesis = "AI インフラ需要が継続し、データセンター投資が収益へ転換するかを観測する。"
observation_focus = [
  "データセンター注文の継続性",
  "粗利率と供給制約の変化"
]
invalidation = [
  "主要クラウドの Capex 減速",
  "注文可視性の低下"
]

[asset_thesis.GOOG]
thesis = "AI 商業化が検索・クラウド・広告の利益構造へ定着するかを観測する。"
observation_focus = ["AI 収益化", "検索防衛力"]
invalidation = ["AI 投資が利益率を継続的に圧迫"]
"#,
    );

    let out = run_cli(&tmp, &["asset-thesis"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("🧭 资产观察命题"));
    assert!(stdout.contains("NVDA · 用户自定义观察说明未提供中文版本。"));
    assert!(stdout.contains("观察焦点:"));
    assert!(stdout.contains("用户自定义观察说明未提供中文版本。"));
    assert!(stdout.contains("失效条件:"));
    assert!(!stdout.contains("データセンター注文の継続性"));
    assert!(!stdout.contains("主要クラウドの Capex 減速"));
    assert!(stdout.contains("观察命题 ≠ 买入理由"));
}

#[test]
fn asset_thesis_empty_config_is_non_blocking() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &["asset-thesis"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("未配置资产观察命题"));
}

#[test]
fn asset_thesis_notify_requires_explicit_telegram_availability() {
    let tmp = prepare_workspace(
        r#"

[asset_thesis.PLTR]
thesis = "企業 AI 組織化が Ontology を通じて定着するかを観測する。"
observation_focus = ["商用導入の継続性"]
invalidation = ["導入拡大が止まる"]
"#,
    );

    let out = run_cli(&tmp, &["asset-thesis", "--notify"]);

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Telegram notification is not available for asset-thesis"));
}

#[test]
fn daily_calibration_combines_audit_attention_and_thesis_without_new_signal() {
    let tmp = prepare_workspace(
        r#"

[research_attention.TSLA]
cognitive_yield = "HIGH"
attention_cost = "HIGH"
information_density = "EXPANDING"
reason = "Physical AI / FSD / 製造自動化は高変化率を維持。"

[asset_thesis.NVDA]
thesis = "AI インフラ需要が継続するかを観測する。"
observation_focus = ["データセンター注文の継続性"]
invalidation = ["主要クラウドの Capex 減速"]

[macro_gravity]
rate_pressure = "RISING"
real_yield_pressure = "TIGHT"
yield_curve = "FLAT"
credit_stress = "NORMAL"
liquidity = "NEUTRAL"
growth_valuation_impact = "COMPRESSING"
note = "長期金利は成長株のバリュエーション重力として観測する。"

[gray_rhino_escalation]
enable = true
risk_expansion_rate = "ELEVATED"
constraint_growth_rate = "MODERATE"
dependency_centralization = "HIGH"
awareness_decay = "ELEVATED"
narrative_overconfidence = "MODERATE"
single_point_fragility = "MODERATE"
fallback_survivability_risk = "MODERATE"
"#,
    );

    let out = run_cli(&tmp, &["daily-calibration"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("🧭 每日认知校准"));
    assert!(stdout.contains("## 1. 今日审计摘要"));
    assert!(stdout.contains("未找到可用的 state_transitions.jsonl 记录。"));
    assert!(stdout.contains("## 2. 日报校准问题"));
    assert!(stdout.contains("今天是市场理解变化，还是只是噪音变化？"));
    assert!(stdout.contains("- 战术状态: NO AUDIT"));
    assert!(stdout.contains("- 需校准认知对象数: 1"));
    assert!(stdout.contains("- 需复查观察命题数: 1"));
    assert!(stdout.contains("只用于复盘，不构成新信号"));
    assert!(stdout.contains("## 3. 认知关注校准"));
    assert!(stdout.contains("TSLA · 信息密度 EXPANDING · 注意力成本 HIGH"));
    assert!(stdout.contains("## 4. 资产观察命题"));
    assert!(stdout.contains("NVDA · 用户自定义观察说明未提供中文版本。"));
    assert!(!stdout.contains("AI インフラ需要"));
    assert!(!stdout.contains("データセンター注文の継続性"));
    assert!(stdout.contains("## 5. 宏观重力校准"));
    assert!(stdout.contains("- 利率压力: RISING"));
    assert!(stdout.contains("- 成长股估值: COMPRESSING"));
    assert!(stdout.contains("不参与 Gate，不生成交易指令"));
    assert!(stdout.contains("## 6. 灰犀牛升级监控"));
    assert!(stdout.contains("输入来源: 人工结构基线（配置输入）"));
    assert!(stdout.contains("审计链: 人工结构基线 -> 七项观测 -> 日次快照"));
    assert!(stdout.contains("状态:"));
    assert!(stdout.contains("风险扩张速度: 偏高"));
    assert!(stdout.contains("相比前次日次评估: 首次记录（无前次快照）"));
    assert!(stdout.contains("不代表自动事实发现"));
    assert!(stdout.contains("边界声明: 灰犀牛升级监控仅观察结构性风险升级，不生成交易信号。"));
    assert!(!stdout.contains("State:"));
    assert!(stdout.contains("不生成新的交易指令"));
}

#[test]
fn fixture_keeps_required_config_after_manually_placed_gray_rhino_section() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &["--help"]);

    assert!(out.status.success());
    let config = fs::read_to_string(tmp.path().join("config.toml")).unwrap();
    assert!(config.contains("[[watchlist]]"));
    assert!(!config.contains("[gray_rhino_escalation]"));
}

#[test]
fn macro_gravity_outputs_context_without_trade_signal() {
    let tmp = prepare_workspace(
        r#"

[macro_gravity]
rate_pressure = "RISING"
real_yield_pressure = "TIGHT"
yield_curve = "INVERTED"
credit_stress = "WATCH"
liquidity = "TIGHT"
growth_valuation_impact = "COMPRESSING"
note = "割引率上昇により、好業績でも時間コストが上がる可能性を観測する。"
"#,
    );

    let out = run_cli(&tmp, &["daily-calibration"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("🌐 宏观重力"));
    assert!(stdout.contains("- 实际利率: TIGHT"));
    assert!(stdout.contains("- 信用压力: WATCH"));
    assert!(!stdout.contains("割引率上昇"));
    assert!(stdout.contains("不参与 Gate"));
    assert!(!stdout.contains("买入"));
}

#[test]
fn daily_calibration_empty_config_is_non_blocking() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &["daily-calibration"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("未配置认知观察对象"));
    assert!(stdout.contains("未配置资产观察命题"));
}

#[test]
fn daily_calibration_notify_requires_explicit_telegram_availability() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &["daily-calibration", "--notify"]);

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Telegram notification is not available for daily-calibration"));
}

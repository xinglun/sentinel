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
    let mut raw = strip_finnhub_section(&strip_optional_calibration_sections(&raw));

    let save_to = tmp.path().to_string_lossy().to_string();
    raw = raw.replace(
        "save_to = \"./reports\"",
        &format!("save_to = \"{}\"", save_to),
    );
    raw.push_str(extra_config);
    if !extra_config.contains("[capital_absorption]") {
        raw.push_str(
            r#"

[capital_absorption]
auto_enable = false
status = "NORMAL"

[capital_absorption.capital_demand]
trend = "STABLE"

[capital_absorption.capital_supply]
trend = "STABLE"

[capital_absorption.absorption_ratio]
state = "LOW"
"#,
        );
    }

    fs::write(tmp.path().join("config.toml"), raw).expect("failed to write temp config.toml");
    tmp
}

fn prepare_workspace_without_capital_absorption_default(extra_config: &str) -> TempDir {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = root.join("config.toml");
    let raw = fs::read_to_string(&config_path).expect("failed to read base config.toml");
    let mut raw = strip_finnhub_section(&strip_optional_calibration_sections(&raw));

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
                || line == "[capital_absorption]"
                || line.starts_with("[capital_absorption.")
                || line.starts_with("[[capital_absorption.")
                || line == "[capital_dynamics]"
                || line == "[capital_dynamics.flow_layer]"
                || line == "[capital_dynamics.flow_layer.breadth]"
                || line.starts_with("[[capital_dynamics.flow_layer.observations]]")
                || line.starts_with("[[capital_dynamics.flow_layer.divergences]]")
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

fn strip_finnhub_section(raw: &str) -> String {
    let mut output = String::new();
    let mut skipping = false;
    for line in raw.lines() {
        if line.starts_with('[') {
            skipping = line == "[finnhub]";
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
    assert!(stdout.contains("已摄取 GovernanceConcentration 证据。"));
    assert!(stdout.contains("边界声明: 仅限证据处理，不更新升级、闸门、执行或交易状态。"));
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
    assert!(stdout.contains("已摄取 DependencyConcentration 证据。"));
    assert!(stdout.contains("边界声明: 仅限证据处理，不更新升级、闸门、执行或交易状态。"));
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
    assert!(stdout.contains("已摄取 InstitutionalMaturity 证据。"));
    assert!(stdout.contains("边界声明: 仅限证据处理，不更新升级、闸门、执行或交易状态。"));
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
    assert!(stdout.contains("已摄取 Redundancy 证据。"));
    assert!(stdout.contains("边界声明: 仅限证据处理，不更新升级、闸门、执行或交易状态。"));
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
    assert!(stdout.contains("Gray Rhino Temperature Change"));
    assert!(stdout.contains("Temperature: High"));
    assert!(stdout.contains("Velocity: Rising"));
    assert!(stdout.contains("Evidence acceleration: Rising"));
    assert!(stdout.contains("Survivability Assessment"));
    assert!(stdout.contains("Capital access: Unknown"));
    assert!(stdout.contains("Dependency risk: Medium"));
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
    assert!(stdout.contains("Quiet"));
    assert!(!stdout.contains("Gray Rhino Inline Reference (semantic isolation)"));
    assert!(!stdout.contains("Market active candidates: 0"));
    assert!(!stdout.contains("Company active candidates: none"));
    assert!(!stdout.contains("SPACEX / Company / Governance Concentration / Expanding"));
    assert!(!stdout.contains("IPO voting terms"));
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
    assert!(stdout.contains("Gray Rhino Summary (semantic isolation)"));
    assert!(stdout.contains("Quiet"));
    assert!(!stdout.contains("Gray Rhino Inline Reference (semantic isolation)"));
    assert!(!stdout.contains("TSLA / Company / Governance Concentration / Expanding"));
    assert!(!stdout.contains("STALE / Company / Governance Concentration"));
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
fn capital_absorption_ipo_queue_persistence_make_target_is_operational_entrypoint() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let makefile = fs::read_to_string(root.join("Makefile")).expect("failed to read Makefile");

    assert!(makefile.contains("test-capital-absorption-ipo-queue-persistence:"));
    assert!(makefile.contains("cargo test capital_absorption_ipo_queue --all-targets"));
    assert!(makefile.contains("test-capital-absorption-ipo-queue-persistence"));
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
        r#"{"subject":"Example issuer","category":"DependencyConcentration","source":{"source_type":"SupplierDisclosure","source_title":"Legacy dependency disclosure","publisher":"Example issuer","source_url":"https://example.com/legacy","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.86,"extraction_note":"Legacy record without risk effect.","structural_fact":"Dependency concentration is disclosed."}
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
    assert!(stdout.contains("machine_provider_status: skipped"));
    assert!(stdout.contains("machine_accepted: 1"));
    assert!(stdout.contains("machine_rejected: 0"));
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
                "fallback",
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
                "fallback",
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
fn gray_rhino_report_blocks_fallback_mixed_language() {
    let tmp = prepare_standard_workspace("zh-cn");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"DependencyConcentration","subject":"GOOG","state":"Visible","evidence":["Single dependency or missing fallback detected."],"watch_triggers":["fallback disclosure change"],"source_title":"Dependency disclosure","observed_at":"2026-05-25","source_published_at":"2026-05-25","last_confirmed_at":"2026-05-25","resolved_at":null}
"#,
    )
    .expect("failed to write candidates");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("后备路径"));
    assert!(!stdout.contains("fallback"));
}

#[test]
fn gray_rhino_compact_summary_excludes_cooling_and_resolved_from_active() {
    let tmp = prepare_standard_workspace("zh-cn");
    fs::write(
        tmp.path().join("gray_rhino_candidates.jsonl"),
        r#"{"scope":"Company","kind":"GovernanceConcentration","subject":"GOOG","state":"Cooling","evidence":["Governance risk is cooling."],"watch_triggers":["proxy update"],"source_title":"GOOG proxy","observed_at":"2026-05-25","source_published_at":"2026-05-25","last_confirmed_at":"2026-05-25","resolved_at":null}
{"scope":"Company","kind":"DependencyConcentration","subject":"TSLA","state":"Resolved","evidence":["Dependency risk was resolved."],"watch_triggers":["supplier update"],"source_title":"TSLA supplier update","observed_at":"2026-05-25","source_published_at":"2026-05-25","last_confirmed_at":"2026-05-25","resolved_at":"2026-05-25"}
"#,
    )
    .expect("failed to write candidates");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("公司活跃候选: 无"));
    assert!(stdout.contains("公司降温候选: GOOG"));
    assert!(stdout.contains("公司已解除候选: TSLA"));
    assert!(!stdout.contains("公司活跃候选: GOOG"));
    assert!(!stdout.contains("公司活跃候选: TSLA"));
}

#[test]
fn gray_rhino_sensor_health_excludes_subjectless_evidence_from_readiness() {
    let tmp = prepare_standard_workspace("zh-cn");
    fs::write(
        tmp.path().join("gray_rhino_evidence.jsonl"),
        r#"{"subject":"","category":"DependencyConcentration","source":{"source_type":"SupplierDisclosure","source_title":"Legacy dependency disclosure","publisher":"Legacy issuer","source_url":"https://example.com/dependency","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.95,"risk_effect":"Amplifying","extraction_note":"Legacy subjectless record.","structural_fact":"Critical supplier dependency has no disclosed fallback."}
"#,
    )
    .expect("failed to write evidence store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("风险升级评估: 尚无正式证据"));
    assert!(stdout.contains("不可评分证据记录: 1"));
    assert!(stdout.contains("缺少主体或风险作用不可用于正式评分"));
    assert!(stdout.contains("依赖集中: 0 条证据记录, 准备度=不足"));
    assert!(!stdout.contains("依赖集中: 1 条证据记录, 准备度=就绪"));
}

#[test]
fn gray_rhino_persisted_invalid_confidence_is_rejected_before_formal_assessment() {
    assert_invalid_persisted_gray_rhino_evidence_is_rejected(
        r#"{"subject":"GOOG","category":"DependencyConcentration","source":{"source_type":"SupplierDisclosure","source_title":"Invalid confidence dependency","publisher":"GOOG","source_url":"https://example.com/dependency","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":1.50,"risk_effect":"Amplifying","extraction_note":"Supplier disclosure identifies dependency concentration.","structural_fact":"Critical supplier dependency has no disclosed fallback."}
"#,
        "置信度超出范围",
    );
}

#[test]
fn gray_rhino_persisted_narrative_only_is_rejected_before_formal_assessment() {
    assert_invalid_persisted_gray_rhino_evidence_is_rejected(
        r#"{"subject":"GOOG","category":"DependencyConcentration","source":{"source_type":"SupplierDisclosure","source_title":"Narrative dependency","publisher":"GOOG","source_url":"https://example.com/dependency","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.95,"risk_effect":"Amplifying","extraction_note":"too successful to fail narrative","structural_fact":"too successful to fail"}
"#,
        "仅为叙事性表述",
    );
}

#[test]
fn gray_rhino_persisted_forbidden_boundary_term_is_rejected_before_formal_assessment() {
    assert_invalid_persisted_gray_rhino_evidence_is_rejected(
        r#"{"subject":"GOOG","category":"DependencyConcentration","source":{"source_type":"SupplierDisclosure","source_title":"Forbidden boundary dependency","publisher":"GOOG","source_url":"https://example.com/dependency","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.95,"risk_effect":"Amplifying","extraction_note":"Supplier disclosure identifies dependency concentration.","structural_fact":"Dependency risk is connected to sell decision."}
"#,
        "包含禁止边界词",
    );
}

#[test]
fn gray_rhino_persisted_unsupported_source_type_is_rejected_before_formal_assessment() {
    assert_invalid_persisted_gray_rhino_evidence_is_rejected(
        r#"{"subject":"GOOG","category":"DependencyConcentration","source":{"source_type":"MarketNarrativeCorpus","source_title":"Unsupported dependency source","publisher":"GOOG","source_url":"https://example.com/dependency","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.95,"risk_effect":"Amplifying","extraction_note":"Supplier disclosure identifies dependency concentration.","structural_fact":"Critical supplier dependency has no disclosed fallback."}
"#,
        "来源类型不支持",
    );
}

#[test]
fn gray_rhino_rejected_evidence_reasons_do_not_leak_enum_names() {
    let tmp = prepare_standard_workspace("zh-cn");
    fs::write(
        tmp.path().join("gray_rhino_evidence.jsonl"),
        r#"{"subject":"GOOG","category":"DependencyConcentration","source":{"source_type":"SupplierDisclosure","source_title":"Missing publisher dependency","publisher":"","source_url":"https://example.com/dependency","repository_path":null,"observed_at":"2026-05-25","retrieved_at":"2026-05-25"},"confidence":0.95,"risk_effect":"Amplifying","extraction_note":"Supplier disclosure identifies dependency concentration.","structural_fact":"Critical supplier dependency has no disclosed fallback."}
"#,
    )
    .expect("failed to write evidence store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("缺少发布方"));
    assert!(!stdout.contains("MissingPublisher"));
    assert!(!stdout.contains("UnsupportedSourceType"));
    assert!(!stdout.contains("ConfidenceOutOfRange"));
}

fn assert_invalid_persisted_gray_rhino_evidence_is_rejected(jsonl: &str, reason: &str) {
    let tmp = prepare_standard_workspace("zh-cn");
    fs::write(tmp.path().join("gray_rhino_evidence.jsonl"), jsonl)
        .expect("failed to write evidence store");

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("风险升级评估: 尚无正式证据"));
    assert!(stdout.contains("不可评分证据记录: 1"));
    assert!(stdout.contains(reason));
    assert!(!stdout.contains("状态: 风险扩张"));
    assert!(!stdout.contains("scoreable average confidence: 1.50"));
    assert!(!stdout.contains("依赖集中: 1 条证据记录, 准备度=就绪"));
}

#[test]
fn gray_rhino_evidence_store_reads_accepted_and_rejected_once() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let app = fs::read_to_string(
        root.join("src/features/research/application/gray_rhino_daily_report.rs"),
    )
    .expect("failed to read daily report app");
    let repository = fs::read_to_string(
        root.join("src/features/research/infrastructure/gray_rhino_daily_report_repository.rs"),
    )
    .expect("failed to read daily report repository");
    let store = fs::read_to_string(
        root.join("src/features/research/infrastructure/gray_rhino_evidence_store.rs"),
    )
    .expect("failed to read evidence store");

    assert!(app.contains("fn load_evidence_read_model"));
    assert!(!app.contains("fn load_evidence_records(&self"));
    assert!(!app.contains("fn load_rejected_evidence_records("));
    assert!(repository.contains("load_evidence_read_batch()?"));
    assert!(store.contains("pub(crate) fn load_evidence_read_batch"));
    assert!(store.contains("GrayRhinoEvidenceReadBatch"));
}

#[test]
fn gray_rhino_domain_policy_owns_discovery_and_assessment_rules() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let discovery_app =
        fs::read_to_string(root.join("src/features/research/application/gray_rhino_discovery.rs"))
            .expect("failed to read discovery app");
    let discovery_policy = fs::read_to_string(
        root.join("src/features/research/domain/gray_rhino_discovery_policy.rs"),
    )
    .expect("failed to read discovery policy");
    let assessment_app =
        fs::read_to_string(root.join("src/features/research/application/gray_rhino_assessment.rs"))
            .expect("failed to read assessment app");
    let assessment_policy = fs::read_to_string(
        root.join("src/features/research/domain/gray_rhino_assessment_policy.rs"),
    )
    .expect("failed to read assessment policy");

    assert!(!discovery_app.contains("dual class"));
    assert!(discovery_policy.contains("dual class"));
    assert!(!assessment_app.contains("latest_effective_subject_category_records"));
    assert!(assessment_policy.contains("latest_effective_subject_category_records"));
}

#[test]
fn cli_does_not_own_dependency_source_infrastructure() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cli = fs::read_to_string(root.join("src/cli.rs")).expect("failed to read cli");
    let adapter = fs::read_to_string(
        root.join("src/features/research/infrastructure/dependency_source_adapter.rs"),
    )
    .expect("failed to read dependency source adapter");
    let checker = fs::read_to_string(root.join("scripts/check_architecture_boundaries.py"))
        .expect("failed to read architecture checker");

    assert!(!cli.contains("reqwest::"));
    assert!(!cli.contains("impl DependencySourceAdapter"));
    assert!(adapter.contains("reqwest::"));
    assert!(checker.contains("cli_infrastructure_escape_violations"));
}

#[test]
fn gray_rhino_monitoring_policy_is_domain_owned() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let application = fs::read_to_string(
        root.join("src/features/research/application/gray_rhino_monitoring_state.rs"),
    )
    .expect("failed to read monitoring application");
    let domain = fs::read_to_string(
        root.join("src/features/research/domain/gray_rhino_monitoring_policy.rs"),
    )
    .expect("failed to read monitoring domain policy");
    let checker = fs::read_to_string(root.join("scripts/check_gray_rhino_evidence_contract.py"))
        .expect("failed to read contract checker");

    assert!(!application.contains("fn classify_state"));
    assert!(!application.contains("fn stale_state_for_kind"));
    assert!(domain.contains("fn classify_state"));
    assert!(domain.contains("fn stale_state_for_kind"));
    assert!(checker.contains("monitoring application must not contain lifecycle policy"));
}

#[test]
fn cli_does_not_own_gray_rhino_backfill_infrastructure() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cli = fs::read_to_string(root.join("src/cli.rs")).expect("failed to read cli");
    let runner = fs::read_to_string(
        root.join("src/features/research/infrastructure/gray_rhino_backfill_runner.rs"),
    )
    .expect("failed to read gray rhino backfill runner");
    let checker = fs::read_to_string(root.join("scripts/check_architecture_boundaries.py"))
        .expect("failed to read architecture checker");

    assert!(!cli.contains("append_cli_jsonl"));
    assert!(!cli.contains("metric_aliases("));
    assert!(!cli.contains("std::fs::OpenOptions"));
    assert!(runner.contains("metric_aliases("));
    assert!(runner.contains("std::fs::OpenOptions"));
    assert!(checker.contains("std::fs::OpenOptions"));
    assert!(checker.contains("metric_aliases("));
}

#[test]
fn gray_rhino_evidence_projection_policy_is_domain_owned() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let application = fs::read_to_string(
        root.join("src/features/research/application/gray_rhino_daily_report.rs"),
    )
    .expect("failed to read daily report app");
    let domain = fs::read_to_string(
        root.join("src/features/research/domain/gray_rhino_evidence_projection_policy.rs"),
    )
    .expect("failed to read projection policy");
    let checker = fs::read_to_string(root.join("scripts/check_gray_rhino_evidence_contract.py"))
        .expect("failed to read contract checker");

    assert!(!application.contains("fn evidence_resolved_candidates"));
    assert!(!application.contains("fn latest_effective_evidence"));
    assert!(domain.contains("fn evidence_resolved_candidates"));
    assert!(domain.contains("fn latest_effective_evidence"));
    assert!(
        checker.contains("daily report application must not contain evidence projection policy")
    );
}

#[test]
fn gray_rhino_interface_does_not_own_evidence_scoreability() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let interface =
        fs::read_to_string(root.join("src/features/research/interface/gray_rhino_report.rs"))
            .expect("failed to read gray rhino report");
    let domain =
        fs::read_to_string(root.join("src/features/research/domain/gray_rhino_evidence.rs"))
            .expect("failed to read evidence domain");
    let application = fs::read_to_string(
        root.join("src/features/research/application/gray_rhino_daily_report.rs"),
    )
    .expect("failed to read daily report app");
    let checker = fs::read_to_string(root.join("scripts/check_gray_rhino_evidence_contract.py"))
        .expect("failed to read contract checker");

    assert!(!interface.contains("is_scoreable_evidence_record"));
    assert!(
        !interface.contains("GrayRhinoRiskEffect::Amplifying | GrayRhinoRiskEffect::Mitigating")
    );
    assert!(application.contains("scoreable_evidence_records("));
    assert!(!application.contains("fn scoreable_evidence_records"));
    assert!(domain.contains("fn is_scoreable_evidence_record"));
    assert!(domain.contains("fn scoreable_evidence_records"));
    assert!(checker.contains("interface must not contain evidence eligibility policy"));
    assert!(
        checker.contains("daily report application must not define evidence scoreability policy")
    );
}

#[test]
fn gray_rhino_temporal_and_survivability_policies_are_domain_owned() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let application = fs::read_to_string(
        root.join("src/features/research/application/gray_rhino_daily_report.rs"),
    )
    .expect("failed to read daily report app");
    let temporal =
        fs::read_to_string(root.join("src/features/research/domain/gray_rhino_temporal_policy.rs"))
            .expect("failed to read temporal policy");
    let survivability = fs::read_to_string(
        root.join("src/features/research/domain/gray_rhino_survivability_policy.rs"),
    )
    .expect("failed to read survivability policy");
    let checker = fs::read_to_string(root.join("scripts/check_gray_rhino_evidence_contract.py"))
        .expect("failed to read contract checker");

    assert!(temporal.contains("fn build_temporal_summary"));
    assert!(survivability.contains("fn build_survivability_summary"));
    assert!(!application.contains("fn build_temporal_summary"));
    assert!(!application.contains("fn build_survivability_summary"));
    assert!(checker.contains("grayRhinoTemporalPolicy:"));
    assert!(checker.contains("grayRhinoSurvivabilityPolicy:"));
}

#[test]
fn gray_rhino_typed_validators_reuse_category_source_type_policy() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let domain =
        fs::read_to_string(root.join("src/features/research/domain/gray_rhino_evidence.rs"))
            .expect("failed to read evidence domain");
    let source_policy = fs::read_to_string(
        root.join("src/features/research/domain/gray_rhino_evidence_source_policy.rs"),
    )
    .expect("failed to read evidence source policy");
    let checker = fs::read_to_string(root.join("scripts/check_gray_rhino_evidence_contract.py"))
        .expect("failed to read contract checker");

    assert_eq!(
        source_policy
            .matches("fn source_type_allowed_for_category")
            .count(),
        1
    );
    assert!(!domain.contains("fn source_type_allowed_for_category"));
    assert!(source_policy.contains("fn validate_source_type_for_category"));
    assert_eq!(
        domain.matches("validate_source_type_for_category(").count(),
        5
    );
    assert!(!domain.contains("if !matches!(\n            self.source.source_type"));
    assert!(checker.contains("typed evidence validators must reuse category source type policy"));
}

#[test]
fn gray_rhino_source_type_policy_is_domain_owned() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let domain_mod = fs::read_to_string(root.join("src/features/research/domain/mod.rs"))
        .expect("failed to read domain mod");
    let source_policy = fs::read_to_string(
        root.join("src/features/research/domain/gray_rhino_evidence_source_policy.rs"),
    )
    .expect("failed to read evidence source policy");
    let evidence =
        fs::read_to_string(root.join("src/features/research/domain/gray_rhino_evidence.rs"))
            .expect("failed to read evidence domain");
    let checker = fs::read_to_string(root.join("scripts/check_gray_rhino_evidence_contract.py"))
        .expect("failed to read contract checker");

    assert!(domain_mod.contains("gray_rhino_evidence_source_policy"));
    assert!(source_policy.contains("pub(crate) fn validate_source_type_for_category"));
    assert!(source_policy.contains("fn source_type_allowed_for_category"));
    assert!(
        evidence.contains("gray_rhino_evidence_source_policy::validate_source_type_for_category")
    );
    assert!(checker
        .contains("evidence source policy must define exactly one category/source_type allowlist"));
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
            .expect("failed to read gray rhino interface")
            + &fs::read_to_string(
                root.join(
                    "src/features/research/interface/gray_rhino_inline_reference_renderer.rs",
                ),
            )
            .expect("failed to read gray rhino inline reference renderer");
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
    assert!(makefile.contains("refresh_date_arg"));
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
    assert!(stdout.contains("--- 灰犀牛依赖证据采集 ---"));
    assert!(stdout.contains("来源数:  1"));
    assert!(stdout.contains("已接受: 1"));
    assert!(stdout.contains("已保存:    0"));
    assert!(stdout.contains("清单: 1"));
    assert!(stdout.contains("审计:    1"));
    assert!(stdout.contains("正式证据已持久化: 否"));
    assert!(stdout.contains("字段覆盖率:"));
    assert!(stdout.contains("concentration_ratio: 100.0% (1/1 extracted"));
    assert!(stdout.contains("已拒绝: 0"));
    assert!(stdout.contains("边界声明: 仅限证据处理，不更新升级、闸门、执行或交易状态。"));
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
    assert!(stdout.contains("已拒绝: 1"));
    assert!(stdout.contains("[REJECTED:MetriclessSource]"));
    assert!(stdout.contains("正式证据已持久化: 否"));
    assert!(stdout.contains("边界声明: 仅限证据处理，不更新升级、闸门、执行或交易状态。"));
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
    assert!(institutional_stdout.contains("--- 灰犀牛 InstitutionalMaturity 证据采集 ---"));
    assert!(institutional_stdout.contains("覆盖率: 100.0%"));
    assert!(institutional_stdout.contains("正式证据已持久化: 否"));
    assert!(redundancy_stdout.contains("--- 灰犀牛 Redundancy 证据采集 ---"));
    assert!(redundancy_stdout.contains("覆盖率: 100.0%"));
    assert!(
        redundancy_stdout.contains("边界声明: 仅限证据处理，不更新升级、闸门、执行或交易状态。")
    );
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
    assert!(stdout.contains("--- 灰犀牛治理证据采集 ---"));
    assert!(stdout.contains("来源数:  1"));
    assert!(stdout.contains("已接受: 1"));
    assert!(stdout.contains("清单: 1"));
    assert!(stdout.contains("审计:    1"));
    assert!(stdout.contains("干运行:  false"));
    assert!(stdout.contains("正式证据已持久化: 是"));
    assert!(stdout.contains("覆盖率: 100.0%"));
    assert!(stdout.contains("字段覆盖率:"));
    assert!(stdout.contains("founder_voting_power: 100.0% (1/1 extracted"));
    assert!(stdout.contains("已拒绝: 0"));
    assert!(stdout.contains("边界声明: 仅限证据处理，不更新升级、闸门、执行或交易状态。"));
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
    assert!(stdout.contains("来源数:  1"));
    assert!(stdout.contains("已接受: 0"));
    assert!(stdout.contains("已拒绝: 1"));
    assert!(stdout.contains("干运行:  false"));
    assert!(stdout.contains("正式证据已持久化: 是"));
    assert!(stdout.contains("覆盖率: 0.0%"));
    assert!(stdout.contains("字段覆盖率:"));
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

#[test]
fn daily_calibration_rejects_future_date_without_writing_valuation_snapshot() {
    let tmp = prepare_workspace("");

    let output = run_cli(&tmp, &["daily-calibration", "--date", "2999-01-01"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("估值重力层不接受未来日期"));
    assert!(!tmp.path().join("valuation_gravity_latest.json").exists());
    assert!(!tmp
        .path()
        .join("valuation_gravity_2999-01-01.json")
        .exists());
}

#[test]
fn daily_calibration_expectation_uses_requested_market_date() {
    let tmp = prepare_workspace("");

    let output = run_cli(&tmp, &["daily-calibration", "--date", "2026-08-12"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expectation = stdout
        .split("## 9. Expectation Layer（市场预期观测）")
        .last()
        .expect("expectation section should be rendered");
    assert!(expectation.contains("- As of: 2026-08-12"), "{expectation}");
    assert!(
        !expectation.contains("- As of: 2026-08-13"),
        "{expectation}"
    );
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
fn config_check_validates_config_without_running_reports() {
    let tmp = prepare_workspace("");

    let out = run_cli(&tmp, &["config-check"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("config.toml OK"));
    assert!(stdout.contains("watchlist entries"));
    assert!(!stdout.contains("每日认知校准"));
    assert!(!stdout.contains("Daily Cognitive Calibration"));
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
fn spcx_standard_catalog_does_not_require_explicit_config_translations() {
    let config = r#"

[research_attention.SPCX]
cognitive_yield = "HIGH"
attention_cost = "HIGH"
information_density = "EXPANDING"
reason = "宇宙輸送、Starlink、衛星通信、政府契約、AI / compute infrastructure との接続が公開市場でどのように評価されるかを観測する価値が高い。"

[asset_thesis.SPCX]
thesis = "SpaceX が宇宙輸送、Starlink、衛星通信、政府契約を通じて、長期インフラ企業として公開市場で評価されるかを観測する。"
observation_focus = [
  "Starlink の成長率と利益率",
  "打ち上げ事業の価格競争力"
]
invalidation = [
  "Starlink の成長または利益率が期待を下回る",
  "governance または key-person dependency が評価を圧迫する"
]
"#;

    for (language, expected_reason, expected_thesis, expected_focus, expected_invalidation) in [
        (
            "zh-cn",
            "观察 SpaceX 在宇宙运输、Starlink、卫星通信、政府合同",
            "观察 SpaceX 是否能通过宇宙运输、Starlink、卫星通信和政府合同",
            "IPO 后的供需、lockup 与流通股结构",
            "治理结构或关键人物依赖压制估值",
        ),
        (
            "en-us",
            "Observe how public markets price SpaceX across launch",
            "Observe whether SpaceX can be valued by public markets as a long-term infrastructure company",
            "Post-IPO supply-demand, lockup, and public float structure",
            "Governance or key-person dependency pressures valuation",
        ),
        (
            "ja-jp",
            "宇宙輸送、Starlink、衛星通信、政府契約",
            "SpaceX が宇宙輸送、Starlink、衛星通信、政府契約を通じて",
            "Starlink の成長率と利益率",
            "governance または key-person dependency が評価を圧迫する",
        ),
    ] {
        let tmp = prepare_workspace(config);
        set_output_language(&tmp, language);

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
fn asset_thesis_outputs_anti_narrative_governance() {
    let tmp = prepare_workspace(
        r#"

[asset_thesis.MSFT]
thesis = "Azure、Copilot、企業 AI 導入がデータセンター投資を正当化し続けるかを観測する。"
observation_focus = ["Azure 成長率と AI 寄与"]
invalidation = ["AI 関連 Capex が収益成長に接続しない"]
time_horizon = "LONG"
materialization_window = "12-36 months"

[asset_thesis.MSFT.narrative_state]
consensus_level = "CROWDED"
skepticism_level = "LOW"
valuation_reflection = "PARTIAL"

[asset_thesis.MSFT.reality_override]
observable_contradiction = true
confidence_decay = true
"#,
    );

    let out = run_cli(&tmp, &["asset-thesis"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("反叙事治理:"));
    assert!(stdout.contains("共识 CROWDED / 怀疑 LOW / 定价反映 PARTIAL"));
    assert!(stdout.contains("时间尺度 LONG / 兑现窗口 12-36 months"));
    assert!(stdout.contains("现实覆盖 TRUE / 置信衰减 TRUE"));
    assert!(stdout.contains("叙事越顺，越需要现实覆盖"));
    assert!(!stdout.contains("自动买入"));
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

[capital_absorption]
auto_enable = false
status = "WATCH"
structural_impact = "Observation Only"
upgrade_to_active = ["Second mega cap financing", "Large AI IPO starts"]
upgrade_to_stressed = ["Capital Demand > Capital Supply", "ETF net inflow remains weaker than financing scale"]

[[capital_absorption.observed_events]]
category = "MEGA_CAP_FINANCING"
subject = "Alphabet"
description = "Manual observation: secondary offering for AI CapEx"
amount_usd_b = 80.0
ai_capex_related = true
source_url = "https://example.com/alphabet-offering"

[capital_absorption.capital_demand]
trend = "INCREASING"
rolling_12m_usd_b = 80.0
score = 0.60
secondary_offering_usd_b = 80.0
ai_related_financing_usd_b = 80.0

[capital_absorption.capital_supply]
trend = "STABLE"
rolling_12m_usd_b = 500.0
score = 0.75
etf_net_inflow_usd_b = 120.0
corporate_buyback_usd_b = 380.0

[capital_absorption.absorption_ratio]
state = "NEUTRAL"
value = 0.16

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
    assert!(stdout.contains("## 9. Expectation Layer（市场预期观测）"));
    assert!(stdout.contains("Boundary: Expectation Layer 仅用于观测市场预期"));
    assert!(!stdout.contains("データセンター注文の継続性"));
    assert!(stdout.contains("## 5. 宏观重力校准"));
    assert!(stdout.contains("- 利率压力: RISING"));
    assert!(stdout.contains("- 成长股估值: COMPRESSING"));
    assert!(stdout.contains("不参与 Gate，不生成交易指令"));
    assert!(stdout.contains("## 6. Capital Dynamics（供需观察）"));
    assert!(stdout.contains("🧱 Capital Dynamics（供需观察）"));
    assert!(stdout.contains("### 6.1 Supply Layer（Capital Absorption）"));
    assert!(stdout.contains("📊 资本吸收早期预警传感器"));
    assert!(stdout.contains("资本吸收状态: 观察（WATCH）"));
    assert!(stdout.contains("实际供给事件"));
    assert!(stdout.contains("- Mega Cap 融资: 1"));
    assert!(stdout.contains("实际资本供给"));
    assert!(stdout.contains("- 已观察实际供给: $80.0B"));
    assert!(stdout.contains("潜在供给趋势"));
    assert!(stdout.contains("- 趋势: 稳定（STABLE）"));
    assert!(stdout.contains("Future IPO 队列"));
    assert!(stdout.contains(
        "Subject: Anthropic · Event Type: 传闻（Rumor） · Expected Window: future window · Status: 传闻（Rumor） · Source Quality: unavailable · Lifecycle: 传闻（Rumor）"
    ));
    assert!(stdout.contains("发现"));
    assert!(stdout.contains("新增:"));
    assert!(stdout.contains("- Alphabet x1"));
    assert!(stdout.contains("- 增发融资: $80.0B"));
    assert!(!stdout.contains("资本需求趋势"));
    assert!(!stdout.contains("ACCELERATING"));
    assert!(stdout.contains("资本供给趋势"));
    assert!(stdout.contains("- ETF 净流入: $120.0B"));
    assert!(stdout.contains("资本吸收比率: 本阶段未启用完整量化"));
    assert!(stdout.contains("结构影响: 仅观察"));
    assert!(!stdout.contains("结构影响: Observation Only"));
    assert!(stdout.contains("当前阶段: Narrative Observation Only"));
    assert!(stdout
        .contains("观察对象: Potential Future Capital Supply，而不是 Actual Capital Absorption"));
    assert!(stdout.contains("IPO 新闻增加不等于资本供给增加"));
    assert!(stdout.contains("当前阶段仅允许 NORMAL / WATCH"));
    assert!(!stdout.contains("升级到 ACTIVE 的条件"));
    assert!(!stdout.contains("升级到 STRESSED 的条件"));
    assert!(stdout.contains("不生成交易信号"));
    assert!(stdout.contains("不测量实际资本吸收"));
    assert!(stdout.contains("不测量市场流动性"));
    assert!(stdout.contains("不产生市场结论"));
    assert!(stdout.contains("不影响 READY / EXECUTE / Position Sizing / Gate / Trend Layer"));
    assert!(!stdout.contains("https://example.com/alphabet-offering"));
    assert!(stdout.contains("## 7. 估值重力层"));
    assert!(stdout.contains("🪢 Gravity Layer（估值重力层）"));
    assert!(stdout.contains("未形成估值分类（外部证据不足）"));
    assert!(stdout.contains("快照持久化: 已保存"));
    assert!(stdout.contains("来源状态: 不可用"));
    assert!(stdout.contains("证据数量: 0"));
    assert!(stdout.contains("数据质量原因: 未配置外部数据凭证"));
    assert!(stdout.contains("Gravity 与 Trend 独立"));
    assert!(!stdout.contains("Gravity: Unknown"));
    assert!(stdout.contains("## 8. 灰犀牛升级监控"));
    assert!(stdout.contains("输入来源: 人工结构基线（配置输入）"));
    assert!(stdout.contains("审计链: 人工结构基线 -> 七项观测 -> 日次快照"));
    assert!(stdout.contains("状态:"));
    assert!(stdout.contains("风险扩张速度: 偏高"));
    assert!(stdout.contains("相比前次日次评估: 首次记录（无前次快照）"));
    assert!(stdout.contains("不代表自动事实发现"));
    assert!(stdout.contains("边界声明: 灰犀牛升级监控仅观察结构性风险升级，不生成交易信号。"));
    assert!(!stdout.contains("State:"));
    assert!(stdout.contains("不生成新的交易指令"));
    assert!(stdout.contains("## 9. Expectation Layer（市场预期观测）"));
    assert!(stdout.contains("decision_weight"));
    assert!(stdout.contains("trade_signal"));
}

#[test]
fn capital_absorption_unavailable_source_does_not_render_default_ipo_queue() {
    let tmp = prepare_workspace_without_capital_absorption_default("");

    let out = run_cli(&tmp, &["daily-calibration"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("📊 资本吸收早期预警传感器"));
    assert!(stdout.contains("自动来源: Finnhub company-news"));
    assert!(stdout.contains("未观察到大型资本吸收事件"));
    assert!(!stdout.contains("Anthropic:"));
    assert!(!stdout.contains("OpenAI:"));
    assert!(!stdout.contains("SpaceX:"));
    assert!(!stdout.contains("Databricks:"));
    assert!(!stdout.contains("Stripe:"));
    assert!(!stdout.contains("Figure:"));
}

#[test]
fn capital_absorption_new_sections_are_locked_in_en_and_ja() {
    for (
        language,
        title,
        actual_supply,
        potential_trend,
        discovery_label,
        summary_event,
        boundary,
    ) in [
        (
            "en-us",
            "📊 Capital Absorption Early Warning Sensor",
            "Actual Capital Supply",
            "Potential Supply Trend",
            "Observed Events",
            "- Alphabet x1",
            "does not affect READY / EXECUTE / Position Sizing / Gate / Trend Layer",
        ),
        (
            "ja-jp",
            "📊 資本吸収早期警戒センサー",
            "実際の資本供給",
            "潜在供給トレンド",
            "観測イベント",
            "- Alphabet x1",
            "READY / EXECUTE / Position Sizing / Gate / Trend Layer に影響しない",
        ),
    ] {
        let tmp = prepare_workspace(capital_absorption_manual_config());
        set_output_language(&tmp, language);

        let out = run_cli(&tmp, &["daily-calibration"]);

        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains(title));
        assert!(stdout.contains(actual_supply));
        assert!(
            stdout.contains("- Observed actual supply: $80.0B")
                || stdout.contains("- 観測済み実供給: $80.0B")
        );
        assert!(stdout.contains(potential_trend));
        assert!(stdout.contains(discovery_label));
        assert!(stdout.contains(summary_event));
        assert!(stdout.contains(boundary));
        assert!(!stdout.contains("Capital Demand"));
        assert!(!stdout.contains("ACCELERATING"));
    }
}

fn capital_absorption_manual_config() -> &'static str {
    r#"

[capital_absorption]
auto_enable = false
status = "WATCH"
structural_impact = "Observation Only"

[[capital_absorption.observed_events]]
category = "MEGA_CAP_FINANCING"
subject = "Alphabet"
description = "Manual observation: secondary offering for AI CapEx"
amount_usd_b = 80.0
ai_capex_related = true
source_url = "https://example.com/alphabet-offering"

[capital_absorption.capital_demand]
trend = "INCREASING"
rolling_12m_usd_b = 80.0
score = 0.60
secondary_offering_usd_b = 80.0
ai_related_financing_usd_b = 80.0

[capital_absorption.capital_supply]
trend = "STABLE"

[capital_absorption.absorption_ratio]
state = "NEUTRAL"
"#
}

fn flow_layer_manual_config() -> &'static str {
    r#"

[capital_dynamics]
enable = true

[capital_dynamics.flow_layer]
enable = true
as_of_date = "2026-05-25"
observation_only = true
decision_weight_percent = 0
trend_override_allowed = false

[capital_dynamics.flow_layer.breadth]
market_breadth = "UNAVAILABLE"
sector_breadth = "DIVERGENT"
watchlist_breadth = "SUPPORTIVE"
core_holding_breadth = "NEUTRAL"

[[capital_dynamics.flow_layer.observations]]
as_of_date = "2026-05-25"
observed_at = "2026-05-25"
scope = "ASSET"
subject = "NVDA"
provider = "Manual"
source_kind = "CapitalFlow"
direction = "INFLOW"
strength = "STRONG"
quality = "HEALTHY"
continuity_days = 5
net_flow = 12.5
main_net_flow = 8.2
source_health = "SUCCEEDED"

[[capital_dynamics.flow_layer.divergences]]
subject = "GOOG"
price_direction = "UP"
flow_direction = "OUTFLOW"
divergence_type = "NEGATIVE"
severity = "HIGH"
explanation_key = "negative_divergence"
"#
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
fn daily_calibration_renders_flow_layer_as_observation_only_demand_section() {
    let tmp = prepare_workspace(flow_layer_manual_config());

    let out = run_cli(&tmp, &["daily-calibration", "--date", "2026-05-25"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("## 6. Capital Dynamics（供需观察）"));
    assert!(stdout.contains("🧱 Capital Dynamics（供需观察）"));
    assert!(stdout.contains("### 6.2 Demand Layer（Flow Layer）"));
    assert!(stdout.contains("🌊 Flow Layer（需求侧观察）"));
    assert!(stdout.contains("NVDA [ASSET]"));
    assert!(stdout.contains("INFLOW"));
    assert!(stdout.contains("HEALTHY"));
    assert!(stdout.contains("GOOG"));
    assert!(stdout.contains("NEGATIVE"));
    assert!(stdout.contains("不生成新的交易信号"));
    assert!(stdout.contains("不覆盖 Trend Layer"));
    assert!(!stdout.contains("Position Sizing:"));
    assert!(stdout.contains("## 9. Expectation Layer（市场预期观测）"));
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

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
                || line == "[gray_rhino_escalation]";
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
    let mut raw =
        fs::read_to_string(root.join("config.toml")).expect("failed to read base config.toml");
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
    assert!(stdout.contains("Gray Rhino Escalation"));
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
    for (language, expected, notice, forbidden) in [
        ("zh-cn", "状态: 风险常态化", "不生成交易信号。", "State:"),
        (
            "en-us",
            "State: Normalized",
            "It does not generate trading signals.",
            "风险扩张速度",
        ),
        (
            "ja-jp",
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
        assert!(stdout.contains("Gray Rhino Escalation"));
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
    assert!(stdout.contains("Governance sensor health"));
    assert!(stdout.contains("coverage ratio"));
    assert!(stdout.contains("Boundary: Governance sensor health only"));
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

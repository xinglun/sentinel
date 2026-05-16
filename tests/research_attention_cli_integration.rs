// Research Attention の CLI 境界を固定する統合テスト。

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn prepare_workspace(extra_config: &str) -> TempDir {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = root.join("config.toml");
    let mut raw = fs::read_to_string(&config_path).expect("failed to read base config.toml");
    let research_start = raw.find("\n[research_attention.");
    let thesis_start = raw.find("\n[asset_thesis.");
    let macro_start = raw.find("\n[macro_gravity]");
    if let Some(start) = [research_start, thesis_start, macro_start]
        .into_iter()
        .flatten()
        .min()
    {
        raw.truncate(start);
    }

    let save_to = tmp.path().to_string_lossy().to_string();
    raw = raw.replace(
        "save_to = \"./reports\"",
        &format!("save_to = \"{}\"", save_to),
    );
    raw.push_str(extra_config);

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
    assert!(stdout.contains("🧠 Research Attention"));
    assert!(stdout.contains("HIGH:"));
    assert!(stdout.contains("TSLA · 信息密度 EXPANDING · 注意力成本 HIGH"));
    assert!(stdout.contains("MEDIUM:"));
    assert!(stdout.contains("GOOG · 信息密度 STABLE · 注意力成本 LOW"));
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
    assert!(stdout.contains("🧭 Asset Thesis Registry"));
    assert!(stdout.contains("NVDA · AI インフラ需要"));
    assert!(stdout.contains("观察焦点:"));
    assert!(stdout.contains("データセンター注文の継続性"));
    assert!(stdout.contains("失效条件:"));
    assert!(stdout.contains("主要クラウドの Capex 減速"));
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
"#,
    );

    let out = run_cli(&tmp, &["daily-calibration"]);

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("🧭 Daily Cognitive Calibration"));
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
    assert!(stdout.contains("NVDA · AI インフラ需要"));
    assert!(stdout.contains("## 5. 宏观重力校准"));
    assert!(stdout.contains("- 利率压力: RISING"));
    assert!(stdout.contains("- 成长股估值: COMPRESSING"));
    assert!(stdout.contains("不参与 Gate，不生成交易指令"));
    assert!(stdout.contains("不生成新的交易指令"));
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
    assert!(stdout.contains("🌐 Macro Gravity"));
    assert!(stdout.contains("- 实际利率: TIGHT"));
    assert!(stdout.contains("- 信用压力: WATCH"));
    assert!(stdout.contains("割引率上昇"));
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

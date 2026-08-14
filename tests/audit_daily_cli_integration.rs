use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn prepare_workspace_with_language(language: &str) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = root.join("config.toml");
    let raw = fs::read_to_string(&config_path).expect("failed to read base config.toml");
    let raw = strip_optional_local_sections(&raw);
    let patched = raw.replacen(
        "language = \"zh-cn\"",
        &format!("language = \"{}\"", language),
        1,
    );
    fs::write(tmp.path().join("config.toml"), patched).expect("failed to write temp config.toml");
    tmp
}

fn strip_optional_local_sections(raw: &str) -> String {
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

fn run_cli(tmp: &tempfile::TempDir, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_stock-sentinel"))
        .current_dir(tmp.path())
        .args(args)
        .output()
        .expect("failed to execute stock-sentinel")
}

#[test]
fn audit_daily_cli_rejects_missing_date_value_zh_cn() {
    let tmp = prepare_workspace_with_language("zh-cn");
    let out = run_cli(&tmp, &["audit_daily", "--date"]);
    assert!(
        !out.status.success(),
        "command should fail on invalid --date"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Handler 境界移動後も、引数エラーは stderr と usage の既存契約を保持する。
    assert!(out.stdout.is_empty());
    assert!(stderr.contains("--date 需要 YYYY-MM-DD 参数"));
    assert!(stderr.contains("用法:"));
}

#[test]
fn audit_daily_cli_rejects_invalid_days_value_en_us() {
    let tmp = prepare_workspace_with_language("en-us");
    let out = run_cli(&tmp, &["audit_daily", "--days", "foo"]);
    assert!(
        !out.status.success(),
        "command should fail on invalid --days"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--days must be an integer greater than 0"));
    assert!(stderr.contains("Usage:"));
}

#[test]
fn audit_daily_cli_rejects_missing_days_value_ja_jp() {
    let tmp = prepare_workspace_with_language("ja-jp");
    let out = run_cli(&tmp, &["audit_daily", "--days"]);
    assert!(
        !out.status.success(),
        "command should fail on missing --days value"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--days には正の整数値が必要です"));
    assert!(stderr.contains("使い方:"));
}

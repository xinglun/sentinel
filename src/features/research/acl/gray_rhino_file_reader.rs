use anyhow::{Context, Result};

/// CLI 由来の local file 入力を research interface から分離して読み込む。
pub(crate) fn read_gray_rhino_text_file(path: &str, context: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("{context}: {path}"))
}

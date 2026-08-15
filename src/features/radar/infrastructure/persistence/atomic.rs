use anyhow::{Context, Result};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

/// 一時ファイルを同じディレクトリに作成し、完成後に対象へ原子的に置換する。
pub(super) fn write_file_atomically(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("原子的書き込み先の作成に失敗: {parent:?}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("原子的書き込み用一時ファイルの作成に失敗: {path:?}"))?;
    temporary
        .write_all(content)
        .with_context(|| format!("原子的書き込み用一時ファイルへの書き込みに失敗: {path:?}"))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("原子的書き込み用一時ファイルの同期に失敗: {path:?}"))?;
    if let Ok(metadata) = std::fs::metadata(path) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())
            .with_context(|| format!("原子的書き込み先の権限設定に失敗: {path:?}"))?;
    }
    temporary
        .persist(path)
        .map(|_| ())
        .map_err(|error| error.error)
        .with_context(|| format!("原子的書き込み先の置換に失敗: {path:?}"))
}

pub(crate) struct HistoryWriteTransaction {
    pub(super) files: Vec<(PathBuf, Option<Vec<u8>>)>,
    pub(super) committed: bool,
}

impl HistoryWriteTransaction {
    pub(crate) fn commit(mut self) {
        self.committed = true;
    }

    fn rollback(&self) -> Result<()> {
        for (path, content) in self.files.iter().rev() {
            match content {
                Some(content) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).with_context(|| {
                            format!("履歴ロールバック先の作成に失敗: {parent:?}")
                        })?;
                    }
                    std::fs::write(path, content)
                        .with_context(|| format!("履歴ファイルの復元に失敗: {path:?}"))?;
                }
                None => match std::fs::remove_file(path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(error)
                            .with_context(|| format!("履歴ファイルの削除に失敗: {path:?}"));
                    }
                },
            }
        }
        let known_paths = self
            .files
            .iter()
            .map(|(path, _)| path)
            .collect::<HashSet<_>>();
        for entry in std::fs::read_dir(self.files[0].0.parent().unwrap_or(Path::new(".")))
            .context("履歴ロールバック対象の確認に失敗")?
        {
            let path = entry
                .context("履歴ロールバック対象の読み込みに失敗")?
                .path();
            let is_new_legacy_transition = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("state_transitions_legacy_") && name.ends_with(".csv")
                });
            if is_new_legacy_transition && !known_paths.contains(&path) {
                std::fs::remove_file(&path)
                    .with_context(|| format!("新規 legacy 履歴の削除に失敗: {path:?}"))?;
            }
        }
        Ok(())
    }
}

impl Drop for HistoryWriteTransaction {
    fn drop(&mut self) {
        if !self.committed {
            if let Err(error) = self.rollback() {
                eprintln!("履歴トランザクションのロールバックに失敗: {error:#}");
            }
        }
    }
}

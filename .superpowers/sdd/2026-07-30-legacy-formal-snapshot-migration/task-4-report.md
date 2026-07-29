---
author: Ray
title: Task 4 実施報告
description: legacy packet から formal snapshot への起動時移行統合の実施証跡。
key: task-4-legacy-formal-snapshot-migration-report
---

# Task 4 実施報告

## 本タスクの提出証跡

- `PersistenceLayer::legacy_history_migration_needed` を追加した。formal snapshot が一件でもある場合は移行せず、formal snapshot が無く日付付き legacy packet がある場合だけ移行する。
- runner は基線解決の前に上記判定と `migrate_legacy_history` を実行し、移行後に state と formal history を読み直す。
- migration、formal snapshot 解決、packet history、leader history の読み込み失敗は `?` で呼出元へ返す。`BASELINE_UNAVAILABLE` への変換は、実際に基線が存在しない場合だけに残る。
- persistence harness に、formal snapshot 既存時の移行抑止と、移行後の二回目起動で cycle を維持して新しい市場日を追加する回帰試験を追加した。
- 実行結果: 共有作業区では `cargo test features::radar::infrastructure::persistence::tests --lib` が 26 passed、Task 4 の提出差分だけでは同 command が 24 passed である。両方で `cargo fmt --all -- --check` が passed した。

完全な runner の実行は、既存 harness に日付と外部依存を注入する入口がないため追加していない。二回実行の試験は実際の `PersistenceLayer`、legacy packet、formal snapshot、state を使い、runner が使う起動判定と同じ永続化境界を検証する。

## 共有作業区の既存未提出変更

本タスクの開始時点で、次の変更は共有作業区に既に存在していたため、本タスクの提出証跡には含めない。

- `.github/workflows/daily_radar.yml` の未提出変更。
- `persistence.rs` の degraded historical fact、`latest_cycle_id_before`、既存 migration 回帰試験に関する未提出変更。
- `radar_pipeline_runner.rs` の既存 state 読み込み、latest cycle recovery、cycle 選択に関する未提出変更。
- `.ai/work-items/active/`、`docs/superpowers/plans/`、`docs/superpowers/specs/` の未追跡 artifact。

これらは変更・削除・本タスクの commit への混入をしていない。Task 4 の runner 差分は上記の共有 recovery helper に依存するため、Task 4 commit 単体ではなく現在の共有作業区と組み合わせて検証した。

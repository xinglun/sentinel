---
author: Ray
title: Task 1 legacy formal snapshot migration 実施報告
description: legacy packet から formal snapshot への移行 API を定義する失敗テストの実施結果。
key: legacy-formal-snapshot-migration-task-1-report
---

# Task 1 実施報告

## 変更ファイル

- `src/features/radar/infrastructure/persistence.rs`
  - `migrate_legacy_history` の単一 packet projection 失敗テストを追加。
  - `source_status`、`decision_state`、`data_quality["history"]`、state count、最終市場日、cycle_id を実際に検証。
  - 逆順に作成した 2026-07-28 / 2026-07-29 packet について、日付順、同一の stable cycle_id、snapshot file 名、state 集計を検証。
- `.superpowers/sdd/2026-07-30-legacy-formal-snapshot-migration/task-1-report.md`
  - 本報告を追加。

既存の前序 persistence 修正と、それ以外の作業ツリー変更は変更していない。

## テスト

実行コマンド：

```text
cargo test migrate_legacy_history --lib
```

期待結果：失敗。`PersistenceLayer` に `migrate_legacy_history` がまだ存在しないため、コンパイル時に `no method named migrate_legacy_history` が 2 箇所で報告された。これは Task 1 のテスト先行段階における想定どおりの missing-interface failure である。

補助確認：

```text
cargo fmt --all -- --check
```

テスト整形後は成功する。

## 顧慮

- production API は実装していない。後続 Task で `migrate_legacy_history` を実装する必要がある。
- migration API が未実装のため、focused test は成功状態ではなく、formal snapshot や observation state の実行時検証も未実施である。

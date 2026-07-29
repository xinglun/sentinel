---
author: Ray
title: Task 2 legacy formal snapshot migration 実施報告
description: legacy packet を formal snapshot と observation history state へ安全に移行する API の実施結果。
key: legacy-formal-snapshot-migration-task-2-report
---

# Task 2 実施報告

## 変更ファイル

- `src/features/radar/infrastructure/persistence.rs`
  - `migrate_legacy_history` を追加し、save_dir 直下の dated legacy packet を日付順で読み込み、日付重複を解決するようにした。
  - ファイル名の日付、packet 本文、既存 state、formal snapshot の競合をすべて書き込み前に検証する fail-closed 処理を追加した。
  - legacy packet を `degraded`、`NO_TRADE`、position limit `0.0`、`MIGRATED_LEGACY` を持つ formal snapshot へ写像した。
  - state の既存非空 cycle_id を再利用し、存在しない場合だけ日付範囲から決定的な cycle_id を生成するようにした。
  - 同一 semantics の既存 snapshot は保存し直さず、元の JSON を保持するようにした。
  - `observation_history_state.json` の保存を既存の原子的書き込み helper に統一した。
- `src/features/radar/infrastructure/persistence.rs` の inline test
  - 安全な projection、決定的 cycle、既存 state cycle の再利用、同一 snapshot の非上書き、繰り返し実行時の state/snapshot 保持を検証した。
  - dated legacy packet に該当しない通常ファイルが移行を妨げず無視されることを検証した。
- `.superpowers/sdd/2026-07-30-legacy-formal-snapshot-migration/task-2-report.md`
  - 本報告を追加した。

交易 gate、action matrix、position sizing は変更していない。

## テスト結果

実行コマンド：

```text
cargo test migrate_legacy_history --lib
```

結果：成功。3 passed、0 failed。

```text
cargo fmt --all -- --check
```

結果：成功。

## 設計上の選択

- `decision_packet_YYYY-MM-DD.json` だけを移行入力にした。通常ファイルは名前が一致しないため無視する。ファイル名の日付と packet 日付が異なる場合、または JSON を復号できない場合は、出力を書き込まずにエラーを返す。
- cycle_id は既存 state の非空値を優先し、なければ `legacy-<first>-<last>` を使用する。時刻や UUID は使用しない。
- packet にない文字列値は `UNAVAILABLE`、collection は空、連続日数は安全な `0` とした。取引判断は復元せず、常に `NO_TRADE` と position limit `0.0` に固定した。
- 全 snapshot の既存競合検証を通過してから、欠けている snapshot だけを保存し、最後に state を原子的に保存する。意味的差異は既存 validator により `SNAPSHOT_CONFLICT` を返す。完全に同一の再実行は、同じ cycle と既存 snapshot JSON を保持する。

## 残余顧慮

- snapshot 複数ファイルの保存は既存の file-per-snapshot 方式に従う。全 conflict は先行検証するが、保存中の OS I/O 障害に対する複数 snapshot 間の filesystem transaction は既存 schema にない。
- malformed input と semantic conflict の包括的な回帰ケースは Task 3 の範囲として残る。

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
  - `observation_history_state.json` の保存を既存の原子的書き込み helper に統一した。
- `.superpowers/sdd/2026-07-30-legacy-formal-snapshot-migration/task-2-report.md`
  - 本報告を追加した。

前序の degraded snapshot を歴史事実として扱う修正、および formal snapshot から latest cycle を復元する修正は保持した。交易 gate、action matrix、position sizing は変更していない。

## テスト結果

実行コマンド：

```text
cargo test migrate_legacy_history --lib
```

結果：成功。2 passed、0 failed。

関連する persistence 回帰確認：

```text
cargo test formal_snapshot --lib
cargo test latest_cycle_id_before_date_is_recovered_from_formal_snapshot --lib
make fmt-check
make clippy
make test
```

結果：それぞれ 5 passed、1 passed、成功、成功、629 library tests と integration tests が成功。

## 設計上の選択

- `decision_packet_YYYY-MM-DD.json` だけを移行入力にした。ファイル名の日付と packet 日付が異なる場合、または JSON を復号できない場合は、出力を書き込まずにエラーを返す。
- cycle_id は既存 state の非空値を優先し、なければ `legacy-<first>-<last>` を使用する。時刻や UUID は使用しない。
- packet にない文字列値は `UNAVAILABLE`、collection は空、連続日数は安全な `0` とした。取引判断は復元せず、常に `NO_TRADE` と position limit `0.0` に固定した。
- 全 snapshot の既存競合検証を通過してから snapshot を保存し、最後に state を原子的に保存する。意味的差異は既存 validator により `SNAPSHOT_CONFLICT` を返す。完全に同一の再実行は同じ cycle と snapshot 内容を再利用する。

## 残余顧慮

- snapshot 複数ファイルの保存は既存の file-per-snapshot 方式に従う。全 conflict は先行検証するが、保存中の OS I/O 障害に対する複数 snapshot 間の filesystem transaction は既存 schema にない。
- AI Cockpit resolver はこの repository の Make lifecycle entrypoint を検出できなかったため、Contract 専用の make gate は実行できなかった。指定された focused test と関連 persistence test は実行済みである。

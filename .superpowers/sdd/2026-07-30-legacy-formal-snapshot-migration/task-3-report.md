---
author: Ray
title: Task 3 legacy formal snapshot migration 回帰試験報告
description: malformed packet、既存 snapshot の冪等性、semantic conflict の fail-closed 回帰試験結果。
key: legacy-formal-snapshot-migration-task-3-report
---

# Task 3 回帰試験報告

## 変更範囲

- `src/features/radar/infrastructure/persistence.rs` の inline test
  - 正常 packet と壊れた dated packet が共存する場合、migration が失敗し、`snapshots/` と `observation_history_state.json` を公開しないことを実ファイルと Persistence API で確認した。
  - 既存 snapshot の冪等性 test で、2 回目の migration が generated timestamp を含む既存 snapshot JSON を保持し、state の count と cycle を変えないことを確認した。
  - migration key に異なる `market_state` の snapshot が存在する場合、`SNAPSHOT_CONFLICT` を含む error を返し、既存 snapshot と state が変化しないことを確認した。

production logic、runner、workflow、既存 document は変更していない。

## 実行結果

```text
cargo test migrate_legacy_history --lib
```

結果：成功。5 passed、0 failed。

```text
cargo fmt --all -- --check
```

結果：成功。

## 実装上の欠陥

今回の focused regression test では migration API の実装上の欠陥は検出されなかった。

## 残余事項

- AI Cockpit の Make entrypoint resolver は、この repository で canonical lifecycle target を検出できず fail-closed になった。そのため Contract 専用の Make gate は実行していない。
- 指定範囲に従い、focused migration test と format check 以外の suite は実行していない。

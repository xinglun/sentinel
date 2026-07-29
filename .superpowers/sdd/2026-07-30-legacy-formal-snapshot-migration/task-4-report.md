---
author: Ray
title: Task 4 実施報告
description: legacy packet から formal snapshot への起動時移行統合の実施証跡。
key: task-4-legacy-formal-snapshot-migration-report
---

# Task 4 実施報告

## Status

`BLOCKED`

## 完了した修正

- `source_status=degraded` かつ有効取引日の formal snapshot を historical baseline として resolver が受理する条件を追加した。
- `load_observation_history_state` の runner 呼出しは `?` でエラーを伝播し、`unwrap_or(None)` を削除した。
- state が無い場合は、直前の formal snapshot から cycle id を回復する経路を維持した。
- 手動で snapshot を clone し、二回目 state を保存していた疑似二回実行 test は削除した。
- execution gate、threshold、action matrix、trader、position sizing は変更していない。

## 未完了の critical 項目

- migration は formal snapshot と `ObservationHistoryState(count=N)` を保存するが、同じ N 件の observation timeline/history entry を統一 persistence API で保存していない。初回 runner は `history_state_would_regress` の `timeline entries=0 / state.count=N` 比較で `HISTORY_REGRESSION` になる。
- 実 pipeline runner を temporary reports directory に対して異なる market date で二回呼ぶ回帰 test は未実装である。既存 `src/cli.rs` に実 runner test harness はあるが、runner の日付注入入口を追加して二回実行を証明する作業は完了していない。

上記二項目が未完了のため、Task 4 全体を完了または ready と報告しない。

## Verification

- `make fmt-check`: 未実行。ユーザー指示により長時間検証を直ちに停止した。
- `make test`: 未実行。ユーザー指示により長時間検証を直ちに停止した。
- `make clippy`: 未実行。ユーザー指示により長時間検証を直ちに停止した。

実行していない gate を passed と記録しない。

## 共有作業区

`.github/workflows/daily_radar.yml`、`docs/superpowers/plans/`、`docs/superpowers/specs/` の前序 approved changes は保持し、本 Task 4 commit には含めない。

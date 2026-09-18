---
author: Ray
title: DAY_ONLY 観測精度 実装計画
description: Signal Context の日付精度観測を構造化し、時刻推測なしで監査と表示に反映する。
key: day-only-observation-precision-plan
---

# DAY_ONLY 観測精度 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `observed_at` の精度を `TIMESTAMP`、`DAY_ONLY`、`UNAVAILABLE` として保持し、日付だけの観測に時刻を推測しない。

**Architecture:** Research interface に精度 enum と決定的 classifier を置き、Radar coverage が一度だけ正規化する。TemporalBinding は既存の RFC3339 比較を維持し、`DAY_ONLY` と不正値を eligible にせず、read model と監査 JSON に reason と precision を残す。

**Tech Stack:** Rust、Serde、Chrono、Cargo test、AI Cockpit Runtime。

**Spec:** `docs/superpowers/plans/2026-09-18-signal-context-presentation-observation.md` の 4b。

## Global Constraints

- `DAY_ONLY` に RFC3339 の時刻を補完しない。
- TemporalBinding の `source_published_at <= observed_at` 不変条件と fail-closed を変更しない。
- Decision、Gate、RS、Leadership、Action Matrix、Execution、Trader、Position Sizing は変更しない。
- `decision_weight=0`、`trade_signal=false` と evidence provenance を維持する。

### Task 1: 精度 classifier の RED/GREEN

**Files:**
- Modify: `src/features/research/interface/macro_event_observation.rs`
- Test: 同ファイルの focused tests

- [ ] `YYYY-MM-DD`、RFC3339、empty、malformed の4ケースが enum へ決定的に写る失敗テストを書く。
- [ ] テストが classifier 未実装を理由に失敗することを確認する。
- [ ] `ObservationTimePrecision` と `classify_observation_time_precision` を最小実装する。
- [ ] focused test を再実行し、4ケースが通ることを確認する。

### Task 2: read model と temporal binding の精度保持

**Files:**
- Modify: `src/features/research/interface/macro_event_observation.rs`
- Modify: `src/features/radar/interface/signal_context_coverage.rs`
- Test: 同ファイルの focused tests

- [ ] `MarketReaction` と `TemporalBinding` に `observation_time_precision` を追加する。
- [ ] normalization が precision を一度だけ付与し、日付だけを時刻へ変換しない失敗テストを書く。
- [ ] `DAY_ONLY` と `UNAVAILABLE` が temporal eligible にならないことをテストする。
- [ ] serialization に `DAY_ONLY` が残ることを確認する。

### Task 3: presentation/audit 回帰と品質検証

**Files:**
- Modify: `src/features/radar/interface/signal_context_read_model.rs`
- Modify: `src/features/radar/interface/presentation.rs`
- Test: `src/features/radar/interface/signal_context_read_model.rs`、`src/features/radar/interface/presentation.rs`

- [ ] eligible な RFC3339 observation の表示に `precision=TIMESTAMP` が含まれる回帰を書く。
- [ ] date-only observation は表示上の eligible reaction に昇格せず、V1/audit に `DAY_ONLY` と reason を残す回帰を書く。
- [ ] `make fmt-check`、`make test`、`make clippy`、`make check-signal-context-consistency`、`make quality` を実行し、結果を Work Item evidence に保存する。
- [ ] Work Item を verify、finish、archive、close し、PR merge 後に branch/worktree を削除する。

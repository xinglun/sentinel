---
author: Ray
title: 観測履歴の跨日永続化修復 実装計画
description: Daily Radar の reports persistence boundary、cycle_id、formal snapshot の跨日追加を修復する計画。
key: repair-observation-history-persistence
---

# 観測履歴の跨日永続化修復 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 同じ reports/ persistence boundary を fresh process と連続 workflow run で再利用し、正式観測履歴を市場日単位で追加できるようにする。

**Architecture:** まず persistence loader/writer と pipeline の実際のファイル境界を cross-run test で固定する。次に failing test が示す最小箇所だけを修正し、workflow の restore/mirror 契約を shell integration test で確認する。履歴が無い場合の safe downgrade は保持する。

**Tech Stack:** Rust、serde_json、既存 cargo test、GitHub Actions YAML、repository の make gate。

## Global Constraints

- Gate、execution、trader、action matrix、position sizing、NO TRADE の意味論を変更しない。
- 履歴が利用できない場合は unavailable/degraded へ降格し、跨日変化を捏造しない。
- data branch の既存履歴を削除・再初期化しない。
- 検証入口は既存の `make` target を使用する。

### Task 1: Persistence boundary の failing test を固定

**Files:**
- Modify: `src/cli.rs`
- Test: `src/cli.rs`

- [x] 同じ temp reports directory で異なる二つの market date を別 pipeline run として実行し、二回目の state count、last date、cycle_id、formal snapshot 数を検証する回帰 assertion を fresh `PersistenceLayer` で固定する。
- [x] `make test-radar-cross-run-pipeline` で、既存の Rust persistence が跨进程条件でも通過することを確認する。
- [x] 远端 data branch と workflow を調査し、断絶点が restore failure 後の空 reports/ 継続であることを特定する。

### Task 2: 最小の persistence 修正を実装

**Files:**
- Inspect: `src/features/radar/infrastructure/persistence.rs`
- Inspect: `src/features/radar/interface/radar_pipeline_runner.rs`
- Modify: `src/cli.rs`

- [x] 調査で Rust の同一 reports/ 境界と cycle_id 初期化は fresh `PersistenceLayer` でも正常に追加できることを確認し、production persistence 実装は変更しない。
- [x] 実際の断絶点である workflow の restore failure が空 reports/ へ続行する経路を修正する。
- [x] 既存の same-day rerun、corrupt state、full-fetch failure のテストを維持する。
- [x] focused test を再実行し、二日目の state count が増え、cycle_id が同一であることを確認する。

### Task 3: Workflow restore/mirror 契約を固定

**Files:**
- Modify: `.github/workflows/daily_radar.yml`
- Test: `tests/daily_radar_workflow_integration.rs`

- [x] restore source、runtime reports path、mirror destination が同じ persistence boundary を参照することを検証する YAML 契約テストを追加または強化する。
- [x] data branch の履歴を削除・再初期化せず、新 market date の追加を検査する既存 workflow gate を維持する。
- [x] workflow の restore failure fail-closed 修正を実施し、local Rust tests と YAML integration test を通す。
- [x] weekly backtest の restore/bootstrap も同じ data branch fail-closed 契約へ揃える。
- [x] Daily Radar の commit/bootstrap 入口も remote lookup の不確定状態で停止する契約へ揃える。
- [x] Daily/Weekly の data branch writer を共通 concurrency group で直列化する。
- [x] Daily workflow の跨日検証に formal snapshot 数だけでなく observation_history_state.count の増加も含める。
- [x] Daily の push 後に remote data branch を再取得し、state count と cycle_id の永続化結果を検証する。
- [x] ls-remote と fetch の間に空 data branch が作成される競合でも fail-closed になることを固定する。
- [x] weekly backtest が履歴を持つ場合も push 後の remote state count/cycle_id を検証し、初回 bootstrap は明示的に skip する。

### Task 4: Required verification と Cockpit summary を更新

**Files:**
- Modify: `.ai/work-items/active/repair-observation-history-persistence.summary.json`
- Modify: `.ai/cockpit/current_status.md`

- [x] focused test、fmt、full test、clippy と全 Contract checks を make 経由で実行する。
- [x] 各 checkpoint の Contract hash、scenario coverage、residual risk、review focus を Summary に記録する。
- [x] hosted 連続市場日だけを residual risk として残し、ready_with_risks として Summary に記録する。

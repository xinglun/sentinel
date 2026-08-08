---
author: Ray
title: Price-Volume Functional Completion 実装計画
description: S-28 の三つの未充足を test-first で修復する実装計画。
key: price-volume-functional-completion-plan
---

# Price-Volume Functional Completion 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Price-Volume Structure が runtime の TIME_COST_RISING、期限付き Supply Event、price behavior confirmation を観測専用で利用する。

**Architecture:** domain classifier は既存 metrics だけで confirmation を導出し、runner は market observation と date-aware supply context を input に投影する。report と decision pipeline の契約は変更しない。

**Tech Stack:** Rust、chrono、既存 `make` quality gate。

## Global Constraints

- Decision Weight は 0%、trade signal は false のままにする。
- 米国祝日 calendar は追加しない。
- Supply Event 有効期間は event_date の前後 20 calendar days とする。

### Task 1: Failing behavior tests

**Files:**
- Modify: `src/features/radar/domain/price_volume_structure.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`

- [ ] 期限内・期限外 Supply Event、TIME_COST_RISING、price behavior confirmation の failing test を追加する。
- [ ] `make test` を実行し、未実装の期待値で失敗することを確認する。

### Task 2: Minimal observation-only implementation

**Files:**
- Modify: `src/features/radar/domain/price_volume_structure.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`

- [ ] Supply Event の date window helper と runner の TIME_COST_RISING 投影を実装する。
- [ ] 既存 metrics を四分類の confirmation predicate に接続する。
- [ ] `make test` を実行し、追加した behavior test と既存 test の成功を確認する。

### Task 3: Cockpit verification and delivery

**Files:**
- Modify: `.ai/work-items/active/s-28-14-price-volume-functional-completion.summary.json`
- Modify: `.ai/cockpit/current_status.md`

- [ ] Contract / scope / guard / summary / status checks と Rust quality checks を `make` 経由で実行する。
- [ ] Summary に実測 verification、残余 risk、review focus を記録する。
- [ ] `make ai-finish TASK=s-28-14-price-volume-functional-completion` を実行して archive する。

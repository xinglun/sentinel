---
author: Ray
title: 基線比較契約の実装計画
description: Breadth の raw 値と semantic classification を formal snapshot で整合させる実装手順。
key: baseline-comparison-contract-plan
---

# Baseline Comparison Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Change Log、Change Driver、Evolution が同一 formal snapshot の breadth 事実を使用し、分類比較の欠損を虚偽の変更に変えない。

**Architecture:** `TradingDaySnapshot` に optional semantic classification を保存し、runner は previous/current の raw 値と classification を別々に組み立てる。`MarketChangeSnapshot` は optional classification を比較し、両日で利用可能かつ異なる場合だけ MODERATE driver を生成する。

**Tech Stack:** Rust、serde、cargo test、Cargo fmt、Clippy、Sentinel AI Cockpit。

## Global Constraints

- Gate、Execution、Trader、Action Matrix、Position Sizing を変更しない。
- snapshot field は `#[serde(default)]` により旧 JSON を読み込める optional field とする。
- previous classification が欠損した時は Breadth driver を作らず、baseline 全体を unavailable にしない。
- 検証 command は `make ...` 経由だけを使用する。

---

## File Structure

- `src/features/radar/domain/market_change_driver.rs`: classification の可用性を持つ Change Driver input と分類比較規則。
- `src/features/radar/infrastructure/persistence.rs`: formal snapshot の optional classification と旧 snapshot 互換性。
- `src/features/radar/interface/radar_pipeline_runner.rs`: current/previous comparison context、保存、Change Log の explanation。
- `docs/superpowers/specs/2026-08-04-baseline-comparison-contract-design.md`: 発見した semantic classification の保存境界を反映。
- `tests/audit_daily_cli_integration.rs`: CLI 経路が必要になった場合だけ追加する end-to-end evidence。

### Task 1: Change Driver の分類欠損境界

**Files:**

- Modify: `src/features/radar/domain/market_change_driver.rs`

**Interfaces:**

- Consumes: `MarketChangeSnapshot` の `breadth_classification: Option<String>`。
- Produces: 両 side が `Some` で内容が異なる場合だけ `breadth_classification` を `change_drivers` に含める `build_market_change_driver`。

- [ ] **Step 1: 失敗する domain regression test を書く**

```rust
#[test]
fn missing_previous_breadth_classification_does_not_create_a_driver() {
    let mut previous = snapshot();
    previous.breadth_classification = None;
    let current = snapshot();

    let change = build_market_change_driver(&previous, &current);

    assert!(!change.change_drivers.contains(&"breadth_classification".to_string()));
    assert!(!change.unchanged_dimensions.contains(&"breadth_classification".to_string()));
}
```

- [ ] **Step 2: RED を確認する**

Run: `make test`

Expected: `MarketChangeSnapshot` の field 型が `String` のため、`None` を代入できず compile failure になる。

- [ ] **Step 3: 最小の domain 実装を追加する**

```rust
pub struct MarketChangeSnapshot {
    pub breadth_classification: Option<String>,
    // 既存 field
}

if let (Some(previous), Some(current)) = (
    previous.breadth_classification.as_deref(),
    current.breadth_classification.as_deref(),
) {
    compare_dimension(&mut moderate, &mut unchanged, "breadth_classification", previous, current);
}
```

- [ ] **Step 4: GREEN を確認する**

Run: `make test`

Expected: 新規 regression と既存 Change Driver tests が通過する。

- [ ] **Step 5: 真の分類変更 regression を追加する**

```rust
#[test]
fn persisted_breadth_classification_change_is_moderate() {
    let previous = snapshot();
    let mut current = previous.clone();
    current.breadth_classification = Some("Very Narrow".to_string());

    let change = build_market_change_driver(&previous, &current);

    assert_eq!(change.change_level, ChangeLevel::Moderate);
    assert_eq!(change.change_drivers, vec!["breadth_classification"]);
}
```

- [ ] **Step 6: Commit**

```bash
git add src/features/radar/domain/market_change_driver.rs
git commit -m "fix: Breadth分類欠損で偽の変化を防止"
```

### Task 2: Formal snapshot の semantic classification 保存

**Files:**

- Modify: `src/features/radar/infrastructure/persistence.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`

**Interfaces:**

- Consumes: 当日の `signal_summary.breadth_semantic_value`。
- Produces: `TradingDaySnapshot.breadth_classification: Option<String>`。

- [ ] **Step 1: 失敗する persistence regression test を書く**

```rust
#[test]
fn formal_snapshot_deserializes_without_breadth_classification() {
    let snapshot: TradingDaySnapshot = serde_json::from_value(serde_json::json!({
        // 既存必須 field を含め、breadth_classification は省略する。
    })).unwrap();

    assert_eq!(snapshot.breadth_classification, None);
}
```

- [ ] **Step 2: RED を確認する**

Run: `make test`

Expected: field 不在の JSON を読めない、または field が未定義であることによる失敗。

- [ ] **Step 3: 後方互換な field を実装する**

```rust
#[serde(default)]
pub breadth_classification: Option<String>,
```

`TradingDaySnapshot` のすべての test fixture と legacy projection には `None` を設定する。runner が新規 snapshot を保存する時だけ `Some(pres_packet.signal_summary.breadth_semantic_value.clone())` を設定する。

- [ ] **Step 4: GREEN を確認する**

Run: `make test`

Expected: legacy snapshot read と新規 formal snapshot save/load regression が通過する。

- [ ] **Step 5: Commit**

```bash
git add src/features/radar/infrastructure/persistence.rs src/features/radar/interface/radar_pipeline_runner.rs
git commit -m "fix: 基線のBreadth分類を保存"
```

### Task 3: Change Log を同一 snapshot 比較へ接続

**Files:**

- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`
- Test: `src/features/radar/interface/radar_pipeline_runner.rs`

**Interfaces:**

- Consumes: `TradingDaySnapshot.breadth` と `TradingDaySnapshot.breadth_classification`、当日の semantic classification。
- Produces: raw series と classification explanation が同じ previous snapshot を指す `MarketChangeLogViewModel`。

- [ ] **Step 1: 失敗する S-18 regression test を書く**

```rust
#[test]
fn equal_persisted_breadth_classification_does_not_create_moderate_change() {
    let baseline = formal_snapshot_with_breadth(35.0, Some("Very Narrow"));
    let log = build_market_change_log_for_test(baseline, 35.0, "Very Narrow");

    assert!(!log.change_drivers.contains(&"breadth_classification".to_string()));
    assert!(!log.summary_values.iter().any(|line| line.contains("35.0 to Very Narrow")));
}
```

- [ ] **Step 2: RED を確認する**

Run: `make test`

Expected: 現在の runner は `35.0` と `Very Narrow` を比較するため assertion が失敗する。

- [ ] **Step 3: 最小の runner 実装を追加する**

```rust
let current_breadth_classification = Some(pres_packet.signal_summary.breadth_semantic_value.clone());
let previous_breadth_classification = formal_baseline
    .and_then(|snapshot| snapshot.breadth_classification.clone());
```

`MarketChangeSnapshot` へ Option を渡し、summary は両 classification がある場合だけ classification-to-classification を表示する。previous classification がない場合は comparison unavailable を明示し、raw 値と分類を同じ文として比較しない。

- [ ] **Step 4: GREEN を確認する**

Run: `make test`

Expected: S-18 regression が通過し、legacy snapshot は fake driver を生成しない。

- [ ] **Step 5: 文言 regression を追加する**

```rust
assert!(log.summary_values.iter().any(|line| line.contains("Breadth remains Very Narrow.")));
assert!(!log.summary_values.iter().any(|line| line.contains("Breadth shifted from 35.0 to Very Narrow.")));
```

- [ ] **Step 6: Commit**

```bash
git add src/features/radar/interface/radar_pipeline_runner.rs
git commit -m "fix: Change LogのBreadth比較口径を統一"
```

### Task 4: Cockpit evidence と全量検証

**Files:**

- Modify: `.ai/work-items/active/baseline-comparison-contract.summary.json`
- Modify: `.ai/cockpit/current_status.md`

- [ ] **Step 1: scenario coverage を実証箇所へ更新する**

`35.0 -> 35.0`、真の classification change、legacy field absence、baseline unavailable、i18n report の evidence を Summary に記録する。

- [ ] **Step 2: Contract の required checks を実行する**

Run: `make fmt-check && make test && make clippy`

Expected: exit code 0。

- [ ] **Step 3: Cockpit checks を実行する**

Run: `make check-ai-contract CONTRACT=.ai/work-items/active/baseline-comparison-contract.contract.json && make check-ai-scope CONTRACT=.ai/work-items/active/baseline-comparison-contract.contract.json && make check-ai-guards CONTRACT=.ai/work-items/active/baseline-comparison-contract.contract.json && make check-ai-backtrack CONTRACT=.ai/work-items/active/baseline-comparison-contract.contract.json SUMMARY=.ai/work-items/active/baseline-comparison-contract.summary.json && make check-ai-coverage-guard && make check-ai-scenario-coverage CONTRACT=.ai/work-items/active/baseline-comparison-contract.contract.json SUMMARY=.ai/work-items/active/baseline-comparison-contract.summary.json && make check-ai-change-summary SUMMARY=.ai/work-items/active/baseline-comparison-contract.summary.json CONTRACT=.ai/work-items/active/baseline-comparison-contract.contract.json`

Expected: exit code 0。

- [ ] **Step 4: status を生成・検証する**

Run: `make generate-cockpit-status CONTRACT=.ai/work-items/active/baseline-comparison-contract.contract.json SUMMARY=.ai/work-items/active/baseline-comparison-contract.summary.json && make check-ai-status CONTRACT=.ai/work-items/active/baseline-comparison-contract.contract.json SUMMARY=.ai/work-items/active/baseline-comparison-contract.summary.json`

Expected: exit code 0。

- [ ] **Step 5: Finish と archive を実行する**

Run: `make ai-finish TASK=baseline-comparison-contract`

Expected: required checks 成功時だけ archive が作成される。

## Self-Review

- S-18 と S-11 は Task 1--3 に、旧 snapshot 互換性は Task 2--3 に対応する。
- S-03、S-19、PR cleanup はこの Work Item の非目標であり、次の Work Item に残す。
- placeholder、未決定実装、裸 command は含めない。

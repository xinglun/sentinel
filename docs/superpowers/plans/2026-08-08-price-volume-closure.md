---
author: Ray
title: Price-Volume Closure 実装計画
description: S-28 の監査で判明した供給 evidence、データ品質、runtime acceptance を TDD で閉じる計画。
key: price-volume-closure-plan
---

# Price-Volume Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Price-Volume Structure の observation-only 境界を維持し、供給 evidence と品質 fail-closed を runtime まで検証可能にする。

**Architecture:** Domain は完全な OHLCV と high-confidence supply evidence だけを分類する。Interface は symbol-local context を渡し、構造化 supply context を report と JSONL に投影する。Persistence は既存 JSONL を serde default で読み、同日 rerun は一 record に upsert する。

**Tech Stack:** Rust、serde、chrono、Cargo test、Make、AI Cockpit。

## Global Constraints

- Observation only: decision weight 0%、trade signal false、すべての decision effect none。
- Gate、execution、trader、action matrix、position sizing を変更しない。
- 供給 event は明示 fact のみで、institutional buying を確認しない。
- 米国祝日 calendar を追加しない。

---

### Task 1: 供給 evidence と JSONL schema

**Files:**
- Modify: `src/features/shared/domain/supply_event_context.rs`
- Modify: `src/features/radar/infrastructure/persistence.rs`
- Modify: `src/features/radar/interface/price_volume_structure_report.rs`

- [ ] **Step 1: Write the failing tests**

```rust
assert_eq!(record.supply_context.as_ref().unwrap().confidence, SupplyEventConfidence::High);
assert!(report.contains("Supply Confidence: HIGH"));
```

- [ ] **Step 2: Run the focused tests and confirm the assertions fail because context is string-only or absent from report.**

Run: `make test`

- [ ] **Step 3: Implement the minimal structured context projection and serde-compatible record field.**

- [ ] **Step 4: Run the focused tests and confirm they pass.**

Run: `make test`

### Task 2: fail-closed quality and supply priority

**Files:**
- Modify: `src/features/radar/domain/price_volume_structure.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`

- [ ] **Step 1: Write failing tests for a partial volume history, missing OHLC, 20 bars, incomplete ATR, low-confidence supply, and same-symbol multiple events.**

```rust
assert_eq!(assessment.structure, PriceVolumeStructure::Unavailable);
assert_eq!(assessment.supply_absorption, SupplyAbsorption::None);
```

- [ ] **Step 2: Run the focused tests and confirm the prior classifier continues or selects the first configured event.**

Run: `make test`

- [ ] **Step 3: Require complete 21-bar OHLCV and 14 true ranges; select a deterministic high-confidence increasing supply event.**

- [ ] **Step 4: Run the focused tests and confirm they pass.**

Run: `make test`

### Task 3: symbol-local runtime acceptance

**Files:**
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`
- Modify: `src/cli.rs`
- Modify: `config.toml`

- [ ] **Step 1: Write failing runtime tests for SpaceX and Microsoft that assert Markdown, JSONL evidence, and the 0% boundary.**

```rust
assert!(report.contains("Structure: ACCUMULATION"));
assert!(report.contains("Structure: EXHAUSTED_ADVANCE"));
assert!(!report.contains("Sell immediately"));
```

- [ ] **Step 2: Run the focused tests and confirm runtime does not yet retain all supply evidence or applies global time cost.**

Run: `make test`

- [ ] **Step 3: Remove global time-cost transfer, wire symbol-local context, and add an audit-only configuration example.**

- [ ] **Step 4: Run the focused tests and confirm they pass.**

Run: `make test`

### Task 4: governance completion

**Files:**
- Modify: `.ai/work-items/active/s-28-16-price-volume-closure.contract.json`
- Modify: `.ai/work-items/active/s-28-16-price-volume-closure.summary.json`

- [ ] **Step 1: Record changed files, TDD evidence, scenario coverage, output surfaces, residual risks, and user-correction solidification.**

- [ ] **Step 2: Run all required Make verification commands and archive only after they pass.**

Run: `make ai-finish TASK=s-28-16-price-volume-closure`

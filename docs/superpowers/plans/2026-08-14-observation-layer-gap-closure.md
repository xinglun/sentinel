# Observation Layer Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Observation Layer の受入れ不足を補完し、観測事実と不可用理由を runtime/report/snapshot に一貫して残す。

**Architecture:** 既存の radar read model、report formatter、snapshot persistence、coverage builder を最小変更する。新しい取引判断経路や外部 provider は追加せず、履歴 replay は既存 CLI/pipeline を Make target で固定する。

**Tech Stack:** Rust、Serde JSON、既存の Makefile、組み込み unit/integration tests。

## Global Constraints

- Gate、execution、Action Matrix、Trader、Position Sizing、automatic trading、既存閾値を変更しない。
- Supply Context 不可用は `UNAVAILABLE` と機械可読 reason を出す。
- Breadth raw 値と classification score を別フィールドとして扱う。
- 取得できない coverage は healthy として補完しない。
- 新しい運用入口は `make` target 経由にする。

### Task 1: Contract と replay 境界を固定する

**Files:**
- Modify: `.ai/work-items/active/observation-layer-gap-closure.contract.json`
- Modify: `.ai/work-items/active/observation-layer-gap-closure.summary.json`
- Create: `Makefile` target `ai-observation-replay`

- [ ] **Step 1: 既存の履歴 replay CLI と日付引数を確認する**
- [ ] **Step 2: 指定日を保持する最小 Make target を追加する**
- [ ] **Step 3: Payroll/CPI/PPI の三日分を実行して report 日付を確認する**
- [ ] **Step 4: 実行証拠を Summary の scenarioCoverage に記録する**

### Task 2: Supply Context と RVOL の report 契約を補完する

**Files:**
- Modify: `src/features/radar/interface/price_volume_structure_report.rs`

- [ ] **Step 1: unavailable status/reason と baseline/sessions の failing test を追加する**
- [ ] **Step 2: Markdown/HTML 共通 formatter を最小変更する**
- [ ] **Step 3: primary/secondary baseline と unavailable reason の test を通す**

### Task 3: Breadth の構造化保存を補完する

**Files:**
- Modify: `src/features/radar/infrastructure/persistence.rs`
- Modify: `src/features/radar/domain/observation_timeline.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`
- Modify: `src/features/radar/interface/report.rs`

- [ ] **Step 1: raw/counts/universe と timeline raw 値の failing test を追加する**
- [ ] **Step 2: `serde(default)` 付きフィールドを snapshot に追加する**
- [ ] **Step 3: pipeline から実値を渡し、旧 JSON の読み取りを維持する**
- [ ] **Step 4: timeline/report の分類 score と raw 値を分離する**

### Task 4: 六分類 runtime coverage の欠落を補完する

**Files:**
- Modify: `src/features/radar/interface/signal_context_coverage.rs`
- Modify: `src/features/radar/interface/signal_context_read_model.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`

- [ ] **Step 1: 六分類の unavailable/partial 表示を固定する test を追加する**
- [ ] **Step 2: runtime builder が全分類を初期化するよう修正する**
- [ ] **Step 3: incomplete coverage の no-event 表現を確認する**

### Task 5: 全検証と WI 完了処理

**Files:**
- Modify: `.ai/work-items/active/observation-layer-gap-closure.summary.json`

- [ ] **Step 1: `make fmt-check`、`make test`、`make clippy` を実行する**
- [ ] **Step 2: Cockpit required checks と `make quality` を実行する**
- [ ] **Step 3: diff が outOfScope に入っていないことを確認する**
- [ ] **Step 4: residual risk と review focus を Summary に記録する**
- [ ] **Step 5: `make ai-finish TASK=observation-layer-gap-closure` を実行する**

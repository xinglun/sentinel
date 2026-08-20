---
author: Ray
title: Effective Action 再構成実装計画
description: 市場権限、銘柄資格、ポートフォリオ状態を統合して日报の実際の行動表示を整合させる実装計画。
key: effective-action-reconciliation-plan
---

# Effective Action 再構成実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Market Permission、Asset Eligibility、Portfolio State を区別し、実際に実行可能な銘柄がない場合に日报が `NO_NEW_ENTRY` と `0%` を表示する。

**Architecture:** 既存の Gate/Execution/Action Matrix の値は変更せず、`PresentationPacket` の組み立て時に既存の `ADD` intent と非保有 fact から実行資格数を集計する。`FinalExecutionDecision` は市場許可上限と実効上限を分離し、report は減倉 signal と実ポートフォリオ action を別ブロックとして描画する。候補順位・候補ラベルの qualification semantics は後続の observation-semantic-guard Work Item で扱う。

**Tech Stack:** Rust、Serde view model、Markdown/Telegram HTML renderer、Cargo unit/integration tests、Sentinel Make/Cockpit checks。

**Spec:** `docs/superpowers/specs/2026-08-18-observation-semantic-reconciliation-design.md` とユーザー提供の Conditional FAIL 受入結果。

## Global Constraints

- Gate、Execution、Trader、Action Matrix、Position Sizing、Trade Signal の判定と自動取引動作は変更しない。
- `10-30%`、`20-40%`、`30-70%` は市場許可の上限として保持し、実行資格がない場合の実効新規上限は `0%` とする。
- 候補順位・候補ラベルは本 Work Item の変更対象外とし、後続 Work Item で実効資格と表示を固定する。
- REDUCE/EXIT は個別 signal と実保有 portfolio action を分離する。
- zh/en/ja の Markdown、Telegram HTML、archival Markdown は同じ意味を出力する。
- 手書き code comment と repository document 本文は日本語、identifier は英語とする。

### Task 1: 実行資格 0 件の failing-first regression

**Files:**
- Modify: `src/features/radar/interface/presentation_tests.rs`
- Modify: `src/features/radar/interface/report_ui_tests.rs`

**Interfaces:**
- Consumes: existing `PresentationAssembler::assemble` and report fixture builders.
- Produces: failing tests proving `Probe` permission with zero eligible assets yields `NO_NEW_ENTRY`, permission budget remains visible, and Markdown/HTML outputs agree.

- [ ] **Step 1: Write the failing presentation test**

  Add a fixture with `trend_cohesion.gate_passed=true`, `MarketState::IGNITION`, no non-held `DisplayIntent::ADD` assets, and assert the assembled final execution decision retains the permission window/cap while exposing zero eligible assets and an effective `0%` cap.

- [ ] **Step 2: Run the focused test and verify the expected failure**

  Run: `cargo test --lib features::radar::interface::presentation_tests -- --nocapture`

  Expected: the new assertion fails because the current assembler marks the market Probe window executable and uses `10-30%` as the actual position range.

- [ ] **Step 3: Write the failing report-level test**

  Add a zh/en/ja report assertion that checks separate market permission, eligible-asset count, effective action, permission budget, and effective new-entry cap in both Markdown and Telegram HTML.

- [ ] **Step 4: Run the focused report test and verify the expected failure**

  Run: `cargo test --lib features::radar::interface::report_ui_tests -- --nocapture`

  Expected: the new labels are absent or still show `10-30%` as the actionable cap.

### Task 2: Implement Effective Action projection

**Files:**
- Modify: `src/features/radar/interface/presentation.rs`
- Modify: `src/features/radar/interface/presentation_assembler.rs`
- Modify: `src/features/radar/interface/report.rs`
- Modify: `src/features/shared/interface/i18n.rs`

**Interfaces:**
- Consumes: `DisplayIntent::ADD`, `DisplayContext::has_position`, existing market execution window and entry cap.
- Produces: `FinalExecutionDecision.permission_position_range`, `eligible_asset_count`, and effective `position_range` used by all report delivery bodies.

- [ ] **Step 1: Add the minimal view-model fields and localized labels**

  Extend `FinalExecutionDecision` with the market permission cap and eligible-asset count. Add localized labels for market permission, eligible assets, effective action, permission budget, effective cap, signal, and actual portfolio action.

- [ ] **Step 2: Compute eligible assets without changing domain decisions**

  Count only assets with `DisplayIntent::ADD` and `DisplayContext::has_position == false`. Pass the count into `build_final_execution_decision` and leave the original `DecisionPacket` and Action Matrix untouched.

- [ ] **Step 3: Reconcile market permission and effective action**

  Keep the existing market execution window and configured cap as permission data. When the eligible count is zero, set the effective actionability to candidate-only, set the effective position range to `0%`, and render a localized no-new-entry reason. When the count is positive, preserve the current Probe/Add effective range.

- [ ] **Step 4: Render the separated fields in Markdown and Telegram HTML**

  Render market permission, eligible asset count, effective action, permission budget, and effective cap from the same `FinalExecutionDecision` in both report modes. Do not infer actionability from a translated string.

- [ ] **Step 5: Run the focused tests and verify they pass**

  Run: `cargo test --lib features::radar::interface::presentation_tests -- --nocapture` and `cargo test --lib features::radar::interface::report_ui_tests -- --nocapture`

  Expected: the new zero-eligibility and multi-language report tests pass.

### Task 3: Separate reduction signals from portfolio actions

**Files:**
- Modify: `src/features/radar/interface/presentation.rs`
- Modify: `src/features/radar/interface/presentation_assembler.rs`
- Modify: `src/features/radar/interface/report.rs`
- Modify: `src/features/shared/interface/i18n.rs`
- Modify: `src/features/radar/interface/presentation_tests.rs`
- Modify: `src/features/radar/interface/report_ui_tests.rs`

**Interfaces:**
- Consumes: existing asset exit state, unified intent, and portfolio position facts.
- Produces: separate signal items and actual position action items in `ExitDecisionSummaryViewModel` and report output.

- [ ] **Step 1: Write the failing signal/action separation test**

  Add one fixture with REDUCE/EXIT signals for symbols absent from the portfolio and assert the report says the signal is triggered while actual trim/exit action is none. Add a second fixture with the same signal and a held symbol and assert the actual action remains visible.

- [ ] **Step 2: Run the focused tests and verify the expected failure**

  Run: `cargo test --lib features::radar::interface::presentation_tests -- --nocapture` and `cargo test --lib features::radar::interface::report_ui_tests -- --nocapture`

  Expected: the current report either emits non-held risk rows as actions or says no trigger while the risk summary contains structural reduction text.

- [ ] **Step 3: Add explicit signal and actual-action collections**

  Keep `exit_summary.items` portfolio-gated for actual actions. Add a separate signal collection built from the asset exit/action facts without treating non-held symbols as positions.

- [ ] **Step 4: Render both collections with explicit localized headings**

  Render `Reduction Signal` and `Actual Portfolio Action` as separate blocks. For a non-held symbol, render the signal only and an explicit no-action reason; for a held symbol, render the actual TRIM/EXIT item.

- [ ] **Step 5: Run the focused tests and verify they pass**

  Run: `cargo test --lib features::radar::interface::presentation_tests -- --nocapture` and `cargo test --lib features::radar::interface::report_ui_tests -- --nocapture`

  Expected: signal/action separation is stable in zh/en/ja Markdown and HTML output.

### Task 4: Full verification and Cockpit evidence

**Files:**
- Modify: `.ai/work-items/active/effective-action-reconciliation.summary.json`
- Modify: `scripts/ai_check_reference_impact.py`
- Add: `scripts/test_ai_check_reference_impact.py`

**Interfaces:**
- Consumes: implementation diff, focused regression output, and repository Make targets.
- Produces: complete checkpoint evidence, scenario coverage, residual risks, and archived Work Item.

- [ ] **Step 1: Update Summary with changed files and focused evidence**

  Record the exact test names, output surfaces, unchanged decision boundaries, and any residual review focus.

- [ ] **Step 2: Run all required Make checks**

  Run every command listed in the Contract, including `make fmt-check`, `make test`, `make clippy`, `make check-architecture-all`, and `make quality`.

- [ ] **Step 3: Record before-ready and after-verification checkpoints**

  Run: `make ai-checkpoint CONTRACT=.ai/work-items/active/effective-action-reconciliation.contract.json SUMMARY=.ai/work-items/active/effective-action-reconciliation.summary.json STAGE=before_ready` and then `STAGE=after_verification` after every required check passes.

- [ ] **Step 4: Finish and archive the Work Item**

  Run: `make ai-finish TASK=effective-action-reconciliation`

  Expected: required checks pass and Contract/Summary move to `.ai/work-items/archive/2026/`.

- [ ] **Step 5: Commit with Japanese Conventional Commit subject**

  Use: `git commit -m "fix: Effective Action の日报表示を整合"`

  Expected: only Contract scope files are committed and the branch is ready to merge into `develop`.

## Self-Review

- Scope covers only presentation/read-model/report/i18n/tests and Work Item evidence; domain and execution code are explicitly out of scope.
- All required acceptance items have a focused scenario or a full-repository guard.
- No new threshold or trade signal is introduced; the change only reconciles existing facts for display.

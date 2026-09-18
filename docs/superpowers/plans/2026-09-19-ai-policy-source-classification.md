---
author: Ray
title: AI Policy source classification 実装計画
description: 既存ニュース provider から AI Policy の type と stage を決定論的に分類し、観測専用 Signal Context へ接続する。
key: ai-policy-source-classification-plan
---

# AI Policy Source Classification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 既存の Finnhub ニュース入力から、完全な provenance を保った AI Policy observation を deterministic かつ fail-closed に生成する。

**Architecture:** Source classification は Research infrastructure の単一 matcher に集約する。provider は同一ニュース payload から geopolitical と AI Policy を分離して抽出し、ACL が canonical observation に変換する。Radar は既存の observation-only pipeline に policy type/stage を渡して表示・監査へ投影するが、primary context、Decision、Gate、Execution には渡さない。

**Tech Stack:** Rust、既存の serde/chrono、Finnhub general news provider、Signal Context v1 read model、Cargo unit tests。

**Spec:** `.ai/work-items/active/ai-policy-source-classification.contract.json`

## Global Constraints

- `decision_weight=0` と `trade_signal=false` を維持する。
- `Decision`、`Gate`、`Primary Context ranking`、`TemporalBinding`、`MarketObservation`、`Execution`、`Position Sizing` は変更しない。
- LLM、大型 NLP、追加 provider API、secrets、config は導入しない。
- provenance の欠損、空値、RFC3339 でない公開時刻、malformed text は no observation とする。
- repository 内の手書きコメントと Markdown 本文は日本語で記述する。

---

### Task 1: Deterministic AI Policy matcher

**Files:**
- Create: `src/features/research/infrastructure/ai_policy_source_classifier.rs`
- Modify: `src/features/research/infrastructure/mod.rs`
- Test: `src/features/research/infrastructure/ai_policy_source_classifier.rs` の unit tests

**Interfaces:**
- Consumes: headline と summary を結合した UTF-8 text。
- Produces: `AiPolicyClassification { policy_type, policy_stage }` または `None`。

- [ ] **Step 1: Write the failing tests**

  `industry leaders call for slowing frontier AI development` は `FRONTIER_PACING/INDUSTRY_PROPOSAL`、`lawmakers introduce bill to pause advanced AI development` は `LEGISLATIVE_PROPOSAL/LEGISLATIVE_PROPOSAL`、`government enacts an advanced AI deployment restriction` は `DEPLOYMENT_RESTRICTION/ENACTED` とする。`AI startup pauses hiring`、空文字、replacement character、control character は `None` とする。

- [ ] **Step 2: Run the focused test and confirm the expected RED failure**

  `cargo test ai_policy_source_classifier --lib` を実行し、classifier がまだ存在しないため feature missing の失敗を確認する。

- [ ] **Step 3: Implement the minimal matcher**

  token boundary を使う正規化、AI subject、policy action、actor/stage context を実装する。`proposal`、`enacted`、`effective` を同一結果に混同させず、policy type が確定できない文は候補にしない。

- [ ] **Step 4: Run the focused test and confirm GREEN**

  `cargo test ai_policy_source_classifier --lib` で全ケースが通ることを確認する。

- [ ] **Step 5: Commit the isolated classifier**

  `git add src/features/research/infrastructure/ai_policy_source_classifier.rs src/features/research/infrastructure/mod.rs && git commit -m "feat: AI Policy分類matcherを追加"`

### Task 2: Existing provider ingestion and provenance mapping

**Files:**
- Modify: `src/features/research/infrastructure/macro_signal_context_provider.rs`
- Modify: `src/features/research/acl/macro_signal_context_provider_factory.rs`
- Modify: `src/features/research/interface/macro_event_observation.rs`
- Test: provider parser and ACL mapping unit tests

**Interfaces:**
- Consumes: 既存 Finnhub general news response と Task 1 の classification。
- Produces: 完全な `AiPolicyFrontierPacingObservation`。policy type/stage を canonical string として保持する。

- [ ] **Step 1: Write failing provider tests**

  valid policy news が source、url、datetime、headline、provider、policy type/stage を保持し、startup hiring、未来日、missing source/url/datetime は no observation になることを追加する。

- [ ] **Step 2: Run provider focused tests and confirm RED**

  `cargo test macro_signal_context_provider --lib` で新しい observation path が未実装のため失敗することを確認する。

- [ ] **Step 3: Implement one-fetch provider path**

  Finnhub payload を一度だけ取得し、既存 geopolitical parser と AI Policy parser を同じ入力から実行する。provider read model と ACL map に observation を追加し、既存 EvidenceRecord の source provenance を変更しない。

- [ ] **Step 4: Extend canonical observation validation and display formatting**

  `AiPolicyFrontierPacingObservation` に policy type/stage を追加し、許可値以外を fail-closed にする。既存 read model formatter は既に分類済みの値だけを `policy_type` と `policy_stage` として出力する。

- [ ] **Step 5: Run provider and interface tests and confirm GREEN**

  `cargo test macro_signal_context_provider --lib` と `cargo test macro_event_observation --lib` を実行する。

- [ ] **Step 6: Commit provider integration**

  `git add src/features/research/infrastructure/macro_signal_context_provider.rs src/features/research/acl/macro_signal_context_provider_factory.rs src/features/research/interface/macro_event_observation.rs && git commit -m "feat: AI Policy observationをproviderから生成"`

### Task 3: Observation-only Radar projection

**Files:**
- Modify: `src/features/radar/interface/signal_context_coverage.rs`
- Modify: `src/features/radar/interface/signal_context_read_model.rs`
- Modify: `src/features/radar/interface/interpretation_read_model.rs`
- Modify: `src/features/radar/interface/weekly_state_report.rs`
- Test: Signal Context coverage, report, weekly audit tests

**Interfaces:**
- Consumes: ACL が返す optional AI Policy observation。
- Produces: Signal Context、Markdown、Telegram HTML、weekly audit の既存 observation-only 表示。

- [ ] **Step 1: Write failing projection tests**

  runtime observation が Signal Context v1 と report/weekly projection に現れ、`decision_weight=0`、`trade_signal=false` が変わらないことを固定する。

- [ ] **Step 2: Run focused projection tests and confirm RED**

  `cargo test signal_context --lib` で runtime field の接続不足を確認する。

- [ ] **Step 3: Implement the minimal field propagation**

  provider observation を snapshot/external observation と同じ fail-closed normalization path へ接続する。classifier 呼び出しや文字列判定は renderer/read model に追加しない。

- [ ] **Step 4: Run focused projection tests and confirm GREEN**

  `cargo test signal_context --lib` と `cargo test report_renders_ai_policy --lib` を実行する。

- [ ] **Step 5: Commit observation-only projection**

  `git add src/features/radar/interface/signal_context_coverage.rs src/features/radar/interface/signal_context_read_model.rs src/features/radar/interface/interpretation_read_model.rs src/features/radar/interface/weekly_state_report.rs && git commit -m "feat: AI Policy観測をSignal Contextへ投影"`

### Task 4: Contract verification and governed delivery

**Files:**
- Modify: `.ai/work-items/active/ai-policy-source-classification.summary.json`
- Modify: `.ai/work-items/active/ai-policy-source-classification.contract.json` only through Runtime amendment when needed

- [ ] **Step 1: Run all Contract checks**

  `cargo test --workspace`、`make fmt-check`、`make test`、`make clippy`、`make check-signal-context-consistency`、`make quality` を実行し、既存 coverage threshold failure はそのまま evidence に記録する。

- [ ] **Step 2: Verify prohibited paths remain unchanged**

  `git diff --name-only <base>...HEAD` と source diff を確認し、Decision/Gate/Execution/Position Sizing/TemporalBinding/MarketObservation の production behavior に変更がないことを記録する。

- [ ] **Step 3: Finish and archive through Runtime**

  Runtime の `verify → finish → archive` を順に実行し、Outcome に facts、unknowns、verification、residual risk を記録する。

- [ ] **Step 4: Open, verify, and merge one PR**

  dedicated branch の hosted checks が exact head に対して通ることを確認してから PR を merge する。

- [ ] **Step 5: Close and clean exact resources**

  `close`、default branch synchronization、remote branch absence、worktree cleanup を確認する。続いて別の governance-only Work Item で #232 の close receipt を `main` に同期する。

## Self-review checklist

- [ ] `proposal != enacted` と `discussion != effective restriction` がテストで固定されている。
- [ ] `AI startup pauses hiring` は observation を生成しない。
- [ ] provider は API を一度だけ呼び、classifier は単一 SSOT である。
- [ ] renderer は分類を実行しない。
- [ ] `decision_weight=0`、Decision、Gate、Position Sizing、TemporalBinding、MarketObservation は不変である。

---
author: Ray
title: FRED 観測日精度整合性 実装計画
description: FRED の日次観測日をレポート日由来の合成 timestamp として扱わず、Signal Context の DAY_ONLY 境界を保持する実装計画。
key: fred-observation-date-precision-integrity
---

# FRED 観測日精度整合性 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** FRED の事実としての observation date と report market_date を分離し、日次データを `DAY_ONLY` のまま Primary Event の market reaction 投影から除外する。

**Architecture:** FRED provider は observation date と precision を自身の source fact として保持する。Research ACL はその precision を interface read model に明示的に写像し、Signal Context normalization は明示済み precision を再分類で上書きしない。Decision、Gate、Execution、Action Matrix、Position Sizing は read-only observation 境界の外へ出さない。

**Tech Stack:** Rust、chrono、serde、Cargo test、既存 Make quality gate。

**Spec:** `.ai/work-items/active/fred-observation-date-precision-integrity.contract.json`

## Global Constraints

- `report_date=2026-09-21` と FRED `latest observation date=2026-09-15` を同一の時刻事実として扱わない。
- FRED 日次観測は `observation_date=2026-09-15`、`DAY_ONLY` とし、`2026-09-21T00:00:00Z` を生成しない。
- `DAY_ONLY` 観測は temporal eligibility と Primary Event reaction renderer に昇格させない。
- provider が確定した precision を ACL または normalization の文字列推論で上書きしない。
- Decision、Gate、Execution、Trader、Action Matrix、Position Sizing、`decision_weight`、`trade_signal` は変更しない。
- 手書き code comment と Markdown 本文は日本語、identifier は英語とする。

## Review Focus

- FRED observation date が report market_date と異なる入力では、両方の値が read model で混同されないことを provider test で固定する。
- provider が明示した `DAY_ONLY` が ACL と normalization を通過しても保持されることを ACL/runtime test で固定する。
- 日付だけの FRED observation が RFC3339 として temporal eligible にならず、Primary Event reaction に表示されないことを temporal/renderer test で固定する。
- 既存の timestamp observation は引き続き表示可能であり、precision の保護が timestamp semantics を壊さないことを既存 test と focused test で確認する。
- observation-only の Decision 境界が不変であることを Signal Context consistency と quality gate で確認する。

### Task 1: Provider の観測日と precision を分離する

**Files:**
- Modify: `src/features/research/infrastructure/macro_signal_context_provider.rs`
- Test: `src/features/research/infrastructure/macro_signal_context_provider.rs::tests`

**Interfaces:**
- Consumes: `ParsedFredSeries.latest_date` と report `market_date`。
- Produces: provider reaction の `observation_date` と provider-owned observation precision。

- [ ] **Step 1: Write the failing test**

  `build_fred_reactions` に report date `2026-09-21`、latest date `2026-09-15`、Brent value `130.80` の結果を渡し、reaction の observation date が `2026-09-15`、evidence timestamp と source published date が日付のみであることを期待する。`2026-09-21T00:00:00Z` はすべて拒否する。

- [ ] **Step 2: Run test to verify it fails**

  Run: `cargo test fred_reaction_preserves_daily_observation_date_and_day_only_precision --lib`

  Expected: FAIL because the current provider uses `market_date` to build `evidence.timestamp` and has no provider-owned precision/date field.

- [ ] **Step 3: Write minimal implementation**

  `ProviderMarketReaction` に optional typed `observation_date` と provider-local precision enum を追加し、FRED では `latest_date` を保存する。`fred_evidence`、event、reaction の timestamp/source publication は `latest_date.to_string()` を使い、report `market_date` を observation timestamp に使わない。

- [ ] **Step 4: Run test to verify it passes**

  Run: `cargo test fred_reaction_preserves_daily_observation_date_and_day_only_precision --lib`

  Expected: PASS.

### Task 2: ACL が provider-owned precision を read model に写像する

**Files:**
- Modify: `src/features/research/acl/macro_signal_context_provider_factory.rs`
- Modify: `src/features/research/interface/macro_event_observation.rs`
- Test: `src/features/research/acl/macro_signal_context_provider_factory.rs::tests`

**Interfaces:**
- Consumes: Task 1 の provider `observation_date` と precision。
- Produces: `MarketReaction.observation_date`、date-only `observed_at`、明示的な `ObservationTimePrecision::DayOnly`。

- [ ] **Step 1: Write the failing test**

  provider reaction を ACL へ渡し、mapped reaction が `observation_date=2026-09-15`、`observed_at=2026-09-15`、precision `DAY_ONLY` であることを検証する。

- [ ] **Step 2: Run test to verify it fails**

  Run: `cargo test maps_fred_reaction_precision_from_provider_fact --lib`

  Expected: FAIL because ACL currently calls `classify_observation_time_precision` on a synthetic RFC3339 string.

- [ ] **Step 3: Write minimal implementation**

  provider precision を interface enum へ写像し、ACL で RFC3339 syntax を precision の source にしない。provider date が欠けた場合は empty/unavailable として fail-closed にする。

- [ ] **Step 4: Run test to verify it passes**

  Run: `cargo test maps_fred_reaction_precision_from_provider_fact --lib`

  Expected: PASS.

### Task 3: Runtime normalization と Primary Event projection を保護する

**Files:**
- Modify: `src/features/radar/interface/signal_context_coverage.rs`
- Modify: `src/features/radar/interface/signal_context_read_model.rs`
- Test: `src/features/radar/interface/signal_context_coverage.rs::tests`
- Test: `src/features/radar/interface/signal_context_read_model.rs::tests`

**Interfaces:**
- Consumes: Task 2 の explicit `ObservationTimePrecision::DayOnly` reaction。
- Produces: DAY_ONLY を保持した SignalContextV1 と、Primary Event reaction の空投影。

- [ ] **Step 1: Write the failing tests**

  `normalize_observations` が explicit `DAY_ONLY` を `TIMESTAMP` に変更しないことを検証する。さらに report date `2026-09-21` の Primary Event と observation date `2026-09-15` の FRED reaction を使い、temporal binding が不適格で renderer が reaction を返さないことを検証する。

- [ ] **Step 2: Run tests to verify they fail**

  Run: `cargo test signal_context_normalization_preserves_provider_observation_precision --lib` と `cargo test fred_day_only_observation_is_not_rendered_as_primary_reaction --lib`

  Expected: normalization test fails because the current implementation always reclassifies from `observed_at`; projection test fails if the provider-shaped input is converted into a synthetic timestamp.

- [ ] **Step 3: Write minimal implementation**

  `normalize_observations` は precision が `Unavailable` の場合だけ syntax fallback を行い、明示された precision を保持する。既存 renderer の timestamp-only filter と temporal binding の fail-closed semantics は維持し、Decision surface へ新しい入力を渡さない。

- [ ] **Step 4: Run tests to verify they pass**

  Run: `cargo test signal_context_normalization_preserves_provider_observation_precision --lib` と `cargo test fred_day_only_observation_is_not_rendered_as_primary_reaction --lib`

  Expected: PASS.

### Task 4: Contract verification and quality gates

**Files:**
- Modify: `.ai/work-items/active/fred-observation-date-precision-integrity.summary.json` (Runtime generated only)
- Verify: `src/features/research/infrastructure/macro_signal_context_provider.rs`
- Verify: `src/features/research/acl/macro_signal_context_provider_factory.rs`
- Verify: `src/features/radar/interface/signal_context_coverage.rs`
- Verify: `src/features/radar/interface/signal_context_read_model.rs`

**Interfaces:**
- Consumes: Tasks 1–3 の provider/ACL/runtime regression evidence。
- Produces: Runtime verification receipt と Work Item finish-ready evidence。

- [ ] **Step 1: Run focused FRED and temporal tests**

  Run: `cargo test fred --lib`、`cargo test temporal --lib`、`cargo test market_reaction --lib`

  Expected: all focused tests pass with no synthetic FRED report-date timestamp.

- [ ] **Step 2: Run repository quality gates**

  Run: `make fmt-check`、`make test`、`make clippy`、`make check-signal-context-consistency`、`make quality`

  Expected: all commands exit 0; any unrelated pre-existing failure is recorded by name rather than hidden.

- [ ] **Step 3: Record Runtime verification**

  Run: `ai-cockpit verify --repo /Users/sei-rinn/.codex/worktrees/fred-observation-date-precision-integrity/sentinel --work-item fred-observation-date-precision-integrity --command make --args quality`

  Expected: Runtime writes a current verification receipt bound to the Contract, checkpoint, branch, and exact source revision.

- [ ] **Step 4: Finish the Work Item lifecycle**

  Run: `ai-cockpit finish --repo /Users/sei-rinn/.codex/worktrees/fred-observation-date-precision-integrity/sentinel --id fred-observation-date-precision-integrity`

  Expected: finish reports ready-for-review only after required verification evidence and all acceptance bindings are current.

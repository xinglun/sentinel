---
author: Ray
title: Legacy formal snapshot migration implementation plan
description: 旧交易日 packet 到 formal snapshot 与 observation history state 的一次性迁移实现计划。
key: legacy-formal-snapshot-migration-plan
---

# Legacy formal snapshot migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不伪造市场事实的前提下，把 legacy `DecisionPacket` 历史一次性、幂等地迁移为 formal trading-day snapshot 与稳定 cycle state。

**Architecture:** 在 `PersistenceLayer` 内新增 legacy packet 读取与 snapshot projection，使用统一的 `TradingDaySnapshot` 写入/冲突校验逻辑；由 runner 在正式运行前检测并执行迁移。CI 只负责恢复 reports、触发迁移并验证迁移后的 state/snapshot 连续性，不复制 Rust 领域映射逻辑。

**Tech Stack:** Rust、Serde JSON、Chrono、现有 `PersistenceLayer`、GitHub Actions shell/Python 检查、Cargo tests。

## Global Constraints

- 迁移数据只表达观察事实，不能恢复无法从 legacy packet 明确推导的字段。
- 迁移 snapshot 使用 `source_status: degraded`、`decision_state: NO_TRADE`、`data_quality.history: MIGRATED_LEGACY`。
- 迁移必须幂等；已有 formal snapshot 不覆盖，语义冲突必须返回 `SNAPSHOT_CONFLICT`。
- 输入解析失败时整体 fail-closed，不发布部分迁移结果。
- 不改变交易门控、position sizing、action matrix、市场阈值或 Telegram 文案。
- 所有 repository comment 与 Markdown 正文使用日语，identifier 使用英语。
- 验证命令通过 `make` 入口记录；Rust quality gate 为 `make fmt-check`、`make test`、`make clippy`。

## 文件结构

- Modify: `src/features/radar/infrastructure/persistence.rs` — legacy packet 读取、字段投影、stable cycle、幂等迁移写入。
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs` — 在 pipeline 读取历史后触发一次性迁移，并继续使用统一 persistence API。
- Modify: `.github/workflows/daily_radar.yml` — 迁移触发前后的诊断、恢复结果与 append 校验。
- Modify: `tests/daily_radar_workflow_integration.rs` — CI migration shell 契约和语法测试。
- Modify: `.ai/work-items/active/cross-run-observation-persistence.contract.json` — 增加 migration scope、acceptance 和 verification。
- Modify: `.ai/work-items/active/cross-run-observation-persistence.summary.json` — 记录迁移测试和外部 runner 风险。
- Existing: `docs/superpowers/specs/2026-07-30-legacy-formal-snapshot-migration-design.md` — 设计 SSOT，不在实现中扩大范围。

### Task 1: 固定 legacy packet 输入与 projection 的失败测试

**Files:**

- Modify: `src/features/radar/infrastructure/persistence.rs` test module near existing snapshot tests.
- Test: `src/features/radar/infrastructure/persistence.rs` inline tests.

**Interfaces:**

- Consumes: temporary reports directory containing `decision_packet_YYYY-MM-DD.json` files.
- Produces: failing tests defining `migrate_legacy_history` behavior and stable snapshot fields.

- [ ] **Step 1: Write the failing test for one legacy packet.**

  Build a minimal `DecisionPacket` with a known date, save it as `decision_packet_2026-07-28.json`, call the future migration entry point, and assert one formal snapshot plus an observation state are produced with `source_status == "degraded"`, `decision_state == "NO_TRADE"`, and `data_quality["history"] == "MIGRATED_LEGACY"`.

- [ ] **Step 2: Write the failing test for deterministic cycle and date ordering.**

  Save packets for `2026-07-28` and `2026-07-29` in reverse filesystem order; assert both filenames use the same cycle id, state count is `2`, and `last_market_date` is `2026-07-29`.

- [ ] **Step 3: Run the focused tests and verify the expected missing-interface failure.**

  Run:

  ```bash
  cargo test migrate_legacy_history --lib
  ```

  Expected: compilation or test failure because the migration API does not yet exist.

### Task 2: Implement safe legacy packet loading and projection

**Files:**

- Modify: `src/features/radar/infrastructure/persistence.rs`.
- Test: `src/features/radar/infrastructure/persistence.rs` inline tests from Task 1.

**Interfaces:**

- Consumes: `PersistenceLayer`, `DecisionPacket`, `TradingDaySnapshot`.
- Produces: `pub fn migrate_legacy_history(&self) -> Result<Option<ObservationHistoryState>>`.

- [ ] **Step 1: Add deterministic legacy input discovery.**

  Read dated `decision_packet_*.json` files in `save_dir`, parse the date from the filename, deserialize every file, sort by packet date, and deduplicate by date. If a dated packet cannot deserialize, return the contextual error before writing any migration output.

- [ ] **Step 2: Add stable cycle derivation.**

  Derive a cycle string from the sorted date range, for example `legacy-2026-07-28-2026-07-29`; do not use `Uuid::new_v4()`. If an existing `observation_history_state.json` has a non-empty cycle id, reuse it instead of creating a migration cycle.

- [ ] **Step 3: Add packet-to-snapshot projection.**

  Map only packet-backed fields. Set `source_status` to `degraded`, `decision_state` to `NO_TRADE`, `new_position_limit` to `0.0`, `reset_event` to `Some("MIGRATED_LEGACY")`, and `data_quality` to `{"history":"MIGRATED_LEGACY"}`. Use `UNAVAILABLE` or empty collections for values not present in the packet.

- [ ] **Step 4: Add atomic, idempotent migration write.**

  For every date call the existing formal snapshot conflict validator before writing. Preserve an identical existing snapshot, return `SNAPSHOT_CONFLICT` for semantic differences, and write state only after all snapshots pass validation. Use the existing atomic write helper for state.

- [ ] **Step 5: Run the focused tests and verify green.**

  Run:

  ```bash
  cargo test migrate_legacy_history --lib
  ```

  Expected: one-packet, ordering, stable-cycle, and idempotency tests pass.

### Task 3: Add fail-closed and conflict regression tests

**Files:**

- Modify: `src/features/radar/infrastructure/persistence.rs` inline tests.

**Interfaces:**

- Consumes: `migrate_legacy_history` from Task 2.
- Produces: regression coverage for unsafe migration inputs.

- [ ] **Step 1: Add malformed packet test.**

  Place one valid and one invalid dated packet in the reports directory; assert migration returns an error and neither `snapshots/` nor `observation_history_state.json` is published.

- [ ] **Step 2: Add existing snapshot idempotency test.**

  Run migration twice and assert the second run preserves generated timestamps and returns the same state count/cycle.

- [ ] **Step 3: Add semantic conflict test.**

  Pre-create a snapshot at the migration key with a different market state; assert `SNAPSHOT_CONFLICT` and unchanged state.

- [ ] **Step 4: Run the focused regression tests.**

  Run:

  ```bash
  cargo test migrate_legacy_history --lib
  ```

  Expected: all migration tests pass and no temporary migration output remains after failure.

### Task 4: Integrate migration into the radar pipeline

**Files:**

- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`.
- Modify: `src/features/radar/infrastructure/persistence.rs` if the runner needs a narrow migration status helper.
- Test: existing pipeline persistence tests plus a new two-run integration test if the current test harness supports it.

**Interfaces:**

- Consumes: `PersistenceLayer::migrate_legacy_history`.
- Produces: a pipeline startup path that migrates only when formal snapshots are absent and legacy packet history is present.

- [ ] **Step 1: Add the startup migration condition.**

  After loading the reports directory and before resolving the previous snapshot, detect legacy packet presence and formal snapshot absence. Skip migration when formal snapshots already exist or when no legacy packets exist.

- [ ] **Step 2: Propagate migration errors.**

  Do not convert migration errors, snapshot load errors, or snapshot conflicts into `BASELINE_UNAVAILABLE`; return the error so the run cannot publish a misleading one-point history.

- [ ] **Step 3: Re-load state and formal history after migration.**

  Use the migrated `ObservationHistoryState` and snapshots for cycle selection and previous baseline resolution in the same run.

- [ ] **Step 4: Verify the two-run behavior.**

  Run the pipeline twice against a temporary reports directory with legacy packets; assert the first run creates formal history and the second run retains the same cycle and appends the new market date.

### Task 5: Add CI migration trigger and verification

**Files:**

- Modify: `.github/workflows/daily_radar.yml`.
- Modify: `tests/daily_radar_workflow_integration.rs`.

**Interfaces:**

- Consumes: Rust radar startup migration from Task 4 and restored `reports/`.
- Produces: CI behavior that migrates legacy-only reports once, rejects unparseable migration input, and validates append continuity.

- [ ] **Step 1: Add workflow contract assertions first.**

  Extend `daily_radar_workflow_integration.rs` to assert the workflow contains the legacy-history detection, formal snapshot count, migration invocation through `make radar-release`, and fail-closed migration error text. Keep the extracted shell block syntax-checkable with `bash -n`.

- [ ] **Step 2: Add restored-history diagnostics.**

  After data branch restore, compute and export restored formal snapshot count/date and report when legacy JSONL exists without formal snapshots.

- [ ] **Step 3: Trigger migration through the Rust runner.**

  Keep migration inside `make radar-release`; the workflow must not duplicate packet-to-snapshot field mapping in Python.

- [ ] **Step 4: Enforce post-run append validation.**

  For a new market date, require the formal snapshot count to exceed the restored count, require current snapshot/state cycle equality, and fail before data branch commit when the condition is false.

- [ ] **Step 5: Run workflow tests and YAML validation.**

  Run:

  ```bash
  cargo test daily_radar_workflow --test daily_radar_workflow_integration
  ruby -e 'require "yaml"; YAML.load_file(".github/workflows/daily_radar.yml")'
  ```

  Expected: shell-contract tests and YAML parsing pass.

### Task 6: Update Work Item evidence and run repository quality gates

**Files:**

- Modify: `.ai/work-items/active/cross-run-observation-persistence.contract.json`.
- Modify: `.ai/work-items/active/cross-run-observation-persistence.summary.json`.
- Modify: `.ai/cockpit/current_status.md` only through the repository generator.

**Interfaces:**

- Consumes: all implementation and verification evidence from Tasks 1-5.
- Produces: accurate Work Item status with external GitHub Actions verification explicitly separated from local verification.

- [ ] **Step 1: Record migration acceptance and scenario coverage.**

  Mark local migration scenarios verified only with direct test evidence; keep real runner restoration unverified until a hosted run proves data branch continuity.

- [ ] **Step 2: Run the required quality gates.**

  Run:

  ```bash
  make fmt-check
  make test
  make clippy
  make diff-check
  ```

- [ ] **Step 3: Run available AI Cockpit checks.**

  Run the repository-supported `make check-ai-contract`, `make check-ai-scope`, `make check-ai-guards`, `make check-ai-backtrack`, `make check-ai-change-summary`, and status checks. If the resolver continues to reject the repository Makefile because `ai-close-work-item` is absent, record the exact blocker and do not claim those checks passed.

- [ ] **Step 4: Final diff and risk review.**

  Run `git diff --check`, inspect all changed paths against Contract scope, and record the remaining one-time hosted migration risk in Summary.

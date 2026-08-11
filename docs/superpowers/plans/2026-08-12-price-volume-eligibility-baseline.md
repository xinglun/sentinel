# Price-Volume Eligibility / Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 让 Price-Volume Observation 在标准历史不足但局部证据可用时输出可解释的 PARTIAL 基线与候选生命周期，同时保持交易边界不变。

**Architecture:** 在现有 `price_volume_structure` domain 中增加结构化 eligibility、baseline、reason 和 lifecycle 字段；metrics 依据基线窗口计算。Runner 继续只传递现有 `TickerHistory` 和事件 context，report 与 persistence 只投影 assessment，不接入 application/gate/trader。

**Tech Stack:** Rust、Serde、Chrono、Cargo unit tests、Makefile AI Cockpit gates。

## Global Constraints

- `decision_weight_percent=0`、`trade_signal=false`、`gate_effect/execution_effect/position_sizing_effect=None` 固定不变。
- 禁止 ticker-specific 分支；事件只能来自现有 `SupplyEventContext`。
- `PARTIAL` 允许候选和 Developing，不能直接 Confirmed；既有经济定义与确认门槛不变。
- repository 内新增/实质修改 Markdown 使用日文本文与 front matter；commit subject 使用日文 Conventional Commits。

### Task 1: Domain eligibility and baseline model

**Files:**
- Modify: `src/features/radar/domain/price_volume_structure.rs`
- Test: `src/features/radar/domain/price_volume_structure.rs`

**Interfaces:**
- Add serializable enums `EligibilityStatus`, `BaselineType`, `CandidateLifecycle`, `UnavailableReason`.
- Extend `PriceVolumeMetrics` with selected baseline type/days and available-history RVOL fields while preserving old serde fields.
- Extend `PriceVolumeAssessment` with eligibility, primary/secondary baseline, lifecycle, reason and next condition.

- [x] Write failing tests for 3/7/15-day IPO, mature 20-day, event baselines and unavailable reasons.
- [x] Run `cargo test price_volume_structure::tests --lib` and confirm the new assertions fail because short history is currently unavailable.
- [x] Implement minimal valid-session counting, event baseline selection and baseline-window metrics.
- [x] Preserve the existing classification predicates; only replace the hard 20-day availability gate and use selected RVOL fields.
- [x] Run the focused test module and confirm pass.

### Task 2: Candidate lifecycle and boundary regression

**Files:**
- Modify: `src/features/radar/domain/price_volume_structure.rs`
- Test: `src/features/radar/domain/price_volume_structure.rs`

**Interfaces:**
- Lifecycle derives from prior persistence count plus eligibility and never changes observation boundary.

- [x] Add failing tests for candidate/developing/confirmed eligibility combinations and one-day short squeeze/noise.
- [x] Implement the lifecycle guard, including `PARTIAL` prohibition on direct confirmation and invalidation on failed continuation.
- [x] Run focused tests and inspect boundary assertions.

### Task 3: Report and runtime projection

**Files:**
- Modify: `src/features/radar/interface/price_volume_structure_report.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`
- Test: `src/features/radar/interface/price_volume_structure_report.rs`

**Interfaces:**
- Report consumes the extended `PriceVolumeAssessment`; runner passes `TickerHistory.total_trading_days` and existing event context without ticker special cases.

- [x] Add failing report tests for eligibility, primary/secondary baseline, days, reason and next condition in zh/en/ja-safe output.
- [x] Implement output labels and preserve Observation Only text.
- [x] Run focused report and runner tests.

### Task 4: Persistence compatibility and full scenarios

**Files:**
- Modify: `src/features/radar/infrastructure/persistence.rs`
- Modify: `tests/**` only where existing integration contracts need the new fields.
- Test: domain/report/persistence tests and scenario fixtures.

- [x] Add failing serde round-trip tests for new fields and old records missing them.
- [x] Implement defaults/optional compatibility without changing append-only behavior.
- [x] Add all required scenario coverage and anti-overfitting assertions.
- [x] Run `make fmt-check`, `make test`, `make clippy` and all Contract AI gates.

### Task 5: Contract, Summary and closure

**Files:**
- Modify: `.ai/work-items/active/price-volume-eligibility-baseline.summary.json`
- Modify: `.ai/cockpit/current_status.md` via generator only

- [x] Record changed files, scenario evidence, residual risks and review focus.
- [x] Run `make ai-finish TASK=price-volume-eligibility-baseline` and confirm archive/no-active status.
- [x] Run an independent diff audit for forbidden paths and boundary invariants before reporting completion.

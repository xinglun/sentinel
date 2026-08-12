# Observation Layer Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完成事件、短历史价量、Supply Context、Current Relative Strength 与 Leader 缺失语义修复，并证明交易边界未改变。

**Architecture:** 在现有 Radar/Research read model 边界内增加结构化 Observation 字段，保持 Gate 与 Trading 模块只读或不依赖这些字段。每个领域先用失败回归测试固定语义，再实现最小代码并运行对应模块测试。

**Tech Stack:** Rust, chrono, serde, Cargo tests, Makefile AI Cockpit gates。

## Global Constraints

- `decision_weight = 0%`、`trade_signal = false`、`gate_effect = none`、`execution_effect = none`、`position_sizing_effect = none`。
- 不修改 Gate、Execution、Trader、Action Matrix、Position Sizing、Breadth、Market State threshold、Confidence threshold、Leader qualification、Breakout qualification。
- repository 内新增手写注释与文档正文使用日文；用户沟通使用中文。
- 所有 Contract verification 使用 `make` target。

### Task 1: Contract 与当前边界

**Files:**
- Create: `.ai/work-items/active/observation-layer-repair.contract.json`
- Create: `.ai/work-items/active/observation-layer-repair.summary.json`

- [ ] 运行 `make ai-start TASK=observation-layer-repair TITLE="Observation Layer 修复" MODE=code`，确认 `notCodable=false`、`unknowns=[]`、`executionDecision=continue`。
- [ ] 运行 `make ai-preflight` 与 Contract 检查；若 scope 或 acceptance 不足，先更新 Contract。
- [ ] 在 Contract 中写明六类 Coverage、三态 CPI、短历史、Supply、相对强度、Leader absence 和 NO TRADE invariant。

### Task 2: Signal Context 事件事实

**Files:**
- Modify: `src/features/radar/interface/signal_context_event_read_model.rs`
- Modify: `src/features/radar/interface/signal_context_coverage.rs`
- Modify: `src/features/radar/interface/presentation.rs`
- Test: `src/features/radar/interface/*signal_context*` tests and `tests/fixtures/signal_context/`

- [ ] 先加入 CPI UPCOMING、RELEASED、actual unavailable 的失败测试，断言 primary context 不为 None、lifecycle、information content、quality 和 reason。
- [ ] 运行对应测试确认因当前生命周期固定/事件过滤而失败。
- [ ] 引入 discovery/observation 的结构化状态，保留已知事件，即使 observation 失败也输出 `EVENT_DATA_UNAVAILABLE`。
- [ ] 增加 `ACTIVE_REPRICING` 等生命周期推导，保持 event fact、market reaction、interpretation 分离。
- [ ] 增加 Coverage Overall 非 HEALTHY 时的保守文案回归测试。
- [ ] 运行 signal context 单元与集成测试并提交独立 commit。

### Task 3: Price-Volume Eligibility 与 Baseline

**Files:**
- Modify: `src/features/radar/domain/price_volume_structure.rs`
- Modify: `src/features/radar/interface/price_volume_structure_report.rs`
- Modify: `src/features/shared/domain/supply_event_context.rs`
- Test: price-volume domain/report tests and generic IPO/Lockup fixtures

- [ ] 先写 5–19 sessions 的 PARTIAL eligibility、POST_IPO/POST_LOCKUP baseline、RVOL baseline、missing supply reason 的失败测试。
- [ ] 将固定 20 日资格拆为 `FULL/PARTIAL/INSUFFICIENT/UNAVAILABLE`，PARTIAL 仅要求连续局部 OHLCV。
- [ ] 将 Structure 与 Lifecycle 拆开；结构不可判定时 lifecycle 必须 UNAVAILABLE，观察持续天数仍单独保留。
- [ ] 允许局部证据输出低置信度 Structure Hypothesis/CANDIDATE 或 DEVELOPING，不输出 confirmed institutional accumulation。
- [ ] 运行价量领域、报告和既有 S-28 回归测试并提交独立 commit。

### Task 4: Supply read-only bridge

**Files:**
- Modify: `src/features/shared/domain/supply_event_context.rs`
- Modify: `src/features/radar/domain/price_volume_structure.rs`
- Test: supply context and price-volume integration tests

- [ ] 先写覆盖 IPO、Lockup、Secondary、Insider、Employee Liquidity、Convertible、Major Shareholder Sale 的 taxonomy 测试。
- [ ] 为缺失背景增加 status/reason（如 `NO_MAPPED_SUPPLY_EVENT`、`SUPPLY_CONTEXT_MISSING`），不反向修改 Supply Layer。
- [ ] 验证 Supply Event 只向 Price-Volume 流动，boundary 字段仍全为 observation-only。

### Task 5: Current Relative Strength

**Files:**
- Create or modify: `src/features/radar/domain/current_relative_strength.rs`
- Modify: `src/features/radar/interface/report.rs` and relevant presentation read models
- Test: relative-strength domain/report tests

- [ ] 先写无 Leader 但 NVDA 当前相对强、MSFT weakening 的失败测试，断言只产生 observation。
- [ ] 实现 1d/5d vs SPY、price position、volume participation 的有限状态 `IMPROVING/STRONG/NEUTRAL/WEAKENING`。
- [ ] 输出明确 boundary，禁止进入 Leader、Gate、Action Matrix、Position Sizing。

### Task 6: Leader absence

**Files:**
- Modify: `src/features/radar/domain/leader_persistence.rs`
- Modify: leader report/read model files found by existing output path
- Test: leader persistence tests

- [ ] 先写 none 连续 1 日与 6 日失败测试，断言 `ABSENT`、absence duration、previous transition。
- [ ] 实现连续 none 计数，同时不填充资产 Leader 专用字段。
- [ ] 保持非 none Leader 的现有 qualification 与 Gate 行为不变。

### Task 7: Full regression and self-audit

**Files:**
- Modify: `tests/fixtures/signal_context/` and existing regression tests
- Modify: `.ai/work-items/active/observation-layer-repair.summary.json`

- [ ] 添加同日 rerun、partial coverage、strong relative asset while NO TRADE 的回归。
- [ ] 运行 focused tests，再运行 `make fmt-check`、`make test`、`make clippy` 与 Contract 全部 required checks。
- [ ] 用 `rg` 检查新增 observation 字段未进入 gate/trader/action/position sizing。
- [ ] 更新 Summary 的 code/test/docs/i18n/report/data/Make guard 状态、residual risks 和 expected review focus。
- [ ] 运行 `make ai-finish TASK=observation-layer-repair`，仅在所有 required checks 通过后归档。

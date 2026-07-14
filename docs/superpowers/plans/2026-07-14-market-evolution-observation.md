# Market Evolution Observation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将日报从单日快照升级为具备明确历史覆盖、时间轴、变化驱动和供给队列详情的市场演化观察。

**Architecture:** 复用 Leadership Snapshot 作为跨日事实源，在 radar domain 增加持久性与变化等级纯函数；在 interface 层组装压缩日报和 archival read model；持久化层独立保存七个交易日 Observation Timeline。交易决策 packet 与执行链路保持不变。

**Tech Stack:** Rust、Serde、Chrono、现有 radar domain/interface/infrastructure、Cargo tests、Makefile Cockpit checks。

## Global Constraints

- Observation / Interpretation Only；Decision Weight = 0%。
- 不修改 Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing 或 Risk Sizing。
- `DOMINANT` 为连续领导至少 8 个交易日且 breadth / relative strength 均至少 60。
- `FADING` 为仍是 Leader 且每日至少下降 2 分连续 3 日，或三日累计下降至少 5 分。
- 7 个交易日按交易日计算；缺失快照为 `PARTIAL`，无法形成有效连续历史为 `UNAVAILABLE`。
- `MAJOR > MODERATE > MINOR > NONE`；小幅局部排序变化为排名最多变化 1 位且分数变化小于 5。

---

### Task 1: 固化 Contract、设计与测试边界

**Files:**
- Modify: `.ai/work-items/active/market-evolution-observation.contract.json`
- Modify: `.ai/work-items/active/market-evolution-observation.summary.json`
- Create: `docs/superpowers/specs/2026-07-14-market-evolution-observation-design.md`
- Create: `docs/superpowers/plans/2026-07-14-market-evolution-observation.md`

- [x] **Step 1: 写入阈值、历史覆盖、输出范围和 Observation-only 边界。**
- [x] **Step 2: 运行 `make ai-preflight`，确认 Contract 为 `ready`。**
- [x] **Step 3: 在实现后回填 scenario coverage 的测试证据和 Summary。**

### Task 2: Leader Persistence 规则

**Files:**
- Modify: `src/features/radar/domain/leader_persistence.rs`
- Modify: `src/features/radar/domain/mod.rs`
- Test: `src/features/radar/domain/leader_persistence.rs` 内现有 domain tests

**Interfaces:**
- Consumes: 现有 `LeaderObservation` 与 `LeadershipSnapshot` 相关输入。
- Produces: 结构化 `history_coverage`、`first_observed_at`、`previous_leader`、`leadership_state`，并保持现有 Presentation 调用兼容。

- [x] **Step 1: 先添加测试：8 日且 breadth / relative strength 为 60 时为 DOMINANT，任一为 59 时不为 DOMINANT。**
- [x] **Step 2: 运行 `cargo test leader_persistence --all-targets`，确认新测试失败。**
- [x] **Step 3: 添加 FADING 三日下降、单日小幅下降和 leader 切换测试。**
- [x] **Step 4: 实现最小状态与 coverage 计算，区分 Primary Leader streak 和 breakout continuity。**
- [x] **Step 5: 运行同一测试命令，确认通过。**

### Task 3: Observation Timeline 与 Market Change Driver

**Files:**
- Create: `src/features/radar/domain/observation_timeline.rs`
- Create: `src/features/radar/domain/market_change_driver.rs`
- Modify: `src/features/radar/domain/mod.rs`
- Modify: `src/features/radar/interface/market_interpretation_read_model.rs`
- Test: 新 domain module tests

**Interfaces:**
- Consumes: Leadership Snapshot、当前 / 前日 Interpretation read model、Supply Queue facts。
- Produces: 可序列化 `ObservationTimelineEntry`、`ObservationTimeline` 和 `MarketChangeDriver`。

- [x] **Step 1: 先添加 7 日窗口、周末 / 休市排除、缺失日期 PARTIAL / UNAVAILABLE 的失败测试。**
- [x] **Step 2: 运行对应 domain tests，确认失败原因来自缺少新接口或规则。**
- [x] **Step 3: 添加 change level 四级优先级与局部排序阈值失败测试。**
- [x] **Step 4: 实现纯函数和 Serde 结构，保证完整时间轴不进入 decision packet。**
- [x] **Step 5: 运行 domain tests，确认通过并保持 JSON 字段稳定。**

### Task 4: 报告与 archival 输出

**Files:**
- Modify: `src/features/radar/interface/presentation.rs`
- Modify: `src/features/radar/interface/presentation_assembler.rs`
- Modify: `src/features/radar/interface/report.rs`
- Modify: `src/features/radar/interface/audit_daily_report.rs`
- Modify: `src/features/radar/interface/weekly_state_report.rs`
- Modify: `src/features/radar/infrastructure/persistence.rs`
- Test: `tests/audit_daily_cli_integration.rs`
- Test: `tests/daily_radar_workflow_integration.rs`

**Interfaces:**
- Consumes: domain timeline、change driver、现有 capital absorption queue item。
- Produces: 压缩 Markdown / Telegram 摘要与独立 latest / daily JSON / JSONL archival record。

- [x] **Step 1: 先添加日报断言：7 日无结构变化时输出压缩摘要，PARTIAL 不出现“首次观察”。**
- [x] **Step 2: 运行对应 integration test，确认失败。**
- [x] **Step 3: 添加 Future Supply Queue 五个详情字段和 `change_drivers` / `unchanged_dimensions` 断言。**
- [x] **Step 4: 实现报告 read model 与独立 archival 写入，不改 decision packet schema。**
- [x] **Step 5: 运行集成测试与快照更新，确认三语言输出保持 `decision_weight: 0%`。**

### Task 5: 全量验证与 Cockpit Summary

**Files:**
- Modify: `.ai/work-items/active/market-evolution-observation.summary.json`
- Modify: `.ai/cockpit/current_status.md`

- [x] **Step 1: 运行 `make check-ai-contract CONTRACT=.ai/work-items/active/market-evolution-observation.contract.json`。**
- [x] **Step 2: 运行 `make check-ai-scope CONTRACT=.ai/work-items/active/market-evolution-observation.contract.json` 与 `make check-ai-guards CONTRACT=.ai/work-items/active/market-evolution-observation.contract.json`。**
- [x] **Step 3: 运行 `make fmt-check`、`make test`、`make clippy` 与 `make check-architecture-all`。**
- [x] **Step 4: 运行 backtrack、coverage、scenario、change-summary、status checks，并回填每条结果。**
- [ ] **Step 5: 运行 `make ai-finish TASK=market-evolution-observation`；只有 required checks 全部通过后才归档。**

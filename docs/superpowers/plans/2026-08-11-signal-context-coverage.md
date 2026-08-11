# Signal Context Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 Signal Context 的事实覆盖、证据追踪、时区生命周期和日报表达，同时保持交易决策边界冻结。

**Architecture:** 先建立统一的六类 Context / Evidence / Coverage 数据契约，再由确定性聚合器选择 Primary / Secondary，最后将 read model 投影到现有日报与解释层。source adapter 只提供可追溯事实，Interpretation 只生成弱因果摘要。

**Tech Stack:** Rust、Serde、Chrono、现有 Make/Cockpit、Rust unit/integration tests、Markdown/Telegram snapshots。

## Global Constraints

- `decision_weight = 0`、`trade_signal = false`、`gate_effect = none`、`execution_effect = none`、`position_sizing_effect = none`。
- 不修改 NO TRADE、READY、EXECUTE、Action Matrix、Leader、Breadth、Confidence、Position Sizing、Trader、Price-Volume、Supply、Gravity、Expectation。
- 美股交易日使用 `America/New_York`；事件至少保存 UTC 时间、市场时区时间、market date、observed/published time。
- HIGH / MEDIUM 必须有 EvidenceRecord；AI 不得成为事实源。
- 新操作入口必须通过 Make target；所有手写文档使用日文 front matter/正文规范。

---

### Task 1: 修复 Cockpit lifecycle entrypoint

**Files:**
- Modify: `Makefile`
- Create: `scripts/ai_close_work_item.py`
- Test: `scripts/ai_test_close_work_item.py`

- [ ] 增加 `ai-close-work-item` Make target，委托现有 close lifecycle 实现或在不存在时以明确 fail-closed 信息退出。
- [ ] 添加测试，证明 resolver 能识别 direct lifecycle targets，且 close target 不会绕过 archive/status checks。
- [ ] 运行 `make test-ai-pr-lifecycle` 与专用测试。

### Task 2: 建立统一 Context / Evidence / Coverage 契约

**Files:**
- Modify: `src/features/radar/interface/presentation.rs`
- Modify: `src/features/radar/interface/signal_context_read_model.rs`
- Modify: `src/features/radar/interface/signal_context_event_read_model.rs`
- Modify: `src/features/research/interface/macro_event_observation.rs`
- Test: `src/features/radar/interface/*tests.rs`

- [ ] 先写 v1 schema、枚举迁移、serde 与默认值的失败测试。
- [ ] 实现六类 Context、EvidenceRecord、MarketReaction、SourceStatus、Coverage、Lifecycle 与固定 decision boundary。
- [ ] 明确旧 `UNKNOWN` 与旧生命周期的兼容映射，更新既有 snapshot。
- [ ] 运行目标模块测试与 `make fmt-check`。

### Task 3: 实现事实聚合与确定性判定

**Files:**
- Modify: `src/features/radar/interface/signal_context_event_read_model.rs`
- Modify: `src/features/radar/interface/signal_context_read_model.rs`
- Modify: `src/features/research/interface/*`
- Test: `tests/fixtures/signal_context/**`
- Test: `src/features/radar/interface/*tests.rs`

- [ ] 为六类 source 建立结构化输入及失败/429诊断。
- [ ] 实现阈值、coverage 真值表、Primary 排序、Secondary 去重与 no-event 文案约束。
- [ ] 实现 America/New_York market_date 与 freshness/lifecycle 判定。
- [ ] 固化 8/7 Payroll 与 8/10 Geopolitical/Oil fixture。

### Task 4: 接入报告、解释与 consistency gate

**Files:**
- Modify: `src/features/radar/interface/interpretation_read_model.rs`
- Modify: `src/features/radar/interface/report_ui_tests.rs`
- Modify: `src/features/shared/interface/i18n.rs`
- Modify: `docs/archive/PROJECT_AUDIT_ISSUES.md`
- Test: `tests/**`

- [ ] 让 Markdown/Telegram/weekly projection 消费统一 read model，移除无证据的绝对 “No major event today”。
- [ ] 增加 `signal_context_consistency_check` 与 Make target。
- [ ] 更新 S-26 为 Market Signal Context Coverage Defect，并记录 boundary。
- [ ] 补齐 zh/en/ja 文案和 snapshots。

### Task 5: 全量验证与归档

- [ ] 覆盖 22 个验收场景：宏观、财报、地缘政治、商品、利率/信用、VIX/轮动、无事件、429、部分失败、时区、周末、假日、盘前盘后和生命周期。
- [ ] 运行 Contract 中全部 required `make` checks、`make test`、`make clippy`、`make quality`。
- [ ] 更新 Summary 的 code/test/docs/i18n/report/data/guard 证据、残余风险和 review focus。
- [ ] 运行 `make ai-finish TASK=signal-context-coverage-repair` 并完成 status/lifecycle audit。

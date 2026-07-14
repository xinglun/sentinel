---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-07-14T01:15:50.087597+00:00`
- Task: `market-evolution-observation`
- Mode: `code`
- State: `ready_with_risks`
- Contract Path: `.ai/work-items/active/market-evolution-observation.contract.json`
- Summary Path: `.ai/work-items/active/market-evolution-observation.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/market-evolution-observation.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/market-evolution-observation.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/market-evolution-observation.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/market-evolution-observation.contract.json SUMMARY=.ai/work-items/active/market-evolution-observation.summary.json`: passed
- `make check-ai-coverage-guard`: passed
- `make check-ai-scenario-coverage`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/market-evolution-observation.summary.json CONTRACT=.ai/work-items/active/market-evolution-observation.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/market-evolution-observation.contract.json SUMMARY=.ai/work-items/active/market-evolution-observation.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/market-evolution-observation.contract.json SUMMARY=.ai/work-items/active/market-evolution-observation.summary.json`: passed
- `make check-architecture-all`: passed

## Changed Files

- `.ai/cockpit/current_status.md`: Cockpit 状态由 required checks 生成。
- `.ai/work-items/active/market-evolution-observation.contract.json`: 固化阈值、历史覆盖、change_level、显示和 Observation-only 边界。
- `.ai/work-items/active/market-evolution-observation.summary.json`: 记录实现、验证、场景覆盖和残余风险。
- `docs/superpowers/specs/2026-07-14-market-evolution-observation-design.md`: 记录市场演化观察设计和语义契约。
- `docs/superpowers/plans/2026-07-14-market-evolution-observation.md`: 记录按 TDD 执行的实施计划。
- `src/features/radar/domain/leader_persistence.rs`: 增加 DOMINANT/FADING 状态、阈值、first_observed_at 和 history coverage 字段。
- `src/features/radar/domain/market_change_driver.rs`: 新增多维 change_level、drivers 和 unchanged dimensions 领域规则。
- `src/features/radar/domain/observation_timeline.rs`: 新增 7 个交易日 Timeline、coverage 和重复内容摘要规则。
- `src/features/radar/domain/mod.rs`: 注册新的 Observation domain modules。
- `src/features/radar/infrastructure/persistence.rs`: 独立保存 latest JSON、按日 JSON 和 JSONL Timeline artifact。
- `src/features/radar/interface/market_interpretation_read_model.rs`: 投影 Leader Persistence 的历史语义字段。
- `src/features/radar/interface/presentation.rs`: 扩展 Market Change Log 和 Leader Persistence read model 字段。
- `src/features/radar/interface/presentation_assembler.rs`: 保持现有 Presentation packet 默认结构兼容。
- `src/features/radar/interface/radar_pipeline_runner.rs`: 接入多维 Change Driver、Timeline 采集与官方 NYSE 休市日过滤。
- `src/features/radar/interface/report.rs`: 显示 change level、drivers、unchanged dimensions 和 7 日 Observation Timeline 摘要。
- `src/features/radar/interface/report_ui_tests.rs`: 更新 Leader Persistence fixture 并验证主报告渲染 Timeline。
- `src/features/radar/interface/weekly_state_report.rs`: 输出结构化 Leader Persistence 字段别名。
- `src/features/research/interface/capital_absorption_i18n.rs`: 移除不再使用的旧队列标签。
- `src/features/research/interface/capital_absorption_report.rs`: Future Supply Queue 输出 Subject、Event Type、Expected Window、Status、Source Quality。
- `src/features/research/interface/capital_absorption_report_tests.rs`: 固定三语言 Supply Queue 详情、前三条限制和空队列回归。
- `tests/research_attention_cli_integration.rs`: 同步实际 Future Supply Queue 输出契约，修复 P0 集成断言。
- `src/features/research/interface/macro_event_official_calendar_adapter.rs`: 复用官方 NYSE 休市日清单作为 Timeline 窗口输入。

## Review Readiness

- Status: `ready_with_risks`
- Reason: 本轮验收指出的中文/日文标签混用英文与领域摘要固定中文问题已修复；全量质量门禁通过，保留既有低风险 review 项。
- Expected Review Focus:
  - P0 集成测试与新 Supply Queue 文案契约
  - 主报告是否实际渲染 7 日 Timeline 摘要
  - Leader Persistence 当前窗口与 UNAVAILABLE / PARTIAL 语义
  - Timeline 全结构维度变化
  - Market Change Driver score/confidence 分离
  - Supply Queue 前三条和空队列措辞
  - Observation-only 边界

## Scenario Coverage

- State: `complete`

## Residual Risks

- `low` `holiday_calendar`: Runner 当前使用官方 NYSE holiday 列表；跨市场或 provider 自定义交易日仍需 review。
- `low` `report_contract`: 新增 Change Drivers 使用现有多语言报告骨架中的稳定英文字段名，需 review 是否需要进一步本地化。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

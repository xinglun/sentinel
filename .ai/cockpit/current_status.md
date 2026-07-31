---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-07-31T01:57:28.873928+00:00`
- Task: `repair-observation-history-persistence`
- Mode: `code`
- State: `ready_with_risks`
- Contract Path: `.ai/work-items/active/repair-observation-history-persistence.contract.json`
- Summary Path: `.ai/work-items/active/repair-observation-history-persistence.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/repair-observation-history-persistence.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/repair-observation-history-persistence.contract.json`: passed
- `make test-radar-cross-run-pipeline`: passed
- `make test-radar-workflow-contract`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/repair-observation-history-persistence.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/repair-observation-history-persistence.contract.json SUMMARY=.ai/work-items/active/repair-observation-history-persistence.summary.json`: passed
- `make check-ai-coverage-guard`: passed
- `make check-ai-scenario-coverage CONTRACT=.ai/work-items/active/repair-observation-history-persistence.contract.json SUMMARY=.ai/work-items/active/repair-observation-history-persistence.summary.json`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/repair-observation-history-persistence.summary.json CONTRACT=.ai/work-items/active/repair-observation-history-persistence.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/repair-observation-history-persistence.contract.json SUMMARY=.ai/work-items/active/repair-observation-history-persistence.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/repair-observation-history-persistence.contract.json SUMMARY=.ai/work-items/active/repair-observation-history-persistence.summary.json`: passed

## Changed Files

- `.github/workflows/daily_radar.yml`: 已有 data branch 但 restore 失败时，禁止以空 reports/ 继续运行并覆盖历史。
- `.github/workflows/weekly_backtest.yml`: weekly backtest 的 restore/bootstrap 同样 fail-closed，避免 rsync --delete 覆盖历史。
- `tests/daily_radar_workflow_integration.rs`: 新增 restore failure fail-closed regression test。
- `src/cli.rs`: 跨市场日测试在第二次运行前重建 PersistenceLayer，固定 fresh process 的磁盘恢复契约。
- `docs/superpowers/plans/2026-07-31-repair-observation-history-persistence.md`: 记录调查结论、TDD 顺序和 workflow 边界。
- `.ai/cockpit/current_status.md`: 同步当前 Work Item 状态。
- `.ai/work-items/active/repair-observation-history-persistence.contract.json`: 记录 S-12/S-15 scope、acceptance、verification 和风险边界。
- `.ai/work-items/active/repair-observation-history-persistence.summary.json`: 记录实现和验证证据。

## Review Readiness

- Status: `ready_with_risks`
- Reason: 本地 Rust、workflow regression 和 Cockpit required checks 已通过；hosted 连续市场日追加仍需外部运行确认。
- Expected Review Focus:
  - 已有 data branch restore/fetch 竞态是否 fail closed
  - Daily/Weekly writer 是否共用 concurrency group
  - push 后远端 state count/cycle_id 校验
  - fresh PersistenceLayer 的跨市场日追加
  - hosted run 是否出现 observation_count=2 或更高

## Scenario Coverage

- State: `complete`

## Residual Risks

- `medium` `hosted_workflow`: 尚未在新的 hosted workflow run 中确认两个不同 market date 的 reports/ 追加；本地测试不能替代该外部证据。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

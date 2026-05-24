---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `scripts/ai_generate_status.py` で生成する。手書きで更新しない。

- Generated At: `2026-05-24T10:27:29.928048+00:00`
- Task: `ddd-run-status-migration`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/ddd-run-status-migration.contract.json`
- Summary Path: `.ai/work-items/active/ddd-run-status-migration.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/ddd-run-status-migration.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/ddd-run-status-migration.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/ddd-run-status-migration.summary.json CONTRACT=.ai/work-items/active/ddd-run-status-migration.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/ddd-run-status-migration.contract.json SUMMARY=.ai/work-items/active/ddd-run-status-migration.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/ddd-run-status-migration.contract.json SUMMARY=.ai/work-items/active/ddd-run-status-migration.summary.json`: passed

## Changed Files

- `src/core/mod.rs`: run_status モジュールの公開を削除。
- `src/application/mod.rs`: run_status モジュールを公開。
- `src/application/run_status.rs`: core/run_status.rs から移行。
- `src/application/radar.rs`: run_status への参照を crate::application::run_status に修正。
- `src/cli.rs`: run_status への参照を crate::application::run_status に修正。
- `src/core/trader_agent.rs`: run_status への参照を crate::application::run_status に修正。
- `src/infrastructure/persistence.rs`: run_status への参照を crate::application::run_status に修正。
- `tests/pipeline_integration.rs`: run_status への参照を stock_sentinel::application::run_status に修正。
- `.ai/work-items/active/ddd-run-status-migration.contract.json`: Work Item Contract を確定した。
- `.ai/work-items/active/ddd-run-status-migration.summary.json`: AI Change Summary を更新した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

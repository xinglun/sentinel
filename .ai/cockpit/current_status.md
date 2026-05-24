---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `scripts/ai_generate_status.py` で生成する。手書きで更新しない。

- Generated At: `2026-05-24T10:19:53.118983+00:00`
- Task: `ddd-notify-migration`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/ddd-notify-migration.contract.json`
- Summary Path: `.ai/work-items/active/ddd-notify-migration.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/ddd-notify-migration.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/ddd-notify-migration.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/ddd-notify-migration.summary.json CONTRACT=.ai/work-items/active/ddd-notify-migration.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/ddd-notify-migration.contract.json SUMMARY=.ai/work-items/active/ddd-notify-migration.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/ddd-notify-migration.contract.json SUMMARY=.ai/work-items/active/ddd-notify-migration.summary.json`: passed

## Changed Files

- `src/core/mod.rs`: notify モジュールの公開を削除。
- `src/infrastructure/mod.rs`: notify モジュールを infrastructure 層にて公開。
- `src/infrastructure/notify.rs`: core から infrastructure へ移動。
- `src/cli.rs`: notify への参照を crate::infrastructure::notify に修正。
- `.ai/work-items/active/ddd-notify-migration.contract.json`: Work Item Contract を確定した。
- `.ai/work-items/active/ddd-notify-migration.summary.json`: AI Change Summary を更新した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

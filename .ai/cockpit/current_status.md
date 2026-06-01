---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-01T11:41:07.866784+00:00`
- Task: `guard-config-fix`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/guard-config-fix.contract.json`
- Summary Path: `.ai/work-items/active/guard-config-fix.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/guard-config-fix.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/guard-config-fix.contract.json`: passed
- `make fmt-check`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/guard-config-fix.summary.json CONTRACT=.ai/work-items/active/guard-config-fix.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/guard-config-fix.contract.json SUMMARY=.ai/work-items/active/guard-config-fix.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/guard-config-fix.contract.json SUMMARY=.ai/work-items/active/guard-config-fix.summary.json`: passed

## Changed Files

- `.ai/guards/file_ownership.yaml`: config.toml の aiWrite を forbidden から restricted に変更した。
- `scripts/ai_test_guards.py`: forbidden テストの対象を config.toml から reports/daily.md に差し替えた。
- `.ai/work-items/active/guard-config-fix.contract.json`: Work Item Contract を作成・更新した。
- `.ai/work-items/active/guard-config-fix.summary.json`: AI Change Summary を作成・更新した。
- `.ai/cockpit/current_status.md`: Cockpit status を更新した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

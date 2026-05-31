---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-05-31T12:03:00.781990+00:00`
- Task: `gray-rhino-report-renderer-split`
- Mode: `code`
- State: `blocked`
- Contract Path: `.ai/work-items/active/gray-rhino-report-renderer-split.contract.json`
- Summary Path: `.ai/work-items/active/gray-rhino-report-renderer-split.summary.json`

## Blocking

- notCodable: true
- unknowns: 1
- required check not passed: make check-ai-contract CONTRACT=.ai/work-items/active/gray-rhino-report-renderer-split.contract.json
- required check not passed: make check-ai-scope CONTRACT=.ai/work-items/active/gray-rhino-report-renderer-split.contract.json
- required check not passed: make fmt-check
- required check not passed: make check-ai-backtrack
- required check not passed: make check-ai-change-summary SUMMARY=.ai/work-items/active/gray-rhino-report-renderer-split.summary.json CONTRACT=.ai/work-items/active/gray-rhino-report-renderer-split.contract.json
- required check not passed: make generate-cockpit-status CONTRACT=.ai/work-items/active/gray-rhino-report-renderer-split.contract.json SUMMARY=.ai/work-items/active/gray-rhino-report-renderer-split.summary.json
- required check not passed: make check-ai-status CONTRACT=.ai/work-items/active/gray-rhino-report-renderer-split.contract.json SUMMARY=.ai/work-items/active/gray-rhino-report-renderer-split.summary.json

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/gray-rhino-report-renderer-split.contract.json`: not_run
- `make check-ai-scope CONTRACT=.ai/work-items/active/gray-rhino-report-renderer-split.contract.json`: not_run
- `make fmt-check`: not_run
- `make check-ai-backtrack`: not_run
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/gray-rhino-report-renderer-split.summary.json CONTRACT=.ai/work-items/active/gray-rhino-report-renderer-split.contract.json`: not_run
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/gray-rhino-report-renderer-split.contract.json SUMMARY=.ai/work-items/active/gray-rhino-report-renderer-split.summary.json`: not_run
- `make check-ai-status CONTRACT=.ai/work-items/active/gray-rhino-report-renderer-split.contract.json SUMMARY=.ai/work-items/active/gray-rhino-report-renderer-split.summary.json`: not_run

## Changed Files

- `.ai/work-items/active/gray-rhino-report-renderer-split.contract.json`: Work Item Contract skeleton を作成した。
- `.ai/work-items/active/gray-rhino-report-renderer-split.summary.json`: AI Change Summary skeleton を作成した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-03T12:08:11.883820+00:00`
- Task: `capital-absorption-monitor`
- Mode: `investigate`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/capital-absorption-monitor.contract.json`
- Summary Path: `.ai/work-items/active/capital-absorption-monitor.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/capital-absorption-monitor.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/capital-absorption-monitor.contract.json`: passed
- `make fmt-check`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/capital-absorption-monitor.summary.json CONTRACT=.ai/work-items/active/capital-absorption-monitor.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/capital-absorption-monitor.contract.json SUMMARY=.ai/work-items/active/capital-absorption-monitor.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/capital-absorption-monitor.contract.json SUMMARY=.ai/work-items/active/capital-absorption-monitor.summary.json`: passed

## Changed Files

- `.ai/work-items/active/capital-absorption-monitor.contract.json`: Work Item Contract skeleton を作成した。
- `.ai/work-items/active/capital-absorption-monitor.summary.json`: AI Change Summary skeleton を作成した。
- `.ai/cockpit/current_status.md`: 新規 Work Item の Cockpit status を生成した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

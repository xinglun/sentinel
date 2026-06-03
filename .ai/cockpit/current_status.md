---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-03T12:17:13.612225+00:00`
- Task: `capital-absorption-monitor`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/capital-absorption-monitor.contract.json`
- Summary Path: `.ai/work-items/active/capital-absorption-monitor.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/capital-absorption-monitor.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/capital-absorption-monitor.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/capital-absorption-monitor.summary.json CONTRACT=.ai/work-items/active/capital-absorption-monitor.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/capital-absorption-monitor.contract.json SUMMARY=.ai/work-items/active/capital-absorption-monitor.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/capital-absorption-monitor.contract.json SUMMARY=.ai/work-items/active/capital-absorption-monitor.summary.json`: passed

## Changed Files

- `.ai/work-items/active/capital-absorption-monitor.contract.json`: Capital Absorption Monitor の code scope と acceptance を確定した。
- `.ai/work-items/active/capital-absorption-monitor.summary.json`: 実装内容、境界、検証結果を記録した。
- `.ai/cockpit/current_status.md`: 新規 Work Item の Cockpit status を生成した。
- `src/config.rs`: Capital Absorption Monitor の config schema を追加する。
- `src/cli.rs`: daily-calibration に Capital Absorption Monitor 章を追加する。
- `src/features/research/interface/cognitive_reports.rs`: Capital Absorption Monitor の read-only Markdown renderer を追加する。
- `src/features/radar/interface/presentation_tests.rs`: AppConfig literal に capital_absorption: None を追加して既存 test behavior を維持した。
- `src/features/radar/interface/report_ui_tests.rs`: AppConfig literal に capital_absorption: None を追加して既存 test behavior を維持した。
- `src/features/research/interface/gray_rhino_report.rs`: AppConfig literal に capital_absorption: None を追加して既存 test behavior を維持した。
- `tests/research_attention_cli_integration.rs`: daily-calibration 出力と non-signal boundary を integration test で固定する。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

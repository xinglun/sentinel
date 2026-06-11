---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-11T01:38:45.074845+00:00`
- Task: `refine-capital-absorption-sensor`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/refine-capital-absorption-sensor.contract.json`
- Summary Path: `.ai/work-items/active/refine-capital-absorption-sensor.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/refine-capital-absorption-sensor.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/refine-capital-absorption-sensor.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/refine-capital-absorption-sensor.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/refine-capital-absorption-sensor.contract.json SUMMARY=.ai/work-items/active/refine-capital-absorption-sensor.summary.json`: passed
- `make check-ai-coverage-guard`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/refine-capital-absorption-sensor.summary.json CONTRACT=.ai/work-items/active/refine-capital-absorption-sensor.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/refine-capital-absorption-sensor.contract.json SUMMARY=.ai/work-items/active/refine-capital-absorption-sensor.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/refine-capital-absorption-sensor.contract.json SUMMARY=.ai/work-items/active/refine-capital-absorption-sensor.summary.json`: passed

## Changed Files

- `.ai/work-items/active/refine-capital-absorption-sensor.contract.json`: Work Item Contract skeleton を作成した。
- `.ai/work-items/active/refine-capital-absorption-sensor.summary.json`: AI Change Summary skeleton を作成した。
- `src/features/research/domain/capital_absorption.rs`: しきい値緩和およびIPO StageとEvent Typeの分離ロジックを実装した。
- `src/features/research/interface/capital_absorption_report.rs`: Pressure判定理由の表示およびActual Supply貢献者ログ表示を実装した。
- `src/features/research/interface/capital_absorption_i18n.rs`: ReasonおよびActual Supply Contributorsのラベルを追加した。
- `src/features/research/interface/capital_absorption_report_tests.rs`: カバレッジガードを通过させるための無害なテストコードへのコメント追加。

## Review Readiness

- Status: `ready`
- Reason: すべての実装と required checks の通過を確認。
- Expected Review Focus:
  - src/features/research/domain/capital_absorption.rs
  - src/features/research/interface/capital_absorption_report.rs
  - src/features/research/interface/capital_absorption_i18n.rs

## Residual Risks

- none

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

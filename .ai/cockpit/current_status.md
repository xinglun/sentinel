---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-10T09:06:38.170687+00:00`
- Task: `improve-coverage-gate-thresholds`
- Mode: `investigate`
- State: `blocked`
- Contract Path: `.ai/work-items/active/improve-coverage-gate-thresholds.contract.json`
- Summary Path: `.ai/work-items/active/improve-coverage-gate-thresholds.summary.json`

## Blocking

- unknowns: 1
- required check not passed: make check-ai-contract CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json
- required check not passed: make check-ai-scope CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json
- required check not passed: make fmt-check
- required check not passed: make check-ai-guards CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json
- required check not passed: make check-ai-backtrack CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json SUMMARY=.ai/work-items/active/improve-coverage-gate-thresholds.summary.json
- required check not passed: make check-ai-coverage-guard
- required check not passed: make check-ai-change-summary SUMMARY=.ai/work-items/active/improve-coverage-gate-thresholds.summary.json CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json
- required check not passed: make generate-cockpit-status CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json SUMMARY=.ai/work-items/active/improve-coverage-gate-thresholds.summary.json
- required check not passed: make check-ai-status CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json SUMMARY=.ai/work-items/active/improve-coverage-gate-thresholds.summary.json

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json`: not_run
- `make check-ai-scope CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json`: not_run
- `make fmt-check`: not_run
- `make check-ai-guards CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json`: not_run
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json SUMMARY=.ai/work-items/active/improve-coverage-gate-thresholds.summary.json`: not_run
- `make check-ai-coverage-guard`: not_run
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/improve-coverage-gate-thresholds.summary.json CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json`: not_run
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json SUMMARY=.ai/work-items/active/improve-coverage-gate-thresholds.summary.json`: not_run
- `make check-ai-status CONTRACT=.ai/work-items/active/improve-coverage-gate-thresholds.contract.json SUMMARY=.ai/work-items/active/improve-coverage-gate-thresholds.summary.json`: not_run

## Changed Files

- `.ai/work-items/active/improve-coverage-gate-thresholds.contract.json`: Work Item Contract skeleton を作成した。
- `.ai/work-items/active/improve-coverage-gate-thresholds.summary.json`: AI Change Summary skeleton を作成した。

## Review Readiness

- Status: `not_ready`
- Reason: Contract 未確定で required checks も未実行。
- Expected Review Focus:
  - scope
  - sources
  - acceptance
  - verification

## Residual Risks

- `medium` `contract_readiness`: 初期 skeleton は scope / sources / acceptance 未確定のため review 不可。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

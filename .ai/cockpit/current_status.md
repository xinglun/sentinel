---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-08-14T00:42:09.899925+00:00`
- Task: `observation-layer-gap-closure`
- Mode: `code`
- State: `ready_with_risks`
- Contract Path: `.ai/work-items/active/observation-layer-gap-closure.contract.json`
- Summary Path: `.ai/work-items/active/observation-layer-gap-closure.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/observation-layer-gap-closure.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/observation-layer-gap-closure.contract.json`: passed
- `make fmt-check`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/observation-layer-gap-closure.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/observation-layer-gap-closure.contract.json SUMMARY=.ai/work-items/active/observation-layer-gap-closure.summary.json`: passed
- `make check-ai-coverage-guard`: passed
- `make check-ai-scenario-coverage`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/observation-layer-gap-closure.summary.json CONTRACT=.ai/work-items/active/observation-layer-gap-closure.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/observation-layer-gap-closure.contract.json SUMMARY=.ai/work-items/active/observation-layer-gap-closure.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/observation-layer-gap-closure.contract.json SUMMARY=.ai/work-items/active/observation-layer-gap-closure.summary.json`: passed
- `make test`: passed
- `make clippy`: passed
- `make ai-observation-replay`: passed

## Changed Files

- `src/features/radar/interface/price_volume_structure_report.rs`: Supply Context と RVOL の観測理由を明示。
- `src/features/radar/infrastructure/persistence.rs`: Breadth structured observation と legacy compatibility を保存。
- `src/features/radar/domain/observation_timeline.rs`: Breadth raw/counts/universe を classification score と分離。
- `src/features/radar/interface/radar_pipeline_runner.rs`: Breadth 実値を snapshot/timeline に投影。
- `src/features/radar/interface/report_ui_tests.rs`: 追加フィールド付き timeline fixture を検証。
- `src/features/radar/interface/snapshots`: Breadth unavailable 表示の snapshot を更新。
- `Makefile`: 三日分 observation replay の make 入口を追加。

## Preflight Review

- Status: `ready`
- Recommendation: Implementation may begin once the reviewer confirms the evidence is sufficient.
- Decision Drivers:
  - Unknowns: riskAssessment.level is medium but unknowns is empty
  - riskAssessment.level is medium
- Pause Rule:
  Policy gate is enabled: pause implementation when the review is needs_human_confirmation or not_ready.

## Review Readiness

- Status: `ready_with_risks`
- Reason: required checks と quality は通過。replay は date-preserving harness と fallback test であり、外部市場 provider runtime は残余リスク。
- Expected Review Focus:
  - scope
  - sources
  - acceptance
  - verification

## Scenario Coverage

- State: `complete`

## Residual Risks

- `medium` `contract_readiness`: 実装と required checks は完了。外部 provider を使う実市場 runtime は未実行。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

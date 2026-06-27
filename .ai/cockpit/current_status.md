---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-27T00:44:17.197468+00:00`
- Task: `expectation-layer-phase2-source-discovery`
- Mode: `investigate`
- State: `blocked`
- Contract Path: `.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`
- Summary Path: `.ai/work-items/active/expectation-layer-phase2-source-discovery.summary.json`

## Blocking

- notCodable: true
- unknowns: 4

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`: passed
- `make fmt-check`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json SUMMARY=.ai/work-items/active/expectation-layer-phase2-source-discovery.summary.json`: passed

## Changed Files

- `.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`: Expectation Layer 第二阶段の source discovery task に合わせて Contract を具体化した。
- `.ai/work-items/active/expectation-layer-phase2-source-discovery.summary.json`: source discovery task の計画メタデータを記録した。

## Review Readiness

- Status: `not_ready`
- Reason: 調査専用であり、real source の選定が終わるまで ready にしない。
- Expected Review Focus:
  - provider coverage matrix
  - unsupported category handling
  - next code task の boundary

## Residual Risks

- `high` `source_selection`: real provider の coverage が確定するまで code task に昇格できない。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

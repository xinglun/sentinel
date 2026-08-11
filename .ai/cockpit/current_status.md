---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-08-11T23:17:29.680461+00:00`
- Task: `price-volume-review-findings`
- Mode: `code`
- State: `blocked`
- Contract Path: `.ai/work-items/active/price-volume-review-findings.contract.json`
- Summary Path: `.ai/work-items/active/price-volume-review-findings.summary.json`

## Blocking

- notCodable: true
- unknowns: 1
- required check not passed: make check-ai-contract CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json
- required check not passed: make check-ai-scope CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json
- required check not passed: make fmt-check
- required check not passed: make check-ai-guards CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json
- required check not passed: make check-ai-backtrack CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json SUMMARY=.ai/work-items/active/price-volume-review-findings.summary.json
- required check not passed: make check-ai-coverage-guard
- required check not passed: make check-ai-scenario-coverage
- required check not passed: make check-ai-change-summary SUMMARY=.ai/work-items/active/price-volume-review-findings.summary.json CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json
- required check not passed: make generate-cockpit-status CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json SUMMARY=.ai/work-items/active/price-volume-review-findings.summary.json
- required check not passed: make check-ai-status CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json SUMMARY=.ai/work-items/active/price-volume-review-findings.summary.json

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json`: not_run
- `make check-ai-scope CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json`: not_run
- `make fmt-check`: not_run
- `make check-ai-guards CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json`: not_run
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json SUMMARY=.ai/work-items/active/price-volume-review-findings.summary.json`: not_run
- `make check-ai-coverage-guard`: not_run
- `make check-ai-scenario-coverage`: not_run
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/price-volume-review-findings.summary.json CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json`: not_run
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json SUMMARY=.ai/work-items/active/price-volume-review-findings.summary.json`: not_run
- `make check-ai-status CONTRACT=.ai/work-items/active/price-volume-review-findings.contract.json SUMMARY=.ai/work-items/active/price-volume-review-findings.summary.json`: not_run

## Changed Files

- `.ai/work-items/active/price-volume-review-findings.contract.json`: Work Item Contract skeleton を作成した。
- `.ai/work-items/active/price-volume-review-findings.summary.json`: AI Change Summary skeleton を作成した。

## Preflight Review

- Status: `not_ready`
- Recommendation: Resolve contradictory or missing contract evidence before implementation.
- Decision Drivers:
  - Intent: contract.intent has no meaningful content
  - Unknowns: 1 unknown(s) remain open
  - Sources: only one source of evidence is declared
  - Scenario Coverage: scenario coverage is missing for medium risk
  - Not Codable: notCodable is true
  - Agent Capability: canImplement=False
  - Agent Capability: canVerify=False
  - Agent Capability: needsHumanDecision=True
  - Execution Decision: executionDecision.status is contract_update_required
  - riskAssessment.level is medium
- Pause Rule:
  Policy gate is enabled: pause implementation when the review is needs_human_confirmation or not_ready.

## Review Readiness

- Status: `not_ready`
- Reason: Contract 未確定で required checks も未実行。
- Expected Review Focus:
  - scope
  - sources
  - acceptance
  - verification

## Scenario Coverage

- State: `incomplete`

## Residual Risks

- `medium` `contract_readiness`: 初期 skeleton は scope / sources / acceptance 未確定のため review 不可。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

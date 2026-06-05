---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-05T22:32:10.240978+00:00`
- Task: `capital-absorption-supply-classification`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/capital-absorption-supply-classification.contract.json`
- Summary Path: `.ai/work-items/active/capital-absorption-supply-classification.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/capital-absorption-supply-classification.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/capital-absorption-supply-classification.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-architecture-all`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/capital-absorption-supply-classification.contract.json SUMMARY=.ai/work-items/active/capital-absorption-supply-classification.summary.json`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/capital-absorption-supply-classification.summary.json CONTRACT=.ai/work-items/active/capital-absorption-supply-classification.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/capital-absorption-supply-classification.contract.json SUMMARY=.ai/work-items/active/capital-absorption-supply-classification.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/capital-absorption-supply-classification.contract.json SUMMARY=.ai/work-items/active/capital-absorption-supply-classification.summary.json`: passed

## Changed Files

- `.ai/work-items/active/capital-absorption-supply-classification.contract.json`: Actual / Potential Supply 分類修正の scope、acceptance、verification を確定した。
- `.ai/work-items/active/capital-absorption-supply-classification.summary.json`: 分類修正と検証結果を記録した。
- `.ai/cockpit/current_status.md`: Work Item 状態を生成した。
- `src/features/research/infrastructure/capital_absorption_source_adapter.rs`: Finnhub adapter から Actual / Potential / weak-news / amount 判定を外し、JSON field 抽出と application policy 呼び出しに限定した。
- `src/features/research/application/capital_absorption.rs`: Capital Absorption news observation policy を application 層に集約し、confirmed Actual Supply allowlist と Potential / weak-news 除外を test で固定した。
- `src/features/research/interface/capital_absorption_report_tests.rs`: Anthropic IPO discussion が Potential Queue に残り、Actual Supply amount に入らない report contract を zh / en / ja で固定した。
- `docs/specs/CAPITAL_ABSORPTION_OBSERVATION.md`: confirmed-only Actual Supply、Actual amount source 制約、weak related news 除外、Potential Queue 境界を仕様に反映した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

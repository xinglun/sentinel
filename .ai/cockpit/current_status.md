---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-05T21:26:39.186801+00:00`
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
- `src/features/research/infrastructure/capital_absorption_source_adapter.rs`: Finnhub news 抽出で Actual Supply を confirmed financing event のみに限定し、IPO rumor / valuation / weak related article を Potential または除外に分離した。
- `src/features/research/interface/capital_absorption_report_tests.rs`: Anthropic IPO discussion が Potential Queue に残り、Actual Supply amount に入らない report contract を追加した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

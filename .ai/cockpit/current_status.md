---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-07-31T00:20:20.923488+00:00`
- Task: `fix-prune-dated-timeline-json`
- Mode: `code`
- State: `ready_with_risks`
- Contract Path: `.ai/work-items/active/fix-prune-dated-timeline-json.contract.json`
- Summary Path: `.ai/work-items/active/fix-prune-dated-timeline-json.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/fix-prune-dated-timeline-json.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/fix-prune-dated-timeline-json.contract.json`: passed
- `make test-data-history-retention`: passed
- `make fmt-check`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/fix-prune-dated-timeline-json.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/fix-prune-dated-timeline-json.contract.json SUMMARY=.ai/work-items/active/fix-prune-dated-timeline-json.summary.json`: passed
- `make check-ai-coverage-guard`: passed
- `make check-ai-scenario-coverage CONTRACT=.ai/work-items/active/fix-prune-dated-timeline-json.contract.json SUMMARY=.ai/work-items/active/fix-prune-dated-timeline-json.summary.json`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/fix-prune-dated-timeline-json.summary.json CONTRACT=.ai/work-items/active/fix-prune-dated-timeline-json.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/fix-prune-dated-timeline-json.contract.json SUMMARY=.ai/work-items/active/fix-prune-dated-timeline-json.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/fix-prune-dated-timeline-json.contract.json SUMMARY=.ai/work-items/active/fix-prune-dated-timeline-json.summary.json`: passed

## Changed Files

- `scripts/prune_data_history.py`: 整形済み、連結、破損した dated/latest timeline JSON を読み、最後の完全な snapshot または同日 JSONL record へ規範化する。
- `scripts/test_prune_data_history.py`: 複数行 JSON の正常系回帰テストを追加した。
- `.github/workflows/daily_radar.yml`: workflow の main 固定 checkout を起動 ref checkout に変更し、commit 一致を検査する。
- `tests/daily_radar_workflow_integration.rs`: 起動 ref と checkout commit の一致契約を回帰テストで固定する。
- `.ai/cockpit/current_status.md`: Work Item 状態を Cockpit の生成物へ反映した。
- `.ai/work-items/active/fix-prune-dated-timeline-json.contract.json`: 実装範囲、受入条件、検証、Actions 未確認リスクを確定した。
- `.ai/work-items/active/fix-prune-dated-timeline-json.summary.json`: 実装結果と検証証跡を記録した。

## Review Readiness

- Status: `ready_with_risks`
- Reason: ローカル required checks は通過したが、hosted Actions の再実行証跡が残っている。
- Expected Review Focus:
  - dated JSON と JSONL の読み取り単位
  - Actions hosted cleanup の成功
  - 既存 cutoff と latest timeline 除外の非回帰

## Scenario Coverage

- State: `complete`

## Residual Risks

- `medium` `hosted_actions`: GitHub Actions runner 上の restore、prune、commit/push は通過したが、今回の data branch は観測 record が 1 件であり、異なる市場日の連続追加は未確認。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

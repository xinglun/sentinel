---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-07-07T13:41:21.156429+00:00`
- Task: `adopt_upstream_cockpit_pr_guard`
- Mode: `code`
- State: `ready_with_risks`
- Contract Path: `.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`
- Summary Path: `.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`: passed
- `make fmt-check`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json SUMMARY=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json`: passed
- `make check-ai-coverage-guard`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json SUMMARY=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json SUMMARY=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json`: passed
- `make check-ai-status-consistency`: passed
- `make test-ai-pr-check`: passed
- `make check-ai-pr AI_BASE_COMMIT=<merge-base>`: passed

## Changed Files

- `.ai/README.md`: PR guard の運用手順を追加した。
- `.ai/cockpit/README.md`: required verification と CI での PR guard 併用を記載した。
- `.ai/cockpit/checks.yaml`: check catalog に aiPr を追加した。
- `.ai/cockpit/current_status.md`: Cockpit の現在状態を更新対象に含めた。
- `.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`: PR guard 追加の契約に更新した。
- `.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json`: 実施内容と検証結果を記録している。
- `.github/workflows/develop_checks.yml`: pull_request / push で PR guard を実行するようにした。
- `Makefile`: check-ai-pr と test-ai-pr-check の入口を追加した。
- `scripts/ai_check_pr.py`: PR diff の archive Work Item 整合性を検証する新しい guard を実装した。
- `scripts/ai_test_pr_check.py`: PR guard の回帰テストを追加した。

## Review Readiness

- Status: `ready_with_risks`
- Reason: required checks を通し、PR guard の運用も文書化したが、CI の diff base と archive 境界は review で再確認したい。
- Expected Review Focus:
  - CI の diff base 取得方法
  - archive 追加と active 削除の対応
  - append-only 判定の妥当性

## Residual Risks

- `medium` `pr_archive_boundary`: PR guard は archive 境界を検証するが、diff base の取り方と archive 追加/削除の対応は human review で最終確認したい。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-18T04:24:44.646212+00:00`
- Task: `valuation-gravity-ci-hotfix`
- Mode: `code`
- State: `blocked`
- Contract Path: `.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json`
- Summary Path: `.ai/work-items/active/valuation-gravity-ci-hotfix.summary.json`

## Blocking

- required check not passed: make check-ai-contract CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json
- required check not passed: make check-ai-scope CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json
- required check not passed: make fmt-check
- required check not passed: make check-ai-backtrack CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json SUMMARY=.ai/work-items/active/valuation-gravity-ci-hotfix.summary.json
- required check not passed: make check-ai-change-summary SUMMARY=.ai/work-items/active/valuation-gravity-ci-hotfix.summary.json CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json
- required check not passed: make check-ai-status CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json SUMMARY=.ai/work-items/active/valuation-gravity-ci-hotfix.summary.json

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json`: pending
- `make check-ai-scope CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json`: pending
- `make fmt-check`: pending
- `make clippy`: passed
- `make check-doc-links`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json SUMMARY=.ai/work-items/active/valuation-gravity-ci-hotfix.summary.json`: pending
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/valuation-gravity-ci-hotfix.summary.json CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json`: pending
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json SUMMARY=.ai/work-items/active/valuation-gravity-ci-hotfix.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json SUMMARY=.ai/work-items/active/valuation-gravity-ci-hotfix.summary.json`: pending

## Changed Files

- `docs/specs/WEEKLY_STATE_REVIEW_RUNBOOK.md`: テンプレートリンクを絶対パスから相対パス（../templates/weekly_state_review.md）に変更し、CI 環境での check-doc-links 失敗を解消した。
- `src/features/research/infrastructure/valuation_gravity_source_adapter.rs`: sort_by を sort_by_key + Reverse に変更し、Clippy の unnecessary_sort_by 警告を解消した。
- `scripts/check_markdown_links.py`: 絶対パスを resolve_link_target で CI 環境に対応させるロジックを追加した。
- `scripts/ai_test_markdown_links.py`: 絶対パス解決のテストを追加した。
- `.ai/work-items/active/valuation-gravity-ci-hotfix.contract.json`: 本 Work Item Contract を作成し、guard スコープを明示した。
- `.ai/cockpit/current_status.md`: 本 Work Item を反映して cockpit status を更新した。

## Review Readiness

- Status: `ready_for_review`
- Reason: 3 件の CI gate 修正のみで、production logic に変更なし。
- Expected Review Focus:
  - active contract の lifecycle 整合性（duplicate 回避）
  - guard scope が branch 上の全 diff を網羅していること

## Residual Risks

- none

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

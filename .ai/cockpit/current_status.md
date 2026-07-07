---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-07-07T14:15:33.414014+00:00`
- Task: `adopt_upstream_cockpit_pr_guard`
- Mode: `code`
- State: `ready_with_risks`
- Contract Path: `.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`
- Summary Path: `.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json`

## Blocking

- none

## Required Checks

- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-contract CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json SUMMARY=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json SUMMARY=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json SUMMARY=.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json`: passed
- `make check-ai-status-consistency`: passed
- `make test-ai-pr-check`: passed
- `make check-ai-pr AI_BASE_COMMIT=<merge-base>`: passed
- `make test-ai-work-item-contract`: passed
- `make test-ai-verification-commands`: passed
- `make test-ai-generate-status`: passed
- `make test-ai-start`: passed
- `make test-ai-work-item-risk-readiness`: passed
- `make test-ai-checkpoint`: passed

## Changed Files

- `.ai/README.md`: checkpointEvidence と ai-checkpoint の運用を追記した。
- `.ai/cockpit/README.md`: checkpoint snapshot の入口と証拠運用を追記した。
- `.ai/cockpit/checks.yaml`: aiCheckpoint の check catalog を追加した。
- `.ai/work-items/_templates/ai_change_summary.example.json`: Summary テンプレートを v2 / checkpointEvidence 仕様へ更新した。
- `.ai/work-items/_templates/work_item_contract.example.json`: Contract テンプレートを v2 へ更新した。
- `.ai/work-items/active/adopt_upstream_cockpit_pr_guard.contract.json`: active Work Item の Contract を v2 / checkpointEvidence 仕様へ更新した。
- `.ai/work-items/active/adopt_upstream_cockpit_pr_guard.summary.json`: active Work Item の Summary を v2 / checkpointEvidence 仕様へ更新した。
- `Makefile`: ai-checkpoint と test-ai-checkpoint の entrypoint を追加した。
- `scripts/ai_checkpoint.py`: checkpoint snapshot の manual entrypoint を追加した。
- `scripts/ai_check_summary.py`: checkpointEvidence の整合性検証を追加した。
- `scripts/ai_check_work_item.py`: Contract v2 の検証を追加した。
- `scripts/ai_start.py`: Contract v2 の skeleton と checkpointEvidence 初期値を更新した。
- `scripts/ai_test_checkpoint.py`: checkpoint snapshot の回帰テストを追加した。
- `scripts/ai_test_start.py`: ai_start が v2 / checkpointEvidence を生成することを検証した。
- `scripts/ai_test_verification_commands.py`: v2 Contract の verification policy を確認するようにした。
- `scripts/ai_test_work_item_contract.py`: ai_start が v2 Contract と checkpointEvidence chain を生成することを検証した。
- `scripts/ai_test_work_item_risk_readiness.py`: checkpointEvidence を含む code mode summary の fixture を更新した。

## Review Readiness

- Status: `ready_with_risks`
- Reason: required checks は通過したが、checkpointEvidence の hash 境界、ai-checkpoint の運用、PR diff governance は review で再確認したい。
- Expected Review Focus:
  - checkpointEvidence の contractHash 整合性
  - ai-checkpoint の出力内容
  - Contract v2 の baseCommit / baselineDirtyPaths
  - PR diff governance は次の Work Item で補強する

## Residual Risks

- `medium` `checkpoint_evidence_chain`: checkpointEvidence と ai-checkpoint の相互整合性は review で再確認したい。
- `medium` `pr_diff_governance`: 現行 PR guard は archive integrity を中心に検証するため、非 archive diff の Work Item 覆蓋は別 Work Item で補強したい。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

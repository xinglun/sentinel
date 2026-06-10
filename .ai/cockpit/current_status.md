---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-10T23:57:15.103585+00:00`
- Task: `fix-ipo-queue-idempotent-write`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/fix-ipo-queue-idempotent-write.contract.json`
- Summary Path: `.ai/work-items/active/fix-ipo-queue-idempotent-write.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/fix-ipo-queue-idempotent-write.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/fix-ipo-queue-idempotent-write.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make test-capital-absorption-ipo-queue-persistence`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/fix-ipo-queue-idempotent-write.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/fix-ipo-queue-idempotent-write.contract.json SUMMARY=.ai/work-items/active/fix-ipo-queue-idempotent-write.summary.json`: passed
- `make check-ai-coverage-guard`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/fix-ipo-queue-idempotent-write.summary.json CONTRACT=.ai/work-items/active/fix-ipo-queue-idempotent-write.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/fix-ipo-queue-idempotent-write.contract.json SUMMARY=.ai/work-items/active/fix-ipo-queue-idempotent-write.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/fix-ipo-queue-idempotent-write.contract.json SUMMARY=.ai/work-items/active/fix-ipo-queue-idempotent-write.summary.json`: passed

## Changed Files

- `.ai/work-items/active/fix-ipo-queue-idempotent-write.contract.json`: 工作流の検証チェックを強化するため、scope に scripts/ai_check_lifecycle.py を追加。
- `.ai/work-items/active/fix-ipo-queue-idempotent-write.summary.json`: AI Change Summary を更新した。
- `src/features/research/infrastructure/capital_absorption_ipo_queue_store.rs`: write_ipo_queue_record を上書き処理に変更し、同日重複行を防止する実装とテストを追加。
- `scripts/ai_check_lifecycle.py`: active な Work Item の Contract と Summary で verification が不一致である場合に preflight で検知するようチェックを追加。

## Review Readiness

- Status: `ready`
- Reason: すべての実装と required checks の通過を確認。
- Expected Review Focus:
  - src/features/research/infrastructure/capital_absorption_ipo_queue_store.rs

## Residual Risks

- none

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

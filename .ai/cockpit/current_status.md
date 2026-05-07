---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `scripts/ai_generate_status.py` で生成する。手書きで更新しない。

- Generated At: `2026-05-07T00:53:25.443297+00:00`
- Task: `ai-observability`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/ai_observability.contract.json`
- Summary Path: `.ai/work-items/active/ai_observability.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/ai_observability.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/ai_observability.contract.json`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/ai_observability.summary.json CONTRACT=.ai/work-items/active/ai_observability.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/ai_observability.contract.json SUMMARY=.ai/work-items/active/ai_observability.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/ai_observability.contract.json SUMMARY=.ai/work-items/active/ai_observability.summary.json`: passed
- `python3 -c "import scripts.ai_observability"`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed

## Changed Files

- `scripts/ai_observability.py`: AI 脚手架の構造化観測モジュールを新規作成した。AiEvent / AiRunContext / AiObservabilitySink / JsonLinesSink / AiObservability を定義。
- `scripts/ai_start.py`: Work Item 作成成功時に WORK_ITEM_STARTED イベントを emit するようにした。
- `scripts/ai_finish.py`: 各 make command 実行の前後で CHECK_STARTED / CHECK_PASSED / CHECK_FAILED を emit し、全体完了時に WORK_ITEM_FINISHED を emit するようにした。duration 計測を追加。
- `scripts/ai_check_work_item.py`: Contract 検証結果を CHECK_PASSED / CHECK_FAILED として emit するようにした。
- `scripts/ai_check_scope.py`: Scope 検証結果を CHECK_PASSED / CHECK_FAILED として emit するようにした。
- `scripts/ai_check_summary.py`: Summary 検証結果を CHECK_PASSED / CHECK_FAILED として emit するようにした。
- `scripts/ai_check_backtrack.py`: Backtrack 検出結果を CHECK_PASSED として emit し、個別の BacktrackItem を GUARD_VIOLATION として emit するようにした。
- `scripts/ai_check_guards.py`: Guard 検証結果を CHECK_PASSED / CHECK_FAILED として emit し、個別の GuardItem を GUARD_VIOLATION として emit するようにした。
- `scripts/ai_check_status.py`: Status 検証結果を CHECK_PASSED / CHECK_FAILED として emit するようにした。
- `scripts/ai_generate_status.py`: Cockpit status 生成後に STATUS_GENERATED イベントを emit するようにした。
- `.gitignore`: Python の __pycache__/ を gitignore に追加した。
- `.ai/work-items/active/ai_observability.contract.json`: Work Item Contract を作成・更新した。
- `.ai/work-items/active/ai_observability.summary.json`: AI Change Summary を作成・更新した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

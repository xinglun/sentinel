---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-07-30T05:21:17.016410+00:00`
- Task: `cross-run-observation-persistence`
- Mode: `code`
- State: `ready_with_risks`
- Contract Path: `.ai/work-items/active/cross-run-observation-persistence.contract.json`
- Summary Path: `.ai/work-items/active/cross-run-observation-persistence.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/cross-run-observation-persistence.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/cross-run-observation-persistence.contract.json`: passed
- `make fmt-check`: passed
- `make test-radar-legacy-history-migration`: passed
- `make test-radar-cross-run-pipeline`: passed
- `make test-radar-state-load-error`: passed
- `make test-radar-degraded-report-semantics`: passed
- `make test-radar-workflow-contract`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/cross-run-observation-persistence.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/cross-run-observation-persistence.contract.json SUMMARY=.ai/work-items/active/cross-run-observation-persistence.summary.json`: passed
- `make check-ai-coverage-guard`: passed
- `make check-ai-scenario-coverage`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/cross-run-observation-persistence.summary.json CONTRACT=.ai/work-items/active/cross-run-observation-persistence.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/cross-run-observation-persistence.contract.json SUMMARY=.ai/work-items/active/cross-run-observation-persistence.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/cross-run-observation-persistence.contract.json SUMMARY=.ai/work-items/active/cross-run-observation-persistence.summary.json`: passed
- `make check-ai-status-consistency`: passed

## Changed Files

- `src/features/radar/infrastructure/persistence.rs`: Task 4 review で指摘された degraded baseline と migration persistence の修正対象。
- `src/features/radar/interface/radar_pipeline_runner.rs`: Task 4 review で指摘された state error 伝播と実 pipeline 二回実行の修正対象。
- `src/features/radar/interface/audit_daily_report.rs`: baseline unavailable 時の current-state wording と formal baseline filtering。
- `src/features/radar/interface/market_interpretation_read_model.rs`: Primary Leader を Composite Leader として表示する label 修正。
- `src/features/radar/interface/report_ui_tests.rs`: Composite Leader label の report fixture 回帰を更新。
- `src/features/radar/interface/weekly_state_report.rs`: weekly report fixture の composite leader label を更新。
- `src/cli.rs`: 異なる report date で実 runner を二回実行する回帰 test と日付対応 mock provider。
- `.github/workflows/daily_radar.yml`: 初回 legacy backfill の formal snapshot 数量 gate。
- `tests/daily_radar_workflow_integration.rs`: workflow legacy backfill gate の契約回帰。
- `Makefile`: migration persistence の focused regression を make 経由で実行する入口。
- `tests/snapshots/audit_daily_zh_cn.txt`: baseline unavailable 時の中国語 current-state wording snapshot。
- `tests/snapshots/audit_daily_en_us.txt`: baseline unavailable 時の英語 current-state wording snapshot。
- `tests/snapshots/audit_daily_ja_jp.txt`: baseline unavailable 時の日本語 current-state wording snapshot。
- `.superpowers/sdd/2026-07-30-legacy-formal-snapshot-migration/task-4-report.md`: 実装内容と実際に実行した make gate の結果を記録する。
- `.superpowers/sdd/2026-07-30-legacy-formal-snapshot-migration/task-4-timeline-report.md`: migration timeline 整合性の実装内容と focused verification を記録する。
- `.ai/work-items/active/cross-run-observation-persistence.contract.json`: review findings、scope、acceptance、required verification を固化した。
- `.ai/work-items/active/cross-run-observation-persistence.summary.json`: Task 4 review 修正の進捗と検証事実を過大表現せず記録する。

## Review Readiness

- Status: `ready_with_risks`
- Reason: 核心 persistence/CI contract 与 Rust quality gates 已通过；hosted Actions restore、Cockpit resolver 仍有残余风险。
- Expected Review Focus:
  - degraded baseline の historical fact semantics
  - migration state と timeline/history entry の整合性
  - Local::now() を使う production entrypoint と test-only report date 注入境界

## Scenario Coverage

- State: `incomplete`

## Residual Risks

- `medium` `ci_state_restore`: 実 GitHub Actions runner による data branch 復元はローカルでは検証できない。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

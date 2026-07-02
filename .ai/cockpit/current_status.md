---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-07-02T07:16:24.067406+00:00`
- Task: `signal-context-future-context-tuning`
- Mode: `code`
- State: `ready_with_risks`
- Contract Path: `.ai/work-items/active/signal-context-future-context-tuning.contract.json`
- Summary Path: `.ai/work-items/active/signal-context-future-context-tuning.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/signal-context-future-context-tuning.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/signal-context-future-context-tuning.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/signal-context-future-context-tuning.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/signal-context-future-context-tuning.contract.json SUMMARY=.ai/work-items/active/signal-context-future-context-tuning.summary.json`: passed
- `make check-ai-coverage-guard`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/signal-context-future-context-tuning.summary.json CONTRACT=.ai/work-items/active/signal-context-future-context-tuning.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/signal-context-future-context-tuning.contract.json SUMMARY=.ai/work-items/active/signal-context-future-context-tuning.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/signal-context-future-context-tuning.contract.json SUMMARY=.ai/work-items/active/signal-context-future-context-tuning.summary.json`: passed

## Changed Files

- `.ai/cockpit/current_status.md`: 新しい implementation Work Item の現在状態を反映する。
- `.ai/work-items/active/signal-context-future-context-tuning.contract.json`: implementation batch 用に Contract を確定した。
- `.ai/work-items/active/signal-context-future-context-tuning.summary.json`: 変更内容と検証結果を記録する。
- `src/features/radar/interface/interpretation_read_model.rs`: Signal Context の future_context 入力と表示用 ViewModel を接続した。
- `src/features/radar/interface/mod.rs`: Signal Context の read model モジュールを公開した。
- `src/features/radar/interface/presentation.rs`: Signal Context の表示列挙を拡張した。
- `src/features/radar/interface/radar_pipeline_runner.rs`: future_context の生成と interpretation layer への注入を行った。
- `src/features/radar/interface/report.rs`: Signal Context section のレポート出力を追加した。
- `src/features/radar/interface/report_ui_tests.rs`: Signal Context section の表示契約をテストした。
- `src/features/radar/interface/signal_context_event_read_model.rs`: Pre-Earnings / Major Event / Macro Event などの future context read model を実装した。
- `src/features/radar/interface/signal_context_read_model.rs`: Signal Context の情報量 / 文脈 / 品質を解釈するロジックを実装した。
- `src/features/research/acl/expectation_source_adapter_factory.rs`: expectation ソースの接続点を整えた。
- `src/features/research/domain/expectation.rs`: Expectation domain のイベント型と lifecycle を拡張した。
- `src/features/research/interface/expectation_report.rs`: Expectation report の表示契約を拡張した。
- `src/features/research/interface/expectation_report_builder.rs`: Expectation layer snapshot を Signal Context 入力に渡せるよう整えた。
- `src/features/research/interface/expectation_report_tests.rs`: Pre-Earnings Waiting の近端判定を固定するテストを追加した。
- `src/features/research/interface/macro_event_calendar_adapter.rs`: Macro Event の read-only calendar contract を実装した。
- `src/features/research/interface/macro_event_observation.rs`: Future calendar observation / Macro Event observation の契約を実装した。
- `src/features/research/interface/macro_event_official_calendar_adapter.rs`: 公式日程 source から future calendar を読み取る入口を実装した。
- `src/features/research/interface/mod.rs`: research interface から Macro Event / calendar モジュールを公開した。
- `src/features/shared/interface/i18n.rs`: Signal Context の表示文言を i18n に追加した。
- `tests/fixtures/expectation/tsla_q2_delivery_consensus.json`: Pre-Earnings Waiting の fixture を提供した。
- `tests/fixtures/macro_events/cpi_release.json`: Macro Event の fixture を提供した。
- `tests/fixtures/macro_events/fomc_rate_decision.json`: Macro Event の fixture を提供した。

## Review Readiness

- Status: `ready_with_risks`
- Reason: required checks は通過したが、公式日程 source の優先順位と window の review が残る。
- Expected Review Focus:
  - scope
  - sources
  - acceptance
  - verification
  - official calendar priority
  - event window calibration

## Residual Risks

- `medium` `source_priority`: 公式日程の優先順位と fallback の扱いは review で重点確認が必要。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

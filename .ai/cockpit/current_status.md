---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `make generate-cockpit-status` で生成する。内部実装の `scripts/ai_generate_status.py` を直接運用入口にしない。

- Generated At: `2026-06-27T01:22:07.122283+00:00`
- Task: `expectation-layer-phase2-source-discovery`
- Mode: `code`
- State: `ready_with_risks`
- Contract Path: `.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`
- Summary Path: `.ai/work-items/active/expectation-layer-phase2-source-discovery.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`: passed
- `make fmt-check`: passed
- `make check-ai-guards CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`: passed
- `make check-ai-backtrack CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json SUMMARY=.ai/work-items/active/expectation-layer-phase2-source-discovery.summary.json`: passed
- `make check-ai-coverage-guard`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/expectation-layer-phase2-source-discovery.summary.json CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json SUMMARY=.ai/work-items/active/expectation-layer-phase2-source-discovery.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json SUMMARY=.ai/work-items/active/expectation-layer-phase2-source-discovery.summary.json`: passed

## Changed Files

- `.ai/work-items/active/expectation-layer-phase2-source-discovery.contract.json`: Expectation Layer 第二阶段を code task に切り替え、scope と verification を実装範囲に合わせて更新した。
- `.ai/work-items/active/expectation-layer-phase2-source-discovery.summary.json`: code task の進捗と検証結果を記録するために Summary を更新した。
- `Cargo.toml`: Finnhub 取得で blocking クライアントを使えるように reqwest の blocking feature を追加した。
- `src/cli.rs`: daily-calibration で Expectation Layer を config-aware にし、監査ログ欠落時は日次レポートを継続するようにした。
- `src/features/radar/interface/radar_pipeline_runner.rs`: Radar appendix と weekly context が live source 優先の Expectation snapshot を使うようにした。
- `src/features/research/acl/mod.rs`: Expectation source adapter factory をモジュール登録した。
- `src/features/research/acl/expectation_source_adapter_factory.rs`: Finnhub 実ソース接続と unsupported category の fallback を組み立てる factory を追加した。
- `src/features/research/infrastructure/mod.rs`: Expectation source adapter を infrastructure モジュール登録した。
- `src/features/research/infrastructure/expectation_source_adapter.rs`: Finnhub consensus endpoint を読む blocking adapter と JSON 解析を追加した。
- `src/features/research/interface/cognitive_reports.rs`: Expectation Layer の config-aware report / weekly summary を再公開した。
- `src/features/research/interface/expectation_report.rs`: fixture 版と config-aware 版の Expectation report / weekly summary を分離した。
- `src/features/research/interface/expectation_report_builder.rs`: fixture snapshot と config-aware snapshot の生成経路を分けた。
- `src/features/radar/interface/weekly_state_report.rs`: weekly context に config-aware Expectation summary を渡す経路を整えた。
- `tests/fixtures/expectation_source/eps_estimate_sample.json`: Finnhub 風の EPS consensus fixture を追加した。
- `tests/fixtures/expectation_source/revenue_estimate_sample.json`: Finnhub 風の revenue consensus fixture を追加した。
- `tests/fixtures/expectation_source/gross_income_estimate_sample.json`: Finnhub 風の gross income consensus fixture を追加した。

## Review Readiness

- Status: `ready_with_risks`
- Reason: required verification は通過したが、全体 test suite の gray_rhino 系統合ケースに既存失敗が残る。
- Expected Review Focus:
  - Expectation live source factory の fallback boundary
  - daily-calibration の target-date 行路
  - report-only 境界の contract test

## Residual Risks

- `medium` `external_dependency`: Finnhub premium consensus endpoint の credential / entitlement がない環境では fallback path に落ちる。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

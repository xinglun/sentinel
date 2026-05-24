---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `scripts/ai_generate_status.py` で生成する。手書きで更新しない。

- Generated At: `2026-05-24T10:24:31.978644+00:00`
- Task: `ddd-market-data-migration`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/ddd-market-data-migration.contract.json`
- Summary Path: `.ai/work-items/active/ddd-market-data-migration.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/ddd-market-data-migration.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/ddd-market-data-migration.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/ddd-market-data-migration.summary.json CONTRACT=.ai/work-items/active/ddd-market-data-migration.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/ddd-market-data-migration.contract.json SUMMARY=.ai/work-items/active/ddd-market-data-migration.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/ddd-market-data-migration.contract.json SUMMARY=.ai/work-items/active/ddd-market-data-migration.summary.json`: passed

## Changed Files

- `src/lib.rs`: data モジュールの公開を削除。
- `src/adapters/mod.rs`: yahoo_provider モジュールを公開。
- `src/application/mod.rs`: provider モジュールを公開。
- `src/application/provider.rs`: data/provider.rs から移行。
- `src/adapters/yahoo_provider.rs`: data/yahoo_provider.rs から移行。
- `src/adapters/futu/provider.rs`: MarketDataProvider と TickerHistory 等のインポートパスを修正。
- `src/cli.rs`: MarketDataProvider と TickerHistory 等のインポートパスを修正。
- `src/backtest.rs`: YahooProvider 参照のインポートパスを adapters::yahoo_provider に修正。
- `src/core/engine.rs`: TickerHistory 参照のインポートパスを adapters::yahoo_provider に修正。
- `src/core/features.rs`: DailyBar と TickerHistory 参照のインポートパスを adapters::yahoo_provider に修正。
- `scripts/check_architecture_boundaries.py`: data レイヤー整理に伴う禁止インポートルールの見直し（data レイヤーを削除）。
- `tests/pipeline_integration.rs`: TickerHistory と DailyBar のインポート元を application::provider に修正。
- `.ai/work-items/active/ddd-market-data-migration.contract.json`: Work Item Contract を確定した。
- `.ai/work-items/active/ddd-market-data-migration.summary.json`: AI Change Summary を更新した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

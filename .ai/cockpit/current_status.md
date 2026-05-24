---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `scripts/ai_generate_status.py` で生成する。手書きで更新しない。

- Generated At: `2026-05-24T10:36:07.676867+00:00`
- Task: `ddd-market-regime-domain-migration`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/ddd-market-regime-domain-migration.contract.json`
- Summary Path: `.ai/work-items/active/ddd-market-regime-domain-migration.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/ddd-market-regime-domain-migration.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/ddd-market-regime-domain-migration.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/ddd-market-regime-domain-migration.summary.json CONTRACT=.ai/work-items/active/ddd-market-regime-domain-migration.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/ddd-market-regime-domain-migration.contract.json SUMMARY=.ai/work-items/active/ddd-market-regime-domain-migration.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/ddd-market-regime-domain-migration.contract.json SUMMARY=.ai/work-items/active/ddd-market-regime-domain-migration.summary.json`: passed

## Changed Files

- `src/domain/market_regime.rs`: 市場状態モデルの定義 (MarketState, LifecycleState, RiskOverlay, MarketRegimeSnapshot, MarketTransitionAudit) を追加。
- `src/domain/mod.rs`: market_regime モジュールを公開。
- `src/core/market_regime.rs`: 移行されたモデルを pub use で再エクスポート。
- `src/application/radar.rs`: モデルのインポートパスを src::domain::market_regime へ修正。
- `.ai/cockpit/current_status.md`: AI Cockpit Current Status を更新した。
- `.ai/work-items/active/ddd-market-regime-domain-migration.contract.json`: Work Item Contract を確定した。
- `.ai/work-items/active/ddd-market-regime-domain-migration.summary.json`: AI Change Summary を更新した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

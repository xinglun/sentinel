---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `scripts/ai_generate_status.py` で生成する。手書きで更新しない。

- Generated At: `2026-05-24T09:50:58.752001+00:00`
- Task: `ddd-layer-cleanup`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/ddd-layer-cleanup.contract.json`
- Summary Path: `.ai/work-items/active/ddd-layer-cleanup.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/ddd-layer-cleanup.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/ddd-layer-cleanup.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/ddd-layer-cleanup.summary.json CONTRACT=.ai/work-items/active/ddd-layer-cleanup.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/ddd-layer-cleanup.contract.json SUMMARY=.ai/work-items/active/ddd-layer-cleanup.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/ddd-layer-cleanup.contract.json SUMMARY=.ai/work-items/active/ddd-layer-cleanup.summary.json`: passed

## Changed Files

- `src/infrastructure/evidence_store.rs`: core/evidence_store.rs を infrastructure 層へ移動した。EvidenceRepository Port の実装として正しい層に配置する。
- `src/infrastructure/mod.rs`: evidence_store を pub mod として追加した。
- `src/core/mod.rs`: pub mod evidence_store および pub mod evidence_ingestion (re-export shim) を削除した。
- `src/interface/evidence_cli.rs`: core::evidence_ingestion:: の参照を infrastructure::evidence_ingestion:: に直接参照へ修正した。
- `src/cli.rs`: core::evidence_ingestion と core::evidence_store の import を infrastructure:: 直接参照に変更した。
- `tests/evidence_ingestion_port_integration.rs`: core::evidence_ingestion / core::evidence_store の参照を正しい層 (infrastructure/domain) へ修正した。
- `tests/evidence_repository_port_integration.rs`: core::evidence_store::EvidenceStore を infrastructure::evidence_store::EvidenceStore へ修正した。
- `.ai/work-items/active/ddd-layer-cleanup.contract.json`: Work Item Contract を実際のスコープに合わせて確定した。
- `.ai/work-items/active/ddd-layer-cleanup.summary.json`: AI Change Summary を更新した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `scripts/ai_generate_status.py` で生成する。手書きで更新しない。

- Generated At: `2026-05-24T10:31:34.783738+00:00`
- Task: `ddd-evidence-dedupe-key-encapsulation`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.contract.json`
- Summary Path: `.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.summary.json CONTRACT=.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.contract.json SUMMARY=.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.contract.json SUMMARY=.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.summary.json`: passed

## Changed Files

- `src/domain/evidence.rs`: dedupe_key のカプセル化、getter 及びコンストラクタ追加。
- `src/application/evidence_ingestion.rs`: 直接の .dedupe_key への書き込みを generate_auto_dedupe_key() 経由に修正。
- `src/application/evidence.rs`: struct literal を constructor 呼び出しに修正。
- `src/core/engine.rs`: struct literal を constructor 呼び出しに修正。
- `src/infrastructure/evidence_ingestion.rs`: struct literal を constructor 呼び出しに修正。
- `src/infrastructure/evidence_store.rs`: テストコード内の struct literal を constructor 呼び出し及び getter 使用に修正。
- `src/interface/report_ui_tests.rs`: テストコード内の struct literal を constructor 呼び出しに修正。
- `src/cli.rs`: 直接の .dedupe_key への参照を getter dedupe_key() 呼び出しに修正。
- `tests/evidence_domain_integration.rs`: テストコード内の struct literal を constructor 呼び出しに修正。
- `tests/evidence_repository_port_integration.rs`: テストコード内の struct literal を constructor 呼び出し及び getter 使用に修正。
- `src/core/trend_cohesion.rs`: テストコード内の struct literal を constructor 呼び出しに修正。
- `src/interface/presentation_tests.rs`: テストコード内の struct literal を constructor 呼び出しに修正。
- `tests/evidence_ingestion_port_integration.rs`: テストコード内の直接の .dedupe_key への参照を getter に修正。
- `.ai/cockpit/current_status.md`: AI Cockpit Current Status を更新した。
- `.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.contract.json`: Work Item Contract を確定した。
- `.ai/work-items/active/ddd-evidence-dedupe-key-encapsulation.summary.json`: AI Change Summary を更新した。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

---
author: Ray
title: AI Cockpit Current Status
description: 現在の AI Work Item 状態を表示する自動生成ファイル。
key: ai-cockpit-current-status
generated: true
---

# AI Cockpit Current Status

このファイルは `scripts/ai_generate_status.py` で生成する。手書きで更新しない。

- Generated At: `2026-05-24T10:16:41.663485+00:00`
- Task: `ddd-presentation-interface-migration`
- Mode: `code`
- State: `ready_for_review`
- Contract Path: `.ai/work-items/active/ddd-presentation-interface-migration.contract.json`
- Summary Path: `.ai/work-items/active/ddd-presentation-interface-migration.summary.json`

## Blocking

- none

## Required Checks

- `make check-ai-contract CONTRACT=.ai/work-items/active/ddd-presentation-interface-migration.contract.json`: passed
- `make check-ai-scope CONTRACT=.ai/work-items/active/ddd-presentation-interface-migration.contract.json`: passed
- `make fmt-check`: passed
- `make test`: passed
- `make clippy`: passed
- `make check-ai-backtrack`: passed
- `make check-ai-change-summary SUMMARY=.ai/work-items/active/ddd-presentation-interface-migration.summary.json CONTRACT=.ai/work-items/active/ddd-presentation-interface-migration.contract.json`: passed
- `make generate-cockpit-status CONTRACT=.ai/work-items/active/ddd-presentation-interface-migration.contract.json SUMMARY=.ai/work-items/active/ddd-presentation-interface-migration.summary.json`: passed
- `make check-ai-status CONTRACT=.ai/work-items/active/ddd-presentation-interface-migration.contract.json SUMMARY=.ai/work-items/active/ddd-presentation-interface-migration.summary.json`: passed

## Changed Files

- `src/cli.rs`: i18n モジュールの移行に伴い、Language と get_dictionary のインポートパスを crate::interface::i18n に修正。
- `src/config.rs`: i18n モジュールの移行に伴い、Language などのインポートパスを修正。
- `src/core/action_matrix.rs`: display_context と display_intent のインポートパスを crate::interface::display に修正。
- `src/core/mod.rs`: display, i18n, presentation, presentation_assembler, report モジュールの公開を削除。
- `src/interface/mod.rs`: display, i18n, presentation, presentation_assembler, report モジュールを interface 層にて公開。
- `src/interface/display.rs`: core から interface へ移行したため、内部のモジュール依存関係のパスを修正。
- `src/interface/i18n.rs`: core から interface へ移行。
- `src/interface/presentation.rs`: core から interface へ移行したため、i18n や display へのインポートパスを修正。
- `src/interface/presentation_assembler.rs`: core から interface へ移行したため、presentation や i18n へのインポートパスを修正。
- `src/interface/presentation_tests.rs`: core から interface へ移行したため、インポートパスを修正。
- `src/interface/report.rs`: core から interface へ移行したため、i18n や presentation へのインポートパスを修正。
- `src/interface/report_ui_tests.rs`: core から interface へ移行したため、インポートパスおよび regression snapshot ファイルの読み込みパスを修正。
- `scripts/check_architecture_boundaries.py`: 移行に伴い、禁止インポートのリストから crate::core::report と crate::core::presentation を削除（crate::interface 全体として禁止されているため包含される）。
- `src/interface/snapshots/no_trade_en_us.html.txt`: core から interface へ移行したため移動。
- `src/interface/snapshots/no_trade_en_us.md`: core から interface へ移行したため移動。
- `src/interface/snapshots/no_trade_ja_jp.html.txt`: core から interface へ移行したため移動。
- `src/interface/snapshots/no_trade_ja_jp.md`: core から interface へ移行したため移動。
- `src/interface/snapshots/no_trade_zh_cn.html.txt`: core から interface へ移行したため移動。
- `src/interface/snapshots/no_trade_zh_cn.md`: core から interface へ移行したため移動。

## Backtrack

- Status: `none`
- Report: `target/ai_backtrack_report.json`
- Items: none

## Next Action

- human review / commit decision

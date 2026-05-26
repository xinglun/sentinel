---
author: Ray
title: AI ガバナンス入口
description: Sentinel における AI 作業契約、検証、回帰防止記録の最小運用入口。
key: ai-governance-entry
---

# AI ガバナンス入口

`.ai/` は、AI Agent による複雑な変更を「作業契約」「検証」「完了記録」「回帰防止」の単位で追跡するための機械可読な入口です。

目的は新しい分析機能を追加することではなく、Sentinel のエンジニアリング進化における以下のリスクを抑えることです。

1. scope 外の変更。
2. テストや snapshot の無宣言な削除。
3. Telegram / audit / i18n の契約回帰。
4. 検証未実行のまま完了扱いにすること。

## 構成

| パス | 用途 |
|---|---|
| `.ai/cockpit/` | 現在の Work Item 状態、共通 check catalog、Cockpit 説明。 |
| `.ai/guards/` | Backtrack Guard などの回帰防止 policy。 |
| `.ai/work-items/` | Contract、Summary、Review checklist の template と active task。 |

`current_status.md` の active / no-active 表示、archive 後の同期、参照整合性は `.ai/cockpit/status_policy.yaml` を正とします。

## 運用原則

AI 作業は通常の開発品質を免除しません。複雑な diff を伴う変更では、作業前に Contract を作成し、完了時に Summary と検証結果を残します。

軽微な質問、調査のみ、diff を伴わない説明は Work Item 化しません。対象は reviewer が scope、根拠、検証、リスクを追跡する価値がある変更です。

## 最小フロー

1. `make ai-start TASK=<task> TITLE="..." MODE=code` で Contract / Summary を作成する。
2. `unknowns` と `notCodable` を確認する。
3. `scope` と `outOfScope` の範囲内で実装する。
4. `verification` に記載した command を `make` 経由で実行する。
5. `.ai/work-items/active/<task>.summary.json` に結果を記録する。
6. `make check-ai-guards CONTRACT=.ai/work-items/active/<task>.contract.json`、`make check-ai-backtrack CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json`、`make check-ai-coverage-guard` を実行し、scope 外変更、無宣言な回帰、および test 証跡不足がないことを確認する。
7. `make generate-cockpit-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json` で `.ai/cockpit/current_status.md` を更新する。
8. `make check-ai-status CONTRACT=.ai/work-items/active/<task>.contract.json SUMMARY=.ai/work-items/active/<task>.summary.json` と `make check-ai-status-consistency` で参照整合性を確認する。
9. 完了時は `make ai-finish TASK=<task>` で required checks を再実行し、成功時だけ archive する。

`make test-ai-guards` は `.ai/**/*.yaml` を parse し、YAML として読めない governance 設定を失敗させる。`make check-ai` と `make quality` はこの parse guard を含む。

`scripts/ai_*.py` は Makefile target から呼び出される実装詳細であり、通常の運用入口は `make` target とします。

## 境界

この仕組みは開発工程の hard gate です。`src/**`、`tests/**`、`docs/**`、`scripts/**`、CI、AI governance などの管理対象 diff は Contract scope に明示された変更だけを許可します。加えて、`.ai/guards/file_ownership.yaml` の `aiWrite: restricted` file の無承認変更、test / snapshot / i18n / Work Item evidence の無宣言削除、および production Rust code の test 証跡不足を阻止します。

CI は `AI_DIFF_BASE` で push / pull request の committed diff を検証し、clean checkout で guard が空振りする状態を許容しません。

この hard gate は取引判断、Gate、Telegram 文案ロジック、証拠スコアには影響しません。

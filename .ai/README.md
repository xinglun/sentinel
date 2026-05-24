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

1. `.ai/work-items/active/<task>.contract.json` を作成する。
2. `unknowns` と `notCodable` を確認する。
3. `scope` と `outOfScope` の範囲内で実装する。
4. `verification` に記載した command を実行する。
5. `.ai/work-items/active/<task>.summary.json` に結果を記録する。
6. `scripts/ai_check_backtrack.py` を実行し、無宣言な回帰リスクを確認する。
7. `scripts/ai_generate_status.py` で `.ai/cockpit/current_status.md` を更新する。
8. `scripts/ai_check_status_consistency.py` で status と Work Item 配置の参照整合性を確認する。

## 境界

この仕組みは report-only の工程管理です。取引判断、Gate、Telegram 文案ロジック、証拠スコアには影響しません。

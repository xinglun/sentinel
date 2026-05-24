---
author: Ray
title: Sentinel 状態機改造テストチェックリスト
description: Sentinel 状態機改造テストチェックリスト に関する Sentinel の設計・運用情報。
key: docs-specs-state-machine-test-checklist
---

# Sentinel 状態機改造テストチェックリスト

## 1. 市場状態機 (Market Regime State Machine)

### T1 防御的でない押し目における直接リセットの禁止

シナリオ：

1. 現在の状態：`ESTABLISHED`
2. `confidence / flow / stability` が弱体化。
3. コア資産群はまだ崩壊していない。

期待結果：

1. `EARLY_CONFIRMATION` への降格を許可する。
2. `IGNITION` への直接復帰を禁止する。

### T2 段階的降格の制約

シナリオ：

1. 現在の状態：`CONFIRMED`
2. 通常の押し目（調整）。

期待結果：

1. `ESTABLISHED` への遷移のみを許可する。
2. `NEWBORN` への飛び級降格を禁止する。

### T3 Reset Gate の通過

シナリオ：

以下がすべて成立：

1. `TrendDominant == false`
2. `Structural < 25`
3. `Stability < 10` が3日間連続。
4. `Flow <= 0`
5. `CoreAssetsBreakdown == true`

期待結果：

1. `IGNITION` へのリセットを許可する。

### T4 Reset Gate によるブロック

シナリオ：

1. 上記条件のうち、1項目以上が欠けている。

期待結果：

1. リセットを禁止する。
2. 構造化ログに `blocked_reset` を記録する。

### T5 Defensive Override (防御的オーバーライド)

シナリオ：

1. 明確な防御的トリガーが成立。

期待結果：

1. 任意の状態から直接 `DEFENSIVE` への突入を許可する。
2. `max_step` 制約や duration lock（期間ロック）に阻害されないこと。

## 2. Duration Lock (期間ロック)

### T6 昇格タイムロック

シナリオ：

1. 状態があるライフサイクルに入った直後。
2. 最小滞在時間（Minimum Stay Time）を満たしていない。

期待結果：

1. さらなる昇格を禁止する。

### T7 チャタリング防止が防御を阻害しないことの確認

シナリオ：

1. ロック期間中（ウィンドウ内）。
2. しかし、明確に `DEFENSIVE` トリガーが成立。

期待結果：

1. 依然として直接 `DEFENSIVE` に突入すること。

## 3. Core Assets (コア資産)

### T8 Core Assets 設定の有効性

シナリオ：

1. カスタムの `core_assets` を設定。

期待結果：

1. `CoreAssetsBreakdown` の判定が、ハードコードされたシンボルではなく、設定された集合を使用すること。

### T9 コア資産が崩壊していない場合のリセット禁止

シナリオ：

1. 通常のシグナルが弱体化。
2. コア資産群は依然として主構造を維持。

期待結果：

1. `IGNITION` へのリセットを禁止する。

## 4. 個別銘柄の復帰

### T10 一足飛びの「潔白化」の禁止

シナリオ：

1. 現在の状態：`DEFEND`
2. 単日の価格/乖離（Deviation）が改善。

期待結果：

1. 直接 `OPTIMAL` になることを禁止する。

### T11 段階的復帰パス

シナリオ：

1. 現在の状態：`DEFEND`
2. 復帰条件を連続して満たしている。

期待結果：

1. `DEFEND -> CAUTION`
2. `CAUTION -> CRUISE`
3. `CRUISE -> OPTIMAL`

### T12 Cooldown (クールダウン) の有効性

シナリオ：

1. 復帰シグナルが成立。
2. しかし、クールダウン期間が未了。

期待結果：

1. 状態のアップグレード（昇格）を禁止する。

### T13 Historical Penalty (歴史的ペナルティ) の有効性

シナリオ：

1. 直近20日以内に `DEFEND` が発生。
2. 短期的な構造が改善。

期待結果：

1. デフォルトで `max_state = CRUISE` に制限する。
2. 直接 `OPTIMAL` に入ることを禁止する。

### T14 FORMING (形成中) 資産の保護

シナリオ：

1. `FORMING` 状態の資産。
2. 市場状態の改善またはリセット。

期待結果：

1. 市場に引きずられて自動的に強気状態へ引き上げられないこと。

## 5. ログとレポートの一貫性

### T15 State Transition Log の完全性

期待される最小フィールド：

1. `from` (遷移前)
2. `to` (遷移後)
3. `blocked_reset` (リセット阻害フラグ)
4. `core_assets_breakdown` (コア資産崩壊フラグ)
5. `reasons` (理由)

### T16 Telegram / レポートの一貫性

同一の資産群が同一のバケット（Bucket）を共有しなければならない：

1. `Top Actions`
2. `戦術分区 (Tactical Summary)`
3. `リスクと機会`

期待結果：

1. 「Top Actions にある機会銘柄が、機会エリアに存在しない」という事態が発生しないこと。
2. 「防御資産が同時に機会エリアに表示される」という事態が発生しないこと。

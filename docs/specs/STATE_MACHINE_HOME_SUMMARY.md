---
author: Ray
title: Sentinel 状態機改造ホームサマリー (STATE_MACHINE_HOME_SUMMARY.md)
description: Sentinel 状態機改造ホームサマリー (STATE_MACHINE_HOME_SUMMARY.md) に関する Sentinel の設計・運用情報。
key: docs-specs-state-machine-home-summary
---

# Sentinel 状態機改造ホームサマリー (STATE_MACHINE_HOME_SUMMARY.md)

## 目標

現在のシステムの問題は、シグナルの不足ではなく、「時間の慣性層（Inertia Layer）」の欠如にあります。  
これにより、状態機が「瞬時応答型」に偏りすぎており、以下の事象を誤認しやすくなっています：

1. トレンドの減衰
2. 内部的な振り落とし（洗盤）
3. 局所的な不均衡

これらを以下のように誤判定してしまいます：

1. トレンドの再開
2. ライフサイクルのリセット（ゼロ帰還）
3. 弱い資産の即時「洗浄（ロンダリング）」

今回の改造の核心的な目標は以下の通りです：

**状態機を「反応型システム」から、「記憶を持つ時間システム」へとアップグレードする。**

## 最終的な階層構造

1. `RawSignalLayer` (生シグナル層)
2. `InertiaLayer` (慣性層)
3. `RegimeDecisionLayer` (レジーム意思決定層)
4. `ExecutionLayer` (実行層)
5. `NarrativeLayer` (ナラティブ層)

このうち、現在最も欠落しており、かつ優先度が最高なのは以下の層です：

**`InertiaLayer`**

## 固守すべきシステム原則

1. `Decision Primitive != Narrative` (意思決定プリミティブはナラティブではない)
2. `Execution State != Market State` (実行状態は市場状態ではない)
3. `Trend = Continuity + Strength` (トレンド ＝ 継続性 ＋ 強さ)
4. `Continuity`（継続性）には慣性があり、`Strength`（強さ）は急速に変動しうる。
5. `Reset`（リセット）は証明されるべきものであり、推測されるべきものではない。
6. `Narrative`（ナラティブ）は最終状態を消費するのみであり、判定に逆行して関与してはならない。

## 今回の改造で実装すべきコアルール

### 1. Reset Gate (リセットゲート)

以下の条件をすべて満たす場合にのみ、`IGNITION` への復帰を許可します：

1. `TrendDominant == false`
2. `Structural < 25`
3. `Stability < 10` が3日連続
4. `Flow <= 0`
5. `CoreAssetsBreakdown == true` (コア資産の崩壊)

補足：

1. `CoreAssetsBreakdown` は必須条件です。
2. 単日の `confidence / stability` のみに基づくリセットは許可しません。
3. `core_assets` は設定ファイルから取得すべきであり、コード内にハードコードしてはなりません。

### 2. Downgrade Gate (降格ゲート)

通常のライフサイクル降格：

`max_step = 1` (最大1段階ずつ)

許可される遷移：

1. `CONFIRMED -> ESTABLISHED`
2. `ESTABLISHED -> EARLY_CONFIRMATION`
3. `EARLY_CONFIRMATION -> NEWBORN`

禁止される遷移：

1. `ESTABLISHED -> IGNITION`
2. `CONFIRMED -> NEWBORN`

例外：

1. `ANY -> DEFENSIVE` は常に許可されます。
2. 防御的な降格は、通常の `step` 制限を受けません。

### 3. Duration Lock (期間ロック)

`Duration Lock` が保護するのは以下の遷移のみです：

1. アップグレード（昇格）
2. リセット

以下の遷移はブロックしません：

1. 防御的な降格

追加制約：

1. `soft downgrade`（緩やかな降格）は弱い制約を受けるか、あるいは制約を受けません。
2. `hard defensive override`（硬い防御的上書き）は常に最高優先度を持ちます。

### 4. 個別銘柄の回復パス

個別銘柄の状態回復は、段階的（ステップアップ）である必要があります：

`DEFEND -> CAUTION -> CRUISE -> OPTIMAL`

一足飛びの「洗浄（クリーンアップ）」は許可されません。各ステップにはクールダウン期間が必要です。

### 5. Historical Penalty (歴史的ペナルティ)

直近20日以内に `DEFEND` 状態にあった資産：

1. デフォルトの状態上限を `CRUISE` にロックします。
2. 制限を解除するには、追加の構造的確認条件を満たす必要があります。

## 次のステップでやるべき3つのこと

1. `InertiaLayer` の実装。
2. `State Transition Log` (状態遷移ログ) の追加。
3. `core_assets` 設定の固定。

## 開発者への最後の一言

**パラメータの微調整を続けるのではありません。  
まず、状態機を「瞬時応答器」から「記憶を持つ時間システム」へとアップグレードすることです。**

---
author: Ray
title: Sentinel 状態遷移ルール (TRANSITION_RULES.md)
description: Sentinel 状態遷移ルール (TRANSITION_RULES.md) に関する Sentinel の設計・運用情報。
key: docs-specs-transition-rules
---

# Sentinel 状態遷移ルール (TRANSITION_RULES.md)

## 1. 市場状態の遷移 (Market Regime Transitions)

### 1.1 昇格パス (Lifecycle Progression)

昇格には通常、一定の確信度（Confidence）と期間（Stability）の要件を満たす必要があります。

*   **None -> IGNITION**: 
    *   `stability_structural` が初めて閾値を突破。
    *   主導的な資産が底値圏を脱し始める。
*   **IGNITION -> NEWBORN**:
    *   確信度 >= 60。
    *   継続期間 >= 5 日。
*   **NEWBORN -> EARLY_CONFIRMATION**:
    *   一度の成功した `PULLBACK` を経験し、構造が維持されていること。
    *   確信度 >= 70。
*   **EARLY_CONFIRMATION -> ESTABLISHED**:
    *   `stability_temporal` >= 30 日。
    *   確信度 >= 80。
*   **ESTABLISHED -> CONFIRMED**:
    *   `maturity` 指標が過度に高い (> 80)。
    *   モメンタムに極端なダイバージェンスが発生。

### 1.2 迅速な降格 (Defensive Trigger)

降格は通常、昇格よりも迅速に行われ、「まずは脱出、その後に確認」の原則に従います。

*   **ANY -> DEFENSIVE**:
    *   `flow_acceleration` に大幅なマイナスの変動が発生。
    *   コア資産が揃って `CAUTION` 状態に転落。
    *   マクロな確信度が 50 を割り込む。
*   **ESTABLISHED -> EARLY_CONFIRMATION (降格)**:
    *   確信度が 3 日連続で 70 未満。

---

## 2. 個別銘柄の状態遷移 (Asset State Transitions)

個別銘柄の状態遷移は、引力指標（Deviation, Z-Score, Slope）に基づきます。

| 開始状態 | 目標状態 | トリガー条件 |
| --- | --- | --- |
| `PULLBACK` | `OPTIMAL` | 出来高を伴わない引力中心への回帰後、出来高を伴って反発 |
| `OPTIMAL` | `OVERHEAT` | Z-Score > 2.0 かつ乖離率が過大 |
| `ESTABLISHED` | `CAUTION` | Owner MA を割り込むが、傾斜（Slope）がまだ平坦になっていない |
| `CAUTION` | `DEFEND` | Leash MA を割り込み、かつ傾斜が負に転じる |

---

## 3. チャタリング抑制 (Hysteresis & Smoothing)

*   **確認日数**: 昇格には通常 N 日の確認が必要ですが、降格には 1〜2 日しか必要としません。
*   **バッファ (Buffer)**: 価格が閾値付近で変動する場合、頻繁な切り替えを避けるために 1〜3% のバッファを導入します。
*   **状態ロック (State Lock)**: `DEFENSIVE` に入った後は、少なくとも 3 日間は昇格プロセスをロックします。

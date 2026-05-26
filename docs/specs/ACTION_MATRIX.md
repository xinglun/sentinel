---
author: Ray
title: Sentinel アクションマトリックス (ACTION_MATRIX.md)
description: Sentinel アクションマトリックス (ACTION_MATRIX.md) に関する Sentinel の設計・運用情報。
key: docs-specs-action-matrix
---

# Sentinel アクションマトリックス (ACTION_MATRIX.md)

## 1. コアアクションの定義

| アクション名 | セマンティック記述 | ポートフォリオレベルの制約 |
| --- | --- | --- |
| `ACCUMULATE` | 分割買い / 買い増し | 新規エクスポージャーの追加を許可。通常、`PULLBACK` または `IGNITION` 時に実行。 |
| `HOLD` | 継続保持 | 能動的な買い増しも売却も行わない。 |
| `REDUCE` | 部分売却 / 減配 | `OVERHEAT` または `CAUTION` 時に起動。利益の確定またはリスクコントロールを行う。 |
| `FREEZE` | 凍結アクション | すべての買いを禁止。保持または清算のみを許可。 |
| `AVOID` | 回避 / 傍観 | ポジションを持たず、新規構築も行わない。 |
| `OBSERVE` | 観察期 | 構造が不明確であり、一時的にアクションを行わない。 |

---

## 2. 状態マトリックスのマッピング (Market Regime x Asset State)

| 市場状態 \ 個別銘柄状態 | `OPTIMAL` | `CRUISE` | `PULLBACK` | `CAUTION` | `OVERHEAT` | `DEFEND` | `FORMING` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `IGNITION` | `ACCUM` | `HOLD` | `HOLD` | `AVOID` | `REDUCE` | `AVOID` | `OBSERVE` |
| `NEWBORN` | `ACCUM` | `HOLD` | `ACCUM` | `HOLD` | `REDUCE` | `AVOID` | `OBSERVE` |
| `EARLY_CONFIRM` | `ACCUM` | `HOLD` | `ACCUM` | `HOLD` | `REDUCE` | `REDUCE` | `OBSERVE` |
| `ESTABLISHED` | `HOLD` | `HOLD` | `ACCUM` | `HOLD` | `REDUCE` | `REDUCE` | `OBSERVE` |
| `CONFIRMED` | `HOLD` | `HOLD` | `HOLD` | `REDUCE` | `REDUCE` | `REDUCE` | `OBSERVE` |
| `DEFENSIVE` | `FREEZE` | `FREEZE` | `AVOID` | `AVOID` | `REDUCE` | `AVOID` | `OBSERVE` |

### 2.1 アクションの優先順位
1. `DEFENSIVE` 市場状態は最高優先度を持ち、個別銘柄レベルのすべての買いアクションを上書き（Override）します。
2. `OVERHEAT` 個別銘柄状態は、すべての市場状態において優先的に `REDUCE` をトリガーします。
3. `FORMING` は常に `OBSERVE` にマッピングされます。

---

## 3. ポジションサイジング係数 (Sizing Multipliers)

| アクション | デフォルト係数 | 説明 |
| --- | --- | --- |
| `ACCUMULATE` | 1.0 | 標準的な加筆（買い増し）ユニット。 |
| `HOLD` | 1.0 | 現在のウェイトを維持。 |
| `REDUCE` | 0.5 | 50% 削減、または目標ウェイトまで削減。 |
| `FREEZE` | 0.0 | 新規の買い増しを禁止。 |
| `AVOID` | 0.0 | 清算または不参加。 |

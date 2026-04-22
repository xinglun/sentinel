---
author: Ray
---

# 表示セマンティクス収束の設計説明 (PositionIntent vs DisplayIntent)

## 1. 背景と現状
現在のシステムでは、統一された実行原語として `PositionIntent` (ADD/HOLD/TRIM/EXIT) が導入されています。しかし、表示レイヤー（Telegram/Terminal）において、「保持（HOLD）」と「観察（OBSERVE）」の区別はいまだに古い下層の `AssetAction` に依存せざるを得ない状況です。
これにより、`report.rs` 内に大量の断片的なマッチングロジックが存在し、表示レイヤーの責任が純粋ではなくなっています。

## 2. 責任境界の定義 (Core Concept)

| 次元 | PositionIntent (実行セマンティクス) | DisplayIntent (表示セマンティクス) |
| :--- | :--- | :--- |
| **定義者** | Exit Decision Layer | UI Adaptation Layer (Engine/Report) |
| **関心事** | **「いくら買うべきか/売るべきか？」** | **「ユーザーに何を見せるべきか？」** |
| **列挙値** | ADD, HOLD, TRIM, EXIT | ADD, HOLD, OBSERVE, TRIM, EXIT |
| **ロジックの重点** | 優先度の上書き（EXIT > ADD） | 属性の変換（HOLD intent -> 「保持」 or 「観察」） |
| **消費者** | ExecutionGate, TraderAgent | Telegram, Report, Dashboard |

## 3. 生成ルール (Mapping Rules)

```rust
pub enum DisplayIntent {
    ADD,      // 実行アクションが ADD であり、買い増しとして表現される
    HOLD,     // 実行アクションが HOLD であり、すでにポジションを保有している
    OBSERVE,  // 実行アクションが HOLD であり、ポジションを保有していない (観察状態)
    TRIM,     // 実行アクションが TRIM
    EXIT,     // 実行アクションが EXIT
}
```

**マッピングロジックの推奨案：**
1. もし `PositionIntent == TRIM` ならば -> `DisplayIntent::TRIM`
2. もし `PositionIntent == EXIT` ならば -> `DisplayIntent::EXIT`
3. もし `PositionIntent == ADD` ならば -> `DisplayIntent::ADD`
4. もし `PositionIntent == HOLD` ならば：
   - `AssetAction == ACCUMULATE/HOLD` かつポジション保有中ならば -> `DisplayIntent::HOLD`
   - それ以外ならば -> `DisplayIntent::OBSERVE`

## 4. 実施による影響
- **DecisionPacket**: `display_intent` フィールドを追加します。
- **Engine**: Intent の合成を完了した後、直ちに `display_intent` を計算して格納します。
- **Report**: `action` に対するマッチングを完全に削除し、`display_intent` に基づいてバケット分けとラベル表示を行うようにします。

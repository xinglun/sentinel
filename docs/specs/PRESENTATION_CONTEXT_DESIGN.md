---
author: Ray
---

# 展示セマンティクスの真の独立化設計 (Presentation Context Isolation)

## 1. 核心となる矛盾

現在の `DisplayAdapter` は、コードの配置場所こそ隔離されていますが、そのコアロジックである `derive_display_intent` は依然として `AssetAction` に依存して、それが `HOLD`（保持）なのか `OBSERVE`（観察）なのかを判断しています。
この実装は依然として「低レイヤーのシグナル名」に基づく推測であり、「ビジネス上の事実」（ポジションを保有しているか否か）に基づく記述ではありません。

## 2. 展示コンテキストプリミティブ (Presentation Context)

`DisplayAdapter` の翻訳をガイドするために、明示的な展示コンテキストを導入します。

| フィールド | 定義 | 生成タイミング |
| :--- | :--- | :--- |
| **`has_position`** | 口座内で実際に当該資産を保有しているか。 | エンジンパイプラインが `positions` に基づきリアルタイム生成。 |
| **`is_candidate`** | 単なるウォッチリスト（入庫）観察対象か（過去の保有なし）。 | エンジンパイプラインが保有履歴/状態に基づき生成。 |

## 3. ロジックの再構築 (Rule Refactoring)

`DisplayAdapter` の唯一の入力は `(PositionIntent, PresentationContext)` となります：

- **PositionIntent::ADD** -> `DisplayIntent::ADD`
- **PositionIntent::TRIM** -> `DisplayIntent::TRIM`
- **PositionIntent::EXIT** -> `DisplayIntent::EXIT`
- **PositionIntent::HOLD** -> 
    - `has_position == true` -> `DisplayIntent::HOLD`
    - `has_position == false` -> `DisplayIntent::OBSERVE`

## 4. データ構造の変更

```rust
pub struct AssetActionDecision {
    // ... 執行層フィールド ...
    pub position_intent: PositionIntent,
    
    // ... 展示層プリミティブ ...
    pub has_position: bool,      // NEW
    pub is_candidate: bool,     // NEW
    pub display_intent: DisplayIntent,
}
```

## 5. メリット

- **真のデカップリング**: 将来的に `AssetAction` がリネームまたは廃止されたとしても、展示層のロジックは影響を受けません。
- **セマンティクスの明確化**: UI 表示が「シグナルの推論」ではなく「口座の事実」に直接マッピングされます。
- **テストの容易性**: 複雑な指標シグナルを構築することなく、`has_position=true` をモックするだけで「保持」ラベルのテストを強制的に行うことができます。

---
author: Ray
---

# 表示アダプター層の設計説明 (Display Adapter Design)

## 1. 概念の定義 (Boundary Definition)

表示ロジックとコアエンジンのデカップリングを実現するために、3つの階層概念を導入します：

| 概念 | 定義 | 帰属 | 主な責任 |
| :--- | :--- | :--- | :--- |
| **PositionIntent** | 実行意図 | `exit.rs` | **「どうするか」**。取引の方向（買い、売り、保持、決済）を決定します。 |
| **DisplayIntent** | 表示意図 | `display.rs` | **「どう見せるか」**。実行アクションをユーザーが理解可能な分類（買い増し、保持、観察、減配、撤退）に変換します。 |
| **DisplayBucket** | 表示バケット/レイアウト | `display.rs` | **「どこで見せるか」**。銘柄をレポート内の物理的な位置（買い増しエリア、防御エリア、観察エリア）に振り分けます。 |

## 2. 責任の移譲 (Responsibilities)

- **`engine.rs`**: `DisplayIntent` の具体的なマッピング詳細を認識しなくなります。関与度、ランキング、撤退の意思決定を `PositionIntent` に合成する責任のみを負います。
- **`display.rs` (新規)**: 
  - `DisplayIntent` の生成ロジックをカプセル化します（入力: `PositionIntent` + `AssetAction` + `HoldingStatus`）。
  - 資産のバケット分け（カテゴライズ）ロジックをカプセル化します。
  - 統一された表示ラベル（Labels）を提供します。
- **`report.rs`**: 純粋なテンプレートレンダラーとなります。`DisplayAdapter` によって処理されたバケットデータとラベルを直接消費し、いかなる推論も行いません。

## 3. インターフェースのプレビュー (Interface)

```rust
pub struct DisplayAdapter;

impl DisplayAdapter {
    /// 実行意図、基礎アクション、および持分状態に基づいて、表示意図を計算します
    pub fn compute_display_intent(
        pos_intent: PositionIntent,
        base_action: AssetAction,
        is_held: bool,
    ) -> DisplayIntent;

    /// 資産のバケット分けロジックを統一します
    pub fn categorize(decisions: &[AssetActionDecision]) -> DisplayBuckets;
}
```

## 4. 収益

- **高凝集**: すべての UI 表示文言とルールが一箇所に集中します。
- **テストの容易性**: `OBSERVE` と `HOLD` の分離ロジックを、フルエンジンを起動することなく独立してテストできます。
- **一貫性**: Telegram とターミナルレポートが同じアダプターロジックを共有し、表示の食い違いを根絶します。

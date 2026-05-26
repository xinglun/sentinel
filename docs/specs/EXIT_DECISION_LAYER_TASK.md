---
author: Ray
title: Sentinel 退出意思決定層 (Exit Decision Layer) タスクリスト
description: Sentinel 退出意思決定層 (Exit Decision Layer) タスクリスト に関する Sentinel の設計・運用情報。
key: docs-specs-exit-decision-layer-task
---

# Sentinel 退出意思決定層 (Exit Decision Layer) タスクリスト

## 1. 目標

本タスクは、Sentinel に独立した売却意思決定層を追加することを目的としています：

**Exit Decision Layer**

現在のシステムには以下の要素がすでに備わっています：

1. `Market Regime`
2. `Participation Readiness`
3. `Asset State / Ranking`
4. `ActionMatrix`
5. `ExecutionGate`

しかし、現在の売却セマンティクス（意味論）はまだ十分に独立しておらず、以下の要素と混同されやすい状態です：

1. 買入マッピングロジック
2. レポート文言ロジック
3. 局所的な資産状態の解釈

本タスクの目標は、買いポイントの最適化ではなく、以下の問いに明示的に答えることです：

1. いつ完全に退出（Exit）すべきか
2. いつ減配（Trim）すべきか
3. いつ加倉（追加買い）を停止すべきか
4. いつ利益管理のみを行うべきか

---

## 2. 設計原則

### 2.1 売却は買入の反対ではない

買入の判断基準：

1. 今、リスクを取り始めることができるか？

売却の判断基準：

1. 今、この種のリスクを取ることを停止しなければならないか？

したがって：

1. 退出ロジックを `ActionMatrix` に詰め込み続けてはならない。
2. 退出の優先順位は買入マッピングよりも高くなければならない。

### 2.2 Exit Gate の責務

Exit Decision Layer は以下の責任を負わなければなりません：

1. ハードリスク（致命的リスク）を識別し、強制退出を命じる。
2. 主軸からの脱落を識別し、減配をトリガーする。
3. 市場レベルの冷え込みを識別し、一律に手を引く。
4. 過熱を識別し、利益管理を実行する。

---

## 3. 推奨される実行順序

エンジンのプロセスを以下のように調整することを推奨します：

```text
Raw Signals
→ Inertia / Memory
→ Regime Decision
→ Participation Decision
→ Asset State / Ranking
→ Exit Decision
→ Action Mapping
→ Execution
```

重要な原則：

1. まず撤退すべきかどうかを判断する。
2. 次にまだ追加できるかどうかを判断する。

---

## 4. 新規概念

### 4.1 Position Intent

以下の出力を一元化することを推奨します：

```text
position_intent
- ADD (加倉/新規)
- HOLD (保持)
- TRIM (減配)
- EXIT (清算/退出)
```

### 4.2 Exit メタ情報

以下の構造体を新規追加することを推奨します：

```rust
pub struct ExitDecision {
    pub position_intent: PositionIntent,
    pub asset_exit_state: AssetExitState,
    pub exit_priority: u8,
    pub exit_reasons: Vec<String>,
}
```

推奨される列挙型：

```rust
pub enum AssetExitState {
    None,
    DefensiveExit,        // 防御的退出
    StrengthLoss,         // 強さの喪失
    ParticipationExit,    // 参加条件未達による退出
    OverheatProfitTake,   // 過熱による利食い
}
```

---

## 5. 優先順位ルール

同一資産において複数のシグナルが同時に発生した場合、統一された優先順位で上書きする必要があります：

```text
EXIT > TRIM > HOLD > ADD
```

強制要件：

1. `Exit intent always overrides add intent` (退出意図は常に加倉意図を上書きする)

つまり：

1. ある資産が `ADD` と `TRIM` の両方の条件を満たす場合
2. 必ず `TRIM` を優先する。

---

## 6. 第1版ルール

初版では構造化された売却のみを行い、複雑な損益管理は行いません。

### Rule 1: Defensive Exit (防御的退出)

条件：

1. `asset_state == DEFEND`
2. または `risk_overlay == DEFENSIVE`
3. または明確なハードリスクシグナルがすでに存在する場合

アクション：

1. `position_intent = EXIT`

説明：

1. これは「命を守る」レイヤーです。
2. 2〜3日の確認期間を待たず、即座に実行します。
3. まず生き残り、その後について考えます。

### Rule 2: Strength Loss Exit (強さの喪失による退出)

条件：

1. `asset_out_of_top_tier_streak >= 3` (トップ層からの脱落が3日以上)
2. または `(asset_state が OPTIMAL/CRUISE -> CAUTION へ遷移) が 2日以上持続`

アクション：

1. `position_intent = TRIM`

説明：

1. これは「主軸管理」レイヤーです。
2. 目的は失敗を宣告することではなく、より強い資産にポジションを譲ることです。

### Rule 3: Participation Exit (参加解除による退出)

条件：

1. `participation_ready` が `true -> false` に変化

アクション：

1. 新規買いの禁止。
2. 弱い資産は `TRIM`。
3. コアな強い資産は `HOLD / FREEZE`。

説明：

1. 「市場の門」が閉じました。
2. まず手を止め、その後で分類処理を行います。
3. 全ポジションの清算を意味するわけではありません。

### Rule 4: Overheat Profit-Take (過熱による利食い)

条件：

1. `asset_state == OVERHEAT`

アクション：

1. `position_intent = TRIM`
2. `take_profit_mode = partial` (部分利食い)

説明：

1. 減らすだけで、清算はしません。
2. これは利益管理であり、トレンド転換の判断ではありません。

---

## 7. 追加が必要なデータプリミティブ

UI から逆算して退出ロジックを組むのではなく、システムのプリミティブ（基本要素）を永続化すべきです。

`DecisionPacket` に少なくとも以下の項目を追加、または明示的に露出させることを推奨します：

1. `top_tier_symbols`
2. `participation`
3. `participation_changed`
4. `asset_top_tier_streak`
5. `asset_out_of_top_tier_streak`
6. `asset_state_streak`
7. `risk_overlay`

さらに以下を補完することを推奨します：

1. `asset_previous_state`
2. `asset_exit_blockers`

用途：

1. `asset_previous_state`
   - `OPTIMAL/CRUISE -> CAUTION` への変化を判断するために使用。
2. `asset_exit_blockers`
   - なぜ今回退出がトリガーされなかったのかを説明するために使用。

---

## 8. 推奨されるコードの配置

以下の新規追加を推奨します：

1. [exit.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/domain/exit.rs)

そして以下の場所で接続します：

1. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/application/engine.rs)
2. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/domain/decision.rs)
3. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/domain/action_matrix.rs)
4. [execution_gate.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/application/execution_gate.rs)
5. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/interface/report.rs)

### 8.1 Engine

担当：

1. 資産ランキング完了後、exit decisions を計算する。
2. exit の結果と action mapping を一元的に収束させる。
3. 最終的な `position_intent` を形成する。

### 8.2 ActionMatrix

責務を以下に縮小：

1. 買い増し/保持の基礎的なマッピングを担当。
2. 退出ルールの定義は担当しない。
3. 最終結果は Exit Layer によって上書きされる。

### 8.3 ExecutionGate

以下の消費を担当：

1. `position_intent`

以下の直接的な推測を停止：

1. `ACCUMULATE / REDUCE / HOLD`

---

## 9. 開発タスクの分解

### P0-1 ExitDecision モジュールの新規追加

任務要件：

1. `exit.rs` を新規作成。
2. 以下を定義：
   - `PositionIntent`
   - `AssetExitState`
   - `ExitDecision`
3. 第1版の4つのルールを実装。

### P0-2 DecisionPacket / 資産意思決定構造の拡張

任務要件：

1. exit の結果を `DecisionPacket` に書き込む。
2. 各資産に対して exit 関連のフィールドを記録。
3. 履歴パッケージで streak / previous state / exit intent が追跡可能であることを保証。

### P0-3 Engine への統一意思決定順序の導入

任務要件：

1. まず participation を計算。
2. 次に asset state / ranking を計算。
3. さらに exit decisions を計算。
4. 最後に `position_intent` にマージ。

### P1-1 ExecutionGate での position_intent 消費

任務要件：

1. `ADD` -> 買入パス
2. `TRIM` -> 減配パス
3. `EXIT` -> 清算または強制退出パス
4. `HOLD` -> 取引なし

### P1-2 報告層への退出診断の追加

任務要件：

1. 以下を表示：
   - どの資産が `TRIM` されたか
   - どの資産が `EXIT` されたか
   - その理由は何か
2. 以下を区別：
   - 命を守る退出 (Defensive)
   - 脱落による減配 (Strength Loss)
   - 市場の冷え込みによる減配 (Participation)
   - 過熱による利食い (Profit-Take)

### P1-3 ドキュメントとスキーマの更新

任務要件：

1. [DECISION_PACKET_SCHEMA.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/DECISION_PACKET_SCHEMA.md) を更新。
2. 必要に応じて以下を更新：
   - `ACTION_MATRIX.md`
   - `STATE_MACHINE_HOME_SUMMARY.md`

---

## 10. テストリスト

少なくとも以下のテストを補完してください：

1. `DEFEND` の日は必ず `EXIT` がトリガーされること。
2. 3日連続で `Top Tier` から脱落した場合、必ず `TRIM` がトリガーされること。
3. `participation_ready true -> false` 後は新規買いが禁止されること。
4. `OVERHEAT` は部分的な減配のみを行い、全清算はしないこと。
5. 強い資産は市場が冷え込んだ際、弱い資産と同じ扱いを受けてはならないこと。
6. 強い資産が1日だけ脱落した場合、`TRIM` をトリガーしてはならないこと。
7. 弱い資産が Top Tier に戻って1日目では、歴史的なペナルティを解除してはならないこと。
8. `EXIT` と `ADD` が衝突した場合、必ず `EXIT` を優先すること。
9. `TRIM` と `ADD` が衝突した場合、必ず `TRIM` を優先すること。

推奨されるテストの配置場所：

1. `src/features/radar/domain/exit.rs`
2. `tests/pipeline_integration.rs`
3. `src/features/radar/interface/report_ui_tests.rs`
4. `src/features/radar/application/execution_gate.rs`

---

## 11. 完了基準

本タスク完了後、システムは以下の条件を満たさなければなりません：

1. 売却ロジックが独立した層を持ち、`ActionMatrix` と混在していないこと。
2. `position_intent` が統一された実行プリミティブ（原語）となっていること。
3. `EXIT > TRIM > HOLD > ADD` の明確な上書きルールが有効であること。
4. システムが以下を区別できること：
   - 命を守る退出
   - 構造的な減配
   - 市場レベルのリスク回避
   - 過熱による利食い
5. 報告層が結果だけでなく、「なぜ売るのか」を説明できること。

---

## 12. 非目標 (Out of Scope)

本フェーズでは以下は行いません：

1. 含み益のドローダウンモデル。
2. ATR 利食い。
3. コストライン（買値）利食い。
4. 分割利食い戦略の最適化。
5. より複雑な損益帰属システム。

本フェーズで行うのは：

**売却システムに「構造的な正しさ」をまず備えさせることです。**

---

## 13. 推奨される開発順序

以下の順序でコミットすることを推奨します：

1. `exit.rs` + データ構造
2. `DecisionPacket` / スキーマ拡張
3. `engine.rs` への意思決定順序の導入
4. `execution_gate.rs` での `position_intent` 消費
5. `report.rs` への退出説明の追加
6. 完璧なテスト

---

## 14. 完了の定義

以下の条件がすべて満たされたとき、本タスクは完了と見なされます：

1. 独立した `Exit Decision Layer` が存在すること。
2. すべての資産に明確な `position_intent` が付与されていること。
3. 退出の優先順位上書きルールが機能していること。
4. 買入と売却が `ActionMatrix` 内で互いに汚染し合っていないこと。
5. すべてのコアな退出パスにテストカバレッジが存在すること。
6. 報告層が「なぜ手を引くべきか」を明確に説明できること。

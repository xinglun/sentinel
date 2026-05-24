---
author: Ray
title: トレンド凝集性 V2 タスク (Trend Cohesion V2 Task)
description: トレンド凝集性 V2 タスク (Trend Cohesion V2 Task) に関する Sentinel の設計・運用情報。
key: docs-specs-trend-cohesion-v2-task
---

# トレンド凝集性 V2 タスク (Trend Cohesion V2 Task)

## 1. 目標

「候補銘柄数 + 安定性 + 継続性」を単なる取引可能なメイン・トレンドの定義として扱うのではなく、第2世代の `Trend Cohesion`（トレンド凝集性）レイヤーを追加します。

V2 は、より厳格な問いに回答しなければなりません：

> 今の市場に、**コヒーレント（一貫性のある）で追随可能な主要トレンド**は存在するか？

これは説明の質（Explanation Quality）のアップグレードであり、取引ルールの書き換えではありません。

システムはすでに以下の問いに回答しています：

- 市場に参加してよいか (Participation Readiness)。
- 新規ポジションは禁止されているか。
- 既存ポジションを保持 / 削減 / 決済すべきか。

V2 は、以下に対する明示的な回答を追加します：

- 真の主要トレンド（Primary Trend）が存在するかどうか。
- リーダー銘柄群が、安定して追随可能な構造へと収束（Convergence）しているかどうか。

## 2. 非目標

本タスクにおいて、以下を変更しては**なりません**：

- `ParticipationReadiness` の閾値。
- `NO TRADE` のセマンティクス。
- `ExitDecision` (手仕舞い判定)。
- `ActionMatrix` (アクション・マトリクス)。
- 実行動作。
- セクター/テーマに関する NLP や外部分類システムの導入。

これは診断機能と構造的品質のアップグレードであり、売買ルールの拡張ではありません。

## 3. 現在の問題点

現在の `TrendCohesionStatus::evaluate(...)` は簡易的なプロキシ（代理指標）にすぎません：

- 参加準備ができているか。
- 安定性スコア。
- 継続性ストレーク（日数）。
- トップ層（Top-tier）の銘柄数。

これらは V1 のメッセージングとしては十分ですが、V2 としては不十分です。なぜなら、以下の状態を区別できないからです：

1. 候補銘柄セットは小さいが、内部的な一貫性（Inconsistency）がない。
2. ウォッチリストが分散しており、明確なリーダーが存在しない。
3. 実際に収束しつつある、安定したリーダー銘柄セット。
4. 実際よりも綺麗に見えるだけの、短命な再スタート（Restart）。

要約すると：

V1 は「市場は明らかに準備不足か？」を検知します。

V2 は「追随する価値のある主要トレンドは存在するか？」を検知すべきです。

## 4. 必要な出力

表示の簡潔さを保つため、既存の列挙型（Enum）の形状は維持します：

- `NotFormed` (未形成)
- `Forming` (形成中)
- `Cohesive` (収束/凝集済み)

ただし、その背後に構造化された要因（Factors）を追加します。

新しいドメイン構造体を導入します：

```rust
pub struct TrendCohesionSnapshot {
    pub status: TrendCohesionStatus,
    pub candidate_count: usize,
    pub leader_count: usize,
    pub leader_concentration_score: f64,
    pub continuity_quality_score: f64,
    pub dispersion_score: f64,
    pub reasons: Vec<String>,
}
```

`DecisionPacket` は列挙型だけでなく、この完全なスナップショットを保存する必要があります。

## 5. V2 評価モデル

### 5.1 入力

V2 では、ドメイン層ですでに利用可能なデータのみを使用します：

- `participation.participation_ready`
- `participation.core_tier_streak`
- `market_features.stability_score`
- 現在の `top_tier_symbols`
- 直近の意思決定履歴パケット (Decision History Packets)
- アセットレベルの `unified_position_intent`
- アセットレベルの状態スナップショット

V2 で外部の市場タクソノミー（分類学）を追加しないでください。

### 5.2 派生要因

V2 は、少なくとも以下の要因を計算する必要があります：

1. `candidate_count` (候補銘柄数)
   現在のトップ層銘柄の数。

2. `leader_count` (リーダー銘柄数)
   以下の両方を満たす銘柄の数：
   - 現在のトップ層に含まれている。
   - 直近の履歴ウィンドウにおいて繰り返し出現している。

3. `leader_concentration_score` (リーダー集中スコア)
   少数の反復するリーダーが直近のトップ層構成を支配している場合に高くなります。

4. `continuity_quality_score` (継続性品質スコア)
   直近のトップ層セットが毎日入れ替わる（Churn）のではなく、ゆっくりと変化する場合に高くなります。

5. `dispersion_score` (分散スコア)
   候補プールが広すぎて、リーダーシップが断片化（Fragmented）している場合に高くなります。

### 5.3 推奨される初期ヒューリスティック

V2 では、直近 3 取引日の履歴ウィンドウを使用します。

推奨される初期ロジック：

- `NotFormed` (未形成)
  以下のいずれかが真である場合：
  - `stability_score < 10.0`
  - `core_tier_streak < 2`
  - `candidate_count == 0`
  - `candidate_count >= 6`
  - 直近のトップ層メンバーの入れ替わりが激しい。
  - 反復するリーダーが存在しない。

- `Cohesive` (収束)
  以下のすべてが真である場合：
  - `participation_ready == true`
  - `stability_score >= 10.0`
  - `core_tier_streak >= 3`
  - `candidate_count <= 4`
  - 直近のウィンドウを通じて、少なくとも 2 つの反復リーダーが存続している。
  - メンバーの入れ替わりが少ない。

- `Forming` (形成中)
  上記二つの中間すべて。

これは意図的に保守的な設計です。

## 6. アーキテクチャ要件

### 6.1 ドメイン層

追加：

- `src/core/trend_cohesion.rs`

V2 は以下を公開（Expose）する必要があります：

- `TrendCohesionStatus`
- `TrendCohesionSnapshot`
- `TrendCohesionEvaluator`

### 6.2 意思決定層 (Decision Layer)

更新：

- `src/core/decision.rs`

現在のスカラフィールドを以下に置き換えます：

```rust
pub trend_cohesion: TrendCohesionSnapshot
```

互換性：

- `#[serde(default)]` を追加。
- レガシー・パケットの読み込みを維持。

### 6.3 エンジン / ドメイン・アセンブリ

更新：

- `src/core/engine.rs`

エンジンは、ドメインの事実と直近の履歴からスナップショットを計算します。
ここでは最終的な表示用テキストを計算しないでください。

### 6.4 プレゼンテーション層

更新：

- `src/core/presentation.rs`
- `src/core/presentation_assembler.rs`
- `src/core/i18n.rs`

プレゼンテーション層は以下を派生させます：

- `主線状態 / Primary Trend / 主線状態`
- ローカライズされた値：
  - `主線未形成`
  - `主線形成中`
  - `主線已収斂`
- 理由（Reasons）に基づくオプションの短い説明。

### 6.5 レポート層

更新：

- `src/core/report.rs`

`report.rs` は引き続き「レンダリング専用」でなければなりません。
以下を表示することができます：

- トレンド凝集性ラベル。
- トレンド凝集性の値。
- オプションの 1 行説明。

レポート層自体で凝集性を評価してはなりません。

## 7. 必要な動作

### 7.1 現在の典型的な `NO TRADE` 再スタート時

以下のような状態の場合：

- `stability = 1.5`
- `continuity = 1d`
- `candidate_count = 8+`
- `participation_ready = false`

レポートは明示的に以下を表示しなければなりません：

- `主線状態: 主線未形成`

### 7.2 重要なセマンティックの分離

V2 では、以下の概念を混同（Collapse）してはなりません：

- `NO TRADE` (取引禁止)
- `DEFENSIVE` (防御的姿勢)
- `Trend Not Formed` (トレンド未形成)

これらはそれぞれ異なります：

- `NO TRADE` = 新規ポジションを建てない。
- `DEFENSIVE` = 市場/リスクの姿勢。
- `Trend Not Formed` = 一貫して追随可能な主要トレンドが存在しない。

## 8. 必要なテスト

### 8.1 ドメイン・テスト

以下をカバーする `trend_cohesion` テストを追加してください：

1. 低安定性 + 1日の継続性 + 分散した候補 -> `NotFormed`
2. 準備完了 + 安定 + 存続するリーダー + 低入れ替わり -> `Cohesive`
3. 中間ケース -> `Forming`

### 8.2 履歴を考慮したテスト

直近のパケットを使用して以下を検証するテストを追加してください：

1. リーダーの反復出現が凝集性を向上させること。
2. トップ層の頻繁な入れ替わりが凝集性を低下させること。

### 8.3 プレゼンテーション・テスト

追加：

- zh/en/ja ローカライズ出力テスト。
- `DecisionSummaryViewModel` にトレンド凝集性フィールドが含まれていることの確認。

### 8.4 UI テスト

最終的なレンダリングテストで以下をアサートしてください：

1. `NO TRADE + 弱い再スタート` の場合に `主線未形成` と表示されること。
2. レポート出力にトレンドラベルが表示されていること。
3. レポートのテキストが、ユーザーに対して候補銘柄数のみから手動で凝集性を推論させるようになっていないこと。

## 9. 承認基準

以下のすべてが真である場合にのみ、本タスクは完了とみなされます：

1. `Trend Cohesion` が単なる表示文字列ではなく、本物のドメイン・スナップショットになっている。
2. `DecisionPacket` が構造化されたスナップショットを保存している。
3. `PresentationAssembler` がローカライズされた「主要トレンド」行をレンダリングしている。
4. 一般的な「弱い再スタート」シナリオで明示的に `主線未形成` と表示される。
5. `report.rs` が凝集性を計算していない。
6. レガシー・パケットの読み込みが引き続き機能する。
7. `cargo fmt` にパスする。
8. `cargo test --quiet` にパスする。
9. `cargo clippy --all-targets --all-features -- -D warnings` にパスする。

## 10. 最終原則

V2 の成功とは、システムが単に：

- 「取引するな (do not trade)」

と言うだけではなく、明示的に：

- 「まだ追随すべき一貫した主要トレンドが存在しない」

と言えるようになることです。この区別こそが、本アップグレードの全目的です。

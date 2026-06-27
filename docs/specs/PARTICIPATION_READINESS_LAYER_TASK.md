---
author: Ray
title: Sentinel 参加準備レイヤー (Participation Readiness Layer) タスクリスト
description: Sentinel 参加準備レイヤー (Participation Readiness Layer) タスクリスト に関する Sentinel の設計・運用情報。
key: docs-specs-participation-readiness-layer-task
---

# Sentinel 参加準備レイヤー (Participation Readiness Layer) タスクリスト

> 現行コードベースでは独立した `ParticipationReadiness` モジュールはまだ実装されていません。
> いま運用上の近似ゲートを担っているのは `trend_cohesion.gate_passed` と `trend_cohesion.continuity_streak` です。
> 本資料は、そのギャップを明示した将来タスクとして残します。

## 1. 目標

本タスクは、Sentinel の現行アーキテクチャに「参加許可メカニズム」という明確な階層を新規追加することを目的としています。

現在のシステムには以下の要素がすでに備わっています：

1. 市場層の状態機 `Market Regime`
2. 個別銘柄層の継続性と記憶 `Relative Strength Memory`
3. アクションマッピング層 `ActionMatrix`
4. 実行リスク管理層 `ExecutionGate`

しかし、依然として重要な階層が欠落しています：

**システムは「今、市場への参加を開始してもよいか」という問いに対して、明示的に回答していません。**

現在の課題の現れ：

1. 資産層は、継続性に基づいて `OPTIMAL` まで昇格できる。
2. 市場層も、`IGNITION / NEWBORN / ...` を識別できる。
3. しかし、システム全体として「今、これらの資産層の判断を信頼してもよいか」を判断するグローバルなスイッチが欠如している。

したがって、本タスクは銘柄選定のさらなる最適化ではなく、以下の階層を新設します：

**Participation Readiness Layer**

以下の問いに答えるための層です：

1. 現在、リスク資産への参加が許可されているか。
2. 現在は、候補の監視のみが許可されている状態か。
3. いつ「候補段階」から「参加可能段階」へと移行すべきか。

---

## 2. なぜこの層が必要なのか

### 2.1 現状のギャップ

現在のシステムには以下のロジックしかありません：

1. 資産層が3日連続で強い場合、`<= CRUISE -> OPTIMAL` への昇格を許可する。
2. 市場層が `IGNITION + Stability < 10` の時、`ACCUMULATE`（加倉）を抑制する。

しかし、この両者の間には依然として隙間（エアポケット）が存在します：

1. ある資産が3日連続で非常に強い。
2. しかし、市場全体は依然として混沌とした始動期（Chaos/Ignition）にある。
3. システムが「局所的には正しいが、全体としては誤っている」リスクが発生する。

### 2.2 解決すべきは「何を買うか」ではない

この層が解決するのは以下の点です：

1. いつ、チャンスを考慮し始めてよいか。

以下の点ではありません：

1. 具体的にどの銘柄を買うべきか。

つまり、システムを以下のようにアップグレードする必要があります：

1. 純粋なシグナルシステム

から：

1. 「取引許可層」を持つ意思決定システム

---

## 3. 設計目標

Participation Readiness Layer は以下の条件を満たさなければなりません：

1. 「市場に参加できるかどうか」を、暗黙的なロジックから明示的なフィールドへと変換する。
2. 新しいルールを `ActionMatrix` に詰め込み続けない。
3. 資産層の memory/friction（記憶/摩擦）の責務を変更しない。
4. 報告層に対して、説明可能な readiness（準備完了）診断を提供する。
5. 実行層に対して、統一されたスイッチを提供する。

---

## 4. 新規概念と出力フィールド

独立した構造体を新規追加することを推奨します。例：

```rust
pub struct ParticipationReadiness {
    pub participation_ready: bool,
    pub stability_ready: bool,
    pub core_tier_streak_ready: bool,
    pub core_tier_streak: usize,
    pub reasons: Vec<String>,
}
```

最低限必要なフィールド：

1. `participation_ready: bool`
2. `core_tier_streak: usize`
3. `reasons: Vec<String>`

推奨される補完フィールド：

1. `stability_ready: bool`
2. `core_tier_streak_ready: bool`

---

## 5. 第1版の判定ルール

初版のルールは以下のように定義します：

```text
participation_ready =
    stability_score >= 10
    AND core_tier_streak >= 3
```

### 5.1 Stability 条件

現在システムですでに統一されている基準をそのまま利用します：

1. `stability_score >= 10` で合格。

### 5.2 Core Tier Streak 条件

ここで言う streak（連続性）は「特定の資産が3日連続で出現した」ことではなく、以下のことを指します：

**Top Tier（トップ層）の集合が 3日以上 連続して安定していること**

これは、本タスクにおける最も重要な定義の一つです。

---

## 6. Core Tier の定義

初版では複雑な類似度アルゴリズムは導入せず、説明可能でテストしやすいバージョンを採用します。

### 6.1 推奨される初版の定義

日々の Top Tier 集合を、以下のいずれかとして定義します：

1. 当日の `ACCUMULATE + HOLD` の中でランキング上位 2位 または 3位 までの資産。
2. より厳格に：当日の `OPTIMAL / PULLBACK / CRUISE` かつレポートの Top Actions に掲載された資産集合。

初版では以下を優先的に採用することを推奨します：

1. **最終的なソート後の上位3つのコア候補集合をそのまま利用する。**

要件：

1. 定義が安定していること。
2. 報告層、判定層、テスト層で同じ基準（口径）を使用すること。

### 6.2 連続性の判定

初版では以下の厳格なルールを使用することを推奨します：

1. 直近の連続3日間で、Top Tier 集合が完全に同一であること。

例：

1. Day1: `TSLA / MSFT / FIG`
2. Day2: `TSLA / MSFT / FIG`
3. Day3: `TSLA / MSFT / FIG`

この場合：

1. `core_tier_streak = 3`

一方、以下のような場合：

1. Day1: `TSLA / MSFT / FIG`
2. Day2: `PLTR / MSFT / FIG`

この場合、streak は途切れ、カウントがリセットされます。

注意：

1. 本フェーズでは曖昧な類似度計算は行いません。
2. 本フェーズで確定させたいのは「主軸が安定していること」であり、「毎日誰かしら強者がいること」ではありません。

---

## 7. 推奨されるコードの配置

独立したモジュールを追加し、`ActionMatrix` には詰め込まないようにします。

新規ファイルの追加推奨：

1. `src/features/radar/domain/participation_readiness.rs`

そして以下の場所で接続します：

1. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/application/engine.rs)
2. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/domain/decision.rs)
3. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/domain/action_matrix.rs)
4. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/interface/report.rs)
5. [DECISION_PACKET_SCHEMA.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/DECISION_PACKET_SCHEMA.md)

### 7.1 Engine の責務

資産の最終ランキング完了後：

1. 当日の Top Tier 集合を計算する。
2. 過去の `DecisionPacket` と組み合わせて `core_tier_streak` を計算する。
3. `ParticipationReadiness` を生成する。
4. 結果を `DecisionPacket` に書き込む。

### 7.2 ActionMatrix の責務

readiness ルールを独自に定義するのを停止します。

以下の消費のみを行います：

1. `participation_ready`

もし `false` の場合：

1. `ACCUMULATE` の出力を禁止します。

### 7.3 Report の責務

以下を明示的に表示します：

1. 現在 `Participation Ready` かどうか。
2. 現在の `core_tier_streak`。
3. readiness が合格しなかった理由。

---

## 8. DecisionPacket の拡張

[decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/domain/decision.rs) に以下のフィールドを新規追加することを推奨します：

```rust
pub participation: ParticipationReadiness
```

要件：

1. シリアライズによる永続化。
2. 履歴パッケージからの読み取り。
3. 将来的な streak 計算への利用。

一度に大規模な変更を避けたい場合でも、少なくとも以下は保証する必要があります：

1. 当日の Top Tier 集合が永続化されること。
2. readiness の結果が永続化されること。

以下を同期して記録することを推奨します：

1. `top_tier_symbols: Vec<String>`

これにより、将来の streak 計算が UI からの推論に依存しなくて済むようになります。

---

## 9. 行動ルール

### 9.1 未準備（Not Ready）時

条件：

1. `participation_ready == false`

挙動：

1. `ACCUMULATE` は一律禁止。
2. 強い資産であっても、候補（Candidate）としてのみ表示される。
3. 報告層で以下の内容を明確に提示する：
   - 現在は候補段階であること。
   - 市場参加の許可がまだ下りていないこと。

### 9.2 準備完了（Ready）時

条件：

1. `participation_ready == true`

挙動：

1. 通常のアクションマッピングを再開。
2. 報告層のトーンを「候補」から「参加可能」へとシフト。

### 9.3 既存ルールとの関係

本レイヤーはグローバルなゲート（Gating）層であり、以下を代替するものではありません：

1. Asset Memory
2. Promotion Cap
3. Upgrade Friction
4. ExecutionGate

これらすべてのルールの上に位置し、以下を決定します：

1. 資産層の「強さ」を「参加アクション」へと翻訳することを許可するかどうか。

---

## 10. 開発タスクの分解

### P0-1 ParticipationReadiness モジュールの新規追加

任務要件：

1. 独立した構造体と計算エントリポイントを新規作成。
2. 入力：
   - 現在の `MarketFeatures`
   - 現在の Top Tier 集合
   - 過去の `DecisionPacket`
3. 出力：
   - `participation_ready`
   - `core_tier_streak`
   - `reasons`

### P0-2 DecisionPacket への永続化

任務要件：

1. readiness フィールドを追加。
2. 当日の Top Tier 集合フィールドを追加。
3. 履歴パッケージが将来の streak 計算に利用可能であることを保証。

### P0-3 ActionMatrix による readiness の消費への変更

任務要件：

1. 断片的な条件による「候補期」の独自定義を停止。
2. `participation_ready == false` 時は `ACCUMULATE` を禁止。
3. 既存の資産状態マッピングは維持するが、最終的には readiness によって上書きされるようにする。

### P1-1 報告層への readiness 診断の追加

任務要件：

1. `Participation: Ready / Not Ready` を表示。
2. `Core Tier Streak` を表示。
3. 理由の表示：
   - `Stability below threshold` (安定性閾値未満)
   - `Top tier continuity not confirmed` (トップ層の継続性未確認)

### P1-2 テストとドキュメントの同期

任務要件：

1. 単体テストの追加。
2. 統合テストの追加。
3. ドキュメントの更新：
   - `DECISION_PACKET_SCHEMA.md`
   - 必要に応じて `STATE_MACHINE_HOME_SUMMARY.md` を更新。

---

## 11. テストリスト

少なくとも以下のテストを補完してください：

1. `stability >= 10` だが `core_tier_streak < 3` の場合
   - `participation_ready == false` となること。
2. `stability < 10` だが `core_tier_streak >= 3` の場合
   - `participation_ready == false` となること。
3. `stability >= 10` かつ `core_tier_streak >= 3` の場合
   - `participation_ready == true` となること。
4. Top Tier 集合が変化した際に streak がリセットされること。
5. `participation_ready == false` 時に `ACCUMULATE` が禁止されること。
6. 報告層に readiness の理由が表示されること。

推奨されるテストの配置場所：

1. `src/features/radar/domain/participation_readiness.rs`
2. `tests/pipeline_integration.rs`
3. `src/features/radar/interface/report_ui_tests.rs`

---

## 12. 完了基準

本タスク完了後、システムは以下の条件を満たさなければなりません：

1. 「市場に参加できるかどうか」を示す独立した明示的なフィールドが存在すること。
2. readiness の判定が `ActionMatrix` や report 内に散在していないこと。
3. 資産層の継続性と、市場への参加許可が明確に分離されていること。
4. 市場が Ready でない時、すべての強い資産が「候補」としてのみ扱われること。
5. 市場が Ready になった時、システムが強い資産を「正式な参加アクション」へと翻訳することを許可すること。

---

## 13. 非目標 (Out of Scope)

本フェーズでは以下のことは行いません：

1. 曖昧な集合類似度アルゴリズムの導入。
2. より複雑な Top Tier クラスリング。
3. 既存の Asset Memory 公式の再構築。
4. ExecutionGate リスク管理フレームワークの書き換え。
5. 既存のレジーム（Regime）昇降格閾値の調整。

本フェーズで行うのは以下のことのみです：

**「市場への参加が許可されているか」を、暗黙的なロジックから明示的な意思決定層へとアップグレードすること。**

---

## 14. 推奨される開発順序

以下の順序での実施を推奨します：

1. `ParticipationReadiness` 構造体と計算機の新規追加。
2. 結果を `DecisionPacket` に書き込む。
3. `ActionMatrix` 内で readiness を消費するように変更。
4. レポート表示の更新。
5. テストとドキュメントの補完。

---

## 15. 完了の定義

以下の条件がすべて満たされたとき、本タスクは完了と見なされます：

1. コード内に独立した readiness 層が存在すること。
2. `participation_ready` が永続化パッケージに含まれていること。
3. `core_tier_streak` が追跡可能であること。
4. `ACCUMULATE` が readiness 合格後にのみ出現すること。
5. テストが readiness の合格・不合格の両方のシナリオをカバーしていること。
6. 報告層が「なぜ現在は候補しか見られないのか」を明確に説明できること。

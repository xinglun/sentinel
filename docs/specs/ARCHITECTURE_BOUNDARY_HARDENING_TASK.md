---
author: Ray
title: Sentinel アーキテクチャ境界の強化タスク (ARCHITECTURE_BOUNDARY_HARDENING_TASK.md)
description: Sentinel アーキテクチャ境界の強化タスク (ARCHITECTURE_BOUNDARY_HARDENING_TASK.md) に関する Sentinel の設計・運用情報。
key: docs-specs-architecture-boundary-hardening-task
---

# Sentinel アーキテクチャ境界の強化タスク (ARCHITECTURE_BOUNDARY_HARDENING_TASK.md)

## 1. 目標

本タスクは新しい戦略機能の追加ではなく、現在のエンジニアリングにおけるモジュール境界を強化し、「ルールは動くが、責務が曖昧になる」という進化リスクを回避することを目的としています。

現在のシステムですでに完了しているもの：

1. `Market Regime`
2. `Participation Readiness`
3. `Asset State / Memory`
4. `Exit Decision`
5. `Action Matrix`
6. `Display Adapter`
7. `Execution Gate`

しかし、現在のプロジェクトにおいて3つの新しいアーキテクチャ上の問題が発生しています：

1. `ExitDecision` が買い（Entry）のセマンティクスまで侵食している。
2. `Engine` が肥大化し、展示層の派生ロジックを直接負担している。
3. `DecisionPacket` が構造化された事実と旧式の Telegram 文言を同時に保持し、二重のセマンティックソースを形成している。

本タスクの目標は以下の通りです：

1. 退出層を「撤退すべきかどうか」のみを担当するように戻す。
2. エンジンを純粋なオーケストレーションの役割に戻す。
3. 展示セマンティクスをコア意思決定パッケージから解耦（デカップリング）する。
4. 将来的な戦略の拡張や展示端の追加に備え、明確な境界を維持する。

---

## 2. 現在のアーキテクチャ上の問題

### 2.1 ExitDecision の越境

現在の `ExitDecision::compute()` は `EXIT / TRIM / HOLD` を出力するだけでなく、安全な条件下で `ADD` にフォールバックします。

これにより：

1. 退出層がエントリー（Entry）の許可を表現し始めている。
2. `ActionMatrix` と `ExitDecision` の両方が買いのセマンティクスを保持している。
3. 最終的な `position_intent` を `Engine` で再度合成する必要がある。

結果：

1. 境界が不明瞭。
2. デバッグが困難。
3. 将来的にルールを追加した際に衝突が発生しやすい。

### 2.2 Engine の肥大化

現在、`Engine` は以下のすべてを担当しています：

1. 特徴抽出
2. 市場状態の推進
3. 資産状態の計算
4. ランキング
5. 参加許可
6. 退出意思決定
7. 最終意図の合成
8. `DisplayContext`
9. `DisplayIntent`

これは以下を意味します：

1. 意思決定の変化で `engine.rs` を修正する必要がある。
2. 表示の変化でも `engine.rs` を修正する必要がある。
3. エンジンが新しい「神ファイル（God File）」化している。

### 2.3 DecisionPacket の二重セマンティックソース

現在、`DecisionPacket` は以下を同時に保存しています：

1. 構造化された事実: `market_regime / participation / assets / display_*`
2. 旧式の文言入力: `telegram.headline / summary / bias`

これにより：

1. テストで `telegram` を手動で設定できてしまう。
2. 実行時に構造化フィールドに基づいてレポートが再構成される。
3. 同一のセマンティクスに対して2つのソースが存在する。

長期的なリスク：

1. レポート層とデータ層の乖離。
2. テストが文字列のみを検証し、実際の構造化データの連鎖を検証しなくなる。

---

## 3. 目標とする境界

期待される責務レイヤーは以下の通りです：

```text
Raw Data
→ Features
→ Market Regime
→ Participation Readiness
→ Asset State / Ranking
→ Exit Decision
→ Action Mapping
→ Intent Synthesis
→ Presentation Assembly
→ Report / Execution
```

コア原則：

1. 退出層は買いの許可を担当しない。
2. エンジンはオーケストレーションのみを行い、展示の解釈は行わない。
3. レポートは presentation output を消費し、作りかけのセマンティクスを消費しない。

---

## 4. 本改造の範囲

### P0-1 ExitDecision 境界の強化

要件：

1. `ExitDecision` は `ADD` を返さない。
2. `ExitDecision` は以下のみを表現する：
   - `EXIT`
   - `TRIM`
   - `NONE` または同等の「退出アクションなし」
3. 最終的な `ADD / HOLD` は以下の要素によってのみ決定される：
   - `ActionMatrix`
   - `ParticipationReadiness`
   - Intent synthesis

推奨されるリファクタリング：

```rust
pub enum ExitIntent {
    None,
    Trim,
    Exit,
}
```

または `PositionIntent` を維持しつつ、`ExitDecision` が出力できるものを以下に強制的に制限します：

1. `EXIT`
2. `TRIM`
3. `HOLD`

退出層で `ADD` を生成することは厳禁です。

### P0-2 独立した Intent Synthesizer の新設

`engine.rs` 内で最終的な intent 合成をハードコードし続けるのをやめます。

新規追加案：

1. `src/core/intent.rs`
2. または `src/core/intent_synthesizer.rs`

責務：

1. `ActionMatrix` の基礎アクションを受け取る。
2. `ParticipationReadiness` を受け取る。
3. `ExitDecision` を受け取る。
4. 唯一の `position_intent` を出力する。

優先順位は以下のように明確化します：

```text
EXIT > TRIM > HOLD > ADD
```

### P0-3 展示コンテキスト派生ロジックの分離

独立した presentation assembler を追加することを推奨します：

1. `src/core/presentation.rs`

責務：

1. ドメインの事実に基づいて `DisplayContext` を生成する。
2. `DisplayAdapter` を呼び出して `DisplayIntent` を生成する。
3. 展示層が必要とする view models を生成する。

`engine.rs` は以下を直接構築すべきではありません：

1. `DisplayContext`
2. `DisplayIntent`

### P0-4 DecisionPacket セマンティクスの強化

本フェーズで `telegram` を強制的に削除はしませんが、その責務を明確に降格させます。

要件：

1. `telegram` をコアなセマンティックソースと見なさない。
2. `DecisionPacket` の構造化フィールドを唯一の「真実のソース（SSOT）」とする。
3. `telegram` を保持する場合、それは以下としてのみ機能する：
   - レガシー互換性
   - 最小限のサマリー入力

推奨：

1. ドキュメント内で `telegram` を legacy summary layer としてマークする。
2. 戦略セマンティクスを今後 `telegram` に詰め込むことを禁止するコメントを追加する。

---

## 5. 推奨されるモジュールの配置

以下のように新規追加または調整を推奨します：

1. [intent.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/intent.rs)
   - 最終的な `position_intent` の合成を一元化。

2. [presentation.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/presentation.rs)
   - `DisplayContext` の生成。
   - `DisplayIntent` の生成。
   - レポート向け展示データの生成。

3. [exit.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/exit.rs)
   - 純粋な退出意思決定に収束。

4. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/engine.rs)
   - オーケストレーションのみを担当。

5. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs)
   - `telegram` のレガシー位置付けを明記。

---

## 6. 実行順序

### Step 1

まず `ExitDecision` の境界強化を行います。

検証項目：

1. `exit.rs` 内から「デフォルト ADD」のセマンティクスが消失していること。
2. 退出層の単体テストが引き続き defensive / strength loss / participation / overheat をカバーしていること。

### Step 2

独立した intent synthesis モジュールを新規追加します。

検証項目：

1. `engine.rs` で `EXIT > TRIM > HOLD > ADD` の合成ロジックを直接記述していないこと。
2. intent 合成ルールに独立したテストが存在すること。

### Step 3

`DisplayContext / DisplayIntent` の派生ロジックを presentation モジュールに移動します。

検証項目：

1. `engine.rs` が直接 `DisplayContext` を生成していないこと。
2. `engine.rs` が直接 `DisplayAdapter::derive_display_intent` を呼び出していないこと。

### Step 4

`DecisionPacket` のセマンティクスを強化し、schema およびドキュメントを更新します。

検証項目：

1. `telegram` の位置付けが明示的に示されていること。
2. `DECISION_PACKET_SCHEMA.md` に構造化フィールドの優先順位が完全に記録されていること。

---

## 7. 非目標 (Out of Scope)

本フェーズでは以下のことは行いません：

1. `Market Regime` の閾値の変更。
2. `ParticipationReadiness` 判定ルールの変更。
3. `AssetState` 分類基準の変更。
4. 取引接続層の変更。
5. Telegram のビジュアルスタイルの変更。
6. 新しい複雑な戦略シグナルの追加。

---

## 8. テスト要件

少なくとも以下のテストを補完してください：

1. `ExitDecision` が `ADD` を出力しなくなったことの確認。
2. `IntentSynthesizer` が様々な組み合わせにおいて唯一正しい `position_intent` を出力することの確認。
3. `PresentationAssembler` が生成する `DisplayContext / DisplayIntent` が旧ロジックと一致することの確認。
4. 旧バージョンの履歴パッケージが引き続きデシリアライズ可能であることの確認。
5. `DecisionPacket` の構造化フィールドが優先される際、レポート出力が手動で偽造された `telegram` に依存しないことの確認。

推奨される新規テストケース：

1. `test_exit_decision_never_promotes_add`
2. `test_intent_synthesizer_priority`
3. `test_presentation_assembler_display_context`
4. `test_legacy_packet_compatibility_after_boundary_refactor`

---

## 9. 完了基準

以下の条件をすべて満たした場合に本タスク完了と見なします：

1. `ExitDecision` がエントリー（Entry）セマンティクスを保持していないこと。
2. `Engine` が展示層の派生を直接担当していないこと。
3. `position_intent` が独立したモジュールで一元的に合成されていること。
4. `DecisionPacket` の構造化フィールドが唯一の真実のソースとなっていること。
5. 全量テストがパスすること。

単に「コードを移動させた」だけで責務が強化されていない場合は、完了とは見なしません。

---

## 10. 開発者への指示

本タスクリストに従って実施し、スコープを広げないでください。本フェーズの目標は戦略の追加ではなく、境界の統治（Governance）です。

実行順序：

1. まず `ExitDecision` を強化する。
2. 次に `IntentSynthesizer` を抽出する。
3. さらに `PresentationAssembler` を抽出する。
4. 最後に `DecisionPacket` のセマンティックな位置付けとドキュメントを強化する。

納品要件：

1. 各ステップを可能な限り独立してコミットすること。
2. 境界統治とビジュアル/文言の修正を混在させないこと。
3. コミット時にテスト結果を添付すること。
4. 既存の UI テストがレガシーな `telegram` に過度に依存している場合は、構造化フィールドに依存するように修正すること。

検証は第8節および第9節に基づきます。責務の境界が真に強化されていない限り、完了とは認められません。

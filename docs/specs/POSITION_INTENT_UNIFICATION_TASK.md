---
author: Ray
title: ポジション意図 (Position Intent) 統合レイヤータスクドキュメント
description: ポジション意図 (Position Intent) 統合レイヤータスクドキュメント に関する Sentinel の設計・運用情報。
key: docs-specs-position-intent-unification-task
---

# ポジション意図 (Position Intent) 統合レイヤータスクドキュメント

## 1. 目標

本タスクは、新しい戦略を追加することではなく、現在システム内に分散している「買い」「保持」「減パ」「退出」の判断を、唯一の最終アクションプリミティブ（原語）へと収束させることを目的としています。

現在のシステムには以下の要素がすでに備わっています：

1. `NO TRADE` (取引禁止)
   - 「今日は能動的に新規建玉を行ってもよいか」という問いに答える。
2. `Exit Decision Summary` (退出判断サマリー)
   - 「既存のポジションをどのように処理すべきか」という問いに答える。
3. `候補・監視リスト`
   - 「今後動けるようになった際、まず何を見るべきか」という問いに答える。

これら表示層のループは閉じていますが、システム内部には依然として2つの並行するセマンティック（意味論）チェーンが存在しています：

1. `Entry / Participation` (エントリー / 参加) セマンティクス
2. `Exit / Position handling` (退出 / ポジション処理) セマンティクス

本タスクの目標は以下の通りです：

> 各資産に対して最終的に唯一のアクションセマンティクスのみを出力させ、システム内部およびマルチデバイス表示における「唯一の真実のソース (SSOT)」として統一する。

---

## 2. なぜ行うのか

現在のレポートは以下の問いに答えることができます：

1. 買えるかどうか
2. 売るべきかどうか
3. まず何を見るべきか

しかし、システム内部は依然として階層的に表現されており、表示層とユーザーが共同で最終的な合成（シンセサイズ）を行っている状態です。

これは長期的に3つの問題を引き起こします：

1. 意思決定チェーン上に「複数のアクション言語」が存在する。
2. マルチデバイス表示において、`NO TRADE`、`Exit Summary`、`候補リスト` をそれぞれ個別に理解する必要がある。
3. 将来的に実際の執行層（Execution Layer）と接続する際、統一された「最終アクションプリミティブ」が欠落している。

統合後の目標構造は以下の通りです：

```text
Domain Facts (ドメイン事実)
→ Entry / Participation Gate (エントリーゲート)
→ Exit Decision (退出判断)
→ Position Intent Synthesizer (ポジション意図合成器)
→ Presentation Assembler (表示アセンブラ)
→ Report / UI / Execution (レポート / UI / 執行)
```

---

## 3. 設計原則

### 3.1 Position Intent は唯一の最終アクションである

初版では以下に統一します：

1. `ADD` (加筆/買い増し)
2. `HOLD` (保持)
3. `TRIM` (削減/減パ)
4. `EXIT` (退出/全決済)
5. `WATCH` (監視)

説明：

1. `ADD`
   - 能動的にリスクエクスポージャーを増やすことを許可する。
2. `HOLD`
   - 既存ポジションを継続して保持する。
3. `TRIM`
   - 既存ポジションの一部を削減する。
4. `EXIT`
   - 既存ポジションをすべて解消する。
5. `WATCH`
   - 新規建玉は許可されず、退出アクションも発生していないが、継続的な注視が必要な状態。

### 3.2 NO_TRADE は削除せず、グローバルゲートとして機能させる

以下の点を明確にする必要があります：

1. `NO TRADE` は引き続き存在します。
2. それは「能動的な新規建玉を禁止する」というグローバルな行動制限を表します。
3. ただし、それは最終的な資産アクションではありません。

つまり：

1. `NO TRADE` はポートフォリオレベルの制約です。
2. `Position Intent` は資産レベルの最終アクションです。

### 3.3 Entry と Exit は Intent で収束させ、互いに上書きしない

以下の事態を避ける必要があります：

1. `NO TRADE == EXIT`
2. `Exit == Market Bearish`
3. `WATCH == Candidate List`

統合の原則：

1. `NO TRADE` は「新規建玉ができるか」に答える。
2. `Position Intent` は「この資産を最終的にどう処理するか」に答える。

### 3.4 report.rs は統合後の Intent のみを消費する

`Position Intent` 統合レイヤーが導入された後は：

1. `report.rs` は、エントリー/退出の二重のセマンティクスを独自に繋ぎ合わせるのを停止します。
2. すべての文言、セクション分け、アクションラベルは、統一された intent から優先的に派生させます。

---

## 4. 推奨モデル

### 4.1 統一列挙型 (Enum)

推奨される配置場所：

[position_intent.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/position_intent.rs)

推奨される定義：

```rust
pub enum UnifiedPositionIntent {
    Add,
    Hold,
    Trim,
    Exit,
    Watch,
}
```

### 4.2 統一説明構造体

推奨：

```rust
pub struct PositionIntentDecision {
    pub intent: UnifiedPositionIntent,
    pub reasons: Vec<String>,
    pub source: PositionIntentSource,
}

pub enum PositionIntentSource {
    EntryGate,
    ExitGate,
    Synthesized,
}
```

説明：

1. `intent`
   - 最終アクション。
2. `reasons`
   - プレゼンテーション向けの理由のソース。
3. `source`
   - デバッグおよび監査用。

---

## 5. 統合ルール

### 5.1 優先順位

統合後の優先順位を以下のように固定します：

```text
EXIT > TRIM > HOLD > ADD > WATCH
```

### 5.2 基本的なマッピング

初版のマッピング推奨案：

1. `ExitDecision == EXIT`
   -> `UnifiedPositionIntent::Exit`
2. `ExitDecision == TRIM`
   -> `UnifiedPositionIntent::Trim`
3. `NO TRADE && has_position` (ポジションあり)
   -> `Hold` または `Watch`
   (資産状態と退出ルールに依存)
4. `NO TRADE && !has_position` (ポジションなし)
   -> `Watch`
5. `participation_ready && action/add path allowed`
   -> `Add`
6. `participation_ready && no add/no trim/no exit`
   -> `Hold`

### 5.3 Watch の定義

`WATCH` は明確に定義されなければなりません：

> 能動的な新規建玉は許可されず、現在は退出条件も満たしていないが、当該資産/状態は引き続き注視する価値がある。

これは以下の両方に適用されます：

1. ポジションのない候補資産。
2. ポジションはあるが、まだ退出条件に抵触していない「観察状態」の資産。

将来的にこれらを分離すべきと判断された場合は、次のフェーズで詳細化します。

---

## 6. 実装範囲

### Step 1: 統合レイヤーの新規追加

新規追加：

1. `src/core/position_intent.rs`
2. または、既存の `intent_synthesizer.rs` を進化させる。

責務：

1. `ParticipationReadiness` を受信する。
2. `ExitDecision` を受信する。
3. 資産のポジション事実 / 状態事実を受信する。
4. 統合された `UnifiedPositionIntent` を出力する。

### Step 2: DecisionPacket は純粋なドメイン事実を維持する

要件：

1. プレゼンテーション用の文言を `DecisionPacket` に詰め込まない。
2. 永続化が必要な場合は、説明文ではなく、統合 intent の構造化された結果を永続化する。

### Step 3: PresentationAssembler を統一 Intent の消費に変更

要件：

1. `Top Actions`
2. `Exit Summary`
3. `候補・監視リスト`
4. `戦術セクション`

これらすべてにおいて、統合された `UnifiedPositionIntent` を優先的に消費するようにします。

### Step 4: report.rs はレンダリングに専念する

要件：

1. intent に関するいかなる判断も新たに追加しない。
2. アセンブラ (Assembler) が産出した統一 View Model のみを消費する。

---

## 7. 「ポジションなし、処理不要」の将来的な拡張について

この提案は、完全に独立した将来の課題とするのではなく、本タスクに含めるべきです。

現状：

```text
現在はポジションがなく、処理は不要です。
```

アップグレード案：

```text
現在はポジションがなく、処理は不要です。
退出条件は一切満たされていません。
```

理由：

1. ポジションがある時の「退出判定のトーン」と一致させるため。
2. `Exit Layer` を常に説明レイヤーではなく「判定レイヤー」として機能させるため。
3. 将来的に `WATCH / HOLD / TRIM / EXIT` という同一のナラティブ（語り口）体系に統合しやすくするため。

要件：

1. この説明文はアセンブラによって一元的に生成されること。
2. `report.rs` 内で一時的に繋ぎ合わせないこと。
3. 多言語対応を同期させること。

---

## 8. 完了基準

### 8.1 構造の検収

以下の条件を満たすこと：

1. システム内に単一の統一アクションプリミティブが存在すること。
2. エントリーと退出がそれぞれ独自の最終アクション言語を形成していないこと。
3. `report.rs` は統合 intent から派生した結果のみを消費していること。

### 8.2 挙動の検収

以下の条件を満たすこと：

1. `NO TRADE` シナリオにおいて：
   - ポジションなし資産 -> `WATCH`
   - ポジションあり資産 -> `HOLD / TRIM / EXIT / WATCH`
   - すべてを一律に `SELL` と解釈しないこと。

2. `participation_ready` シナリオにおいて：
   - `ADD` の出現を許可する。
   - ただし、退出ルールがトリガーされた場合は、必ず `TRIM / EXIT` が優先（上書き）されること。

### 8.3 表示の検収

以下の条件を満たすこと：

1. レポート内で同一資産に対して矛盾するアクションが表示されないこと。
2. `Top Actions`、`ポジション処理提案`、`戦術セクション` の内容が一致していること。
3. `ポジションなし、処理不要` のシナリオにおいて、判定のトーンが維持されていること。

### 8.4 品質ゲート

以下のチェックを通過すること：

1. `cargo fmt`
2. `cargo test --quiet`
3. `cargo clippy --all-targets --all-features -- -D warnings`

---

## 9. テスト要件

少なくとも以下のテストを補完してください：

1. `NO TRADE + ポジションなし -> WATCH`
2. `NO TRADE + コアポジション -> HOLD`
3. `NO TRADE + 弱いポジション -> TRIM`
4. `DEFEND -> EXIT`
5. `OVERHEAT -> TRIM`
6. `participation_ready + 強い資産 -> ADD`
7. `ポジションなし、処理不要。退出条件は一切満たされていません。` の多言語出力。

さらに、少なくとも1つの完全な UI 契約（Contract）テストを追加し、以下を固定（Lock）してください：

1. ポートフォリオレベルの `NO TRADE`
2. 資産レベルの `Position Intent`
3. `ポジション処理提案`
4. `候補・監視リスト`

これら4つの階層が最終レポート内で矛盾なく共存していることを確認します。

---

## 10. 非目標 (Out of Scope)

本フェーズでは以下のことは行いません：

1. 低レイヤーの戦略の書き換え。
2. 複雑な利確・損切り体系への拡張。
3. `DecisionPacket` を表示用モデルへと先祖返りさせること。
4. `report.rs` にビジネスロジック（判断）を追加すること。

本フェーズで行うのは以下のことのみです：

> 統一された `Position Intent` プリミティブを用いて、「買えない」と「売るべきか」を同一のシステム言語へと収束させること。

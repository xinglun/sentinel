---
author: Ray
title: 退出意思決定サマリー (Exit Decision Summary) タスクリスト
description: 退出意思決定サマリー (Exit Decision Summary) タスクリスト に関する Sentinel の設計・運用情報。
key: docs-specs-exit-decision-summary-task
---

# 退出意思決定サマリー (Exit Decision Summary) タスクリスト

## 1. 目標

本タスクは退出システムを作り直すことではなく、下層にすでに存在する `ExitDecision` を、`NO TRADE` と同レベルの「フロントエンド結論層」に引き上げることを目的としています。

現在のシステムですでに明確に回答できていること：

1. 今日、新規開倉（エントリー）できるか。
2. 現在、`NO TRADE` 状態にあるか。
3. 新規エントリーの上限はいくらか。

しかし、レポートの「ファーストビュー（第一屏）」でまだ明確に回答できていないこと：

1. 既存の持分（ポジション）を継続保有すべきか、それとも減配すべきか。
2. すでに退出がトリガーされているか。
3. 現在の「買入不許可」が、同時に「売却の必要性」を意味しているか。

本フェーズの目標は、このレイヤーを補完することです：

> レポートで「買えない」と言うだけでなく、「売るべきか / 減配すべきか / 保持すべきか」を明確に伝えられるようにする。

---

## 2. 背景と課題

現在、Telegram / Markdown の戦況ボードには以下の要素が備わっています：

1. `NO TRADE` 行動禁止命令
2. 候補監視リスト
3. 監視シグナル
4. 戦術パーティション
5. リスクと機会

しかし、「既存持分の処理提案」がいまだに欠落しています。

これにより、以下のような典型的な問題が発生します：

1. システムは誤った買入を阻止できる。
2. しかし、既存のポジションが `HOLD`、`TRIM`、または `EXIT` のいずれであるべきかを明確に指導できない。

製品の観点から見ると、これは以下を意味します：

1. `Entry Gate` はすでにクローズドループ化（完結）している。
2. `Exit Gate` はいまだに下層のセマンティクスに留まっており、フロントページ（ファーストビュー）に反映されていない。

---

## 3. 設計原則

### 3.1 ExitDecisionSummary は NO_TRADE とデカップリング（分離）させる

以下のことを明確にする必要があります：

1. `NO TRADE` は、能動的な新規エントリーの禁止のみを意味する。
2. `NO TRADE` は、全売却が必須であることを意味しない。
3. `ExitDecisionSummary` が、既存持分の処理方法を個別に決定する。

つまり：

| シナリオ | 買入許可 | 継続保有許可 | 減配/退出許可 |
|---|---|---|---|
| `NO TRADE` | 否 | 可 | 可 |
| `DEFENSIVE` | 否 | 対象による | 可 |
| `ACCUMULATE` | 可 | 可 | 可 |

### 3.2 最小クローズドループ優先

初版では、複雑な利食い、コストライン（買値）、ATR、または損益ドローダウンロジックを導入しません。

以下の4つのコアルールのみを実装します：

1. `DEFEND -> EXIT`
2. `コア圏からの脱落が3日以上 -> TRIM`
3. `participation` が `true -> false` に変化した時：
   - 強い資産：`HOLD`
   - 弱い資産：`TRIM`
4. `OVERHEAT -> TRIM`

### 3.3 report.rs はレンダリングのみを行う

退出の判断は、すべて `PresentationAssembler` が下層の事実に基づいて生成しなければなりません。

`report.rs` は以下のレンダリングのみを行います：

1. タイトル
2. 状態タグ
3. 退出提案
4. 理由

退出ロジックの判断を `report.rs` に新規追加してはなりません。

---

## 4. データモデルの変更

### 4.1 ExitDecisionSummaryViewModel の新規追加

ファイル：
[presentation.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/presentation.rs)

以下の新規追加を推奨します：

```rust
pub struct ExitDecisionSummaryViewModel {
    pub title: String,
    pub items: Vec<ExitDecisionItemViewModel>,
}

pub struct ExitDecisionItemViewModel {
    pub symbol: String,
    pub intent: ExitDisplayIntent,
    pub intent_label: String,
    pub reason: String,
}

pub enum ExitDisplayIntent {
    Hold,  // 継続保有
    Trim,  // 減配
    Exit,  // 退出
    Watch, // 監視（売却条件未達だが注視が必要）
}
```

説明：

1. `Hold`
   既存ポジションの継続保有を示す。
2. `Trim`
   既存ポジションの減配（部分売却）を示す。
3. `Exit`
   既存ポジションからの退出（全売却）を示す。
4. `Watch`
   売却条件は満たしていないが、引き続き注視が必要であることを示す。

### 4.2 PresentationPacket の拡張

`PresentationPacket` に以下を追加します：

```rust
pub exit_summary: Option<ExitDecisionSummaryViewModel>,
```

要件：

1. 既存持分がない場合、または表示すべき項目がない場合は `None` とすることができる。
2. ポジション処理提案がある場合は、必ずファーストビュー（第一屏）に表示させる。

---

## 5. PresentationAssembler の組み立てルール

ファイル：
[presentation_assembler.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/presentation_assembler.rs)

### 5.1 入力ソース（事実の源泉）

Assembler は、既存のドメイン事実と意思決定出力に基づいて `ExitDecisionSummary` を構築しなければなりません。独自の新しい取引ルールを推測してはなりません。

利用可能な入力には以下が含まれます：

1. `asset.exit_decision`
2. `asset.position_intent`
3. `asset.display_context.has_position`
4. `asset.display_context.is_core_holding`
5. `participation_ready`
6. 資産状態と streak（連続性）情報

### 5.2 第1版のルール

初版の最小ルールは以下に固定します：

1. `DEFEND -> EXIT`
2. `asset_out_of_top_tier_streak >= 3 -> TRIM`
3. `participation_ready` が `true -> false` へ変化：
   - `is_core_holding == true -> HOLD`
   - それ以外 -> `TRIM`
4. `OVERHEAT -> TRIM`
5. その他、ポジションはあるが退出ルールに抵触しないもの -> `WATCH` または `HOLD`

### 5.3 NO_TRADE との関係

以下のことを明確にします：

1. `NO TRADE` シナリオ下でも `HOLD` は許可される。
2. `NO TRADE` シナリオ下でも `TRIM` が発生する可能性がある。
3. `NO TRADE` シナリオ下で、すべての資産を一律に `EXIT` と出力してはならない。

### 5.4 推奨される出力スタイル

日本語：

```text
### 📉 ポジション処理提案

- NVDA · 継続保有
  構造未破壊、継続保有

- TSLA · 監視継続
  押し目中、まだ減資条件未達

- FIG · 減資
  核心圏離脱が 3 日継続
```

---

## 6. i18n 辞書の要件

ファイル：
[i18n.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/i18n.rs)

少なくとも以下のフィールドを追加します：

```rust
exit_summary_title
exit_intent_hold
exit_intent_trim
exit_intent_exit
exit_intent_watch
exit_reason_defend
exit_reason_strength_loss
exit_reason_participation_fallback
exit_reason_overheat
exit_reason_hold_core
exit_reason_watch_pullback
```

要件：

1. 中・英・日の3言語すべてを完備すること。
2. `report.rs` 内で退出理由をハードコードすることを禁止する。
3. 理由は「製品としての言語」であるべきで、下層のデバッグ文であってはならない。

---

## 7. report.rs のレンダリング要件

ファイル：
[report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

新規セクションの追加：

```text
### 📉 ポジション処理提案
```

配置順序は以下に固定します：

1. 市場サマリー
2. 意思決定の結論
3. ポジション処理提案
4. 候補監視リスト / 主要アクション
5. 監視シグナル
6. 戦術パーティション
7. リスクと機会

要件：

1. `report.rs` は `exit_summary` のレンダリングのみを行う。
2. `report.rs` 内で `HOLD / TRIM / EXIT / WATCH` を再判断してはならない。
3. `NO TRADE` シナリオ下で以下が同時に表示されることを許可する：
   - `行動禁止（NO TRADE）`
   - `ポジション処理提案`

---

## 8. 完了基準

### 8.1 機能検証

ファーストビューで以下を同時に表現できていること：

1. 買入が許可されているか。
2. 既存ポジションの売却 / 減配 / 保持が必要か。

つまり：

1. `NO TRADE` が「全売却」と誤解されないこと。
2. `ExitDecisionSummary` が欠落していないこと。
3. 以下の区別がついていること：
   - `HOLD`
   - `TRIM`
   - `EXIT`
   - `WATCH`

### 8.2 構造検証

以下の条件を満たすこと：

1. `PresentationAssembler` が退出提案を組み立てる唯一のレイヤーであること。
2. `report.rs` はレンダリングのみを行い、退出ロジックを新規追加していないこと。
3. `DecisionPacket` は純粋なドメイン事実を維持し、展示用フィールドの書き戻しを行わないこと。

### 8.3 セマンティクス（意味論）検証

以下の条件を満たすこと：

1. `NO TRADE` は引き続き、能動的な新規エントリーの禁止を意味すること。
2. `HOLD` / `TRIM` / `EXIT` は既存ポジションの処理を意味すること。
3. 両者が混同されないこと。

### 8.4 品質ゲート

以下をパスすること：

1. `cargo fmt`
2. `cargo test --quiet`
3. `cargo clippy --all-targets --all-features -- -D warnings`

---

## 9. テスト要件

### 9.1 Presentation Tests

少なくとも以下のケースを補完すること：

1. `DEFEND -> EXIT`
2. `out_of_top_tier_streak >= 3 -> TRIM`
3. `NO TRADE + core holding -> HOLD`
4. `NO TRADE + weak holding -> TRIM`
5. `OVERHEAT -> TRIM`

### 9.2 Report UI Tests

少なくとも以下のケースを補完すること：

1. ファーストビューに以下が同時に出現すること：
   - `行動禁止（NO TRADE）`
   - `ポジション処理提案`
2. `NO TRADE` シナリオ下で、すべての資産が `売却` とレンダリングされないこと。
3. `HOLD / TRIM / EXIT / WATCH` のローカライズ文言が正しいこと。

### 9.3 多言語回帰テスト

少なくとも以下をカバーすること：

1. `zh-cn`
2. `ja-jp`
3. `en-us`

そして以下を検証すること：

1. タイトルが存在すること。
2. 意図（Intent）タグが存在すること。
3. 理由の文言が英語のデバッグテキストにフォールバックしていないこと。

---

## 10. 非目標 (Out of Scope)

本フェーズでは以下のことは行いません：

1. ATR、コストライン、含み益ドローダウンなどの複雑な利食いロジックの導入。
2. `Market Regime` の変更。
3. `ParticipationReadiness` の変更。
4. `ActionMatrix` の変更。
5. `ExitDecision` の下層ルール体系の再構築。

本フェーズで行うのは：

> 既存の退出セマンティクスを、`NO TRADE` と同レベルの「展示結論層」に引き上げることです。

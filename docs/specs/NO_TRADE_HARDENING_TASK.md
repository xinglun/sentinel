---
author: Ray
title: NO_TRADE 強制拘束化タスクリスト (NO_TRADE_HARDENING_TASK.md)
description: NO_TRADE 強制拘束化タスクリスト (NO_TRADE_HARDENING_TASK.md) に関する Sentinel の設計・運用情報。
key: docs-specs-no-trade-hardening-task
---

# NO_TRADE 強制拘束化タスクリスト (NO_TRADE_HARDENING_TASK.md)

## 1. 背景

現在、Telegram の戦況ボードでは `NO TRADE` を表示できるようになっており、低安定性、連続性の不足、`trend_cohesion` 未成立時に「行動禁止」という主結論を出せるようになっています。

しかし、現在の実装にはまだいくつかの「セマンティック（意味的）な漏水」が存在します：

1. `0-10%` といったポジション（倉位）表現が、ユーザーに対して「少しなら試してもいいのではないか」という心理的な逃げ道を残してしまっている。
2. `NO TRADE` が依然として展示用の結論に留まっており、行動を強制するハードゲートになっていない。
3. 候補監視リストは降格（ダウングレード）されているものの、より強力なルール制約が欠けており、将来的に文言が「取引候補」へと漂流するリスクがある。
4. `NO TRADE` の設計上の意味が明確に定義されていないため、将来的に `DEFENSIVE`（防御的）や「弱気市場」と誤認される可能性がある。

本タスクの目標は、単なる美化ではなく、`NO TRADE` を「動かないように注意する」レベルから「システムが明確に行動を禁止する」レベルへとアップグレードすることです。

---

## 2. 設計原則

### 2.1 NO_TRADE の正式な定義

ドキュメント、コードのセマンティクス、および展示出力において、以下を明確に定義します：

> `NO TRADE` は展示状態であり、市場への弱気判断を意味するものではない。「現時点で能動的な新規エントリーを許可しない」ことを意味する。

これは以下を意味します：

1. `NO TRADE` は `DEFENSIVE` ではない。
2. `NO TRADE` は強制的な全清算を意味しない。
3. `NO TRADE` はトレンド転換の判断ではない。
4. `NO TRADE` は「現時点で能動的なリスク露出（リスクテイク）を増加させない」という行動制約である。

### 2.2 行動制約は情報表示に優先する

`NO TRADE` シナリオ下での出力順序は、以下を遵守しなければなりません：

1. まず禁止命令。
2. 次に行動モード。
3. 次に枠（キャパシティ）制限。
4. 次に戦況サマリー。
5. 最後に理由説明と候補リスト。

「候補リスト」を「行動禁止命令」の前に配置してはなりません。

### 2.3 候補リストのさらなる降格（ダウングレード）

`NO TRADE` シナリオ下では、いかなる資産リストも以下のように表現されなければなりません：

1. 観察 (Observe)
2. 候補 (Candidate)
3. 準備 (Preparing)
4. 強度確認中 (Confirming)
5. 押し目観察 (Pullback Watch)

以下の表現は禁止します：

1. 加倉 (Add)
2. 買入 (Buy)
3. 建倉 (Enter)
4. 取引候補 (Trade Candidate)

---

## 3. 最終的な表示契約 (Display Contract)

`NO TRADE` シナリオ下において、最終的な表示順序は以下のように固定されます：

```text
### 禁止動作（NO TRADE）

> いかなる能動的な売買行為も、システム規律違反となります。

> 状態：未確認始動期
> 行動：取引禁止
> 新規建て上限 · 0%

> 戦況サマリー · 観察 8 | 保持 0 | 収縮 0
> チャンス · 明確な機会なし
> リスク · 顕著なリスクなし

- 未完了の理由
- 候補監視リスト
- 監視シグナル
```

現在の実装では、さらに「実行優先」の順序へと収束させています：

```text
1) 意思決定層（ファーストビュー）
   - 禁止動作（NO TRADE）
   - 新規建て上限 · 0%

2) 原因層（簡略版）
   - 安定性 x/10
   - 連続性 x/3
   - 主軸構造（例：主軸なし）

3) 監視重点層
   - ブレイクアウト（Breakout）識別
   - breakout 状態の表示に時間感覚を持たせる（例：ブレイクアウト初期（1日目））

4) 根拠層（セカンドビュー以降）
   - 状態遷移の根拠（フロントエンドではコンパクトに、アーカイブでは詳細に）
```

そのうち、以下の3つがハードルールとなります：

1. `NO TRADE` が必ずファーストビューに現れること。
2. `新規建て上限 · 0%` が必ず現れること。
3. `いかなる能動的な売買行為も、システム規律違反となります。` という文言が必ず現れること。

追加のハードルール：

4. フロントエンドの根拠層が、意思決定層よりも前の位置を占拠してはならない。
5. テンプレートのプレースホルダー（例：`{}` / `{:.1}`）が最終的なレポートテキストに残っていてはならない。

---

## 4. データモデルの変更

### 4.1 DecisionSummaryViewModel への新規フィールド追加

ファイル：
[presentation.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/interface/presentation.rs)

以下のフィールドを追加します：

```rust
pub state_tag_label: String,
pub state_tag_value: String,
pub action_tag_label: String,
pub action_tag_value: String,
pub hard_rule_note: String,
pub entry_cap_label: String,
pub entry_cap_value: String,
pub entry_cap_note: Option<String>,
```

各フィールドの責務：

1. `state_tag_*`
   `状態：未確認始動期` などの出力に使用。

2. `action_tag_*`
   `行動：取引禁止` などの出力に使用。

3. `hard_rule_note`
   行動禁止命令の出力に使用。例：
   `いかなる能動的な売買行為も、システム規律違反となります。`

4. `entry_cap_*`
   以下の出力に使用：
   `新規建て上限 · 0%`

5. `entry_cap_note`
   補足説明の出力に使用。例：
   `既存保有の自然変動のみ許容し、新規建ては行わない。`

---

## 5. PresentationAssembler の組み立てルール

ファイル：
[presentation_assembler.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/interface/presentation_assembler.rs)

### 5.1 行動ルール

初版の明確なルール：

```text
if is_data_missing || !trend_gate_changed:
    action_status = NO_TRADE
```

同時に、以下のセマンティック（意味的）な説明を満たさなければなりません：

```text
NO_TRADE != DEFENSIVE
NO_TRADE は能動的な新規エントリーの禁止を意味する
```

### 5.2 state_tag ルール

初版の推奨案：

1. `IGNITION + !trend_gate_changed`
   -> `未確認始動期`

2. `is_data_missing`
   -> `データ利用不可`

3. その他の非 ready シナリオ
   -> `参加条件未達`

### 5.3 action_tag ルール

以下のように固定します：

1. `NO_TRADE` -> `取引禁止`
2. `PROBE` -> `試行的参加`
3. `ACCUMULATE` -> `能動的加倉`
4. `TREND_FOLLOW` -> `トレンドフォロー`
5. `DEFENSIVE` -> `防御的収縮`

### 5.4 entry_cap ルール

`NO TRADE` シナリオ下では以下に固定します：

```text
entry_cap_label = 新規建て上限
entry_cap_value = 0%
entry_cap_note = 既存保有の自然変動のみ許容し、新規建ては行わない。
```

以下の生成は禁止します：

1. `ポジション提案 · 0-10%`
2. `0-10%`

### 5.5 candidate_only_note ルール

Assembler が一元的に出力しなければなりません：

```text
以下は候補監視リストであり、取引指示を構成するものではありません。
```

`report.rs` 内でその場しのぎの連結を行ってはなりません。

---

## 6. i18n 辞書の要件

ファイル：
[i18n.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/shared/interface/i18n.rs)

少なくとも以下の項目を追加します：

```rust
state_tag
action_tag
entry_cap
entry_cap_note
state_ignition_unconfirmed
state_data_unavailable
state_participation_blocked
no_trade_rule
```

### 6.1 日本語の推奨文言

1. `state_tag`: `状態`
2. `action_tag`: `行動`
3. `entry_cap`: `新規建て上限`
4. `entry_cap_note`: `既存保有の自然変動のみ許容し、新規建ては行わない。`
5. `state_ignition_unconfirmed`: `未確認始動期`
6. `state_data_unavailable`: `データ利用不可`
7. `state_participation_blocked`: `参加条件未達`
8. `no_trade_rule`: `あらゆる能動売買はシステム規律違反となる。`

### 6.2 中国語（簡体字）の推奨文言

1. `state_tag`: `状态`
2. `action_tag`: `行为`
3. `entry_cap`: `新开仓上限`
4. `entry_cap_note`: `仅允许已有持仓自然波动，不允许主动开仓。`
5. `state_ignition_unconfirmed`: `未确认启动期`
6. `state_data_unavailable`: `数据不可用`
7. `state_participation_blocked`: `参与条件未满足`
8. `no_trade_rule`: `任何主动交易行为都将违反系统规则。`

---

## 7. report 層の責務

ファイル：
[report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/features/radar/interface/report.rs)

要件：

1. `DecisionSummaryViewModel` のみを消費すること。
2. 固定された順序でレンダリングすること。
3. ビジネス上の判断を一切新規追加しないこと。
4. `report.rs` 内でルールの文言を連結しないこと。

### 7.1 禁止事項

`report.rs` において以下を禁止します：

1. `market_state` に基づいて独自に `NO TRADE` かどうかを推測すること。
2. `trend_gate_changed` に基づいて独自に状態タグを組み立てること。
3. 資産状態に基づいて独自に候補リストの説明文を組み立てること。

---

## 8. Top Actions / 候補監視リストの制約

`NO TRADE` シナリオ下において：

1. タイトルを `候補監視リスト` へと降格（ダウングレード）させなければなりません。
2. 資産項目において「取引を唆るセマンティクス（意味）」が現れてはなりません。

### 8.1 禁止されるセマンティクス

候補監視リスト内のいかなる資産に対しても、以下の表現を禁止します：

1. 加倉
2. 買入
3. 建倉

### 8.2 許可されるセマンティクス

以下の表現は許可されます：

1. 観察 (Observe)
2. 候補 (Candidate)
3. 準備 (Preparing)
4. 押し目 (Pullback)
5. 強度確認中 (Confirming)

---

## 9. 完了基準

### 9.1 機能検証

`NO TRADE` シナリオ下で以下を満たさなければなりません：

1. ファーストビューに `禁止動作（NO TRADE）` が現れること。
2. `状態：未確認始動期` が必ず現れること。
3. `行動：取引禁止` が必ず現れること。
4. `新規建て上限 · 0%` が必ず現れること。
5. `あらゆる能動売買はシステム規律違反となる。` が必ず現れること。
6. `以下は候補監視リストであり、取引指示を構成するものではありません。` が必ず現れること。

### 9.2 逆方向からの検証（Negative Test）

`NO TRADE` シナリオ下で以下を満たしてはなりません：

1. `0-10%` が現れてはならない。
2. 旧フィールドである `ポジション提案` が現れてはならない。
3. 候補監視リストの中に `加倉` が現れてはならない。
4. 候補監視リストの中に `買入` が現れてはならない。
5. 候補監視リストの中に `建倉` が現れてはならない。

### 9.3 アーキテクチャ検証

1. ルールに関連するすべての文言が Assembler によって生成されていること。
2. `report.rs` がビジネス判断を新規追加していないこと。
3. `DecisionPacket` に展示用フィールドが混入していないこと。
4. `DecisionPacket -> PresentationPacket -> report` という単方向のデータフローを維持していること。

### 9.4 品質ゲート

以下を同時にパスすること：

1. `cargo fmt`
2. `cargo test --quiet`
3. `cargo clippy --all-targets --all-features -- -D warnings`

---

## 10. テスト要件

### 10.1 presentation_tests

少なくとも以下の断言（Assertion）を新規追加または強化すること：

1. `entry_cap_value == "0%"`
2. `hard_rule_note` が存在すること。
3. `state_tag_value == "未確認始動期"`
4. `action_tag_value == "取引禁止"`
5. `candidate_only_note` が存在すること。

### 10.2 report_ui_tests

少なくとも以下の断言を新規追加または強化すること：

1. `新規建て上限 · 0%` を含んでいること。
2. `0-10%` を含んでいないこと。
3. 旧フィールドである `ポジション提案` を含んでいないこと。
4. `状態：未確認始動期` を含んでいること。
5. `行動：取引禁止` を含んでいること。
6. 候補監視リストの中に `加倉 / 買入 / 建倉` を含んでいないこと。

### 10.3 i18n 回帰テスト

少なくとも以下を検証すること：

1. 日本語、英語、中国語の `NO TRADE` ルールの文言が正しく注入されていること。
2. `entry_cap_note` に文言の欠落がないこと。
3. `state_tag / action_tag` に文言の欠落がないこと。

---

## 11. 非目標 (Out of Scope)

本フェーズでは以下のことは行いません：

1. `ParticipationReadiness` を current SSOT と誤認させる文言の追加。
2. `ExitDecision` 判定ルールの変更。
3. `Engine` の変更。
4. `DecisionPacket` の変更。
5. 取引実行層の変更。

本フェーズで行うのは以下のことのみです：

1. `NO TRADE` 表示の強制拘束化。
2. ポジション表現の引き締め。
3. 候補リスト降格（ダウングレード）プロトコルの固定化。

---

## 12. 最終的な納品定義

本フェーズの完了の証は「文言がよりスムーズになった」ことではなく、以下の点にあります：

> 優位性がない状態において、システムがユーザーに「動かないように注意する」のではなく、ユーザーによる能動的な新規エントリーを「明確に禁止する」こと。

以下の5項目が同時に成立したとき、本タスクは完了と見なされます：

1. 状態タグ。
2. 行動タグ。
3. 新規建て上限 0%。
4. ルールに基づく禁止命令。
5. 候補リスト降格（ダウングレード）の説明文。

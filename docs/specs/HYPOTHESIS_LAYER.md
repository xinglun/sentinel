---
author: Ray
title: Hypothesis Layer / Future Map Layer
description: Reality Layer と分離して将来構造仮説を表示するための仕様。
key: hypothesis-layer
---

# Hypothesis Layer / Future Map Layer

Hypothesis Layer は、現在の事実を扱う Reality Layer とは分離して、将来起こり得る構造変化を推測として表示するための表示専用レイヤーである。

## 目的

このレイヤーは、まだ事実として確定していないが観察に値する可能性を構造化する。

対象は次の通りである。

- 利益プールの移動仮説
- 資本配分の変化
- 市場が十分に織り込んでいない可能性
- 潜在的受益者
- コンセンサス状態
- 市場織り込み状態
- ナラティブ飽和度
- 時間軸と実現ウィンドウ
- 現実優先による確信度減衰
- 失敗パス

## 境界

Hypothesis Layer は表示専用であり、次へ接続してはならない。

- Engine
- Gate
- execution_gate
- trend_cohesion
- action_matrix
- trader_agent
- exit
- 新規建玉上限
- 買い増し・一部削減判断

Reality Layer の結果を参照してよいが、Reality Layer を上書きしてはならない。

## Anti-Narrative Governance

Hypothesis Layer は、未来の物語を強化するだけの narrative machine になってはならない。すべての長期 thesis は、次の問いを表示または設定で表現できる必要がある。

- この hypothesis は市場に先行しているのか、すでに consensus を増幅しているだけなのか。
- この hypothesis はどの時間軸で検証されるのか。
- どの observable reality が継続的に反証した場合、confidence を減衰させるのか。

`asset_thesis` は次の optional governance fields を持てる。

```toml
[asset_thesis.MSFT]
time_horizon = "LONG"
materialization_window = "12-36 months"

[asset_thesis.MSFT.narrative_state]
consensus_level = "EARLY | DEVELOPING | CROWDED | SATURATED"
skepticism_level = "HIGH | NORMAL | LOW"
valuation_reflection = "UNDERREFLECTED | PARTIAL | FULLY_PRICED"

[asset_thesis.MSFT.reality_override]
observable_contradiction = true
confidence_decay = true
```

`consensus_level = "CROWDED"` または `valuation_reflection = "FULLY_PRICED"` は売却 signal ではない。これは「正しいか」ではなく「すでに信じられすぎていないか」を観測するための metadata である。

`time_horizon` は `SHORT`、`MEDIUM`、`LONG`、`CIVILIZATION` のいずれかとする。短期の `NO TRADE` と長期 thesis は同じ意味層に混ぜない。

Reality Override Rule は憲法級の制約である。observable reality が hypothesis と継続的に矛盾する場合、hypothesis の confidence は自動的に下がるべきであり、物語の整合性だけで維持してはならない。

## Responsibility Split

Reality Layer の責任主体はシステムである。現在の市場状態、価格構造、Breakout、Breadth、Macro Gravity、Crowding Risk、Gate、NO TRADE、Main Theme Persistence を厳格に観測する。

Hypothesis Layer の責任主体はユーザーである。未来の可能性、利益移動仮説、産業構造の変化、市場がまだ十分に織り込んでいない候補を推測として扱う。

## 表示ルール

表示には必ず speculative notice を含める。

```text
以下は将来構造の仮説であり、現在の事実ではなく、売買シグナルを生成しない。
```

Hypothesis Candidate は必ず failure risks を持つ。failure risks が空の candidate は表示しない。

`Confirmed` という confidence は使わない。Hypothesis Layer で confirmed を使うと Reality Layer と混同するためである。

## Phase 1

Phase 1 は表示層だけを実装する。

対象ファイルは次の通りである。

- `src/features/radar/interface/presentation.rs`
- `src/features/radar/interface/presentation_assembler.rs`
- `src/features/shared/interface/i18n.rs`
- `src/features/radar/interface/report.rs`
- `report_ui_tests.rs`
- `presentation_tests.rs`

Phase 1 では、既存の Reality evidence から表示用 hypothesis を組み立てる。ただし、組み立て結果は `PresentationPacket` 内に閉じ、engine / Gate / execution には渡さない。

Hypothesis Layer は `State Transition Evidence` の内部に入れず、独立した表示セクションとして描画する。これにより、状態転移ログが存在しない安定日でも、Reality evidence が条件を満たす限り未来地図を表示できる。

## 初期 Candidate

初期 candidate は `ProfitPoolMigration` である。

仮説は次の通りである。

```text
AI 利益プールが GPU layer から cloud / platform layer へ移動する可能性
```

生成条件は、既存の実体的証拠から次が確認できる場合に限定する。

- Capex Payoff が存在する
- Earnings Quality または Order Visibility が存在する
- conviction score が一定以上である

これは売買判断ではなく、観察対象の未来地図である。

## 禁止語

Hypothesis Layer は次の語を実行文脈で使ってはならない。

- 買入
- 売却
- 買い増し
- 一部削減
- 必須
- 確定受益
- confirmed

## テスト契約

最低限、次を固定する。

- Hypothesis が存在しても Gate / NO TRADE / 新規建玉上限は変化しない。
- Telegram / Markdown に speculative notice が表示される。
- failure risks が空の candidate は表示されない。
- `HypothesisConfidence` に `Confirmed` を追加しない。
- Reality section に Hypothesis 専用語が混入しない。
- Hypothesis section は状態転移証拠の内部に入らず、`transition_log` がなくても表示できる。

## 文書ガード

日本語 Markdown に中国語の売買用語が混入した場合は `make check-doc-forbidden-terms` で検出する。`make quality` はこのチェックを含む。

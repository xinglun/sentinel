---
author: Ray
title: Gray Rhino Escalation Framework Implementation Spec
description: 灰色のサイのリスク昇格監視レイヤーの実装仕様
key: gray-rhino-escalation-implementation-spec
---

# Gray Rhino Escalation Framework Implementation Spec

Gray Rhino Escalation Framework は、長期構造リスクが背景リスクから危険な臨界域へ移り始めたかを観測するための Long-Term Structural Risk Layer である。

この layer は市場の上昇・下落、強気・弱気、売買、減倉、Gate、execution、trend cohesion を判断しない。Reality Layer、Tactical Layer、Execution Layer を上書きしない。

## SSOT

本ドキュメントを Gray Rhino Escalation Framework の repository 内 SSOT とする。

実装は次の境界に従う。

- domain model と状態判定は `src/features/research/domain/gray_rhino.rs` に置く。
- CLI / Markdown / Telegram 表示は `src/features/research/interface/gray_rhino_report.rs` に置く。
- CLI command は観測レポート出力だけを行い、取引・Gate・execution へ接続しない。
- 設定入力は `gray_rhino_escalation` に限定する。

## 目的

目的は、長期構造リスクそのものではなく、システムや市場がそのリスクに慣れ始めているかを観測することである。

危険なのは、リスクが可視化されていないことだけではない。長期的な成功のあと、市場がなぜそれをリスクと見なしていたかを忘れることである。

## 入力項目

`GrayRhinoEscalation` は次の観測項目を持つ。

- `risk_expansion_rate`: リスク拡張速度。
- `constraint_growth_rate`: governance、audit、redundancy、oversight、succession などの制度成熟速度。
- `dependency_centralization`: founder、infra、compute stack、cloud provider、model ecosystem などへの依存集中。
- `awareness_decay`: 市場のリスク感知低下。
- `narrative_overconfidence`: 「今回は違う」という過信。
- `single_point_fragility`: 単一 founder、単一 infra、単一 compute、単一 cloud、単一 ecosystem への単一点脆弱性。
- `fallback_survivability_risk`: redundancy、correction capability、institutional stability、fallback survivability が脅かされるリスク。
- `notes`: 構造的観測メモ。取引指示、人格評価、政治攻撃、恐怖表現は出力してはならない。

各 risk 入力は `LOW`、`MODERATE`、`ELEVATED`、`HIGH` を取る。

## 状態

`RhinoEscalationState` は次の状態を持つ。

- `Background`: リスクは存在するが、システム影響力はまだ限定的。
- `Visible`: 市場がリスクを認識し始める。
- `Expanding`: リスクとシステム規模が同時に拡大している。
- `Normalized`: リスクが拡大しているにもかかわらず、市場の感知が低下している。最も危険な通常化局面。
- `Critical`: 単一点リスクが redundancy、correction capability、institutional stability、fallback survivability を明確に脅かしている。

## 判定ルール

内部 score は次の形を基準にする。

```text
EscalationScore =
  risk_expansion_rate
  + dependency_centralization
  + awareness_decay
  + narrative_overconfidence
  + single_point_fragility
  + fallback_survivability_risk
  - constraint_growth_rate
```

score は内部の状態判定補助であり、取引信号として表示・利用してはならない。

`Normalized` は、リスク拡張、感知低下、ナラティブ過信が同時に高まる場合に成立する。

`Critical` は、リスク拡張と依存集中が高く、単一点脆弱性と fallback survivability risk が高まり、制度成熟が追いついていない場合に成立する。

## 出力境界

出力は構造的な昇格警告に限定する。

許可される表現:

- Gray Rhino Escalation
- State
- Escalation
- Observation
- This is not a trading signal.
- This is a structural escalation warning.

禁止される表現:

- BUY / SELL
- 自動的な bearish 判定
- 自動減倉
- Gate / execution / trend cohesion の変更
- Reality Layer の上書き
- 陰謀論
- 政治攻撃
- 人格評価
- 恐怖を煽る narrative

`notes` に禁止表現が含まれる場合、表示せず抑制件数だけを出力する。

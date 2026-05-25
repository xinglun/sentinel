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
- 日次評価の生成は `src/features/research/application/gray_rhino_assessment.rs` に置く。
- 日次 snapshot の JSONL 永続化は `src/features/research/infrastructure/gray_rhino_snapshot_store.rs` に置く。
- CLI command は観測レポート出力だけを行い、取引・Gate・execution へ接続しない。
- 設定入力は `gray_rhino_escalation` に限定する。
- `daily-calibration` は独立した Gray Rhino セクションを出力し、他の校正セクションや判断結果を変更しない。

## 設定方法

`config.toml` は運用者が管理する設定であり、AI hard gate では自動更新しない。日次校正レポートに Gray Rhino を表示する場合、運用者は次の観測初期値を `config.toml` に追加する。

```toml
[gray_rhino_escalation]
enable = true
risk_expansion_rate = "MODERATE"
constraint_growth_rate = "MODERATE"
dependency_centralization = "ELEVATED"
awareness_decay = "MODERATE"
narrative_overconfidence = "MODERATE"
single_point_fragility = "MODERATE"
fallback_survivability_risk = "MODERATE"
notes = []
```

この初期値は取引判断ではなく、依存集中を含む構造的観測の開始点である。各観測値と `notes` は事実確認済みの運用入力に基づいて更新する。`notes` を追加する場合は、`output.language` の表示言語に合わせて入力する。

現在の入力由来は `ManualConfiguration`（手動構造ベースライン）である。専用の外部リスク evidence source は未接続であり、設定入力を自動収集した事実として表示してはならない。

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

## 日次監査 snapshot

`daily-calibration` 実行時、表示された評価は `gray_rhino_snapshots.jsonl` に追記型 snapshot として保持する。

snapshot は次を含む。

- `as_of_date`: 市場監査ログと一致する業務日。監査ログがない場合は実行日。
- `source`: 現在は `ManualConfiguration` のみ。
- `escalation`: 状態と入力観測項目。

日報には評価日、入力由来、前回日次評価との差分を表示する。同一日の同一 snapshot の再実行は重複追記しない。これにより、灰色のサイ観測値がいつ変更されたかを監査可能にする。

日報は `手動構造ベースライン -> 7 観測項目 -> 日次 snapshot` という監査チェーンと、明示ルール判定で再生可能であることを表示する。このチェーンは外部 fact evidence chain ではなく、現在の手動入力評価がどの経路で状態へ変換されたかを示す lineage である。

本 snapshot は自動情報収集を意味しない。将来、専用の構造リスク evidence adapter を追加する場合も、由来と証拠日を snapshot に保持し、手動入力と区別する。

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

score は内部の状態判定補助であり、Markdown / Telegram などの通常出力には表示しない。取引信号として表示・利用してはならない。

`Normalized` は、リスク拡張、感知低下、ナラティブ過信が同時に高まる場合に成立する。

`Critical` は、リスク拡張と依存集中が高く、単一点脆弱性と fallback survivability risk が高まり、制度成熟が追いついていない場合に成立する。

現在の実装では、score が 2 以上で `Visible`、4 以上で `Expanding`、5 以上で `Normalized` の候補になる。`Critical` は score だけでなく、risk expansion、dependency centralization、single point fragility、fallback survivability risk、constraint growth の組み合わせを必須条件とする。

## 出力境界

出力は構造的な昇格警告に限定する。

固定 UI 文言は選択された表示言語で統一する。ユーザー入力である `notes` の翻訳は自動で行わず、運用者が表示言語に合わせて記述する。

risk level の表示値も選択言語へローカライズする。設定値の enum 表現は設定契約に限定し、日報本文へ漏らさない。

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

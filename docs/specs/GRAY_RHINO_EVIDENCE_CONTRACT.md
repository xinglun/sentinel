---
author: Ray
title: Gray Rhino Evidence Contract
description: 灰色のサイ専用 evidence の分類、出所、監査境界を定義する契約
key: gray-rhino-evidence-contract
---

# Gray Rhino Evidence Contract

Gray Rhino Evidence Contract は、Gray Rhino Escalation を「手動構造ベースライン」から「外部 evidence に裏付けられた構造リスク intelligence」へ進めるための契約である。

この契約は表示項目を増やすための仕様ではない。何を evidence と見なせるか、何を narrative として拒否するか、どの source traceability が必要かを定義する。

## SSOT

本ドキュメントを Gray Rhino evidence の human-readable SSOT とする。

machine-readable SSOT は `.ai/architecture/gray_rhino_evidence_schema.yaml` とする。実装、checker、将来の collector はこの schema に従う。

## 現在の状態

現在の Gray Rhino Escalation は `ManualConfiguration` を source とする Human-Structured Observation System である。

運用者が構造的観測値を定義し、system は表示、三言語出力、snapshot、差分、監査 chain を提供する。外部事実 source を自動的に発見しているわけではない。

## Evidence と Narrative の境界

Gray Rhino evidence は、長期構造リスクに関する外部 source 由来の観測事実でなければならない。

次のものは evidence ではない。

- 価格変動だけから作った解釈。
- 強気 / 弱気の市場 narrative。
- founder や個人への人格評価。
- 政治的攻撃や陰謀論。
- 「危険そう」「成功しすぎている」などの未検証感想。
- trade、Gate、execution、trend cohesion を変えるための信号。

これらは hypothesis や operator note として保存できる場合があるが、Gray Rhino evidence として escalation engine へ入力してはならない。

## Evidence Categories

Gray Rhino evidence は次の category のいずれかに分類する。

### Governance Concentration Evidence

governance 権限や oversight が単一主体に集中している証拠。

例:

- voting structure。
- board independence。
- dual class shares。
- founder override power。
- succession governance。

Phase 3-A では、この category を最初の低ノイズ sensor として実装する。入力は repository-local structured JSON または SEC / governance document source とし、次の構造項目のうち少なくとも 1 つを source から抽出する。

- `founder_voting_power`。
- `independent_board_ratio`。
- `dual_class_structure`。
- `super_voting_rights`。
- `succession_disclosure`。

許可する source type は `RegulatoryFiling`、`GovernanceDocument`、`CompanyDisclosure`、`OperatorCuratedSource` に限定する。保存先は `gray_rhino_evidence.jsonl` とし、raw source は `gray_rhino_sources/governance` に cache する。保存された evidence は escalation state、Gate、execution、trading state を変更しない。

### Dependency Concentration Evidence

business、infrastructure、compute、cloud、launch、supplier、ecosystem などの依存集中を示す証拠。

例:

- infrastructure dependency。
- compute dependency。
- cloud dependency。
- launch dependency。
- single supplier dependency。

### Institutional Maturity Evidence

組織規模や社会的重要性に対して、制度成熟が追いついているかを示す証拠。

例:

- succession structure。
- external audit。
- disclosure quality。
- oversight evolution。
- compliance maturity。

### Risk Normalization Evidence

長期リスクが認識されながら、市場や組織がそれを通常化している証拠。

例:

- narrative compression。
- skepticism decline。
- "too successful to fail" narrative。
- risk language disappearance。
- dependency risk が成功 narrative に吸収されること。

### Redundancy Evidence

代替手段や fallback が存在するかを示す証拠。

例:

- fallback availability。
- alternative suppliers。
- ecosystem decentralization。
- operational redundancy。
- recovery path maturity。

## Source Contract

Gray Rhino evidence は source を必須とする。

必須項目:

- `source_url` または repository-local source path。
- `source_title`。
- `publisher`。
- `published_at` または `observed_at`。
- `retrieved_at`。
- source type。
- evidence category。
- confidence。
- extraction note。

source が不明なものは evidence として扱わない。

## Quality Contract

collector または adapter は次を満たす必要がある。

- source と extraction note を分離する。
- quote / fact と operator interpretation を分離する。
- category を schema enum から選択する。
- confidence は evidence quality を表し、risk severity を表さない。
- evidence は escalation state を直接指定しない。
- evidence は trade signal、Gate、execution を生成しない。

## Phase Boundary

自動情報収集は evidence contract を満たす adapter が存在するまで開始しない。

### Phase 1: Human Structured Governance Observation

完了済み。手動設定、三言語表示、snapshot、差分、監査 chain を提供する。

### Phase 2: Gray Rhino Evidence Schema

完了済み。evidence category、source contract、quality contract、narrative boundary を定義する。

### Phase 3-A: Governance Concentration Evidence Pipeline

完了済み。最初の低ノイズ構造 sensor として GovernanceConcentration evidence を repository-local structured JSON から取り込み、contract validation 後に JSONL store へ保存する。

この段階では外部 site の自由解析や AI risk judgment は行わない。目的は machine-readable governance facts の ingestion、source traceability、rejection boundary、deduplication を確立することである。

### Phase 3-A: Governance Source Adapter

本 Work Item の対象。GovernanceConcentration source adapter は local governance document と SEC governance filing metadata path を扱い、raw source を repository-local cache に保存する。

adapter は deterministic extraction のみを許可する。`founder_voting_power`、`independent_board_ratio`、`dual_class_structure`、`super_voting_rights`、`succession_disclosure` のいずれも抽出できない source は rejection とし、evidence store へ保存しない。

### Phase 3-B: Dependency Concentration Evidence Pipeline

本 Work Item の対象外。dependency graph は動的であるため、GovernanceConcentration sensor が安定してから追加する。

### Phase 4: Escalation Detection Engine

本 Work Item の対象外。evidence が十分に蓄積されるまで、自動 escalation 判定を追加しない。

### Phase 5: Long-term Civilization Risk Mapping

本 Work Item の対象外。事実 layer、hypothesis layer、Gray Rhino layer の分離を維持した後に扱う。

## Non-Goals

- UI 表示項目の増加。
- AI による自由記述のリスク判定。
- 既存 business positive evidence から Gray Rhino risk を推論すること。
- Gray Rhino evidence を Reality Layer、Gate、Execution Layer へ接続すること。
- 外部 source がない状態で intelligence と表示すること。

## 実装境界

- domain model は `src/features/research/domain/gray_rhino_evidence.rs` に置く。
- Gray Rhino escalation report は、外部 evidence 未接続の場合に `ManualConfiguration` と表示し続ける。
- checker は `.ai/architecture/gray_rhino_evidence_schema.yaml`、本ドキュメント、domain enum の整合を検証する。
- collector を追加する場合は、source contract を満たさない record を拒否する。

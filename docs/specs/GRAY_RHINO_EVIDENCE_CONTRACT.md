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

許可する source type は `RegulatoryFiling`、`GovernanceDocument`、`CompanyDisclosure`、`OperatorCuratedSource` に限定する。保存先は `gray_rhino_evidence.jsonl` とし、raw source は `gray_rhino_sources/governance` に cache する。source manifest は `gray_rhino_governance_source_manifest.jsonl`、extraction audit は `gray_rhino_governance_extraction_audit.jsonl` に保存する。保存された evidence と sensor health は escalation state、Gate、execution、trading state を変更しない。

### Dependency Concentration Evidence

business、infrastructure、compute、cloud、launch、supplier、ecosystem などの依存集中を示す証拠。

例:

- infrastructure dependency。
- compute dependency。
- cloud dependency。
- launch dependency。
- single supplier dependency。

Phase 3-B では、この category を GovernanceConcentration の次の構造 sensor として実装する。入力は repository-local structured JSON から開始し、`ingest-gray-rhino-dependency --file <json>` で取り込む。source collection は `collect-gray-rhino-dependency --file <source>` または `collect-gray-rhino-dependency --url <url>` に限定し、live dependency graph builder はまだ追加しない。

次の構造項目のうち少なくとも 1 つを source から抽出する。

- `concentration_ratio`。
- `single_point_of_failure`。
- `fallback_disclosed`。

`dependency_kind` と `dependency_name` は必須である。許可する dependency kind は `Infrastructure`、`Compute`、`Cloud`、`Launch`、`Supplier`、`Ecosystem` に限定する。許可する source type は `CompanyDisclosure`、`InfrastructureStatus`、`SupplierDisclosure`、`IndependentAudit`、`OperatorCuratedSource` に限定する。保存先は `gray_rhino_evidence.jsonl` とし、保存された evidence は escalation state、Gate、execution、trading state を変更しない。

source manifest は `gray_rhino_dependency_source_manifest.jsonl`、extraction audit は `gray_rhino_dependency_extraction_audit.jsonl` に保存する。fixture replay pack は `tests/fixtures/dependency_local` に置き、field coverage と rejection taxonomy を検証する。

### Institutional Maturity Evidence

組織規模や社会的重要性に対して、制度成熟が追いついているかを示す証拠。

例:

- succession structure。
- external audit。
- disclosure quality。
- oversight evolution。
- compliance maturity。

Phase 3-C では、repository-local structured JSON から開始し、`ingest-gray-rhino-institutional --file <json>` で取り込む。次の構造項目のうち少なくとも 1 つを source から抽出する。

- `succession_structure_disclosed`。
- `external_audit_present`。
- `disclosure_quality_score`。
- `oversight_evolution_disclosed`。
- `compliance_maturity_level`。

許可する source type は `RegulatoryFiling`、`GovernanceDocument`、`CompanyDisclosure`、`IndependentAudit`、`OperatorCuratedSource` に限定する。保存された evidence は escalation state、Gate、execution、trading state を変更しない。

Phase v1.1 では `collect-gray-rhino-institutional --file <source>` を追加し、local source replay、field coverage、source manifest、extraction audit を生成する。dry-run では formal evidence を保存しない。

Phase v1.2 では InstitutionalMaturity extractor が `succession planning`、`independent auditor`、`comprehensive disclosure`、`board oversight expanded`、`developing compliance` などの deterministic disclosure labels を扱う。

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

Phase 3-D では、repository-local structured JSON から開始し、`ingest-gray-rhino-redundancy --file <json>` で取り込む。次の構造項目のうち少なくとも 1 つを source から抽出する。

- `fallback_available`。
- `alternative_supplier_count`。
- `redundancy_ratio`。
- `recovery_path_disclosed`。
- `failover_tested`。

許可する source type は `InfrastructureStatus`、`SupplierDisclosure`、`IndependentAudit`、`CompanyDisclosure`、`OperatorCuratedSource` に限定する。保存された evidence は escalation state、Gate、execution、trading state を変更しない。

Phase v1.1 では `collect-gray-rhino-redundancy --file <source>` を追加し、local source replay、field coverage、source manifest、extraction audit を生成する。dry-run では formal evidence を保存しない。

Phase v1.2 では Redundancy extractor が fallback claimed と failover tested を区別する。`backup provider` や `alternative supplier` は `fallback_available` を支えるが、`failover test`、`tested failover`、`drill completed` がない限り `failover_tested` には投影しない。

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

完了済み。GovernanceConcentration source adapter は local governance document と SEC governance filing metadata path を扱い、raw source を repository-local cache に保存する。

adapter は deterministic extraction のみを許可する。`founder_voting_power`、`independent_board_ratio`、`dual_class_structure`、`super_voting_rights`、`succession_disclosure` のいずれも抽出できない source は rejection とし、evidence store へ保存しない。

### Phase 3-A: Governance Backfill And Extraction Audit

本 Work Item の対象。Governance sensor は source manifest、extraction audit ledger、coverage ratio、latest observed date を生成する。

日次 report へ表示できるのは sensor health のみである。sensor health は source count、accepted / rejected count、coverage ratio、latest observed date を示すが、Gray Rhino escalation state を変更しない。

### Phase 3-A: Governance SEC Replay Pack

本 Work Item の対象。`tests/fixtures/governance_sec` に SEC governance filing 風の replay fixture pack を置き、deterministic extractor の field coverage と rejection taxonomy を検証する。

rejection taxonomy は `MetriclessSource`、`SourceInvalid`、`ExtractionInvalid` に限定する。fixture replay は実ネットワークに接続せず、実 source 接続の前に sensor quality を測るためだけに使う。

### Phase 3-A: Governance SEC Live Dry-Run

本 Work Item の対象。SEC live path は default dry-run とし、raw source cache、source manifest、extraction audit、coverage summary だけを生成する。

正式 evidence store への保存は default では行わない。dry-run の目的は SEC User-Agent、rate limit、network failure、filing selection、field extraction coverage を検証することであり、Gray Rhino escalation state、Gate、execution、trading state を変更しない。

### Phase 3-A: Governance SEC Field Coverage Calibration

本 Work Item の対象。live dry-run audit で欠落した field coverage を、実 SEC filing の文面に対する deterministic label calibration で改善する。

最初の対象は `succession_disclosure` である。`succession framework` と `ceo succession framework` は succession disclosure の evidence として扱う。ただし `not disclosed`、`does not`、`do not`、`without`、`no` などの否定形は positive disclosure として扱わない。

### Phase 3-A: Governance SEC Expanded Sample Dry-Run

本 Work Item の対象。SEC live dry-run を 5〜10 issuer に拡大し、accepted/source ratio だけでなく field-level coverage を表示する。

field-level coverage は current run の extraction audit から算出する。JSONL ledger の dedup 結果に依存して coverage が変わってはならない。正式 evidence store、Gray Rhino escalation state、Gate、execution、trading state は変更しない。

### Phase 3-A: Governance SEC Voting Structure Calibration

本 Work Item の対象。実 SEC filing で確認した voting structure phrase を、`dual_class_structure` と `super_voting_rights` の deterministic label として追加する。

`multi-class voting structure`、`multi-class common stock`、`Class B stock has 10 times the voting rights`、`Class B common stock have ten votes per share`、`Class B common stock represents 15 votes` は voting structure evidence として扱う。`one vote for each share` は dual-class / super-voting evidence として扱わない。

### Phase 3-A: Governance SEC Board Independence Calibration

本 Work Item の対象。`independent_board_ratio` は SEC filing の明示的な count pattern からのみ算出する。

`Of the N Board nominees, M are independent`、`M out of N director nominees are independent`、`consists of N directors, M of whom are independent` を許可する。`majority independent` や `except CEO` のように分母が明示されない disclosure は ratio として扱わない。

### Phase 3-A: Governance SEC Founder Voting Power Calibration

本 Work Item の対象。`founder_voting_power` は SEC filing の明示的な voting power / voting control percentage からのみ抽出する。

`controlled X% of the voting power`、`representing X% of the voting power`、`entitled to X% of the voting power`、`hold X% of the voting power` を許可する。beneficial ownership や ownership percentage だけの disclosure は founder voting control として扱わない。

### Phase 3-B: Dependency Concentration Evidence Pipeline

本 Work Item の対象。DependencyConcentration evidence は source traceability、dependency kind、dependency name、category-specific metric validation を必須とする。

Phase 3-B の初期実装では repository-local structured JSON ingestion boundary と domain validation を定義する。`ingest-gray-rhino-dependency --file <json>` は valid evidence を `gray_rhino_evidence.jsonl` に保存する。`collect-gray-rhino-dependency --file <source>` と `collect-gray-rhino-dependency --url <url>` は deterministic extraction、manifest、audit、coverage、rejection taxonomy を生成するが、dependency graph builder、trading、Gate、execution は追加しない。

Phase v1.1 では dependency disclosure labels として `supplier concentration`、`revenue concentration`、`customer concentration`、`workloads hosted by`、`single cloud provider`、`sole supplier`、`alternative supplier`、`backup provider`、`redundant provider` を許可する。metricless source は `MetriclessSource` として extraction audit と CLI output に残す。

Phase v1.2 では Dependency URL adapter を real adapter 境界として扱う。URL source は retry 3 回、timeout 20 秒、raw source cache `gray_rhino_sources/dependency`、content hash、publisher 正規化を持つ。これは supplier/cloud 専用 API adapter ではなく、disclosure URL を監査可能な source として取り込む adapter である。

### Phase 4: Escalation Detection Engine

本 Work Item の対象。validated evidence store から category coverage を読み取り、Gray Rhino escalation input へ投影する。

evidence-driven escalation は `gray_rhino_evidence.jsonl` の検証済み record だけを入力とし、source は `EvidenceStore` と表示する。出力は Gray Rhino report に限定し、trade、Gate、execution、trend cohesion を生成しない。

multi-category sensor health は GovernanceConcentration、DependencyConcentration、InstitutionalMaturity、Redundancy の evidence count と governance source audit health を表示する。

Phase v1.1 では `collect-gray-rhino-backfill --file <manifest>` を追加し、multi-category source manifest から dry-run を実行する。manifest fixture は `tests/fixtures/gray_rhino_backfill/multi_category_manifest.json` に置く。

Phase v1.2 では backfill run summary を `gray_rhino_backfill_runs.jsonl` に保存する。summary は run id、manifest、category、source count、accepted、rejected、coverage、started_at、finished_at、boundary を含む。

sensor health は readiness score を表示する。readiness score は category completeness に基づく evidence quality の説明指標であり、trade、Gate、execution、trend cohesion へ接続しない。

evidence-driven escalation は category completeness を calibration input として使う。insufficient category は report に表示するが、narrative な推測で補完しない。

Phase v1.2 の evidence quality model は `v2` とし、traceability、metric completeness、freshness、confidence、source diversity、rejection ratio を表示する。report は `Evidence Explanation Graph` として dimension -> supporting evidence category -> source class を示す。

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

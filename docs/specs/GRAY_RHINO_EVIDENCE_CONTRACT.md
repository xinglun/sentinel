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

現在の Gray Rhino Escalation は `AutoDiscovery` を source とする構造リスク候補 discovery system へ移行する。

system は filing、prospectus、S-1、annual report、proxy statement、market breadth / liquidity disclosure などの source text を走査し、Company Gray Rhino と Market Gray Rhino の candidate を自動発見する。人工 registry は主機構ではなく、manual source 補助としても既定では使わない。

## Phase 4: Auto Discovery And Inline Reference

Phase 4 では `GrayRhinoCandidate` を導入する。candidate は `Company` または `Market` scope、candidate kind、subject、state、evidence、watch triggers、source title、observed date を持つ。

state は `Background`、`Visible`、`Expanding`、`Critical`、`Cooling`、`Resolved` のいずれかとし、system は source text から deterministically 発見する。例として、prospectus に founder voting control、dual-class / super-voting terms、独立 board constraint の欠如が同時に現れた場合、Company / GovernanceConcentration の Gray Rhino candidate とする。

Daily report では `Gray Rhino Inline Reference` として watchlist 近傍に表示できるが、意味的には完全に隔離する。candidate は trading、Gate、execution、trend、market state を変更してはならない。出力は構造リスク状態、evidence、trigger watch の提示に限定する。

Phase 4-B では `collect-gray-rhino-sources` を source collection entrypoint とする。provider は `sec`、`finnhub`、`fred` を許可し、各 provider は source text を `gray_rhino_sources/**` に cache する。

- SEC は watchlist symbol から filing / disclosure を取得し、`gray_rhino_sources/governance` に保存する。
- Finnhub は company narrative source を article 単位で取得し、`gray_rhino_sources/narrative` に保存する。identity は stable URL または URL 不在時の content hash とし、`source_published_at` は取得日ではなく article の公開日を使う。`retrieved_at` と `last_confirmed_at` は分離し、同じ article content の再取得だけでは新しい confirmed date を作らない。
- FRED は macro series を構造観測 text に投影し、`gray_rhino_sources/macro` に保存する。identity は最新 observation の series、日付、値の組に基づき、`source_published_at` は request date ではなく実際の observation date とする。同じ observation set の再取得は retrieved metadata の更新に留まり、新しい confirmation day を作ってはならない。

source collection の audit は `gray_rhino_discovery_runs.jsonl` に保存する。audit は provider、dry-run、source count、candidate count、content hash、failure taxonomy を記録するが、trading、Gate、execution、trend、market state を変更してはならない。

Phase 4-C では auto-discovered candidate を `gray_rhino_candidates.jsonl` に append-only で保存する。store は `GrayRhinoCandidate` の scope、kind、subject、state、evidence、watch triggers、source title、source published date、last confirmed date、resolved date を保持する。Daily report は persisted candidate history だけを読み、同一 key の本文表示は latest confirmed candidate を選択する。historical replay では candidate、evidence、governance audit、backfill / discovery ops view、refresh status のすべてを `as_of_date` 以下の record だけから選ぶ。refresh status は `gray_rhino_refresh_status.jsonl` を append-only ledger とし、`gray_rhino_refresh_status_latest.json` と `gray_rhino_refresh_status_YYYY-MM-DD.json` は便利な pointer と daily sidecar に留める。date のない legacy latest status は historical replay では採用しない。

`gray_rhino_candidates.jsonl` は構造リスクの観測履歴であり、trade、Gate、execution、trend、market state を変更してはならない。Company candidate は current enabled watchlist に属する場合だけ daily inline reference に表示し、Market candidate は市場側 reference として表示できる。

Phase 4-D では monitoring state machine を導入する。state machine は `gray_rhino_candidates.jsonl` の persisted candidate history から subject / scope / kind ごとの履歴を作り、`Visible`、`Expanding`、`Critical`、`Cooling`、`Resolved` と direction（new、stable、intensifying、cooling、resolved）を deterministic に評価する。GovernanceConcentration、DependencyConcentration、InstitutionalMaturityGap、RedundancyGap などの持続的構造リスクは、source published date が古いだけでは `Resolved` にしない。解除には明示的な `Resolved` candidate、resolved date、または反対 evidence が必要である。formal assessment は `(subject, category)` ごとの最新 effective evidence state を使い、より新しい mitigating evidence は同じ subject と category の古い amplifying evidence だけを閉じる。異なる company の mitigating evidence は別 company の amplifying evidence を閉じてはならない。DependencyConcentration と Redundancy のような互補 category は同じ subject の中だけで相互作用し、別 company の Redundancy evidence は対象 company の fallback survivability risk を下げてはならない。subject のない legacy evidence record は正式 scoring から除外し、unknown subject として集約してはならない。subject が欠落する evidence は `MissingSubject` として拒否し、`MissingStructuralFact` と混同してはならない。Daily report use case は、同じ subject + risk kind に以前の active candidate またはより古い amplifying formal evidence が存在する場合だけ、最新 effective mitigating evidence を `Resolved` candidate として投影する。前置 risk のない mitigating evidence は lifecycle 解除イベントではなく、mitigating evidence としてのみ表示する。解除投影と latest effective evidence selection は Domain policy が所有し、Daily report Application は候補履歴と evidence を渡して投影結果を受け取るだけにする。

Compact summary は monitoring status を唯一の状態ソースとして使う。`Visible`、`Expanding`、`Critical` だけを active とし、`Cooling` と `Resolved` は active candidate から除外して別枠で表示する。sensor health は ingestion coverage と scoreable readiness を混同しない。正式 scoring に使える scoreable evidence（subject があり、directional `risk_effect` を持つ record）で readiness、平均 confidence、source diversity、category coverage を計算し、subject 欠落や非 directional record は不可评分 record 数と理由として別表示する。

monitoring state は `Gray Rhino Monitoring State` として Daily report に表示する。これは臨界点の接近を観察する reference であり、trade、Gate、execution、trend、market state を変更してはならない。

Phase 4-E では FRED threshold calibration を導入する。FRED source adapter は `DGS10`、`T10Y2Y`、`FEDFUNDS`、`BAMLH0A0HYM2`、`WALCL`、`RRPONTSYD` を deterministic threshold assessment に変換し、rate pressure、yield curve constraint、credit stress、liquidity fragility、capex payback risk を `Visible`、`Expanding`、`Critical` の candidate state に投影する。

日次更新の operational entrypoint は `make gray-rhino-refresh` とする。この target は SEC / Finnhub / FRED source collection を provider 単位で実行し、date 付き status を `gray_rhino_refresh_status.jsonl`、`gray_rhino_refresh_status_latest.json`、`gray_rhino_refresh_status_YYYY-MM-DD.json` に更新する。GitHub Actions では主 Radar report を生成する前に refresh を実行し、Telegram と archive が同じ日次 Gray Rhino 状態を参照する。pre-radar refresh は当日の audit record に依存してはならないため、`daily-calibration` は呼び出さない。refresh loop は source と candidate store を更新するだけであり、trade、Gate、execution、trend、market state を変更してはならない。

Phase 4-F では watchlist inline display を導入する。Daily report は Market candidate を `Market Reference` にまとめ、Company candidate と monitoring state は enabled watchlist symbol ごとの `Watchlist Inline Reference` / `Watchlist Inline Monitoring` に表示する。legacy local source など current watchlist に属さない Company candidate は `Other Company Reference` として分離する。この配置は読みやすさのための表示構造であり、candidate は引き続き trading、Gate、execution、trend、market state から意味的に隔離する。

Daily report query path は、永続化済み `gray_rhino_candidates.jsonl` の candidate history だけを読む。`gray_rhino_sources/**` や `gray_rhino_raw_sources/**` の source cache を report rendering 中に再 scan / rediscover してはならない。source discovery は refresh / ingestion use case の責務であり、candidate の `observed_at` は実際の観測日を保持する。古い cache file が残っているだけで、candidate を当日観測として再生成してはならない。

Phase 4-G では noise calibration と compact summary を導入する。Finnhub source adapter は normalization boilerplate に `narrative overcrowding` などの trigger term を含めてはならない。Daily report は `Gray Rhino Summary` を details より前に表示し、Market active candidate 数、Company active subject、intensifying watch subject を短く示す。

Daily GitHub Actions refresh は `.github/workflows/daily_radar.yml` の Daily Radar workflow で実行する。Radar 生成前に Gray Rhino source refresh を non-blocking に実行し、`reports/gray_rhino_refresh_status_latest.json` に `succeeded` / `partial_failure` / `skipped` / `failed` を記録する。secret 未設定や provider failure があっても daily radar の Gate、trend、execution、market state を変更してはならない。

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
- `risk_effect`（`Amplifying` / `Mitigating` / `Neutral` / `Unclassified`）。
- extraction note。

source が不明なものは evidence として扱わない。

## Quality Contract

collector または adapter は次を満たす必要がある。

- source と extraction note を分離する。
- quote / fact と operator interpretation を分離する。
- category を schema enum から選択する。
- confidence は evidence quality を表し、risk severity を表さない。
- `risk_effect` は formal assessment への方向を表す。founder voting power、dual class、single point dependency などは `Amplifying`、high board independence、fallback availability、external audit、redundancy などは `Mitigating`、方向判断に不足する事実は `Neutral` とする。旧 JSONL などで `risk_effect` が欠落する record は `Unclassified` として読み込み、正式な escalation scoring から除外し、日報に不可评分 record 数として表示する。formal scoring の confidence aggregation は `Amplifying` / `Mitigating` の scoreable records のみを対象とし、`Neutral` / `Unclassified` は平均置信度を変えてはならない。
- evidence は escalation state を直接指定しない。
- evidence は trade signal、Gate、execution を生成しない。

## Phase Boundary

自動情報収集は Phase 4 の deterministic discovery scanner で開始する。scanner は source text から candidate を作るが、trade / Gate / execution / trend cohesion を変える signal は作らない。

FRED の設定は `[fred] fred_api_key` または `FRED_API_KEY` 環境変数で与える。API key は source 取得だけに使用し、Gray Rhino candidate は独立した reference として表示する。

Radar Telegram には Gray Rhino reference appendix を追加する。この appendix は refresh 後の candidate / monitoring / formal evidence view model から生成し、daily-calibration と同じ semantic isolation を保ち、Gate、execution、trade、trend、market state を変更しない。

`make gray-rhino-refresh` は provider-level outcome を記録する。SEC、Finnhub、FRED は credential / availability に応じて独立に実行し、いずれかが失敗しても後続 provider を継続する。全 provider 成功は `succeeded`、成功と失敗または skipped が混在する場合は `partial_failure`、実行 provider がすべて失敗した場合は `failed`、実行 provider が存在しない場合は `skipped` を `gray_rhino_refresh_status_latest.json` に保存する。`run_status` では `GrayRhinoCollectionStatus` が status、provider ごとの outcome、accepted / rejected coverage、collection date、failed providers を構造化して保持する。audit record を必要とする calibration report は `make gray-rhino-refresh-report` または通常の `daily-calibration` 側で実行する。

provider collection は、`accepted == 0` かつ `rejected > 0` の場合に provider failure として非ゼロ終了を返す。Daily Radar workflow は non-blocking に継続してよいが、`gray_rhino_refresh_status_latest.json` には failed / partial_failure として正確に記録する。Daily report と Telegram appendix は refresh status を audit context として表示し、新鮮な自動情報か、部分失敗か、完全失敗か、skipped かを読者が判別できるようにする。historical replay では `date <= as_of_date` の status だけを表示する。この status は trade、Gate、execution、trend、market state を変更しない。

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

Phase v1.3 の provider source registry は manual source manifest の実験機構であり、Phase 4 以降の主機構ではない。Gray Rhino discovery は `gray_rhino_sources/**` や raw source cache にある system-owned source text を走査し、registry JSON を要求しない。

Backfill run summary は provider failure taxonomy として `fetch_failure`、`timeout`、`unsupported_format`、`metricless_source`、`stale_source` を記録する。hash drift は `drift_sources`、freshness window 超過は `stale_sources` に集計する。

Report ops view は report date 時点の最新 backfill run、failed source、stale source、drift source を表示する。これは運用監査表示であり、trade、Gate、execution、trend cohesion へ接続しない。

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

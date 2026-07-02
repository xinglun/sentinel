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

本ドキュメントを Gray Rhino Escalation Framework（手動 fallback baseline、formal escalation snapshot、状態判定、出力境界）の human-readable SSOT とする。

Gray Rhino 専用 evidence の分類、source traceability、narrative rejection、collector 境界は `docs/specs/GRAY_RHINO_EVIDENCE_CONTRACT.md` と `.ai/architecture/gray_rhino_evidence_schema.yaml` を SSOT とする。本 framework は evidence contract を満たさない入力を自動 intelligence と表示しない。

SSOT の分担は次の通りである。

| 領域 | SSOT |
| --- | --- |
| Escalation snapshot / manual fallback / `RhinoEscalationState` | 本ドキュメント |
| Evidence 分類 / source type / rejection taxonomy | `docs/specs/GRAY_RHINO_EVIDENCE_CONTRACT.md` と `.ai/architecture/gray_rhino_evidence_schema.yaml` |
| Category source type policy | `src/features/research/domain/gray_rhino_evidence_source_policy.rs` |

実装は次の境界に従う。

- domain model と状態判定は `src/features/research/domain/gray_rhino.rs` に置く。
- CLI / Markdown / Telegram 表示は `src/features/research/interface/gray_rhino_report.rs` に置き、Application use case が返す view model の描画に限定する。
- 日次評価の生成は `src/features/research/application/gray_rhino_assessment.rs` に置き、formal evidence の方向性を保持して評価する。
- Gray Rhino 日報の orchestration は `src/features/research/application/gray_rhino_daily_report.rs` に置き、`GrayRhinoDailyReportRepository` port 経由で evidence、candidate、snapshot、ops view を読む。
- file scan と JSONL store access は `src/features/research/infrastructure/gray_rhino_daily_report_repository.rs` に閉じ込める。
- persisted evidence の read validation と accepted / rejected read batch は `src/features/research/infrastructure/gray_rhino_evidence_store.rs` が担当し、Application の read model へ渡す。
- category/source type の許可表は `src/features/research/domain/gray_rhino_evidence_source_policy.rs` が所有する。
- 日次 snapshot の JSONL 永続化は `src/features/research/infrastructure/gray_rhino_snapshot_store.rs` に置く。
- 自動発見候補の永続化は `gray_rhino_candidates.jsonl`、source / ops audit は `gray_rhino_sources` と `gray_rhino_discovery_runs.jsonl` に保持する。
- CLI command は use case / facade を呼び出す dispatch に限定し、取引・Gate・execution へ接続しない。
- `make daily-calibration` と Radar Telegram appendix は独立した Gray Rhino reference section を出力し、他の校正セクションや判断結果を変更しない。
- 日次 source refresh は `make gray-rhino-refresh` を入口とし、contract 検証は `make check-gray-rhino-evidence-contract` を使う。

## 現在の入力モデル

Gray Rhino は次の 3 系統を明示的に区別する。

- `formal escalation evidence`: evidence store に保存された構造化 evidence。`risk_effect` により `Amplifying` / `Mitigating` / `Neutral` / `Unclassified` を区別し、保護的事実をリスク拡大として扱わない。`risk_effect` が欠落する旧 record は scoring から除外し、日報に不可评分として表示する。
- `auto-discovered observation candidates`: SEC / Finnhub / FRED などから自動収集された source text と threshold assessment から生成される観測候補。日報では追跡参考として表示し、formal escalation score を直接上書きしない。
- `manual fallback baseline`: `config.toml` の `[gray_rhino_escalation]` による運用者管理の初期値。formal evidence がない場合の fallback であり、自動収集 fact として表示しない。

## Manual Fallback Baseline

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

この設定は `manual fallback baseline` であり、自動 fact ではない。formal evidence が存在する場合は `EvidenceStore` 由来の評価を優先し、manual baseline は evidence 未接続時の fallback としてだけ使う。

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

## 監査 snapshot と自動観測

`gray_rhino_snapshots.jsonl` は Gray Rhino escalation を再生するための構造化 snapshot である。`make daily-calibration` の全文 Markdown を毎日保存するための file ではない。長期校正の標準粒度は `weekly_state_metrics.json` と `weekly_state_review_auto.md` の週次記録とする。

formal escalation snapshot は次を含む。`manual fallback baseline` は `source: ManualConfiguration`、validated evidence store 由来の評価は `source: EvidenceStore` として記録する。

- `as_of_date`: 市場監査ログと一致する業務日。監査ログがない場合は実行日。
- `source`: `ManualConfiguration` または `EvidenceStore`。
- `escalation`: 状態と入力観測項目。

日報には評価日、入力由来、前回評価との差分を表示する。同一日の同一 snapshot の再実行は重複追記しない。これにより、灰色のサイ観測値がいつ変更されたかを監査可能にする。

日報は `手動構造ベースライン -> 7 観測項目 -> 構造化 snapshot` という監査チェーンと、明示ルール判定で再生可能であることを表示する。このチェーンは外部 fact evidence chain ではなく、現在の手動入力評価がどの経路で状態へ変換されたかを示す lineage である。

自動発見候補は formal escalation snapshot と分離して `gray_rhino_candidates.jsonl` に保存する。monitoring state machine は candidate state（`Background`、`Visible`、`Expanding`、`Critical`、`Cooling`、`Resolved`）と direction（`new`、`stable`、`intensifying`、`cooling`、`resolved`）を別軸で評価する。候補は watchlist inline reference、market reference、other company reference として表示されるが、Gate、execution、trend、market state を変更しない。

次段階では、candidate state とは別に escalation velocity を表示する。これは risk の存在確認ではなく、長期存在する risk への市場・組織の麻痺を観測するための温度計である。`attention_decay`、`evidence_acceleration`、`institutional_response` は Daily report の reference metadata として扱い、手動 escalation state や取引判断を直接変更しない。

Survivability Assessment は Gray Rhino の反対側にある安全余裕を観測する。`capital_access`、`compute_control`、`governance_resilience`、`dependency_risk`、`retry_capacity` は、企業が誤った後に再試行できるかを示す reference であり、楽観 narrative を生成するための採点ではない。

## 状態

`RhinoEscalationState` は manual / formal escalation snapshot の状態であり、candidate monitoring state とは別の型として扱う。candidate monitoring の語彙は Evidence Contract を SSOT とする。`RhinoEscalationState` は次の状態を持つ。

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

## Interpretation Layer (本日の説明) との境界

Gray Rhino Layer は長期的な構造リスクを監視するための独立した Layer であり、本日の日次意思決定や日報の主たる説明（Today's Explanation）における優先度判定とは以下の境界を維持する。

- **Primary Driver 昇格の禁止**: 灰色のサイの監視ステータスは、本日の市場の値動きを説明する「主要ドライバー (Primary Driver)」には絶対に指定してはならない。
- **デフォルトでの本日無視 (Ignored Today)**: 灰色のサイはデフォルトで `Ignored Today`（本日無視）に分類される。これは、サイが長期的な構造リスクであり、日々のノイズや短期的な値動きの主要因ではないことを明示するためである。
- **補助ドライバー (Secondary Driver) への昇格要件**: リスク評価が明示的にアップグレードされた（`gray_rhino_escalated` が真）場合に限り、補助的な説明要因（Secondary Driver）として出現することを許容する。これにより、構造リスクの顕在化と現在の市場環境との間の補助的な関連性を提示する。

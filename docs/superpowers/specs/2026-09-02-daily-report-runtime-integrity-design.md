---
author: Ray
title: 日次レポート Runtime Integrity 設計
description: 日次レポートの code revision、data snapshot、観測 provenance、Leadership facts、生成・再送 lifecycle を後方互換に結び付ける設計。
key: daily-report-runtime-integrity-design
---

# 日次レポート Runtime Integrity 設計

## 1. 目的と設計境界

本 Work Item は、Sentinel の日次レポートが一つの code revision、一つの data snapshot、
一つの Decision snapshot、完全かつ追跡可能な観測入力から生成されたことを監査できるようにする。
入力が欠落または矛盾する場合は、通常の `NEUTRAL`、`0/0`、空の正常値へ変換せず、
`UNAVAILABLE` と診断を表示する。

対象は Runtime Identity、Data Provenance、RS/Leadership Observation Integrity、正式 snapshot、
Markdown/Telegram 表示、GitHub Actions の生成・再送 lifecycle である。これらは表示・監査専用であり、
Decision の意味を変更しない。

次の不変条件を実装中も維持する。

- `NO_TRADE`、`PROBE`、`READY`、Gate、Action Matrix、Leader threshold、Leadership algorithm、
  RS threshold、RecoveryStrength threshold、RS Diffusion threshold、Breadth、Breakout、Supply、
  Position Sizing、Execution、Trader、Validation Engine は変更しない。
- `decision_weight` は Runtime Integrity が `HEALTHY`、`DEGRADED`、`UNAVAILABLE` のいずれでも
  常に `0` であり、完全性チェックが取引判断へフィードバックしない。
- 既存 JSON と既存 report artifact は読める。新フィールドは `serde(default)` または optional とし、
  過去 artifact の移行・書き換えは行わない。

## 2. 正規データモデル

### 2.1 Runtime Identity

`src/features/shared/application/run_status.rs` に report 共通の identity model を追加する。
生成 run は次の値を一つの immutable な `ReportRuntimeIdentity` として保持し、
`PresentationPacket`、`RunOutcome`、正式 snapshot、Archive Markdown に同じ値を渡す。

```text
report_run_id
report_run_at
git_commit_sha
git_branch
binary_version
decision_snapshot_version
data_snapshot_id
data_snapshot_date
build_git_commit_sha
execution_git_commit_sha
```

`build_git_commit_sha` は `build.rs` が compile-time に埋め込む値、
`execution_git_commit_sha` は実行時に workflow が渡す checkout SHA とする。
`git_commit_sha` は実行対象 revision として保存する。両方が既知で一致しない場合は
`RUNTIME_MISMATCH` とし、branch 名を SHA の代用にしない。SHA が取得できない場合も
`code_revision_known = false` として `UNAVAILABLE` または `DEGRADED` に倒す。

`build.rs` は既存の protobuf compile を維持し、git の値を取得できない場合に build を成功させるため、
未取得値は `UNKNOWN` として埋め込む。GitHub Actions は Checkout 後に確認済みの `GITHUB_SHA`、
`GITHUB_REF_NAME` を radar 実行環境へ渡す。実行側で再び repository の branch 名から revision を推測しない。

### 2.2 Data Provenance

同じ `run_status` module に `DataProvenance` と `DataProvenanceBundle` を追加する。
次の論理入力ごとに、固定 key の record を必ず生成する。

```text
price_history
benchmark_history
relative_strength_input
leadership_history
market_change_baseline
corporate_event_evidence
expectation_data
price_volume_history
```

各 record は次を持つ。

```text
status, source, as_of, snapshot_id, snapshot_digest, record_count, diagnostic
```

`status` は `AVAILABLE`、`PARTIAL`、`UNAVAILABLE`、`FAILED` のいずれかの表示値とする。
`source` は provider または local persisted source の識別子であり、secret と credential を含めない。
`as_of` は入力の観測時点、`snapshot_id` と `snapshot_digest` は取得した入力集合の identity、
`record_count` は実際に採用した record 数、`diagnostic` は secret redaction 済みの理由である。
legacy artifact に provenance がない場合は読み込み時に `UNAVAILABLE` の synthetic record を作らず、
旧形式を維持したまま現 run の provenance だけを `PARTIAL` として記録する。

### 2.3 RS Observation Health

`RelativeStrengthState` に `Unavailable` を追加する。既存の完全な入力では従来どおり
`IMPROVING`、`NEUTRAL`、`WEAKENING` を計算する。1d/5d の比較値が算出できない、asset または
benchmark history がない、比較可能な session がない場合は、該当 ticker の state を `UNAVAILABLE` とし、
`diagnostic` に `benchmark_history_missing`、`price_history_missing`、`comparable_session_missing` などの
機械可読な理由を記録する。

完全な入力で相対値が実際に `0` の場合は `NEUTRAL` のままにする。したがって `0/0` は完全入力の
真の観測値としてのみ許容し、入力欠落の結果として全 ticker を `NEUTRAL` にすることは禁止する。

Engine は watchlist の各 ticker に対して observation を作る。benchmark または asset が見つからない
場合にも observation を落とさず、`UNAVAILABLE` と provenance を保持する。既存の recovery strength、
denominator、完全入力の数値計算は変更しない。

Archive Markdown には ticker ごとの health と diagnostic を出力する。Telegram は ticker 全件の
詳細診断を省略できるが、完全性が `DEGRADED` または `UNAVAILABLE` の場合は先頭に短い notice を出す。

### 2.4 Leadership facts と provenance

Leadership Snapshot を daily report の単一事実源とする。`LeadershipSnapshotViewModel` に
`leadership_snapshot_id`、`previous_snapshot_id`、`last_confirmed_leader`、`absence_since`、
`absence_duration`、`history_coverage`、`calculation_mode` を追加し、同じ read model の値を
Leader Persistence と Market Interpretation に投影する。

`calculation_mode` は次の値を使う。

- `PERSISTED_FACT`: 正式 snapshot と完全な履歴に bind された事実。
- `RECOMPUTED_FROM_PARTIAL_HISTORY`: 部分履歴から再計算した値。`absence_since` は estimated/reconstructed
  と明示する。
- `UNAVAILABLE`: 比較可能な Leadership history がない。

既存の `build_leader_persistence` が計算する streak、since、duration、last confirmed leader は変更せず、
runner が formal baseline と history coverage を用いて provenance を付与する。Summary、
Leader Persistence、Market Interpretation は個別に日付や duration を再計算せず、同じ Leadership facts
projection を参照する。

## 3. Runtime Integrity の判定

`RuntimeIntegrity` は read-only な監査 projection とする。判定入力は次の六つである。

```text
code_revision_known
data_snapshot_known
decision_snapshot_known
rs_input_consistent
leadership_snapshot_consistent
report_artifact_matches_run
```

結果は `HEALTHY`、`DEGRADED`、`UNAVAILABLE` の三値、`decision_weight = 0`、
`diagnostics`、および各 boolean を持つ。

- 全項目が known/consistent なら `HEALTHY`。
- 一部が欠落、部分履歴、legacy provenance、または明示的な conflict なら `DEGRADED`。
- code revision、data snapshot、decision snapshot の identity が確認できない場合は `UNAVAILABLE`。

判定は report body の生成後に artifact digest と identity を照合して確定する。判定は report の
presentation にだけ反映し、decision packet、Gate、Action Matrix、Execution の入力へ戻さない。
`DEGRADED` の場合は Markdown と Telegram の上部に「Runtime Integrity degraded」notice を出し、
Archive Markdown の appendix に全診断を残す。

## 4. 正式 snapshot と digest

`TradingDaySnapshot` に optional な後方互換フィールドを追加する。

```text
report_run_id
git_commit_sha
data_digest
decision_packet_digest
observation_digest
runtime_integrity
```

新しい正式 snapshot は `market_date`、`report_run_id`、code revision、三つの digest を必ず持つ。
`data_digest` は `DataProvenanceBundle`、`decision_packet_digest` は DecisionPacket、
`observation_digest` は RS/Leadership observation facts の canonical JSON digest とする。
volatile な `generated_at`、`run_id`、`snapshot_id` は従来と同じく semantic conflict の比較から除外する。

ただし三つの digest は semantic identity の一部とし、同じ cycle/date の既存 snapshot と異なる場合は
`SNAPSHOT_CONFLICT` を返して既存 bytes を保持する。runner は conflict を握りつぶさず、正式 snapshot を
上書きせずに `Runtime Integrity = DEGRADED` と report/run status に記録する。legacy snapshot に digest が
ない場合は読み取り可能な baseline として扱い、現 run の provenance を `PARTIAL` として表示する。

## 5. Report と lifecycle

### 5.1 生成

`RadarRunContext` の開始時に `report_run_id` と `report_run_at` を固定する。identity、provenance、
Leadership facts、Integrity を `PresentationPacket` に渡してから report を render する。

Markdown の先頭には機械可読な `report_runtime_identity` metadata block を置き、Archive Markdown には
続けて `data_provenance`、`runtime_integrity`、RS ticker diagnostics、Leadership provenance の appendix
を置く。通常の report body は既存 wording を保ち、最新コードで生成した run の wording がそのまま保存される。
Telegram は同じ identity の短縮 metadata と integrity notice のみを表示する。

`RunOutcome` には optional な `runtime_identity`、`data_provenance`、`runtime_integrity`、
`report_lifecycle` を追加する。旧 run_status JSON は既存 default で deserialize できる。

### 5.2 再送

`GENERATED` と `RESENT` を `report_lifecycle.mode` で区別し、次を記録する。

```text
mode, original_generation_revision, original_report_run_id, resend_revision, resent_at, source
```

`GENERATED` は現 run が report、snapshot、run status を作る。`RESENT` は data branch の正式 Markdown と
run status を読み取り、原文と original generation revision を保持して送信するだけとする。
再送で radar の decisioning、snapshot write、report re-render を呼び出さず、resend revision は
workflow 実行の identity としてのみ記録する。旧 report の Risk Summary wording を現在の wording に
置き換えない。

## 6. 実装配置と変更境界

実装は既存の shared run status、RS domain、Leadership read model、snapshot persistence、report renderer、
pipeline runner、GitHub Actions に限定する。新しい application abstraction は既存の
`run_status.rs` と既存 presentation contract に収め、不要な provider や依存を追加しない。

必要な変更の責務は次のとおりである。

| 層 | 責務 |
| --- | --- |
| `build.rs` / workflow | build revision と execution revision の bind |
| shared application | identity、provenance、integrity、lifecycle の型と判定 |
| RS domain / engine | 欠落 observation の `UNAVAILABLE` 化。完全入力の数値は不変 |
| Leadership read model | 単一 facts projection と provenance の共有 |
| persistence | snapshot digest、conflict、legacy read compatibility |
| report | metadata、fail-closed notice、Archive diagnostics |
| runner | 一つの run identity を全 artifact に伝播し、resend と生成を分離 |
| workflow | checkout SHA の検証、resend の read-only 境界と metadata |

決定アルゴリズムの条件式、threshold、action、execution path は変更しない。

## 7. テスト計画

`tests/daily_report_runtime_integrity.rs` と既存 module tests に、次の八シナリオを追加する。

1. 完全 RS 入力では実際の `Neutral` と `5/9` 結果が保持される。
2. benchmark history 欠落は ticker を `UNAVAILABLE` とし、diagnostic を保持する。
3. 全 RS 入力欠落は全 ticker `NEUTRAL` にならず、denominator `0/0` は unavailable と表示される。
4. persisted Leadership snapshot では since/duration/last confirmed leader が三つの consumer で一致する。
5. partial Leadership history は `RECOMPUTED_FROM_PARTIAL_HISTORY` と estimated/reconstructed notice を持つ。
6. 現行 runner の新規生成 report は現在の Risk Summary wording と同じ generation revision を持つ。
7. 旧 report の resend は原文を保持し、original generation revision と resend revision を区別する。
8. 同日 snapshot の digest mismatch は `SNAPSHOT_CONFLICT` または `Runtime Integrity DEGRADED` となり、
   既存 snapshot を上書きしない。

各テストでは、Decision、Gate、Leader calculation、Action Matrix、Position Sizing、Execution が
変更前と semantic-equivalent であることを確認する。文字列上の JSON 並び順ではなく、決定に関係する
domain projection を比較する。

## 8. 検証、リスク、完了条件

実装後は Rust Runtime の `ai-cockpit verify` で Contract の required check を実行し、
`finish`、`archive` の順に進める。プロジェクト品質確認は次の root Make target を使う。

```text
make fmt-check
make test
make clippy
make quality
```

ここで `make` は project quality gate であり、AI Cockpit lifecycle の入口ではない。lifecycle は
installed Rust Runtime の `ai-cockpit` CLI を使う。

残余リスクは、legacy snapshot の digest 不在、CI 外の手動実行で execution SHA が渡されない場合、
既存 report の metadata 不在である。これらは旧 artifact の改変で補正せず、現 run の integrity を
`DEGRADED` または `UNAVAILABLE` と表示する。


---
author: Ray
title: Corporate Event Provider 中立化設計
description: Corporate Event application contract から Finnhub 固有の意味を分離し、既存の観測層と決定境界を維持する WI-1 の設計。
key: corporate-event-provider-neutralization-design
---

# Corporate Event Provider 中立化設計

## 1. 目的

本 Work Item は、Corporate Event の application contract に混入している Finnhub 固有の source、URL、release window の既定値を取り除く。目的は Provider を増やすことではなく、同じ application contract を将来の公式開示、会社 IR、外部 fixture でも利用できる境界にすることである。

既存の Finnhub earnings calendar adapter は WI-1 の唯一の実装対象として残す。既存の Signal Context、Observation/Interpretation 境界、`decision_weight = 0`、および取引判断の結果は変更しない。

## 2. 固定する不変条件

次の領域は本 Work Item の scope 外であり、変更しない。

- Decision、Gate、Leader、Action Matrix、Position Sizing、Execution
- NO_TRADE / PROBE / READY の意味、RS、Universe Breadth、Market Breadth
- Signal Context の表示意味、NVIDIA fixture の業務上の意味
- 価格または RS から Corporate Event を推測する処理
- SEC、Alpha Vantage、Company IR、Resolver、Provider registry
- 設定ファイルへの API key、User-Agent、その他 secret の追加・変更

Corporate Event は引き続き観測・解釈専用であり、取引判断へ影響を与えない。

## 3. Contract モデル

### 3.1 唯一の source 表現

application contract の source 表現は次の型だけに統一する。

```rust
struct CorporateEventSource {
    provider_id: String,
    source_kind: CorporateEventSourceKind,
    source_url: Option<String>,
}
```

`CorporateEventObservation` と `CorporateEventProviderReadModel` は、意味が重複する `source: String` または独立した `source_url: String` を長期的に保持しない。Source object が provider identity、source kind、provenance URL を一体で表す唯一の境界となる。

```rust
enum CorporateEventSourceKind {
    EarningsCalendar,
    OfficialFiling,
    CompanyIr,
    NewsAggregator,
    ExternalFixture,
}
```

この型は application 層に Finnhub の名前や URL を持ち込まない。Finnhub adapter は `provider_id = "finnhub"`、`source_kind = EarningsCalendar`、安全な calendar URL を生成し、将来の adapter は同じ source object を使う。

### 3.2 Provider unavailable

`CorporateEventProviderReadModel::unavailable` は provider/source を引数として明示的に受け取る。application の default が Finnhub を選ぶことは禁止する。

Unavailable read model は次の意味を維持する。

- `health = Unavailable`
- `events = []`
- `diagnostic` は sanitized な理由を持つ
- unavailable は coverage が正常で matching record がない状態を表さない

Provider adapter の失敗は provider health として残し、`No Event` に変換しない。

### 3.3 Release window

`CorporateEventReleaseWindow` は次の四値とする。

```rust
BeforeMarketOpen,
DuringMarketHours,
AfterMarketClose,
Unknown,
```

`Unknown` の意味は、provider が信頼できる release window fact を提供していないことだけである。`Default` は `Unknown` とする。

Finnhub の raw `hour` は adapter の責務として厳密に検証する。`bmo`、`amc`、`dmh` 以外の値、空値、欠損値は `Unknown` に変換せず parse error とし、provider read model 全体を `Unavailable` にする。これにより「未確定の時段」と「provider payload が壊れている」を混同しない。

## 4. データフロー

```text
Finnhub raw response
        ↓
Finnhub infrastructure adapter
  - payload validation
  - source object construction
  - secret redaction
        ↓
CorporateEventProvider port
        ↓
CorporateEventProviderReadModel
  - provider-neutral source
  - health / diagnostic / events
        ↓
Radar ACL
        ↓
Signal Context observation / interpretation
```

Radar Pipeline は既存の ACL 経由で provider port を利用する。WI-1 では provider factory を registry に変更せず、Radar が Finnhub 固有の URL や payload 型を直接参照しない境界だけを維持する。

## 5. エラーと秘密情報

Finnhub API key は既存の header 注入経路だけを使用する。Source URL には token を含めず、transport error や HTTP error の本文が token を含む場合は `[REDACTED]` に置換する。

次の失敗はすべて `health = Unavailable`、空の events、sanitized diagnostic とする。

- credential missing
- HTTP status outside 2xx
- transport failure
- malformed JSON
- invalid date、symbol、fiscal period
- unsupported or missing release window

正常な 2xx response で valid payload に matching record がない場合だけ `health = Healthy` かつ `events = []` とする。

## 6. 既存消費者への移行

Observation の source field を Source object に置き換え、Radar interface の renderer とテストは object の provider id、kind、optional URL を必要な範囲で読む。既存の表示文言、日付、timezone、release window、NVIDIA の Corporate Context は変更しない。

変更後も application contract の default、fallback、fixture helper が Finnhub を暗黙に生成しないようにする。Fixture provider を使うテストでは `provider_id = "fixture"`、`source_kind = ExternalFixture` を明示する。

## 7. テスト設計

### Contract tests

- Source object が provider id、source kind、optional URL を保持する
- application default に Finnhub の source/name/URL がない
- `unavailable(source, diagnostic)` が明示 source を保持する
- release window の default が `Unknown` である

### Adapter tests

- valid Finnhub NVIDIA fixture が同じ event facts を生成する
- `bmo`、`amc`、`dmh` が対応する enum に変換される
- unsupported、missing、empty `hour` は parse error になる
- parse error は provider read model の `Unavailable` に fail-closed する
- missing key、HTTP failure、malformed payload の diagnostic が token を含まない
- source URL に token が含まれない

### Regression tests

- 既存 NVIDIA/Finnhub fixture の Signal Context が semantic-equivalent である
- 同じ price/RS/universe input で Corporate Event provider の metadata を変更しても Decision、Gate、Leader、Action Matrix、Position Sizing、Execution が同一である
- 差分が許されるのは provider metadata、Observation/Interpretation、diagnostic のみである

## 8. 検証と完了条件

WI-1 では次の Make target をすべて実行する。

```text
make fmt-check
make test
make clippy
make quality
```

AI Cockpit は Contract の checkpoint 後に required verification を記録し、`verify → finish → archive` を順に実行する。最終 PR の説明には scope、out of scope、変更・不変の semantics、provider failure、Decision invariance、verification、residual risks を含める。

この設計の完了は WI-1 の完了だけを意味する。SEC、Alpha Vantage、Resolver は WI-1 の archive 後に別々の Work Item として開始する。

## 9. 既知の残余リスク

WI-1 完了時点では provider health は `Healthy` / `Unavailable` のままとし、`Partial` / `Stale` は後続 Resolver の scope とする。既存の Signal Context は provider-neutral metadata を受け取るが、複数 provider の照合や lifecycle 判定はまだ提供しない。

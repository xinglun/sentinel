---
author: Ray
title: Alpha Vantage Earnings Calendar Expected Provider 設計
description: Alpha Vantage の earnings calendar を確認前の企業イベント期待値へ fail-closed に正規化する設計。
key: alpha-vantage-earnings-calendar-provider-design
---

# Alpha Vantage Earnings Calendar Expected Provider 設計

## 目的

Alpha Vantage の `EARNINGS_CALENDAR` CSV を、確認前の企業イベント期待値として扱う。対象は「どの会社がいつ決算を予定しているか」の観測だけであり、公式開示による確認済み事実、Signal Context、Decision、Gate、Action Matrix、Position Sizing、Execution は扱わない。

## 境界

Application contract には `ExpectedCorporateEvent` と専用の expected provider read model を置く。確認済み `CorporateEventObservation` を再利用して expected を表現しない。すべての source は既存の provider-neutral `CorporateEventSource` object で表現し、legacy な `source: String` は追加しない。

Alpha Vantage adapter は Research infrastructure に隔離し、ACL factory だけが concrete adapter を公開する。API key は `ALPHA_VANTAGE_API_KEY` のプロセス環境から読み、repository、config file、log、fixture、evidence には保存しない。

## API と universe

1 日 1 回の refresh で `EARNINGS_CALENDAR` を 1 回だけ呼び、`horizon=3month` と `datatype=csv` を指定する。symbol ごとの API call は行わない。取得した CSV はローカル observation universe と照合し、対象外 symbol は skip する。

Alpha Vantage の CSV に明示的な fiscal quarter/year がない場合、fiscal period は `None` とする。`fiscalDateEnding` が存在する場合は日付形式だけを検証し、暦四半期を推測して fiscal period を生成しない。

## Cache

Cache は JSON envelope とし、`fetched_at`、`provider`、`schema_version`、`records` を必須にする。freshness の TTL は 24 時間で、fresh cache hit は network call を発生させない。expired cache は stale event を返さず refresh を試み、refresh が失敗した場合は `Unavailable` とする。malformed、provider mismatch、schema mismatch の cache は `Unavailable` とし、推測や stale fallback を行わない。書き込みは同一 directory の temporary file から rename する。

## 正規化と fail-closed

成功した row は `ExpectedCorporateEvent` として次を持つ。

- `symbol`
- `event_type = Earnings`
- `expected_date`
- optional `fiscal_period`
- `CorporateEventSource { provider_id, source_kind, source_url }`
- `observed_at`

Alpha Vantage は `Scheduled` expected event だけを返す。`Confirmed`、`Released`、actual、estimate、surprise、テーマ、因果、価格反応は生成しない。

次の状態は `Unavailable` とし、`No Event` に変換しない。

- API key 不在または空文字
- HTTP transport、timeout、非 2xx、quota/error note
- malformed CSV / JSON error payload
- 対象 symbol の空 symbol、invalid report date、invalid fiscal date
- cache の破損、schema/provider 不一致、cache write failure

unknown symbol はローカル universe 外のため skip する。正常な CSV で対象 symbol が存在しない場合は `Healthy` かつ空の expected event 集合として返すが、それを「会社に earnings がない」と解釈してはならない。

## テスト

transport spy で 1 回の 3-month request、fresh cache hit、expired cache、malformed cache、atomic write failure を確認する。fixture と inline payload で成功、unknown symbol、empty universe、quota note、非 2xx、malformed CSV、invalid date、invalid fiscal date、missing key を確認する。read model の source object が唯一の source 表現であり、既存 Decision surface に差分がないことを diff と quality gate で確認する。

## 対象外

SEC EDGAR、Finnhub、provider registry、Resolver、Signal Context、Decision semantics、Gate、Action Matrix、Position Sizing、Execution、production snapshot、daily report、Telegram、hard-coded credential は対象外とする。

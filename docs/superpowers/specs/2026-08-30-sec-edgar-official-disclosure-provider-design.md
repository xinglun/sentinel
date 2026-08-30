---
author: Ray
title: SEC EDGAR Official Disclosure Provider 設計
description: SEC 公式提出書類を企業イベントの official evidence として取得する WI-2 の境界、契約、失敗時の挙動と検証方針。
key: sec-edgar-official-disclosure-provider-design
---

# SEC EDGAR Official Disclosure Provider 設計

## 目的

この Work Item は、Sentinel の観測ユニバースに含まれる企業について、指定した market date に SEC EDGAR から取得できる公式提出書類を、後続の resolver が消費できる独立した read model として提供する。

SEC は official evidence provider であり、Finnhub の earnings calendar、ニュース provider、取引シグナル provider ではない。したがって、この Work Item は Signal Context、Decision、Gate、Leader、Action Matrix、Position Sizing、Execution の意味を変更しない。

## Application contract

`OfficialDisclosureProvider` は次の入力だけを受け取る。

- market date
- `CompanyIdentity { symbol, cik }` の配列

`OfficialDisclosureObservation` は次の事実を保持する。

- symbol と十桁 zero-padded CIK
- form、accession number、filing date
- 任意の report date と accepted timestamp
- primary document と SEC archive URL
- `OfficialDisclosureKind`
- provider-neutral な `CorporateEventSource`
- Sentinel が取得した時刻

8-K は `items` に明示された `2.02` がある場合だけ `EarningsRelated` とする。Item 情報がない場合は `Unknown`、別の Item の場合は `OtherMaterialDisclosure` とする。10-Q と 10-K はそれぞれ `QuarterlyReport` と `AnnualReport` であり、単独では earnings release の確認を意味しない。

## Identity と cache

CIK は数値または文字列から十桁表現に正規化する。入力 CIK、SEC submissions の CIK、tickers の symbol が一致しない場合は推測せず、対象 symbol を `UNAVAILABLE` とする。

symbol と CIK の mapping は company ticker endpoint から一度取得し、`symbol`、`cik`、`source`、`retrieved_at` を持つ JSON cache に保存できる。cache を使う場合でも不正な CIK は読み込まず、再マッピングによる推測もしない。

## HTTP と fail-closed

- `SENTINEL_SEC_USER_AGENT` を優先し、空の場合だけ既存 `SEC_USER_AGENT` を互換的に読む。
- User-Agent が空または不正なら、SEC への request を開始しない。
- timeout は有限で、response body は上限を持つ。
- request は provider 単位で 500ms 以上空け、最大 2 回の retry（初回を含めて最大 3 試行）に制限する。
- 429 と 5xx、および接続失敗は bounded backoff の対象とし、403 などの恒久的エラーは再試行しない。
- malformed JSON、空または不整合な submissions、無効な date/accession、CIK mismatch は `UNAVAILABLE` とする。
- healthy な空 observation と `UNAVAILABLE` を read model で区別し、失敗を `NO_EVENT` に変換しない。

HTTP URL には credential を含めず、diagnostic と test output にも credential を含めない。User-Agent は設定値であり、個人の連絡先をコードに hard-code しない。

## 範囲外

この Work Item では次を行わない。

- Alpha Vantage、Corporate Event Evidence Resolver、provider registry
- Form 4、13F、DEF 14A、S-1、S-3、13D、13G
- Finnhub adapter や既存 Signal Context contract
- 取引 Decision の semantics、look-ahead 判定、theme、causal explanation
- secret の設定ファイルへの書き込み

## 検証

fixture transport で network を隔離し、NVDA 8-K Item 2.02、別 Item の 8-K、10-Q、10-K、Item 不明の 8-K と、CIK mismatch、malformed JSON、空 submissions、invalid date/accession、403、429、5xx、timeout/connection failure、User-Agent 欠落を検証する。

プロジェクト全体の quality gate は `make fmt-check`、`make test`、`make clippy`、`make quality` とする。後続 WI がこの read model を resolver に接続するまで、Radar の Decision surface は既存実装を維持する。

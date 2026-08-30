---
author: Ray
title: Corporate Event Evidence Resolver 設計
description: 期待値、公式開示、aggregator、外部 enrichment を時間境界付きで canonical corporate event evidence に統合する設計。
key: corporate-event-evidence-resolver-design
---

# Corporate Event Evidence Resolver

## 目的

企業イベントについて「どの Provider が勝ったか」ではなく、「現在どの事実を知っているか」を application 層で表現する。Alpha Vantage の expected event、SEC EDGAR の公式開示、Finnhub の aggregator observation、既存の外部構造化 JSON を、source provenance と取得時刻を失わずに統合する。

本 Work Item は Observation / Interpretation 専用である。`DecisionPacket`、Gate、Leader、Action Matrix、Position Sizing、Execution、Trader および既存の Decision semantics には入力を追加しない。

## 境界と入力

Resolver は次の application contract だけを入力にする。

- `ExpectedCorporateEventProviderReadModel`: 発表予定日を持つ expected event。
- `OfficialDisclosureProviderReadModel`: SEC などの公式提出事実。`EarningsRelated` だけを earnings の公式確認候補とし、filing の存在だけで earnings と解釈しない。
- `CorporateEventProviderReadModel`: Finnhub の normalized observation。actual 値が存在しても official confirmation には昇格しない。
- `ExternalCorporateEventEnrichment`: 既存の構造化 JSON から ACL で変換された theme、importance、説明、source。
- `report_run_at`: 現在の snapshot が観測できる時刻。

Infrastructure の JSON、CSV、HTTP response 型は Resolver に漏らさない。Provider の生成と network/cache failure は Research ACL に閉じ込め、Radar Pipeline は `CorporateEventEvidenceResolution` だけを受け取る。

## Canonical model

`CorporateEventEvidence` は一つの symbol/event type について次を保持する。

- lifecycle: `Scheduled`、`PendingConfirmation`、`Confirmed`、`Historical`、`Unavailable`。
- `expected_date` と `confirmed_event_date` を別フィールドで保持する。
- `confirmed_at` は Sentinel が公式 evidence を accepted した時刻であり、event date とは混同しない。
- `evidence` は source object、event date、`observed_at`、`accepted_at`、source timestamp を保持する。
- `diagnostics` は date conflict、future evidence、untraceable evidence、expected/confirmed date difference を記録する。
- 外部 enrichment の theme/importance は generic provider fact より優先して保持し、provenance も evidence として残す。symbol 名から theme を推測しない。

Provider health は Resolver の診断用に `Healthy`、`Partial`、`Stale`、`Unavailable` を持つ。既存 provider の read model は変更せず、ACL read model の内容から Resolver health に投影する。health が `Unavailable` でも event がないという意味にはしない。

## Reconciliation rule

公式 `EarningsRelated` evidence が report cutoff 以前に存在する場合だけ `Confirmed` とする。公式確認が event date より前に Sentinel に受理されている場合は `Confirmed`、過去日付の公式 event は `Historical` とする。

公式 evidence がない場合、expected event または aggregator observation は `Scheduled` / `PendingConfirmation` に留める。expected date と aggregator date が異なる場合、片方を選ばず両 evidence を保持し、`ProviderDateConflict` を付与する。公式 date と expected date が異なる場合は両方を保持し、`ExpectedConfirmedDateDifference` を診断する。

`QuarterlyReport`、`AnnualReport`、その他 filing は official source であっても earnings confirmation には使用しない。Finnhub の actual/estimate は `AggregatorObserved` として evidence に残すが lifecycle は `Confirmed` にしない。

## Look-ahead と fail-closed

各 evidence の visibility は次で制限する。

- external / expected / aggregator: `observed_at <= report_run_at`。
- official: `accepted_at` があればそれを、なければ `retrieved_at` を cutoff と比較する。
- timestamp が parse 不能、future、source または event type が空の場合はその evidence を採用せず diagnostic にする。

したがって 2026-08-27 取引中の snapshot は、その後に accepted された filing を見ない。Provider failure、malformed payload、cache stale/unavailable は空 event と同じ扱いにせず、health と diagnostic に残す。

## Signal Context と報告

Resolver の canonical event を既存の Signal Context observation item に投影する。Scheduled/PendingConfirmation は予定中、Confirmed/Historical は確認済みとして表示し、event fact に expected date、confirmed date、lifecycle、source list を明記する。既存の外部 corporate context が同じ event を enrich する場合は、Resolver の generic fact が外部の theme/importance/説明を上書きしない。

既存の日報 source diagnostics appendix を再利用し、provider health と symbol ごとの Scheduled/Confirmed/source を表示する。Telegram の新規文言や Decision surface は追加しない。

## 検証

- application reconciliation tests: lifecycle、official boundary、conflict、enrichment、provenance。
- adapter/ACL tests: current provider read model の投影、empty/unavailable、外部 JSON の fail-closed 変換。
- NVIDIA lifecycle regression: expected tomorrow、pre-release pending、post-release official confirmed、T+1 historical。
- look-ahead test: `accepted_at > report_run_at` を不可視にする。
- decision invariance: corporate evidence の A/B/C/D 入力で Decision、Gate、Leader、Action Matrix、Position Sizing、Execution を比較する。
- `make fmt-check`、`make test`、`make clippy`、`make quality`。

## 制限

現在の provider adapter は既存の bounded timeout / retry / body limit / secret redaction をそのまま利用する。Resolver は provider を直接 HTTP 呼び出ししない。`Stale` は canonical health vocabulary として表現するが、provider adapter が stale cache を返さないケースでは実際の run は `Unavailable` になる。これは stale cache を fresh fact として誤って扱わないための保守的な境界である。

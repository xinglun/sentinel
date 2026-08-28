---
author: Ray
title: 企業イベント Provider / Discovery 設計
description: Finnhub earnings calendar を Signal Context の観察層へ fail-closed に接続する設計。
key: corporate-event-provider-discovery-design
---

# 企業イベント Provider / Discovery 設計

## 目的

既存の `SignalContextV1.corporate_events` に、決定論的な外部 JSON 以外の本番入力を追加する。第一実装は Finnhub earnings calendar とし、取得した事実を Observation / Interpretation に投影する。企業イベントは説明用情報であり、Decision、Gate、Leader、Action Matrix、Position Sizing、Execution へは入力しない。

## 根拠とデータ境界

Finnhub の公式 API は `/api/v1/calendar/earnings` で日付範囲、銘柄、国際市場フラグを受け取り、`date`、`symbol`、`hour`、`quarter`、`year`、EPS / Revenue の actual・estimate を返す。`hour` は `bmo`、`amc`、`dmh` の発表時間帯であり、正確な発表時刻やテーマ分類ではない。認証は token または `X-Finnhub-Token` を使い、rate limit 超過時は HTTP 429 となる。

参照: [Finnhub API Documentation](https://finnhub.io/docs/api/quote)

Provider は Finnhub のフィールドだけを事実として扱う。`NVIDIA`、`AI_INFRASTRUCTURE`、Data Center、良い決算、価格反応などを symbol や price / RS から推論しない。既存の `SENTINEL_SIGNAL_CONTEXT_JSON_PATH` に同一イベントの enrichment がある場合は、Provider の基礎事実と重複させず、既存 enrichment のテーマ・証拠・説明を保持する。

## アーキテクチャ

### Provider port

`src/features/research/interface/corporate_event_provider.rs` に provider-neutral な入力契約を置く。契約は次を返す。

- `CorporateEventProviderReadModel`: market date、source、source URL、health、fetch time、diagnostic、normalized events。
- `CorporateEventObservation`: symbol、event date、release window、fiscal quarter/year、actual / estimate 値、source evidence。
- `CorporateEventProvider` trait: market date と対象 symbol 群を受け、read model を返す。

Provider の health と diagnostic は読み取り専用の監査情報であり、Decision の重みではない。

### Finnhub adapter

`src/features/research/infrastructure/finnhub_corporate_event_provider.rs` に `FINNHUB_API_KEY` を既存 `AppConfig.finnhub` から読む adapter を置く。HTTP transport は trait 境界の内側へ分離し、fixture transport でネットワークなしの parser / error test を可能にする。production transport は `reqwest::blocking::Client`、有限 timeout、`X-Finnhub-Token` header、token を含めない source URL を使う。

要求は対象 market date の一日範囲と US watchlist symbol 群に限定する。レスポンスは required object / array、symbol、ISO date、許容 release window を検証し、対象日以外を破棄する。actual EPS または actual Revenue がある released event は HIGH、それ以外の予定 event は MEDIUM とする。

### Signal Context 接続

Radar pipeline は report date ごとに Provider を一度呼び、`SignalContextEventReadModel` に corporate provider read model を渡す。`signal_context_coverage` が normalized event を `SignalContextItem` へ投影する。

- title: `<SYMBOL> EARNINGS`
- type: `CORPORATE`
- evidence event type: `EARNINGS`
- release window: `BMO` / `AMC` / `DMH` を event fact に保持
- market timezone: `America/New_York`
- lifecycle: 対象日なら `RELEASED`、将来日なら `UPCOMING`
- `decision_weight=0`、`trade_signal=false`、execution 系効果は `none`

同一 symbol / event date の既存外部 context は Provider event より優先し、Provider は不足事実の補完に限定する。Provider event が無い場合は既存の外部 JSON fallback と `UNAVAILABLE` を変更しない。

## 失敗と fail-closed

次のいずれかでは corporate Provider を `UNAVAILABLE` とし、イベントを推測・部分生成しない。

- API key 不在または空文字
- client 作成、DNS、接続、timeout の失敗
- 非 2xx、401 / 403、429、その他 provider error
- JSON shape、required field、ISO date、release window の不正
- market date / timezone の不一致

失敗診断には secret、token、完全な認証 URL を含めない。macro calendar や既存の他 source が持つ状態を Provider の失敗で上書きしない。

## テストと検証

- 2026-08-27 NVIDIA earnings の raw Finnhub response fixture で normalized event、HIGH、`NVIDIA EARNINGS`、`CORPORATE / EARNINGS`、`AMC`、`America/New_York`、evidence を固定する。
- missing key、401、403、429、timeout / transport error、malformed JSON、date mismatch、unknown release window を fixture transport で fail-closed 検証する。
- 同一 price / RS data の Provider event 有無を比較し、Decision、Gate、Leader、Action Matrix、Position Sizing、Execution が semantic-equivalent であることを固定する。
- 既存の external JSON に AI theme enrichment がある場合、Provider の generic `EARNINGS` がそれを上書きしないことを検証する。
- `make fmt-check`、`make test`、`make clippy`、`make quality` を実行し、Cockpit verification に記録する。

## 明示的な対象外

SEC / IR / News / transcript provider、全市場の corporate event discovery、event impact の因果確定、Factor Concentration、Market Breadth、RS threshold、Leader、Gate、NO_TRADE、Action Matrix、Position Sizing、Execution、Telegram 文案、position sizing は本設計に含めない。

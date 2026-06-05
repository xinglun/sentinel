---
author: Ray
title: Capital Absorption Observation
description: Capital Absorption Early Warning Sensor の現段階契約と表示境界。
key: capital-absorption-observation
---

# Capital Absorption Observation

Capital Absorption Early Warning Sensor は、AI cycle 周辺で将来の株式供給候補が増えているかを観測するための Observation Layer です。

現段階は `Narrative Observation Only` です。Finnhub company-news、Finnhub market-news、headline / summary keyword scan に基づく narrative observation だけを扱い、市場の実際の資本吸収能力や流動性は測定しません。

## 現段階の境界

本 sensor は次を行いません。

- 実際の Capital Absorption を測定しない。
- 市場流動性を測定しない。
- 市場結論を生成しない。
- Trading Signal を生成しない。
- Risk Upgrade を生成しない。
- `READY`、`EXECUTE`、`Gate`、`Position Sizing`、`Trend Layer`、`trader` に接続しない。

Status は現段階では `NORMAL` と `WATCH` だけを許可します。`ACTIVE` と `STRESSED` は、Capital Supply data と Rolling 12M Capital Model を接続した後に再評価する将来状態です。

## Actual Capital Supply

Actual Capital Supply は、すでに発生した大型供給 event だけを扱います。

- Secondary Offering
- Equity Raise
- Convertible Debt
- Follow-on Offering
- ATM Program
- Secondary Liquidity event

Actual event だけが Actual Supply Event Count、observed actual supply amount、現段階の actual financing subtotal に入ります。

## Potential Capital Supply Queue

Potential Capital Supply Queue は、まだ発生していない IPO 候補や準備段階の narrative を扱います。

- IPO Rumor
- IPO Candidate
- IPO Preparation
- Pre-IPO Discussion

対象例は OpenAI、Anthropic、SpaceX、Databricks、Stripe、Figure です。これらは queue observation であり、Actual Capital Supply ではありません。

次の読み替えは禁止します。

- IPO news increase は actual capital supply increase と同義ではない。
- Actual capital supply increase は market absorption failure と同義ではない。
- Market absorption failure は market risk increase と同義ではない。

## Potential Supply Trend

現段階では Capital Demand Trend を出力しません。代わりに Potential Supply Trend を出力します。

値は次の三つに限定します。

- `STABLE`
- `RISING`
- `FALLING`

`ACCELERATING` は現段階では使いません。資本需要の加速や市場吸収失敗を示すには、Capital Supply data と rolling model が不足しているためです。

## IPO Queue History

IPO Queue History は、current observation window 内の queue size を日付別に集計する表示です。

これは長期 persistence ではありません。data branch、JSONL、snapshot、weekly metrics には現段階では保存しません。

長期比較が必要になった場合は、別 Work Item で schema、persistence、weekly aggregation、backfill policy を定義します。

## Event Type

Event Type は observation の成熟度を表示するために使います。

- `Confirmed`: filed、announced、priced、listed、completed など、発生または正式進行が確認できるもの。
- `Reported`: media report として観測されたが、正式確定ではないもの。
- `Rumor`: rumor、speculation、considering、pre-IPO discussion など、潜在 queue に留まるもの。

Source count と confidence は引き続き表示しますが、Event Type は読者が actual event と potential queue を混同しないための主表示です。

## Source Failure

自動 source が unavailable の場合、default AI IPO queue は表示しません。

理由は、静的な候補 issuer list を自動観測結果に見せないためです。source failure 時は source status と no observed event を表示し、Anthropic、OpenAI、SpaceX、Databricks、Stripe、Figure などの default queue は出しません。

## 今後の拡張条件

3 か月から 6 か月の連続運用で大型 financing event が複数連続した場合、次を優先候補として再評価します。

- ETF Net Inflow
- Corporate Buyback
- Capital Absorption Ratio
- Rolling 12M Capital Model

これらを接続するまでは、Capital Absorption Early Warning Sensor は Observation Layer に留まり、decision layer には入れません。

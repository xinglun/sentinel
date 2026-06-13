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

Actual Capital Supply は、すでに発生し、かつ financing amount を確認できる大型供給 event だけを扱います。

- Confirmed Equity Raise
- Confirmed Secondary Offering
- Confirmed Follow-on Offering
- Confirmed Convertible Debt Issuance
- Confirmed ATM Program
- Completed / priced / filed IPO with confirmed amount

Actual event だけが Actual Supply Event Count、observed actual supply amount、現段階の actual financing subtotal に入ります。

Actual Supply amount は confirmed financing amount だけを使います。rumor、expected IPO valuation、projected IPO size、market cap、private valuation は Actual Supply amount に入れません。

次は Actual Capital Supply に入れません。

- IPO rumor
- IPO candidate
- IPO discussion
- Analyst article
- before IPO / ahead of IPO 型の記事
- competitor / related ticker 型の記事
- issuer financing event ではなく IPO keyword だけを含む記事

## Potential Capital Supply Queue

Potential Capital Supply Queue は、まだ発生していない IPO 候補や準備段階の narrative を扱います。IPO 完了済み issuer は Queue から除外し、Observation Asset として Observation Watchlist に移します。

- IPO Rumor
- IPO Candidate
- IPO Preparation
- Pre-IPO Discussion
- IPO Expectation

対象例は OpenAI、Anthropic、Databricks、Stripe、Figure です。これらは queue observation であり、Actual Capital Supply ではありません。

Wall Street analyst research calls、generic stock recommendation article、`3 stocks to buy before X IPO` 型の記事、unrelated ticker mention、issuer financing event を含まない IPO keyword mention は queue observation からも除外します。

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

IPO Queue History は、current observation window と保存済み ledger から queue size を日付別に集計する表示です。自動観測がある場合、`output.save_to` 配下の `capital_absorption_ipo_queue_history.jsonl` を `as_of_date` 以前に限定して replay し、最新観測日を終点として直近 30 日の日次 window を表示します。単日の Queue Size だけで trend を判断しません。

自動観測成功時は、同じ保存先に次の構造化 artifact を保存します。

- `capital_absorption_ipo_queue_history_latest.json`
- `capital_absorption_ipo_queue_history_YYYY-MM-DD.json`
- `capital_absorption_ipo_queue_history.jsonl`

保存 record は `date`、`queue_count`、`reported_count`、`confirmed_count`、`pressure`、`items` を持つ構造化 record です。全文 Markdown report の保存ではありません。

週次成果物では、`weekly_state_metrics.json -> latest_context.capital_absorption_ipo_queue` と `weekly_state_review_auto.md` の観測専用 section に集約します。週次集約は `as_of_date` 以前の ledger record だけを使い、未来日 record を混入させません。

## IPO Lifecycle

IPO Lifecycle は IPO 前の queue 管理と IPO 後の observation 管理を同じ読解軸で扱うために使います。

- `Rumor`: rumor や speculation だけが観測されている。
- `Reported`: media report として IPO narrative が観測されている。
- `Confirmed`: filing、pricing、listing window など、正式進行が観測されている。
- `Listed`: 上場済みで、IPO Queue ではなく Observation Asset として扱う。
- `Observed`: listed 後 30 日以上の observation window に入っている。
- `Graduated`: observation window を終え、別の投資判断 process に渡せる状態。

`Review Candidate` は lifecycle state ではありません。listed 後 90 日以上で review window に到達したことを示す flag として Observation Watchlist に表示します。

## IPO Stage

IPO Stage は IPO lifecycle を表示するために使います。Event Type とは分離し、IPO の進行段階だけを表します。

- `Rumor`: rumor や speculation だけが観測されている。
- `Reported`: media report として IPO narrative が観測されている。
- `Preparation`: IPO preparation、banker / adviser hiring、readiness など準備報道。
- `Pre-IPO`: 上場時期、条件、valuation、roadshow などが具体化している。
- `Filed`: S-1、filed for IPO、files to go public など filing が観測されている。
- `IPO`: priced IPO、listed、begins trading、debut など IPO 実施済み。

`IPO` stage の issuer は Potential Capital Supply Queue と Near-Term Supply から除外し、Observation Watchlist に表示します。

## Near-Term Supply の時間減衰

Near-Term Supply は古い event を圧力として残し続けないため、最新 observation date からの経過日数で重みを落とします。

- `0-30 Days`: High weight。
- `31-90 Days`: Medium weight。
- `90+ Days`: Expired。Near-Term Supply Count と pressure driver から除外する。

この時間減衰は表示・監査専用です。Trading Signal、`READY`、`EXECUTE`、`Position Sizing`、`Trend Layer` には接続しません。

## Upcoming Supply Timeline

Upcoming Supply Timeline は count だけではなく、bucket ごとの issuer と lifecycle status を表示します。

- `0-30 Days`: 近い IPO supply event。
- `1-12 Months`: preparation、pre-IPO など具体化した future supply。
- `Unknown`: reported / rumor 段階の timing 未確定候補。

表示例:

```text
Upcoming Supply Timeline

0-30 Days
- Figure (Confirmed)

1-12 Months
- OpenAI (Reported)

Unknown
- Anthropic (Reported)
```

## Observation Watchlist

Observation Watchlist は IPO 後の Observation Asset と、学習対象として追跡する private holding / watching 対象を表示するための section です。

例:

```text
Observation Watchlist

- SpaceX: Status Listed · Observation Day: 1 · Review Window: 90 Days
```

Observation Watchlist は認知・学習対象管理であり、投資対象管理ではありません。次には接続しません。

- Trading Signal。
- `READY`。
- `EXECUTE`。
- Position Sizing。
- Trend Layer。

## Event Type

Event Type は evidence level を表示するために使います。IPO Stage と混用せず、証拠の強さだけを表します。

- `Confirmed`: confirmed / announced / priced / completed / filed with amount など、発生または正式進行と financing amount が確認できるもの。
- `Reported`: media report として観測されたが、正式確定ではないもの。
- `Rumor`: rumor、speculation、considering、pre-IPO discussion など、潜在 queue に留まるもの。

Source count と confidence は引き続き表示しますが、Event Type は読者が actual event と potential queue を混同しないための主表示です。

## Discovery Summary

Default report の Discoveries / Observed Events は summary だけを表示します。headline detail は本文に展開しません。

表示は `New`、`Upgraded`、`Downgraded`、`Disappeared` の四区分とし、現段階では current observation window 内の issuer / subject 別 source count を `New` に集約します。前回 snapshot との差分比較、headline appendix、debug mode は将来拡張です。

## Source Failure

自動 source が unavailable の場合、default AI IPO queue は表示しません。

理由は、静的な候補 issuer list を自動観測結果に見せないためです。source failure 時は source status と no observed event を表示し、Anthropic、OpenAI、Databricks、Stripe、Figure などの default queue は出しません。

## 今後の拡張条件

3 か月から 6 か月の連続運用で大型 financing event が複数連続した場合、次を優先候補として再評価します。

- ETF Net Inflow
- Corporate Buyback
- Capital Absorption Ratio
- Rolling 12M Capital Model

これらを接続するまでは、Capital Absorption Early Warning Sensor は Observation Layer に留まり、decision layer には入れません。

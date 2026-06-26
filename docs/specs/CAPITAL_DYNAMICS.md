---
author: Ray
title: Capital Dynamics
description: Supply / Demand / Balance と Flow Layer の観測境界を定義する設計メモ。
key: capital-dynamics
---

# Capital Dynamics

Capital Dynamics は、市場が吸収すべき供給と、その供給を支える需要を分けて観測するための上位概念である。

本 document は Flow Layer の初期設計を定義する。Flow Layer は新しい trading signal ではなく、Capital Dynamics の Demand 側 Observation Layer として扱う。

## 中核判断

Supply は市場が吸収すべきものを観測する。

Flow は誰が需要を提供しているかを観測する。

Balance は将来、供給と需要が失衡しているかを評価するための候補である。

## Layer 構造

```text
Capital Dynamics
├── Supply Layer
│   ├── IPO
│   ├── Secondary Offering
│   ├── Convertible
│   ├── Unlock / Insider Supply
│   └── ATM / Follow-on
├── Demand Layer
│   ├── Flow Layer
│   │   ├── Futu Capital Flow
│   │   ├── ETF Flow
│   │   ├── Mutual Fund Flow
│   │   ├── Foreign Capital
│   │   ├── Options / Gamma
│   │   └── Manual Observation
│   └── Buyback / Institutional Demand
└── Balance Layer
    ├── supply_pressure
    ├── demand_support
    └── absorption_balance
```

既存の Capital Absorption は Supply Layer の初期実装として扱う。IPO queue、secondary offering、convertible、confirmed financing event は Supply 側の observation であり、Flow Layer と混同しない。

## Flow Layer の境界

Flow Layer is Observation Only.
Current decision weight is 0%.
It does not connect to Gate, Execution, Trader, Action Matrix, or Position Sizing.

Flow Layer may explain trend quality, but must not override Trend Layer.

Flow Layer は、Trend Layer が示す価格行動を説明する補助情報である。Flow が価格 trend を否定したり、Trend Layer の状態、Gate、execution、position sizing を直接変更したりしてはならない。

Flow Layer が出してよいものは次に限定する。

- Markdown / Telegram / CLI report の観測説明。
- weekly review の読み取り専用 snapshot。
- telemetry または snapshot の研究用 record。
- trend quality に関する説明候補。

Flow Layer が出してはならないものは次である。

- `READY` / `EXECUTE` 判定。
- 売買 action。
- position sizing。
- risk overlay の直接変更。
- Trend Layer の override。
- trader への入力。

## Domain 方針

Domain は provider に依存しない `FlowObservation` として定義する。第一段階の data source が Futu Capital Flow であっても、Domain 名を `CapitalFlowObservation` に固定しない。

将来の provider 候補は次を含む。

- Futu
- Finnhub
- TradingView
- Alpha Vantage
- Polygon
- Yahoo
- Manual

Provider は observation の source metadata であり、FlowObservation の意味そのものではない。

## FlowObservation の概念 model

初期 model は次の概念を持つ。

```text
FlowObservation
- as_of_date
- scope
- subject
- provider
- source_kind
- direction
- strength
- quality
- continuity_days
- net_flow
- main_net_flow
- source_health
- observed_at
```

`scope` は次の粒度を表す。

- `MARKET`
- `SECTOR`
- `WATCHLIST`
- `CORE_HOLDING`
- `ASSET`

`direction` は次の値を持つ。

- `INFLOW`
- `OUTFLOW`
- `MIXED`
- `FLAT`
- `UNKNOWN`

`strength` は次の値を持つ。

- `VERY_WEAK`
- `WEAK`
- `NEUTRAL`
- `STRONG`
- `VERY_STRONG`

`quality` は次の値を持つ。

- `POOR`
- `NORMAL`
- `HEALTHY`
- `EXCELLENT`

Strength は流量の強さを表す。Quality は継続性、breadth、divergence、source consistency に基づく観測品質を表す。

例として、単日だけ大きな流入がある場合は `STRONG` でも `POOR` になり得る。複数日にわたり watchlist と core holding の breadth がそろっている場合は `STRONG` かつ `HEALTHY` または `EXCELLENT` になり得る。

## Divergence

Flow Divergence は一級 object として扱う。

```text
FlowDivergence
- subject
- price_direction
- flow_direction
- divergence_type
- severity
- explanation_key
```

`divergence_type` は次の値を持つ。

- `POSITIVE`
- `NEGATIVE`
- `NONE`

例:

```text
GOOG
Price: UP
Flow: OUTFLOW
Divergence: NEGATIVE
```

```text
MSFT
Price: FLAT
Flow: INFLOW
Divergence: POSITIVE
```

Divergence は weekly review で有用な観測である。ただし、negative divergence は売却 signal ではなく、positive divergence は買付 signal ではない。

## Breadth

Flow Breadth は単一 watchlist の集計だけに限定しない。

```text
FlowBreadth
- market_breadth
- sector_breadth
- watchlist_breadth
- core_holding_breadth
```

第一段階で market breadth または sector breadth の source がない場合は `UNAVAILABLE` と表示する。watchlist breadth を market breadth として代用してはならない。

Breadth state の候補は次とする。

- `SUPPORTIVE`
- `NEUTRAL`
- `DIVERGENT`
- `STRESSED`
- `UNAVAILABLE`

## Source Health

Flow Layer は source unavailable を推測で補完しない。

Source health は少なくとも次を区別する。

- `SUCCEEDED`
- `PARTIAL`
- `UNAVAILABLE`

Provider failure、entitlement 不足、quota 不足、response validation failure は、Flow の状態ではなく source health として表示する。

## 表示と保存

Phase 1 では次に表示する。

- Radar Markdown / Telegram appendix。
- `daily-calibration` の Flow / Capital Dynamics section。
- `weekly_state_metrics.json -> latest_context.capital_dynamics`。
- Supply 側の canonical path は `latest_context.capital_dynamics.supply_layer` とする。
- 既存 consumer 互換のため、`latest_context.capital_absorption_ipo_queue` は legacy compatibility alias として段階移行中のみ併存してよい。
- `weekly_state_review_auto.md` の Capital Dynamics snapshot。

保存候補は次である。

```text
flow_observation_latest.json
flow_observation_YYYY-MM-DD.json
flow_observation.jsonl
```

Daily report の全文 Markdown 保存と構造化 record を混同しない。長期比較は weekly metrics と JSONL / telemetry の粒度で扱う。

## Evolution

### Phase 1: Observation

Flow は report、weekly review、telemetry にのみ出力する。

この段階の decision weight は `0%` である。

### Phase 2: Confidence Modifier

十分な履歴検証後、Flow は Trend confidence の補助 modifier になり得る。

例:

```text
Trend Confidence: 80
Flow Support: weak
Adjusted Explanation Confidence: 65
```

この段階でも Flow は Gate、execution、position sizing へ直接接続しない。

### Phase 3: Market Health

Trend、Gravity、Macro、Supply、Flow、Balance を統合し、Market Health を構成する可能性がある。

Market Health を導入する場合も、Flow Layer 単体が trading action を生成してはならない。

## 既存 Layer との関係

Trend Layer は価格行動を扱う。

Gravity Layer は価格と価値 anchor の距離を扱う。

Macro Gravity は金利、信用、流動性の外部圧力を扱う。

Supply Layer は市場が吸収すべき供給を扱う。

Flow Layer は誰が需要を提供しているかを扱う。

Capital Dynamics は Supply、Demand、Balance を整理する上位概念である。

Flow Layer はこれらを説明する補助 layer であり、単独で市場判断や売買判断を生成しない。

---
author: Ray
title: Signal Context 時間整合設計
description: Event と Market Observation の公開時刻・観測時刻を明示的に結び付け、日付や取引 session を因果境界にしない。
key: signal-context-temporal-alignment-design
---

# Signal Context 時間整合設計

## 目的

Signal Context の macro event と市場反応を `market_date` だけで同じ日として扱うと、夜間に公開された event が、それより前の observation を説明できるように見える。今回の変更では、日次 report の identity を維持したまま、Event と MarketObservation の時間関係を read model に固定する。

中心となる invariant は次の通りである。

```text
Event can explain MarketObservation
iff
source_published_at <= observed_at
```

`accepted_at <= report_run_at` は report がその Event を知っていたかを決める visibility boundary であり、因果境界ではない。`session`、`venue`、`instrument` は observation の provenance であり、eligibility を置き換える hard gate ではない。

## Read model

既存の `MarketReaction` は互換性のため型と JSON 名を維持し、MarketObservation として次の provenance を追加する。

- `observation_id`: 再実行で変化しない識別子
- `observed_at`: observation の UTC RFC3339 時刻
- `session`: `PREMARKET`、`CORE`、`AFTER_HOURS`、`OVERNIGHT` など
- `venue`: 観測 venue または data source
- `instrument`: 観測対象の識別子
- 既存の `market_date`、`subject`、`observation`、`evidence`

Event 表現には `event_id` と `accepted_at` を追加する。既存 payload を読み込めるよう新規フィールドは serde default を持つが、temporal binding の source 公開時刻が空または RFC3339 として不正な場合は eligibility を作らない。

`TemporalBinding` は renderer 内の比較結果ではなく、次の情報を持つ明示的な read-model record とする。

- `event_id`
- `observation_id`
- `source_published_at`
- `observed_at`
- `temporal_eligible`
- `reason`

Resolver はまず `accepted_at <= report_run_at` の Event だけを report に可視化し、可視 Event と MarketObservation の組み合わせごとに `TemporalBinding` を生成する。`source_published_at` または `observed_at` が欠落・不正なら `temporal_eligible=false` とし、文字列の辞書順比較は行わない。

## Compatibility boundary

`SignalContextV1` に `report_run_at`、`observation_window_start`、`observation_window_end`、`temporal_bindings` を追加する。`market_date`、`latestTradingSession` 相当の既存情報、Daily Report の domain identity、weekly・history・delivery contract はこの段階で変更しない。

window は今回の read model projection のために保持し、日次境界を新しい因果規則として再導入しない。`observation_window_end` は report run の時刻、start は利用可能な observation の最小時刻から求める。観測時刻が不明な observation は window の算出と eligible な反応表示から除外する。

provider と pipeline は report run 時刻を resolver まで渡し、Event の `accepted_at` を取得時の report-run context に結び付ける。外部 fixture も同じ visibility と temporal binding を通す。session・venue・instrument が未知の既存 payload は空の provenance として互換読み込みするが、known value がある場合はそのまま保存・表示する。

## 表示と意味境界

「該当 Event 公開後の市場 observation」として表示できるのは `temporal_eligible=true` の binding に含まれる observation だけである。表示には observation 時刻と provenance を残し、表現は「公開後に観測された」「同時系列で観測された」に限定する。「Event が市場変化を引き起こした」とは表現しない。

`decision_weight=0`、`trade_signal=false`、`gate_effect`、`execution_effect`、`position_sizing_effect` の `none` は維持する。TemporalBinding は explanation/audit 用であり Decision、Gate、RS、Leadership、Action Matrix、Execution、Position Sizing の入力ではない。

## 検証シナリオ

Jordan regression fixture は report run より前に公開された event と、次の observation を持つ。

```text
Event published: 2026-09-09T21:42:00-04:00
15:xx SPY / CORE       -> temporal_eligible=false
22:xx SPY / OVERNIGHT  -> temporal_eligible=true
22:xx NQ / FUTURES     -> temporal_eligible=true
22:xx Brent / FUTURES  -> temporal_eligible=true
```

同じ fixture で、`accepted_at > report_run_at` の Event が除外されること、公開時刻が欠落・不正な Event が fail-closed になること、session・venue・instrument が eligibility を変更しない provenance であることを確認する。

## 非対象

今回の変更では 24/7 calendar の定義、causal attribution classifier、weekly・historical snapshot・delivery contract の全面移行、既存の日次 domain identity の廃止を行わない。これらは後続の Snapshot 時間モデルで扱えるよう、新しい read model fields と invariant を拡張点として残す。

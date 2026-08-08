---
author: Ray
title: Price-Volume Closure 設計
description: S-28 の供給根拠、欠損 OHLCV、個別 context、runtime acceptance を閉じる観測専用修正設計。
key: price-volume-closure-design
---

# Price-Volume Closure 設計

## 目的

S-28 の Price-Volume Structure を Observation Layer として維持しつつ、供給 event の根拠、欠損可能な OHLCV、個別銘柄 context、日報と JSONL の監査証跡を一貫させる。

## 固定境界

`decision_weight=0%`、`trade_signal=false`、`gate_effect=none`、`execution_effect=none`、`position_sizing_effect=none` を固定する。Gate、Execution、Trader、Action Matrix、Position Sizing は変更しない。米国祝日 calendar も追加しない。

## 供給 evidence

`SupplyEventContext` を JSONL にそのまま保存できる構造へする。日報には event type、event date、supply direction、confidence を表示する。吸収判定は `Available`、`Increase`、`High` confidence の明示 fact だけを許可する。複数 event が同時に有効な時は Increase を優先し、次に confidence、event date、event type の順で deterministic に選ぶ。

`config.toml` の例は audit-only の template とし、実在 event を既定値として注入しない。

## データ品質

分類には current bar と 20 本の prior bar、すなわち最低 21 本の連続した完全 OHLCV bar を要求する。volume または OHLC の欠損、429、corporate action、volume split adjustment、weekday gap は `UNAVAILABLE` structure に fail closed する。`PARTIAL` は品質の説明値ではなく、構造分類を許す根拠にしない。ATR は 14 本すべての true range が利用可能な時だけ計算する。

## 個別 context

`OVERHEAT` は symbol ごとの asset state から渡す。global `TIME_COST_RISING` は個別銘柄の Exhausted Advance 条件へ転用しない。これにより、Microsoft 型の exhaustion は当該 symbol の価格位置と縮量だけで評価される。

## 検証

unit test は quality、supply priority、分類境界を固定する。runtime integration test は SpaceX 型と Microsoft 型で、config から report、JSONL、0% boundary までを一続きに検証する。同日 rerun は JSONL record を一件に upsert する。

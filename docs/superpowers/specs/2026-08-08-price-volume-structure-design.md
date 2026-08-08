---
author: Ray
title: Price-Volume Structure 設計
description: S-28 の観測専用 Price-Volume Structure Layer の境界と段階的実装設計。
key: price-volume-structure-design
---

# Price-Volume Structure 設計

## 目的

価格変化、相対出来高、価格位置、供給 event を組み合わせ、参加品質と供給吸収を観測する。これは予測や売買指示ではない。

## 境界

全 output は Observation である。`decision_weight=0%`、`trade_signal=false`、`gate_effect=none`、`execution_effect=none`、`position_sizing_effect=none` を固定する。Gate、Execution、Trader、Action Matrix、Position Sizing は変更しない。

## 構成

1. S-28-01 は共有日足を欠損可能な OHLCV 契約へ拡張する。欠損は補推しない。
2. S-28-02 は evidence-backed supply event を symbol ごとの `SupplyEventContext` に限定して投影する。
3. S-28-03 は RVOL 5/20、価格位置、上下行日の出来高、継続日数を利用し、`ACCUMULATION`、`HEALTHY_ADVANCE`、`EXHAUSTED_ADVANCE`、`DISTRIBUTION`、`NEUTRAL`、`UNAVAILABLE` を分類する。
4. S-28-04 は説明 read model、日報、構造化 observation history へ投影する。
5. S-28-05 は SpaceX、Microsoft、false positive、event noise の市場校正を記録する。

## データと品質

`DailyBar` は open/high/low/close/volume を保持する。OHLC または出来高の欠損、履歴不足、非連続日、provider 失敗、corporate action により後続 layer は `HEALTHY`、`PARTIAL`、`DEGRADED`、`UNAVAILABLE` を返す。比較不能な値から状態を補推しない。

## 事実と解釈

`ACCUMULATION` は「供給が吸収されている観測」を表し、機関投資家の買いを確認しない。`EXHAUSTED_ADVANCE` は参加度低下を表し、下落予測や sell signal を生成しない。単日異常は candidate に留め、Day 1 / Day 2 / Day 3+ の持続性を記録する。

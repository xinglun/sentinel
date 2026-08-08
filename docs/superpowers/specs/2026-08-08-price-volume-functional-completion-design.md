---
author: Ray
title: Price-Volume Functional Completion 設計
description: S-28 の runtime context、供給 event 有効期間、価格行動分類を観測専用で補完する設計。
key: price-volume-functional-completion-design
---

# Price-Volume Functional Completion 設計

## 境界

Price-Volume Structure は Observation Layer に留める。Decision Weight は 0%、trade signal は false、Gate、execution、position sizing への effect は none のままとする。米国祝日 calendar は追加せず、既存の月曜から金曜連続性規則を維持する。

## Runtime Context

既存の `StateTransitionViewModel.holding_efficiency` が `TimeCostRising` の時、Price-Volume classifier に market observation context として渡す。個別銘柄の事実、売買判断、または下落予測へ変換しない。

## Supply Event

明示設定された Supply Event は、`event_date` の前後 20 calendar days だけ Price-Volume Structure の context として有効にする。期間外は context を返さず、過去 event が ACCUMULATION を恒久的に誘発することを防ぐ。

## Price Behavior

既存 metrics を判定へ使う。ACCUMULATION は限定的 downside と下影線の回復、HEALTHY_ADVANCE は高値位置、EXHAUSTED_ADVANCE は小さい実体または上影線による追随失速、DISTRIBUTION は新安値または gap-down breakdown を確認条件とする。単日量だけでは新しい分類を生成しない。

## 検証

unit test は SpaceX 型の有効期間内吸収、期限切れ event の非吸収、MSFT 型の TimeCostRising exhaustion、各 price behavior confirmation、固定された observation boundary を検証する。

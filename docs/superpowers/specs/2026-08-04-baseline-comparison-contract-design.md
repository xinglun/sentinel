---
author: Ray
title: 基線比較契約の統一設計
description: Change Log と Evolution が同一の breadth 基線を比較するための観測専用設計。
key: baseline-comparison-contract-design
---

# 基線比較契約の統一設計

## 目的

前日 formal snapshot の raw breadth と当日 report の分類ラベルを直接比較する経路を廃止する。Change Log、Change Driver、Evolution Timeline は、同じ前日 snapshot と同じ当日市場事実から導いた比較文脈だけを使う。

対象は観測と report の意味論である。Gate、Execution、Trader、Action Matrix、Position Sizing には接続しない。

## 現在の不整合

`TradingDaySnapshot.breadth` は 0--100 の raw 数値である。一方、`signal_summary.breadth_semantic_value` は `Very Narrow` のような表示分類である。現在の Change Log は前者を previous value、後者を current value として `MarketChangeSnapshot.breadth_classification` に渡すため、`35.0 -> 35.0` の Evolution と `35.0 -> Very Narrow` の Change Log が同時に出力される。

## 比較文脈

Change Log の組み立て時に、次の不変な入力を持つ比較文脈を作る。

| 値 | 前日 | 当日 | 出所 |
| --- | --- | --- | --- |
| raw breadth | formal snapshot の `breadth` | packet から算出する割合 | 取引日市場事実 |
| breadth classification | formal snapshot に保存した semantic classification | 当日 packet から導出した semantic classification | 同一 report run |
| baseline identity | formal snapshot の `snapshot_id` と `market_date` | 対象外 | formal resolver |

`Very Narrow` は raw breadth だけの閾値分類ではない。trend breadth mode、leader、substantive evidence などから導出される semantic classification である。そのため、前日 classification は raw 値から再構成せず、formal snapshot に optional field として保存する。当日の値は同じ run の packet から導出する。表示用の `signal_summary` は描画に使えるが、前日比較の代替ソースにはしない。

## Change Driver 規則

- raw breadth が同じで保存済み classification も同じなら、Breadth は driver にしない。
- raw breadth が変わっても保存済み classification が同じなら、既存の Change Level 契約を増やさない。raw 値の差は explanation に出せるが、`breadth_classification` は driver にしない。
- 保存済み classification が異なる場合だけ `breadth_classification` を MODERATE driver にする。
- 旧 formal snapshot に classification field がない場合、Breadth classification comparison は unavailable とする。他の比較可能な次元を維持し、raw 値や presentation 文字列から classification を推測しない。
- confidence、score、ranking などの実変化は既存の priority に従う。
- formal baseline がない場合は、比較文脈を生成せず `BASELINE_UNAVAILABLE` を保つ。

この規則により、報告例の `35.0 -> 35.0` / `Very Narrow -> Very Narrow` は Breadth driver を生成せず、confidence の低下だけなら `MINOR`、他に差分がなければ `NONE` になる。

## 表示と保存

Markdown、Telegram、CLI は既存の Change Log と Evolution 表示を維持する。Change Log は分類の前日値・当日値を表示し、Evolution は raw series を表示するが、両者は同一 raw 基線から導出される。

この Work Item は formal snapshot に後方互換な optional `breadth_classification` field を追加する。JSONL と weekly metrics の schema は変更しない。旧 snapshot は field 不在を許容し、比較不能な classification を driver にしない。

## 失敗時の扱い

formal snapshot がない、または raw breadth を安全に導出できない場合、Change Log は比較を行わず `BASELINE_UNAVAILABLE` を出力する。formal snapshot はあるが classification field だけがない場合、Change Log は他の比較を続け、Breadth classification を driver にしない。以前の presentation 文字列、cache、transition log を代替基線にしてはならない。

## 検証

最低限、次を自動テストで固定する。

1. 前日 35.0、当日 35.0、両日 Very Narrow は Breadth driver なしである。
2. raw 値が変化しても保存済み分類が同じなら Breadth driver なしである。
3. 保存済み classification が変化した場合だけ Breadth driver は MODERATE になる。
4. baseline unavailable は比較を安全に降格する。
5. legacy snapshot の field 不在は Breadth driver を生成しない。
6. zh/en/ja の report snapshot は raw series と分類を混同しない。

## 非目標

Leader Persistence の短期履歴状態、`AVAILABLE` / `PARTIAL` / `UNAVAILABLE` の語彙分離、PR merge と branch cleanup は別 Work Item で扱う。

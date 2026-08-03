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
| breadth classification | raw breadth を同一閾値で分類 | raw breadth を同一閾値で分類 | 比較専用 classifier |
| baseline identity | formal snapshot の `snapshot_id` と `market_date` | 対象外 | formal resolver |

表示用の `signal_summary` は人間向けラベルの描画にだけ使う。比較値の入力には使わない。分類器は raw breadth だけを受け取り、表示言語や presentation の文字列に依存しない。

## Change Driver 規則

- raw breadth が同じで分類も同じなら、Breadth は driver にしない。
- raw breadth が変わっても分類が同じなら、既存の Change Level 契約を増やさない。raw 値の差は explanation に出せるが、`breadth_classification` は driver にしない。
- classification が異なる場合だけ `breadth_classification` を MODERATE driver にする。
- confidence、score、ranking などの実変化は既存の priority に従う。
- formal baseline がない場合は、比較文脈を生成せず `BASELINE_UNAVAILABLE` を保つ。

この規則により、報告例の `35.0 -> 35.0` / `Very Narrow -> Very Narrow` は Breadth driver を生成せず、confidence の低下だけなら `MINOR`、他に差分がなければ `NONE` になる。

## 表示と保存

Markdown、Telegram、CLI は既存の Change Log と Evolution 表示を維持する。Change Log は分類の前日値・当日値を表示し、Evolution は raw series を表示するが、両者は同一 raw 基線から導出される。

この Work Item は JSONL、formal snapshot、weekly metrics の schema を変更しない。比較文脈は report 組み立て中の read model であり、保存しない。

## 失敗時の扱い

formal snapshot がない、または raw breadth を安全に導出できない場合、Change Log は比較を行わず `BASELINE_UNAVAILABLE` を出力する。以前の presentation 文字列、cache、transition log を代替基線にしてはならない。

## 検証

最低限、次を自動テストで固定する。

1. 前日 35.0、当日 35.0、両日 Very Narrow は Breadth driver なしである。
2. raw 値が変化しても分類が同じなら Breadth driver なしである。
3. 同一 classifier により分類が変化した場合だけ Breadth driver は MODERATE になる。
4. baseline unavailable は比較を安全に降格する。
5. zh/en/ja の report snapshot は raw series と分類を混同しない。

## 非目標

Leader Persistence の短期履歴状態、`AVAILABLE` / `PARTIAL` / `UNAVAILABLE` の語彙分離、PR merge と branch cleanup は別 Work Item で扱う。

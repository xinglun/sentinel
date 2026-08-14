---
author: Ray
title: Price-Volume Eligibility / Baseline 設計
description: 標準履歴不足を観測不能と同一視せず、資産ライフサイクル別の比較基線と候補ライフサイクルを導入する設計。
key: price-volume-eligibility-baseline-design
---

# Price-Volume Eligibility / Baseline 設計

## 目的

Price-Volume Structure は、標準 20 日履歴が不足していても、価格・OHLCV・イベント文脈の局所証拠が利用可能なら `PARTIAL` として観測を開始する。`PARTIAL` は確認済みを意味せず、取引判断には接続しない。

## 境界

変更対象は Eligibility、Baseline Selection、Candidate Lifecycle と、その構造化された Markdown/Telegram/JSONL 投影だけとする。Decision Pipeline、Gate、Trader、Action Matrix、Position Sizing、既存の経済的分類定義は変更しない。すべての assessment は `decision_weight_percent=0`、`trade_signal=false`、各 effect `None` を保持する。

## データフロー

1. `DailyBar` と `SupplyEventContext` から valid sessions、データ品質、イベント起点を導出する。
2. `Eligibility` を `FULL` / `PARTIAL` / `INSUFFICIENT` / `UNAVAILABLE` に分類する。
3. 成熟資産は `STANDARD_20D`、IPO は `POST_IPO`、解禁は `POST_LOCKUP`、財報は `POST_EARNINGS` を選択する。必要なら primary/secondary を同時に保持し、イベント起点は提供済み context の事実だけから決める。
4. 選択した基線の valid sessions で RVOL と価格 metrics を計算し、短い基線を `RVOL_20` と呼ばない。
5. 既存の経済条件で構造を判定し、Eligibility と observation count により `CANDIDATE` / `DEVELOPING` / `CONFIRMED` / `INVALIDATED` を別に決める。`PARTIAL` は直接 `CONFIRMED` に昇格しない。
6. レポートは基線、日数、reason、next condition を表示し、JSONL は新字段の欠損を許容して旧記録を読み込む。

## データ不足

`API_FAILURE`、`MISSING_VOLUME`、`DATA_GAP`、`CORPORATE_ACTION_CONFLICT`、`INSUFFICIENT_VALID_HISTORY` などを構造化 reason として出す。Supply Context がない場合は供給吸収を確認せず、价量構造だけを候補として表示する。

## 検証

成熟 20 日、IPO 3/7/15 日、Lock-up 1/3/5 日、Earnings 1/3 日、方向別の既存分類、短期ノイズ、API 429、volume gap、context 欠損、SPCX 型汎用シナリオ、MSFT/PLTR 型イベント後候補、観測境界を Rust テストで固定する。

---
author: Ray
title: Observation Layer 意味整合設計
description: Breadth、Leader Persistence、Current Relative Strength、Interpretation、report の事実源と表示意味を統一する観測専用設計。
key: observation-semantic-reconciliation-design
---

# Observation Layer 意味整合設計

## 目的

新しい `Breadth Raw` と `Current Relative Strength` が既存の state-derived narrative と矛盾し始めたため、Decision 自体を変更せずに Observation から Interpretation、Markdown report までの意味を再統合する。

## 境界

本設計で変更するのは Observation / Interpretation / report / snapshot persistence の read model と保存形式だけである。`Decision`、`Action Matrix`、`Gate`、`Execution`、`Trader`、`READY / EXECUTE`、`Position Sizing`、取引閾値、Trade Signal は変更しない。`Current Relative Strength` の conflict と `RECOVERY_WATCH` は表示・監査用の語彙であり、既存の弱勢判定を取り消さない。

## 1. Breadth の canonical facts

`up_count`、`flat_count`、`down_count`、`total_count` を単一の事実源とし、次を同じ pure helper から導出する。

- `total_count == 0`: raw、label、classification score はすべて `UNAVAILABLE`。`0.0` を欠損値として保存しない。
- `raw_percent`: `up_count / total_count * 100`。
- `label`: `<30` は `Very Narrow`、`30.. <60` は `Narrow`、`>=60` は `Broad Participation`。
- `classification_score`: 有効な raw percent と同じ数値を使用する。

`TradingDaySnapshot` と `ObservationTimelineEntry` の数値フィールドは `Option<f64>` とし、旧 JSON の欠落 field は `None` にする。legacy projection、pipeline、data-quality JSON、report formatter の全境界で unavailable を維持する。既存の `TrendBreadthMode` は市場背景の観測として残すが、Raw Breadth の label/score の source にはしない。

## 2. Leader semantics

Leader Persistence の result と view model は次を別フィールドで保持する。

- `Current Leader`: 現在 snapshot の値。
- `Previous Snapshot Leader`: 直前 snapshot の値。`none` も事実として保持する。
- `Last Confirmed Leader`: 直近の `none` ではない leader。
- `Leader Absence Since`: 連続する `none` 区間の開始日。
- `Leader Absence Duration`: 連続する trading-day 件数。

absence が 5 trading days 以上の場合、tactical `Leadership Structure` は `LEADERLESS / FRAGMENTED`、`Market Structure` は「整理中 / 明確な主導なし」とする。戦略的な core-asset 背景を表示する場合も、当日の tactical leadership と同じ field に混ぜない。

## 3. Relative Strength conflict

既存 asset の `StrengthLoss` / `CohesionExit` または `REDUCE` / `AVOID` と、同じ symbol の Current Relative Strength `IMPROVING` が同時に存在する場合、report read model で次を付加する。

- `SIGNAL_CONFLICT`
- `RECOVERY_WATCH`
- 「長期・累積構造は弱いが、短期相対強度は回復中」という説明

既存の Action / Exit 値、NO TRADE、position sizing は変更しない。

## 4. Interpretation

Narrative は leader absence、raw breadth、RS recovery の symbol、shrink/watch の件数、overheat/crowding の state を入力にする。脆弱事実がある場合は「新しい急激な悪化は観測されないが、主導者不在と拡散不足を含む脆弱構造にある」と記述し、従来の無条件な「構造的悪化の証拠なし」は出さない。RS recovery だけでは新しい Leadership と扱わない。

## 5. Renderer と保存

Markdown と archival output は見出し・箇条書きだけの純 Markdown とする。Telegram HTML body は既存の別 channel contract を維持する。Current Relative Strength の Markdown section に `<h3>` / `<li>` を出力しない。既存 snapshot、serde field、weekly projection は backward-compatible に読み取り、現在の write path では unavailable を 0.0 に戻さない。

## 検証

domain test で Breadth facts、欠損履歴、Leader semantics、conflict 判定を固定し、report UI test で zh/en/ja、Markdown/Telegram HTML、Interpretation、NO TRADE、Observation-only boundary を確認する。最後に `make fmt-check`、`make test`、`make clippy` と Cockpit/architecture/quality gate を実行する。

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

## 3. Relative Strength の状態と回復強度

Current Relative Strength は次の二軸を持つ。

- `RelativeStrengthState`: `IMPROVING`、`NEUTRAL`、`WEAKENING`。
- `RecoveryStrength`: `STRONG`、`MODERATE`、`WEAK`、`NONE`。

5 日相対強度が `>= 5%` の改善を `STRONG`、`>= 2%` を `MODERATE`、`> 0%` を `WEAK` とする。`IMPROVING` と `STRONG` / `MODERATE` の組み合わせだけを `SIGNAL_CONFLICT` / `RECOVERY_WATCH` の表示へ昇格する。`WEAK` は「相対強度には初期改善が見られるが、回復を確認するにはまだ不十分」とするが、既存の弱勢判定を上書きしない。

旧 JSON の `status` / `recovery_state` と旧 enum 値は serde alias で読み取る。現在の write path は新しい state 名を使用する。

既存 asset の `StrengthLoss` / `CohesionExit` または `REDUCE` / `AVOID` と、同じ symbol の強い相対強度回復が同時に存在する場合でも、変更するのは report read model の表示だけである。既存の Action / Exit 値、NO TRADE、position sizing は変更しない。

## 4. Leadership と Relative Strength の Interpretation

現在の `Current Leader` が `none` で leader absence が閾値を超える場合、トップページの tactical 表示を次の三項目に統一する。

- `当前无确认主线`
- `无主线 / 分散`
- `未确认启动期`

「長期構造トレンドの強まり」は strategic background に残すが、tactical mainline の存在を生成しない。pipeline は tactical reconciliation を先に完了してから Interpretation を生成し、旧い `主線存在（戦術未許可）` が解釈層へ混入することを防ぐ。

`Relative Strength Recovery Breadth` は benchmark 以外で、1 日と 5 日の RS がともに利用可能な資産だけを分母とし、改善数と強・中程度の改善数を同時に表示する。改善資産が少なくとも 3 件かつ有効母数の 3 分の 1 以上である場合にだけ `RS Diffusion: EMERGING` とする。RS recovery 単独では新しい Leadership を構成せず、実行ウィンドウも開かない。

表示・解釈の境界は次の条件で固定する。

- Current Relative Strength の benchmark 表示は ViewModel の `benchmark_symbol` を使用し、旧データで値が欠落する場合だけ `SPY` に fallback する。US 以外の市場を `SPY` と表示しない。
- Actionable Diffusion は、同一 symbol に Current Leader、`ConfirmedBreakout`、Action Matrix の `ACCUMULATE` がそろう場合だけ `CONFIRMED` とする。いずれかが欠落する場合、又は異なる symbol に分散する場合は `NOT_CONFIRMED` とする。これは Interpretation の観測値であり、Gate、Action Matrix、Execution の判定を変更しない。
- `EmergingBreakout` と `ConfirmedBreakout` は report の status label を保った breakout observation として表示する。confirmed breakout を「候補」又は「萌芽」と再解釈しない。
- Audit Daily の `Trend Cohesion` は tactical mainline と別の観測である。日次 summary と状態変化ラベルでは「Trend Cohesion / 趋势凝聚 / トレンド凝集」と表記し、mainline の存在・不在を上書きしない。
- `Strong/Moderate Recovery` の集計は `status=IMPROVING` を前提とする。旧 JSON の `status=NEUTRAL` と `recovery_state=Recovering` の組み合わせを回復数へ加算しない。
- Narrative では `IMPROVING + STRONG/MODERATE` を recovery、`IMPROVING + WEAK` を「初期改善だが回復未確認」として分離する。弱い改善を「明確な回復」と表現しない。

Narrative は leader absence、raw breadth、RS recovery の symbol、RS Recovery Breadth、拡散状態、shrink/watch 件数、overheat/crowding の state を入力にする。脆弱事実がある場合は「新しい急激な悪化は観測されないが、主導者不在と拡散不足を含む脆弱構造にある」と記述し、従来の無条件な「構造的悪化の証拠なし」は出さない。RS recovery だけでは新しい Leadership と扱わない。

## 5. Renderer と保存

Markdown と archival output は見出し・箇条書きだけの純 Markdown とする。Current Relative Strength block は Telegram delivery body でも同じ純 Markdown とし、`<h3>` / `<li>` を出力しない。他の Telegram section の既存 channel contract は維持する。既存 snapshot、serde field、weekly projection は backward-compatible に読み取り、現在の write path では unavailable を 0.0 に戻さない。

## 検証

domain test で Breadth facts、欠損履歴、Leader semantics、conflict 判定を固定し、report UI test で zh/en/ja、全 delivery body の Markdown、Interpretation、NO TRADE、Observation-only boundary を確認する。最後に `make fmt-check`、`make test`、`make clippy` と Cockpit/architecture/quality gate を実行する。

---
author: Ray
title: Validation Utility Closure V1 事件粒度与净决策价值
description: Validation Epoch の Utility を候補 episode、複数 horizon、reason 別、Net Decision Value の業務契約へ収束させる設計。
key: validation-utility-closure-v1-design
---

# Validation Utility Closure V1

## 目的

現行 Validation は forward outcome と T+20 の Utility facts を保持しているが、同一候補を毎日の asset-day として重複集計し、lifecycle の key が symbol 単独である。そのため、Confirmation Cost、Protection Value、Raw Top-3 baseline が候補 episode の意思決定価値を表さない場合がある。

本 Work Item は DecisionPacket と既存の売買意味論を変更せず、Backtest の検証 read model を候補 episode 単位へ修正する。

## 集計単位

record の保存単位は従来どおり asset-day とする。Coverage と horizon completion は asset-day の観測として保持する。一方、次の business KPI は episode 単位で集計する。

- Protection Utility
- Confirmation Cost
- Raw Top-3 / READY counterfactual baseline
- Net Decision Value

episode key は `decision_snapshot_version + universe_id + symbol + strength_episode_start` とする。Strength は既存の Raw Top-3 初回観測 proxy を継続利用し、新しい strength detector は作らない。

同一 episode が複数日にわたり観測されても、business KPI では一度だけ数える。Ready 後の継続観測は coverage には残すが、Confirmation Cost の分母には重ねない。

## Cohort と context

Lifecycle state map と episode key は snapshot version、universe、symbol を含める。異なる cohort の Strength、Breakout、Ready を相互参照しない。

`classification_available == false` の record は decision outcome と lifecycle 起点から除外する。無効 context が先に観測されたことを理由に、有効 context の episode start を前倒ししない。

## Protection Utility

Protection Utility は `raw_candidate && trend_gate_blocked && decision_class == NO_TRADE` の episode を対象にする。ここで `trend_gate_blocked` は既存 production fact `trend_cohesion.gate_passed == false` の ACL projection であり、完全な Decision Gate blocked を意味しない。

各 episode について T+5、T+10、T+20 を独立に集計する。

- complete sample count
- downside count（forward return < 0）
- MAE average / median / P90 / P95
- MFE average
- positive return average
- top-decile missed upside

未完了 horizon は downside として扱わず、分母にも入れない。

Protection は `TREND_GATE_BLOCKED`、`NO_LEADER`、`BREAKOUT_UNCONFIRMED`、`CONFIDENCE_INSUFFICIENT`、`RISK_OVERLAY_ACTIVE` の reason 別にも集計する。複数 reason を持つ record は reason 別ビューでは各 reason に属するが、全体 episode count は重複計上しない。

## Confirmation Cost

各 episode について次を一度だけ計算する。

- Strength → Breakout sessions
- Breakout → Ready sessions
- Strength → Ready sessions
- Strength → Ready return
- Breakout → Ready return
- Strength → Ready maximum move

Ready が未観測の episode は realized confirmation cost には入れず、未完了 lifecycle count として別表示する。zero-fill はしない。

## Net Decision Value

Protection Benefit と Confirmation Cost は同一 episode 粒度で、各 horizon と lifecycle completion の coverage を明示して計算する。比較対象が空、または分母が異なり比較不能な場合は `None` とし、0 と解釈しない。

表示上は以下を明記する。

```text
Protection Benefit
Confirmation Cost
Net Decision Value = Protection Benefit - Confirmation Cost
```

ここでの Benefit と Cost は候補 episode のリターン／MFE／MAE に基づく研究指標であり、portfolio weight、cash、fee、注文約定を含む金額損益ではない。

## Sample maturity と出力

Maturity は総 record 数だけで決めない。少なくとも asset-day coverage、complete horizon episode 数、lifecycle complete episode 数を分離表示する。`USABLE` 判定には有効 episode 数を使い、censored record の水増しを許さない。

Report の名称は `Trend Gate blocked` とし、full Gate fact と誤認させる `Gate blocked` 表記を避ける。

## テスト

次のシナリオを固定する。

1. 異なる cohort の同一 symbol が lifecycle を共有しない。
2. 同一 episode の複数 asset-day が Confirmation Cost と baseline で一度だけ数えられる。
3. T+5/T+10/T+20 の censoring と utility が独立して計算される。
4. blocker reason 別 utility の合計が全体 episode を重複して膨らませない。
5. 比較不能な Benefit/Cost が Net Decision Value を 0 ではなく unavailable にする。


---
author: Ray
title: Validation Epoch V1 設計
description: Radar の Decision Classification を SSOT として Decision Utility と Counterfactual Baseline を測定する設計。
key: validation-epoch-v1-design
---

# Validation Epoch V1 設計

## 目的

Sentinel の既存 Decision が、いつ資本行動を許可し、いつ待機を強制し、その待機がどの程度の downside を避け、どの程度の upside を失わせたかを測定する。

この段階では新しい Market Layer や取引戦略を追加しない。Validation Engine は Decision Quality を評価し、Portfolio P&L や売買シミュレーションは扱わない。

## 不変条件

1. `DecisionClass` は Radar Domain の production fact である。`NO_TRADE`、`PROBE`、`READY` を Backtest / Validation が再推論してはならない。
2. `DecisionPacket` は `decision_class`、安定した `decision_reasons`、`decision_snapshot_version`、`universe_id` を保持する。
3. Decision semantics を変更する threshold または分類 mapping の変更は、同一 cohort の bug fix ではなく新しい Validation Epoch とする。
4. Validation は価格経路を読むが、口座、ポジション、cash、commission、position sizing は作らない。

## データフロー

```text
Radar Decision Pipeline
        ↓
DecisionClass + reason codes + epoch metadata
        ↓
DecisionPacket
        ↓
Backtest ACL（field mapping のみ）
        ↓
ValidationDecisionRecord
        ↓
Outcome Metrics / Counterfactual Comparison / summary.md
```

## Production Contract

`DecisionClass` は `src/features/radar/domain` に置く。reason は自由文ではなく、`NO_LEADER`、`BREADTH_TOO_NARROW`、`TREND_GATE_BLOCKED`、`BREAKOUT_UNCONFIRMED`、`CONFIDENCE_INSUFFICIENT`、`RISK_OVERLAY_ACTIVE` のような安定 code とする。

`decision_snapshot_version` は decision semantics の変更を識別する明示的な version とし、Cargo package version の暗黙的な代用にはしない。`universe_id` は watchlist / universe の変更を識別し、異なる asset pool の集計混在を防ぐ。

分類は既存の Radar pipeline が生成する。Validation 側では `decision_class` と `decision_reasons` をそのままコピーし、gate/action から同じ意味を再構成しない。

## Validation Record

各 decision date と symbol について、次を保存する。

- `date`、`symbol`、`decision_class`、`decision_reasons`
- `decision_snapshot_version`、`universe_id`
- `decision_session_index`、`decision_close`
- `strength_date`、`breakout_date`、`ready_date`
- Strength→Breakout、Breakout→Ready、Strength→Ready の session latency
- T+5 / T+10 / T+20 の forward return、MFE、MAE
- `validation_status`: `PENDING`、`PARTIAL`、`COMPLETE`

`decision_close` は Decision の観測価格であり、執行価格ではない。`simulated_entry_price` はこの Work Item では追加しない。

## Horizon と Censoring

Horizon は自然日ではなく、履歴中の有効 trading session の index で計算する。将来価格が不足する場合、該当 horizon は return `0` として補完しない。

- T+5 のみ到達: `PARTIAL`、T+5 は complete、T+10 / T+20 は pending
- T+20 まで到達: `COMPLETE`
- T+5 未到達: `PENDING`

各 horizon の coverage は、complete なサンプルを分母として明示する。未完了サンプルを平均値の分母へ混ぜない。

## Outcome Metrics

分類ごとに、raw fact と utility summary を分離して出力する。

### Protection

`NO_TRADE` の blocked candidate について、平均 / 中央値 / P90 または P95 の MAE、forward return、下落サンプル数を出力する。

### Opportunity Cost

同じ `NO_TRADE` サンプルについて、positive forward return、MFE、top-decile missed upside を出力する。下落したサンプルだけを選ばない。

### Confirmation Cost

Strength→Breakout、Breakout→Ready、Strength→Ready の session latency と、Strength から Ready までの return / maximum move を出力する。

### Ready Quality

`READY` について T+5 / T+10 / T+20 return、MFE、MAE、hit rate を出力する。

## Counterfactual Baseline

固定 baseline は `Raw Top-3 without Gate` とする。これは既存 simulation の raw ranking を利用し、decision semantics の最適化には使わない。

Report では次を並べて表示する。

- Raw Candidate: forward return、MAE、MFE
- Sentinel READY subset: forward return、MAE、MFE
- 差分: return sacrifice と downside improvement

`Protection Benefit`、`Opportunity Cost`、`Net Decision Value` は構成項目の後に置く。Net Decision Value は摘要であり、単一の万能スコアとして扱わない。

## Sample Maturity

- `< 30`: `INSUFFICIENT`
- `30–99`: `DEVELOPING`
- `>= 100`: `USABLE`

これは統計的有意性の主張ではなく、少数事例から Gate の有効性を断定しないための表示ルールである。

## Report Boundary

第一版の出力先は既存の `backtest/<run>/summary.md` と構造化 validation artifact に限定する。Telegram、audit daily、weekly review、i18n は変更しない。data branch への push は別 Work Item とする。

## Epoch Freeze

Validation Epoch V1 の cohort では、decision semantics を変えない data correctness、wiring、serialization、event ingestion の修正だけを許可する。修正後に同じ入力の classification が変わる場合は、Validation Epoch V2 として別 cohort を開始する。

## 検証

Unit test で分類、reason code、session horizon、censoring、双方向 Protection、baseline 比較を固定する。Integration test で DecisionPacket→Backtest ACL→Validation record の field preservation と summary 出力を確認する。完了判定は `make` の Work Item required checks と Rust quality gate を通過した場合に限る。

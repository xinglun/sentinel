---
author: Ray
title: Legacy 取引日履歴から formal snapshot への一回限り移行設計
description: 旧 DecisionPacket 履歴から取引日 formal snapshot と cycle state を安全かつ冪等に再構築する設計。
key: legacy-formal-snapshot-migration-design
---

# Legacy 取引日履歴から formal snapshot への一回限り移行設計

## 目的

`reports/decision_packet_YYYY-MM-DD.json` または既存の `decision_history.jsonl` に残る legacy 取引日事実から、formal snapshot 層と `observation_history_state.json` を一度だけ再構築する。移行後は通常の snapshot 書き込みと cycle 継続性の経路を使用する。

移行は取引判断を変更しない。移行された履歴は、根拠が不足するため観測専用・安全降格の証拠として扱う。

## 境界

対象は `reports/` の legacy market observation 履歴である。取引注文、position sizing、action matrix、market threshold、Telegram 文言は変更しない。

入力は各日付の `decision_packet_YYYY-MM-DD.json` を優先し、補助的に `decision_history.jsonl` の日付重複を解決する。formal snapshot が既に存在する日付は上書きしない。

## データフロー

```text
legacy packet files / decision_history.jsonl
        |
        v
日付順・重複排除・全件 deserialize
        |
        v
TradingDaySnapshot projection
        |
        +--> reports/snapshots/<stable-cycle>_<market-date>.json
        |
        +--> reports/observation_history_state.json
```

## 変換規則

- `market_date`、`report_date`、`as_of_date` は packet の業務日から設定する。
- `market_state`、`breadth`、`confidence`、`stability`、leader、breakout は packet から明確に導出できる値だけを保存する。
- 取引日履歴だけでは根拠を復元できない項目は空値または `UNAVAILABLE` とする。推測値を補わない。
- 移行 snapshot は `is_valid_trading_day: true`、`source_status: degraded`、`decision_state: NO_TRADE` とする。
- `data_quality.history` は `MIGRATED_LEGACY` とし、通常の健康な履歴と区別する。
- `cycle_id` は最小・最大 legacy 日付を含む安定した migration key から生成し、実行ごとの UUID を使わない。
- `count` は移行された一意な取引日数、`last_market_date` は最大取引日とする。

## 安全性と冪等性

- 入力のいずれかが壊れている場合、移行全体を失敗させ、部分的な formal 履歴を公開しない。
- 同一 snapshot key が存在し、意味が一致する場合は `SameDayRerun` として保持する。
- 同一 key の意味が異なる場合は `SNAPSHOT_CONFLICT` として停止する。
- state の既存 cycle がある場合、migration cycle で上書きしない。
- 移行後に snapshot 数、state count、最大 market date、cycle_id の整合性を検証する。

## CI 運用

Actions は reports 復元後に formal snapshot の有無を確認する。legacy 履歴があり formal snapshot が不足する場合だけ migration を実行する。migration 後は当日 snapshot と state の cycle 一致、次の取引日での追加を既存の freshness gate で検証する。

移行不能な legacy 履歴を検出した場合は、安全のため radar 実行と data branch push を行わず失敗させる。

## 検証

- legacy packet 一件からの snapshot 変換。
- 複数日付の順序、重複排除、stable cycle 生成。
- 既存 formal snapshot を保持する冪等再実行。
- 壊れた packet と snapshot conflict の fail-closed。
- state count と最大 market date の整合性。
- Actions の migration 判定と shell syntax。

## 非対象

- 過去の不足データの推測補完。
- 既存 report の再生成。
- 取引ロジック、注文、ポジション、閾値の変更。
- 外部 data branch の手動修正。CI の一回限り migration 経路で復元する。

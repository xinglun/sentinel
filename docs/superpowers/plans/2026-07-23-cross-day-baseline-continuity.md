---
author: Ray
title: 日次状態基線の連続性修正実装計画
description: 前回有効取引日基線の選択、基線欠損の降格、同日再実行の冪等性を修正する。
key: cross-day-baseline-continuity
---

# 日次状態基線の連続性修正実装計画

目的：T 日レポートが T−1 の有効取引日に永続化された状態だけを比較し、基線を取得できない場合に偽の変化を生成しないようにする。

構成：radar の観測・永続化境界に業務日で基線を選択する純粋関数を追加する。Leader observation と DecisionPacket は日付で重複排除し、時系列順に並べる。状態遷移は既存のドメイン比較を再利用し、パイプラインから日付検証済みの previous packet を渡す。

技術要素：Rust、chrono、serde JSONL、既存の Makefile quality gate。

全体制約：

- Leader 判定、Gate、Execution、Trader、Position Sizing は変更しない。
- 手書きの repository comment と Markdown は日本語で記述する。
- 先に失敗テストを書き、その後に最小限の修正を実装する。

## Task 1：日付基線選択の契約を追加する

対象：`src/features/radar/infrastructure/persistence.rs`、`src/features/radar/domain/leader_persistence.rs` と対応する単体テスト。

- 前回有効取引日の欠損、週末・休日のスキップ、同日置換について失敗テストを書く。
- 対象日から前回有効取引日の記録を選択するインターフェースを追加し、記録がない場合は明示的な unavailable 結果を返す。
- Leader observation は業務日で upsert し、同日再実行で記録を増やさない。

## Task 2：DecisionPacket と transition の基線を修正する

対象：`src/features/radar/infrastructure/persistence.rs`、`src/features/radar/interface/radar_pipeline_runner.rs`、`src/features/radar/domain/transition_log.rs` とパイプラインテスト。

- 乱順 packet と同日重複 packet が誤った previous にならないことを示す失敗テストを書く。
- packet 履歴のロードを業務日で重複排除し、時系列順にする。
- パイプラインは前回有効取引日の packet だけを transition previous にし、基線欠損時は構造化状態を渡す。
- 古い基線によって Breakout 消失イベントが再生成されないことを確認する。

## Task 3：Leader Persistence と 7 日系列の降格を修正する

対象：`src/features/radar/interface/market_interpretation_read_model.rs`、`src/features/radar/domain/observation_timeline.rs` とレポートテスト。

- Leader の連続、実際の Leader 切替、前日スナップショット欠損を失敗テストでカバーする。
- 前日比較フィールドを有効な前回取引日の記録に結び付け、回顧窓内の任意の最新記録は使わない。
- レポート read model に構造化された `BASELINE_UNAVAILABLE` を保持し、偽の切替を生成しない。
- 7 日系列を有効取引日で並べ、重複排除し、実際の観測点を保持する。

## Task 4：検証と Cockpit の収束

- targeted test を実行し、先に失敗し、その後に成功することを確認する。
- Summary の changedFiles、scenarioCoverage、verification、residualRisks、review focus を更新する。
- Contract、scope、fmt、test、clippy と Cockpit required checks を実行する。
- required checks がすべて通過した後にだけ `make ai-finish TASK=cross-day-baseline-continuity` を実行する。

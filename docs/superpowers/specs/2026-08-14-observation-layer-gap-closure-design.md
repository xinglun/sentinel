---
author: Ray
title: Observation Layer 受入れ不足補完設計
description: Observation Layer の表示、永続化、coverage、runtime replay の不足を補完する設計。
key: observation-layer-gap-closure-design
---

# Observation Layer 受入れ不足補完設計

## 目的

前回の受入れ監査で確認された四つの不足を、取引実行経路を凍結したまま補完する。

- Supply Context と RVOL の不可用理由・baseline 情報を観測出力へ残す。
- Breadth の raw 値、up/flat/down/total、universe integrity、classification を構造化保存する。
- 六分類の runtime coverage で、未取得分類を `Unavailable` / `Partial` として明示する。
- Payroll、CPI、PPI の known-event replay を指定日付きの `make` 入口で再現可能にする。

## 境界

Gate、execution、Action Matrix、Trader、Position Sizing、automatic trading、既存の判定閾値は変更しない。新しい外部 provider や汎用イベント日程フレームワークも本 WI の対象外とする。

## 設計

### 観測レポート

价量レポートは、Supply Context が取得できない場合に `Status: UNAVAILABLE` と `Reason: SUPPLY_CONTEXT_MISSING` を出力する。RVOL は値だけでなく `Baseline` と `Baseline Sessions` を primary / secondary ごとに出力する。これは Markdown、HTML、既存の report unit test で同じ契約を確認する。

### Breadth 保存

`TradingDaySnapshot` に optional/default 付きの raw/counts/universe フィールドを追加し、旧 JSON を壊さない。Observation Timeline には raw breadth と classification score を別フィールドとして保持し、既存の分類スコアを raw 値として扱わない。

### Coverage

runtime coverage の builder は六分類すべてを初期化し、provider の入力がない分類は `Unavailable` のまま保持する。coverage が不完全な場合の read model は「利用可能な source から高情報量イベントを確認できない」と表現し、イベントなしと断定しない。

### Replay

履歴日を引数として受け取る既存の CLI/pipeline 経路を `make ai-observation-replay` に固定し、三日分の report を生成する。取得失敗時も known event、lifecycle、actual unavailable、coverage status を確認できる形にする。実データがない事実を補完値で埋めない。

## 検証

各表示・保存契約に回帰テストを追加し、`make fmt-check`、`make test`、`make clippy`、AI Cockpit checks、三日分 replay を実行する。Summary には code、test、report、data persistence、Make guard、runtime evidence の実績と残余リスクを分離して記録する。

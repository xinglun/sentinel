---
author: Ray
title: Signal Context 時間整合 実装計画
description: Event と Market Observation の temporal binding を互換優先で実装し検証する計画。
key: signal-context-temporal-alignment-plan
---

# Signal Context 時間整合 実装計画

## 概要

`MarketReaction` を互換維持した MarketObservation read model として拡張し、Event の公開・受理時刻、report run 時刻、明示的 `TemporalBinding` を Signal Context の canonical projection に追加する。実装は observation-only に限定し、既存の日次 report identity と取引判断境界を変更しない。

## 実装方針

1. `src/features/research/interface/macro_event_observation.rs` に `event_id`、`accepted_at`、observation provenance、`TemporalBinding`、RFC3339 の fail-closed 判定を追加する。`MarketReaction` の型と既存 JSON fields は残し、`MarketObservation` alias を新しい語彙として公開する。
2. provider と ACL の内部 read model に同じ identifiers/provenance を追加し、FRED と Finnhub の生成値を deterministic にする。provider facade には `report_run_at` を渡し、受理時刻の visibility を report context に固定する。
3. `SignalContextV1` と coverage builder に temporal context、observation window、bindings を追加する。runtime macro、external fixture、corporate merge の各 projection が同一 resolver を通り、renderer は binding の結果だけを読む。
4. pipeline の report run 時刻を macro context と external context に渡す。existing `market_date` filtering は互換の coarse filter として残すが、event と observation の eligibility は timestamp の比較結果で決める。
5. report read model は observation timestamp、session、venue、instrument を表示し、eligible binding 以外を Event 後の reaction として列挙しない。causal attribution の断定語と decision/action 系の read model には変更を入れない。
6. Jordan fixture、invalid/missing publication timestamp、late acceptance、provenance metadata、legacy deserialization を focused test と consistency script で検証する。

## 変更単位

### Step 1: failing tests

- interface の unit test で `source_published_at <= observed_at`、strictly-before observation、invalid/missing timestamp の false、`accepted_at > report_run_at` の visibility を固定する。
- coverage の test で Jordan の core observation が false、overnight SPY/NQ/Brent が true、bindings が read model に保持されることを先に期待する。
- fixture loader と consistency script の test/fixture で新 fields、observation-only boundary、fail-closed を期待する。

### Step 2: domain/interface read model

- `MacroSignalContextEvent`、`MarketReaction`、`MacroSignalContextReadModel` に temporal fields を追加する。
- `SignalContextItem` に stable event identity と accepted timestamp を追加し、legacy item は default を使って読み込む。
- `SignalContextV1` に `report_run_at`、`observation_window_start/end`、`temporal_bindings` を追加し、Default と serde compatibility を更新する。

### Step 3: provider/ACL wiring

- `MacroSignalContextProviderEvent` と `ProviderMarketReaction` に ids、accepted/provenance fields を追加する。
- provider event/reaction の identifiers は source、source URL、published/observed timestamp、instrument を入力に deterministic に生成する。
- ACL mapping は全 temporal fields を lossless に写像する。

### Step 4: resolver and pipeline

- coverage builder に `SignalContextTemporalContext` を渡し、可視 Event のみを対象に `TemporalBinding` を生成する。
- timestamp parse failure は binding を作成しても `temporal_eligible=false` とし、eligible reaction の集合から除外する。
- pipeline の既存 `report_run_at` を macro provider と Signal Context external loader に渡す。
- window は valid observed timestamps の最小値から report run までを read model に保存する。

### Step 5: presentation and guard

- market reaction formatter は binding で許可された observation の時刻/provenance を出力する。
- “caused” ではなく公開後の同時系列 observation であることを示す既存の弱い表現を維持する。
- consistency script は event publication と observation timestamp を RFC3339 として検証し、eligible binding の不整合をエラーにする。

### Step 6: verification

- `make fmt-check`
- `make test`
- `make clippy`
- `make check-signal-context-consistency`
- `make quality`

全チェックを `ai-cockpit verify` からも実行し、Contract の scenarioCoverage と Summary に実行結果、変更パス、残余 risk、未確認項目を記録する。provider 実 API が不要な fixture/unit test を主証拠とし、network availability を temporal invariant の証拠にしない。

## 受入条件との対応

| 受入条件 | 実装・検証場所 |
| --- | --- |
| Event identity、publication/acceptance、fail-closed | macro event interface、coverage unit tests |
| MarketReaction 互換 + MarketObservation provenance | research interface/provider/ACL、fixture |
| explicit TemporalBinding | SignalContextV1 と coverage resolver |
| accepted_at visibility | pipeline/read-model focused test |
| Jordan regression | `tests/fixtures/signal_context/2026-09-09-temporal-alignment.json` と coverage test |
| observation-only boundary | SignalContextV1 defaults、consistency script、既存 regression |
| Japanese docs/comments と make checks | design/plan、全 required verification |

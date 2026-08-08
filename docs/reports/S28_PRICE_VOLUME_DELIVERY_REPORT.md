---
author: Ray
title: S-28 Price-Volume Structure 交付報告
description: 価量構造の観測専用機能、受入証拠、統制、およびリリース境界を要約する。
key: s28-price-volume-delivery-report
---

# S-28 Price-Volume Structure 交付報告

## 交付範囲

S-28 は、価格方向に加えて出来高参加、価格位置、供給イベント文脈を読み、価格変化の参加品質と供給吸収を説明可能に観測する。構造は `ACCUMULATION`、`HEALTHY_ADVANCE`、`EXHAUSTED_ADVANCE`、`DISTRIBUTION`、`NEUTRAL`、`UNAVAILABLE` の六つである。RVOL、価格位置、ローソク足、ギャップ、供給イベント、継続性、出来高データ品質を同じ観測文脈で扱う。

## 安全境界

本機能は Observation only である。`decision_weight = 0%`、`trade_signal = false`、および Gate、Execution、Trader、Action Matrix、Position Sizing への effect はすべて `none` とする。構造観測は方向予測や売買指示ではない。需要主体は確認できず、仮説と事実を混同しない。

## 受入証拠

- SpaceX 型では、高 confidence の増加供給イベント、RVOL 拡大、限定的な下落、新安値なしが継続する場合に、`ACCUMULATION` と Supply Absorption `ACTIVE` を観測する。これは供給吸収の観測であり、需要主体の確定ではない。
- Microsoft 型では、価格上昇、RVOL 低下、個別銘柄の過熱状態が重なる場合に、`EXHAUSTED_ADVANCE` と Participation `WEAKENING` を観測する。短期の横ばい又は調整による再蓄積の可能性は記述できるが、方向転換を断定しない。
- 不十分な OHLCV、部分的な出来高、429、corporate action、split adjustment 異常、平日連続性の欠損は、補完せず `UNAVAILABLE` 又は品質低下として扱う。取引日連続性は米国祝日判定を導入せず、月曜日から金曜日を基準にする。

## Governance

機能受入の詳細証拠は archived S-28-16 Summary に保持する。統制とリリース証拠は PR #15 の archive ownership repair と PR #14 の protected release sync に保持する。本報告は証拠の索引であり、CI retry 時系列、account switching、command の全出力、archive repair の逐次作業は重複記録しない。

## Release status

PR #14 merge 時点で、S-28 runtime content は `develop` から `main` へ同期され、内容一致が検証された。本報告とその設計文書は後続の documentation change であり、同じ protected PR 手順により別途 `main` へ同期する release boundary とする。

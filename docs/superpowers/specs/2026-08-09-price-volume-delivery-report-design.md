---
author: Ray
title: Price-Volume Structure 圧縮交付報告設計
description: S-28 の主報告に残す事実、除外する詳細証跡、および review 境界を定義する。
key: price-volume-delivery-report-design
---

# Price-Volume Structure 圧縮交付報告設計

## 目的

S-28 Price-Volume Structure の交付状態を、一読で確認できる主報告へ圧縮する。主報告は意思決定のための要約であり、実装の逐次 log や Work Item の代替ではない。

## 主報告の構成

主報告は次の五節だけを持つ。

1. 交付範囲: 六つの structure、relative volume、price location、Supply Event Context、persistence、volume data quality。
2. 安全境界: Observation only、`decision_weight = 0%`、`trade_signal = false`、Gate / Execution / Trader / Action Matrix / Position Sizing への effect は none。
3. 受入証拠: SpaceX 型の supply absorption、Microsoft 型の exhausted advance、欠損 / 429 / 短履歴の fail-closed quality。
4. Governance: Work Item archive、CI required checks、branch protection、および source of truth への参照。
5. Release status: `develop` から `main` への同期、両 branch の content equality、local `develop` の clean state。

## 表現境界

SpaceX 型は「供給が吸収されている可能性を観測した」とだけ記録し、institutional buying confirmed または buy を書かない。Microsoft 型は participation weakening と短期の consolidation / pullback probability を観測として記録し、sell immediately、top confirmed、crash expected を書かない。

## 除外対象

主報告には CI retry の時系列、account switching、各 command の全出力、archive ownership repair の逐次作業を含めない。これらは archive Work Item、Pull Request、Git history に残し、主報告から参照する。

## 証拠と検証

機能 boundary と受入 scenario は S-28-16 closure evidence を正とする。release status は Pull Request #14 と #15、および remote `main` / `develop` comparison を正とする。主報告は source of truth を複製せず、必要時に追跡できる参照を保つ。

## 次の Work Item

この設計が user review で承認された後、別 Work Item で `docs/reports/` に圧縮主報告を作成する。report 本文の変更はこの設計の五節と表現境界を超えない。

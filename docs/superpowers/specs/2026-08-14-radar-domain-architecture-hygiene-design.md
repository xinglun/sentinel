---
author: Ray
title: Radar Domain 構造減圧設計
description: Price-Volume と Leadership の behavior-preserving な内部 module 分割方針。
key: radar-domain-architecture-hygiene-2026-08-14
---

# Radar Domain 構造減圧設計

## 目的

Validation Epoch の比較可能性を守るため、判断ロジックを追加せず、Radar Domain の高変化領域だけを責務別に分割する。対象は Price-Volume と Leadership であり、feature topology の再設計や domain 全体の移設は行わない。

## 設計

既存の `price_volume_structure` module path は維持し、内部を `eligibility`、`baseline`、`classification`、`lifecycle` に分割する。型と関数は親 module から必要な範囲だけ再 export し、既存 consumer の import path と可視性を変えない。

既存の `leader_persistence` module path は維持し、内部を `snapshot`、`persistence`、`absence`、`transition` に分割する。snapshot の型、persistence の orchestration、absence の状態判定、transition の切替判定を分離するが、計算式と結果型は変更しない。

## 境界

子 module は同一 domain 内の型、shared value、設定型だけに依存し、application、interface、infrastructure へ依存しない。DecisionPacket、DecisionClass、Gate、snapshot version、表示、i18n、永続化、backtest は対象外とする。

## 検証

既存の domain、integration、snapshot 関連テストを通し、`make check-architecture-all` で依存方向を確認する。さらに親 module の旧 import path がコンパイルされることと、子 module に上位 layer import が存在しないことを boundary test で固定する。差分は構造変更に限定し、意味変更を含めない。

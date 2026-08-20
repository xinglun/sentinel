---
author: Ray
title: 日報観察語彙と候補資格境界修正計画
description: Trend Cohesion、Strategic evidence、候補資格、Breakout 表示と Reason 句読点を整合する実装計画。
key: observation-semantic-guard-plan
---

# 日報観察語彙と候補資格境界修正計画

## 目的

ユーザー提供の Conditional FAIL で残った P1/P2 の表示意味を、既存の Observation-only 境界を保ったまま修正する。

## 実装範囲

1. Trend Cohesion の label と transition evidence を Mainline / Leadership から分離する。
2. Strategic Background の readiness を長期構造証拠として明示し、実行指令と解釈されない文言にする。
3. 相対順位と候補資格を分離し、非保有かつ新增資格のある資産だけを候補表示する。
4. Breakout の setup strength/quality と confirmation status を分離して表示する。
5. RS Diffusion Reason の末尾句読点を delivery body 間で正規化する。
6. zh/en/ja の presentation、report、snapshot と回帰テストを更新する。

## 境界

Gate、Execution、Trader、Action Matrix、Position Sizing、取引閾値、データ branch、週次保存形式は変更しない。候補資格は display/read model の表示判定として扱い、取引資格や注文生成へ接続しない。

## 検証

failing-first の unit/UI regression を先に追加し、`make fmt-check`、`make test`、`make clippy`、Cockpit、reference impact、architecture、`make quality`、`make ai-finish` の順で確認する。

---
author: Ray
title: マクロ重力コンテキスト
description: 債券市場と信用環境を表示層の支線情報として扱うための設計メモ。
key: macro-gravity-context
---

# マクロ重力コンテキスト

Macro Gravity は、債券市場、実質金利、信用環境、流動性を「市場の重力」として観測する表示専用レイヤーである。

## 目的

- AI / Mega-cap の構造証拠と、株価の時間コストを分離して理解する。
- 良い会社や良いトレンドが、割引率上昇により横ばいまたは下落する状況を説明する。
- リスクがトレンド崩壊なのか、評価倍率の圧縮なのかを区別する。

## 非目的

- Gate へ接続しない。
- Engine、trend_cohesion、execution、action_matrix、trader_agent へ接続しない。
- 債券市場から売買指示を生成しない。
- 金利上昇を自動的な弱気判断に変換しない。

## 設定例

```toml
[macro_gravity]
rate_pressure = "RISING"
real_yield_pressure = "TIGHT"
yield_curve = "FLAT"
credit_stress = "NORMAL"
liquidity = "NEUTRAL"
growth_valuation_impact = "COMPRESSING"
note = "債券市場は AI / Mega-cap の構造判断ではなく、割引率と時間コストの重力として観測する。"
```

`note` は内部メモとして扱う。Telegram、Markdown、`daily-calibration`、週次自動レビューには直接表示しない。自由記述をそのまま表示すると、単一言語レポートに別言語の設定文が混入するためである。

## 表示先

- `daily-calibration` の「マクロ重力校正」セクション。
- Telegram / Markdown の「戦略文脈」内。

表示は列挙値と固定辞書に限定する。自由記述 note は表示しない。

## 境界

Macro Gravity は、構造トレンドの否定ではない。割引率、流動性、信用環境を説明するための補助情報であり、取引判断そのものではない。

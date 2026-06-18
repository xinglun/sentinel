---
author: Ray
title: Valuation Gravity Layer
description: 外部評価コンセンサスから価格と価値アンカーの乖離を粗粒度で観測する表示専用レイヤーの仕様。
key: valuation-gravity-layer
---

# Valuation Gravity Layer

## 目的

Valuation Gravity Layer は、現在価格が外部の価値アンカーからどの程度乖離しているかを表現する。価格予測、売買判断、精密な fair value 算出は行わない。

既存の Macro Gravity は金利、信用、流動性を扱う。Valuation Gravity は銘柄別の価格対価値を扱い、両者を混同しない。

## 状態

Gravity は次の 6 値だけを持つ。

- `Deep Undervalued`
- `Undervalued`
- `Fair`
- `Slightly Expensive`
- `Expensive`
- `Very Expensive`

`Unknown` は Gravity 状態として定義しない。外部証拠が不足する場合は source health を `Unavailable` とし、Gravity 自体を形成しない。これは推測による分類を避けるためのデータ品質表現であり、`Unknown` 状態の追加ではない。

Confidence は `Very High`、`High`、`Medium`、`Low`、`Very Low` とする。Source は `Analyst Consensus`、`Market Multiple`、`Manual Override`、`Hybrid` を表現できるが、第一段階で自動生成するのは `Analyst Consensus` と `Market Multiple` だけである。

各銘柄は Gravity とは別に source health、evidence count、data quality reason を保持して表示する。data quality reason は credential 未設定、entitlement 不足、provider failure、不正 response、証拠不足、fallback 種別、履歴 snapshot の欠落・読み取り失敗を区別する。

## 外部 source と fallback

第一段階は既存 Finnhub credential を使用し、次の順で取得する。

1. price target consensus と current quote。
2. current P/E と基準日以前の直近 5 年の annual P/E median。
3. recommendation trends。

上位 source が entitlement、coverage、response validation の理由で利用できない場合だけ次へ降格する。price target は analyst count、freshness、target range dispersion から Confidence を決める。historical multiple と recommendation fallback は粗い観測であるため Confidence を `Low` または `Very Low` に制限する。

annual P/E は upstream 配列順を信用しない。`period` を日付として解釈し、基準日より未来の record を除外し、日付降順へ正規化してから直近 5 件を選ぶ。

annual P/E の sample 数が偶数の場合、median は中央 2 値の平均とする。

現在日の外部取得は最大 10 銘柄を並行処理し、collection 全体を 10 秒の budget に制限する。budget 内に完了しない銘柄は source unavailable として report を継続し、Radar の Markdown 保存と Telegram 通知を外部 source 障害で長時間 block しない。

## Application と port

Application の `ValuationGravityUseCase` が現在日 / 過去日分岐、source fallback、snapshot load / save を調停する。外部接点は `ValuationGravitySourcePort` と `ValuationGravitySnapshotRepository` で表現する。

Infrastructure は Finnhub source port と filesystem repository を実装する。ACL facade は実装を組み立てて use case を呼び出すだけとし、日付分岐、fallback、永続化 error policy を持たない。

Infrastructure は `reqwest::Error`、request URL、credential、provider response body を Application へ渡さない。Application が snapshot に保存する取得失敗 message は typed data quality reason に対応する固定文だけとする。

## 分類

price target と market multiple は、現在値を外部アンカーで割った relative ratio を使う。

| Relative ratio | Gravity |
|---:|---|
| `<= 0.70` | `Deep Undervalued` |
| `<= 0.90` | `Undervalued` |
| `< 1.10` | `Fair` |
| `< 1.25` | `Slightly Expensive` |
| `< 1.50` | `Expensive` |
| `>= 1.50` | `Very Expensive` |

recommendation fallback は `Strong Buy = +2`、`Buy = +1`、`Hold = 0`、`Sell = -1`、`Strong Sell = -2` の加重平均を同じ 6 段階へ圧縮する。この score は外部 analyst 分布の圧縮であり、独自 fair value model ではない。

## 表示と保存

次へ表示する。

- 主 Radar の Markdown / Telegram appendix。
- `daily-calibration` CLI report。

label と Source 値は zh-cn / en-us / ja-jp ごとに一貫して翻訳し、単一言語 report に別言語の Source 値を混在させない。

`audit_daily` 単体と weekly review には追加しない。構造化 snapshot は `output.save_to` 配下の `valuation_gravity_latest.json` と `valuation_gravity_YYYY-MM-DD.json` に保存する。JSONL、weekly metrics、data branch には保存しない。

過去日を再生する場合は同日の保存済み snapshot だけを使用する。保存済み snapshot がなければ現在の API response で過去を埋めず、source unavailable とする。

snapshot の保存・読み取り error は握り潰さない。report は snapshot persistence health と typed reason を表示し、必要な場合は filesystem error detail を監査情報として残す。Gravity の取得に成功しても保存に失敗した場合は、画面上で保存失敗を判別可能にする。

## 責務境界

Valuation Gravity と Trend は独立して同時表示する。Valuation Gravity は次へ接続しない。

- `READY` / `EXECUTE` / Gate。
- action matrix / trader / Position Sizing。
- Trend / Breakout。
- Capital Absorption / Gray Rhino。

DCF、Monte Carlo、独自の複雑な valuation model、精密 fair value、売買 signal、推奨 action は生成しない。

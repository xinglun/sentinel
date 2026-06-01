---
author: Ray
title: Research Attention Daily
description: 認知收益と注意コストを日次旁路レポートとして出力するための仕様。
key: research-attention-daily
---

# Research Attention Daily

Research Attention Daily は、銘柄を売買対象として評価するのではなく、日々の認知リソース配分を確認するための旁路レポートです。

このレポートは Market Radar の主判断とは分離します。`Gate`、`Engine`、`trend_cohesion`、`action_matrix`、`trader_agent`、自動売買には接続しません。

## 目的

日次で次の問いに答えます。

- どの観測対象が高い認知増分を持つか。
- どの観測対象が認知飽和に近いか。
- どの観測対象が高い注意コストを要求しているか。
- どの観測対象が高い情報密度を維持しているか。

## 境界

次の読み替えは禁止します。

- 認知收益が低いことは、銘柄の否定ではありません。
- 注意コストが高いことは、研究価値なしを意味しません。
- 情報密度が低いことは、保有価値なしを意味しません。
- Research Attention は売買、加減倉、清算、トップ予測を行いません。

## 設定

`config.toml` に動的な銘柄テーブルを追加します。

```toml
[research_attention.TSLA]
cognitive_yield = "HIGH"
attention_cost = "HIGH"
information_density = "EXPANDING"
reason = "Physical AI / FSD / 製造自動化は高変化率を維持。"

[research_attention.GOOG]
cognitive_yield = "MEDIUM"
attention_cost = "LOW"
information_density = "STABLE"
reason = "AI 収益化の理解が進み、辺際的な情報増分は低下。"
```

設定変更後は、日次レポートを走らせる前に次の軽量 command で `config.toml` の構文と runtime validation を確認します。

```bash
make config-check
```

`config-check` は `config.toml` を読み込むだけであり、Telegram、report、evidence、data branch、Gate、execution には接続しません。

## 標準ローカライズ

Research Attention / Asset Thesis の本文は `config.toml` では日本語の base text を SSOT とし、標準 catalog に登録済みの銘柄は `output.language` に応じて中国語・英語へ投影します。

`SPCX` は標準 catalog に登録済みです。運用者は `reason_zh`、`reason_en`、`thesis_zh`、`thesis_en` を `config.toml` に重複して書く必要はありません。日本語 base text を変更すると catalog の一致条件から外れるため、文面を変更する場合は catalog と regression test を同時に更新します。

## 値

`cognitive_yield` は次を使います。

- `HIGH`
- `MEDIUM`
- `LOW`
- `DEGRADING`

`attention_cost` は次を使います。

- `LOW`
- `MODERATE`
- `HIGH`
- `DRAINING`

`information_density` は次を使います。

- `EXPANDING`
- `ACTIVE`
- `STABLE`
- `SATURATED`

## コマンド

標準出力だけに出す場合です。

```bash
make research-attention
```

Telegram に単独の旁路メッセージとして送る場合です。

```bash
make research-attention RESEARCH_ATTENTION_ARGS="--notify"
```

`--notify` は Telegram 設定が有効で、token と chat id が揃っている場合だけ送信します。主レポートには混入しません。

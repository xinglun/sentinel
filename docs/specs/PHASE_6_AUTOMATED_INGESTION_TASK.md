---
author: Ray
title: Phase 6 自動収集の本格稼働タスク
description: 外部 API 連携とニュース/IR 自動解析プロトタイプを本格運用へ進めるための開発タスク定義。
key: phase-6-automated-ingestion-task
---

# Phase 6 自動収集の本格稼働タスク

## 目的

Phase 5-C では、`EvidenceStore`、`ingest-evidence`、`AutomatedEvidenceRecord` により、実体的証拠を構造化して保存し、Trend Recognition の確信度へ反映できる基盤を整備した。

Phase 6 では、この基盤の上に外部 API 連携とニュース/IR 自動解析プロトタイプを追加し、手動入力に依存しない実体的証拠の収集を開始する。

目的は、システムが以下を自動で行える状態にすることである。

- 公式 IR / 決算ページ / ニュース API から候補ソースを取得する。
- 取得した情報を保守的に解析し、`AutomatedEvidenceRecord` へ正規化する。
- `EvidenceStore` に保存し、既存の dedupe / decay / report パイプラインへ接続する。
- 収集失敗や解析失敗を監査可能な形で残す。

## 非目標

- Gate 判定を変更しない。
- `NO TRADE / READY` の結論を外部ニュースで反転しない。
- ニュース本文全文を永続化しない。
- LLM の自由回答をそのまま証拠として採用しない。
- 自動売買や自動発注には接続しない。

## 設計原則

1. **Evidence Layer に閉じる**  
   外部 API から得た証拠は `AutomatedEvidenceRecord` に変換し、Decision Layer へ逆流させない。

2. **公式ソースを優先する**  
   公式 IR、決算リリース、企業発表はニュースより高い信頼度を持つ。

3. **ニュースは補助証拠に限定する**  
   ニュース単独で `capex_payoff` や `earnings_validation` を強く立てない。公式ソースまたは価格フォロースルーとの一致で補強する。

4. **本文ではなく構造化結果を保存する**  
   保存対象は URL、タイトル、短い抽出説明、証拠種別、confidence、event_date、dedupe_key とする。

5. **失敗しても日次レポートを止めない**  
   外部 API 障害は非致命エラーとして扱い、既存の価格ベース判断と手動証拠だけで処理を継続する。

## 全体フロー

```text
Source Discovery
  -> Source Fetch
  -> Candidate Extraction
  -> Evidence Classification
  -> AutomatedEvidenceRecord
  -> EvidenceStore
  -> Engine
  -> Report / audit_daily
```

## 対象ソース

### P0: 手動 URL 取り込み

最初は完全自動検索ではなく、URL 指定型の取り込みから開始する。

候補:

- 企業 IR ページ
- 決算リリース
- 公式ブログ
- SEC / EDGAR の提出書類 URL
- ニュース記事 URL

目的:

- 解析器と保存形式を安定させる。
- API 障害や検索品質に依存せず、fixture 化しやすい入力を作る。

### P1: 外部 API 連携

候補:

- News API
- Finnhub
- Alpha Vantage News
- SEC companyfacts / submissions
- 公式 IR RSS が存在する場合は RSS

要件:

- API key は `config.toml` または環境変数から読み込む。
- CI では外部 API に接続しない。
- テストは fixture JSON / HTML で行う。
- rate limit に達した場合は失敗として記録し、処理を継続する。

### P2: ニュース/IR 自動解析プロトタイプ

解析対象:

- AI 投資回収
- クラウド成長
- データセンター CAPEX
- 半導体受注
- ガイダンス上方修正
- 供給制約
- 価格フォロースルーとの整合

出力:

- `EvidenceType::CapexPayoff`
- `EvidenceType::EarningsValidation`
- `EvidenceType::OrderVisibility`
- `EvidenceType::FollowThrough`

## 新規 CLI 案

### URL 取り込み

```bash
cargo run -- ingest-evidence-url \
  --symbol GOOG \
  --source official_ir \
  --url https://example.com/earnings-release
```

### 外部 API 収集

```bash
cargo run -- collect-evidence \
  --symbols GOOG,MSFT,NVDA \
  --days 3
```

### dry-run

```bash
cargo run -- collect-evidence \
  --symbols GOOG,MSFT,NVDA \
  --days 3 \
  --dry-run
```

`dry-run` は `EvidenceStore` に書き込まず、抽出候補だけを表示する。

## 設定案

```toml
[evidence_ingestion]
enabled = false
lookback_days = 3
max_records_per_symbol = 5
default_confidence_news = 0.45
default_confidence_official_ir = 0.85

[evidence_ingestion.sources]
official_ir = true
news = false
sec = false
rss = false
```

デフォルトは `enabled = false` とする。  
本格稼働は設定で明示的に有効化した場合のみ行う。

## データモデル補強

Phase 5-C の `AutomatedEvidenceRecord` に対して、Phase 6 では以下の項目を利用または追加する。

- `symbol`
- `event_date`
- `source_url`
- `dedupe_key`
- `source`
- `evidence_type`
- `confidence`
- `description`

追加候補:

- `source_title`
- `source_published_at`
- `extractor_version`
- `raw_source_hash`

`raw_source_hash` は本文保存を避けながら、同一ソースの再解析検知に使う。

## 分類ルール

### CapexPayoff

成立条件:

- AI / cloud / data center 投資が売上、利益、利用率、顧客需要の改善として確認できる。
- 公式 IR または決算リリースで確認できる。

禁止:

- 株価上昇だけで `CapexPayoff` にしない。
- ニュース見出しだけで高 confidence にしない。

### EarningsValidation

成立条件:

- 決算が予想を上回る。
- ガイダンスが維持または上方修正される。
- 成長の理由が対象テーマと接続している。

禁止:

- 一時的なコスト削減だけを成長証拠にしない。

### OrderVisibility

成立条件:

- 受注残、供給契約、需要見通し、納期、データセンター投資計画が確認できる。

禁止:

- アナリスト推測だけで強い証拠にしない。

## 実装タスク

### Task 1: SourceFetcher trait

- `src/features/evidence/application/evidence_ingestion.rs` を追加する。
- `SourceFetcher` trait を定義する。
- `fetch(url)` と `search(symbol, lookback_days)` を分離する。

受け入れ条件:

- 外部 API なしで fixture fetcher を差し替えられる。
- CI でネットワーク不要のテストができる。

### Task 2: URL 取り込み CLI

- `ingest-evidence-url` を追加する。
- URL、symbol、source type を検証する。
- 取得失敗時は非 0 終了し、既存 evidence を壊さない。

受け入れ条件:

- 有効な fixture URL から `AutomatedEvidenceRecord` を生成できる。
- 無効 URL は明示的に失敗する。
- `--dry-run` では保存しない。

### Task 3: EvidenceExtractor

- 取得済みテキストまたはメタデータから証拠候補を抽出する。
- 最初はルールベースでよい。
- 抽出結果に `extractor_version` を含める。

受け入れ条件:

- GOOG 決算 fixture から `CapexPayoff` と `EarningsValidation` を抽出できる。
- MSFT データセンター fixture から `CapexPayoff` を抽出できる。
- NVDA 受注/供給 fixture から `OrderVisibility` を抽出できる。
- 無関係ニュースからは証拠を出さない。

### Task 4: News API adapter

- API key を設定から読む。
- 取得結果を `SourceDocument` に正規化する。
- rate limit / timeout / empty result を区別してログに残す。

受け入れ条件:

- API 障害時に日次パイプラインを止めない。
- 同一 URL は dedupe される。
- CI では fixture adapter のみ使用する。

### Task 5: Official IR adapter

- URL 指定型を先に作る。
- 可能なら RSS / IR feed へ拡張する。
- PDF は初期段階では対象外またはテキスト抽出 fixture のみに限定する。

受け入れ条件:

- 公式 IR はニュースより高い default confidence を持つ。
- HTML 構造が崩れても失敗として扱い、推測で証拠化しない。

### Task 6: EvidenceStore 接続

- 抽出された `AutomatedEvidenceRecord` を `EvidenceStore` に保存する。
- `dedupe_key` は `source + symbol + evidence_type + event_date + source_url/hash` を基本にする。
- 同一バッチ内重複と既存ファイル重複の両方を防ぐ。

受け入れ条件:

- 再実行しても同じ証拠が増殖しない。
- 同じ銘柄の別イベントは保存できる。

### Task 7: Report / audit_daily 表示

- 自動収集された証拠の件数を表示する。
- 主証拠、source、confidence、event_date を表示する。
- 失敗件数を audit に表示する。

受け入れ条件:

- Telegram で `NO TRADE` と矛盾しない場所に表示される。
- Markdown と Telegram の三言語表示が崩れない。
- `audit_daily` は証拠の増減を説明できる。

## テスト要件

### Unit Tests

- URL / source type / confidence / date validation。
- `dedupe_key` 生成。
- `EvidenceExtractor` の分類。
- confidence の上限・下限。

### Integration Tests

- fixture source から `EvidenceStore` まで保存されること。
- `--dry-run` が保存しないこと。
- API failure が日次レポートを止めないこと。
- 既存 `ingest-evidence` と共存すること。

### Report Tests

- 自動収集証拠が Telegram に表示されること。
- 自動収集証拠が Markdown に表示されること。
- 三言語で表示キーが欠落しないこと。

## 段階的リリース

### Step 1

`SourceDocument`、`SourceFetcher`、`EvidenceExtractor` の trait と fixture 実装だけを作る。

### Step 2

`ingest-evidence-url --dry-run` を追加し、保存せず抽出結果だけ確認する。

### Step 3

`ingest-evidence-url` の保存を有効化し、`EvidenceStore` に接続する。

### Step 4

News API adapter を追加する。ただし本番設定では `enabled = false` のままにする。

### Step 5

日次ジョブに opt-in で組み込む。

## 完了条件

- 外部 API 無効時に既存システムが完全に動作する。
- fixture ベースで自動解析を再現できる。
- 証拠保存は dedupe される。
- 自動収集証拠はレポートで監査可能である。
- `cargo fmt --all -- --check` が通る。
- `cargo test` が通る。
- `cargo clippy --all-targets -- -D warnings` が通る。

## 最初に実装する最小単位

最初の実装は News API ではなく、以下に限定する。

1. `SourceDocument` と `EvidenceExtractor` の fixture 実装。
2. `ingest-evidence-url --dry-run`。
3. GOOG / MSFT / NVDA の静的 fixture。
4. 抽出結果を `AutomatedEvidenceRecord` に変換するテスト。

この順序なら外部依存なしで解析品質を検証でき、既存の Evidence Layer を汚染せずに Phase 6 を開始できる。

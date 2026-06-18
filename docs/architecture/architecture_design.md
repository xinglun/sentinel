---
author: Ray
title: Stock Sentinel アーキテクチャおよび詳細設計
description: Sentinel の現行 feature-first 構造、DDD 境界、データフローを説明する。
key: docs-architecture-architecture-design
tags: [architecture, design, technical, rust]
keywords: [data-structures, concurrency, error-handling, logic]
---

# Stock Sentinel アーキテクチャおよび詳細設計

## 現行アーキテクチャとの関係

この文書は現行実装の構造とデータフローを説明します。DDD / Clean Architecture 境界、feature-first 構造、依存方向、architecture checker の判断は `docs/specs/DDD_CLEAN_ARCHITECTURE.md` と `.ai/architecture/feature_acl.yaml` を SSOT とします。

本ドキュメントでは、PRDにおけるシステム要件を技術的な観点から詳細化します。主にコアデータ構造の設計と境界条件・例外処理のマトリックスを扱い、Rustによるコーディングの明確な指針を提供します。

## 1. コアデータ構造の設計 (Data Structures)

ロジックの明確化とテストの容易性を確保するため、システムを「構成レイヤー」、「基礎データレイヤー」、および「ビジネススナップショットレイヤー」に分離します。

### 1.1 構成マッピング (Config Schema)
`config.toml` のシリアライズ構造に対応します。
```rust
struct AppConfig {
    output: OutputConfig,
    rules: RulesConfig,
    watchlist: Vec<WatchlistEntry>,
}

struct OutputConfig {
    timezone: String, // 例: "Asia/Tokyo"
    format: String,   // "markdown", "json"
    save_to: String,  // "./reports"
    compact_transition_evidence_in_no_trade: bool, // NO TRADE 時、フロントエンドで状態遷移の証拠を圧縮するかどうか
}

struct RulesConfig {
    trend: TrendConfig,
    deviation_bands: Vec<(String, f64)>, // 内部的には f64 の降順にソートされた配列として保持
    actions: std::collections::HashMap<String, String>,
}

// (BearModeConfig removed in v0.1.0)

struct WatchlistEntry {
    symbol: String,
    market: String,
    owner_ma_days: usize,
    leash_ma_days: usize,
    deviation_basis: DeviationBasis, // 列挙型: Owner | Leash
    enable: bool,
}
```

### 3. モジュール構成とデータフロー (Module Data Flow)

本システムは `src/features/<context>/{domain,application,infrastructure,acl,interface}` を基本単位とする feature-first 構造です。

1. `radar` bounded context は市場データを ACL 経由で取得し、Domain / Application で状態、Gate、アクションを評価します。
2. `research` bounded context は外部証拠を収集し、監査・観測用の report を生成します。Valuation Gravity はこの context に属する display-only の観測レイヤーです。
3. `backtest` bounded context は履歴データを使って Radar の判断契約を検証し、実運用の外部取得とは分離します。
4. `shared` は複数 context で共有する安定した interface と value を提供します。feature 固有の判断ロジックは置きません。
5. `src/cli.rs` と各 feature の ACL が composition root を担当し、Domain が Infrastructure へ直接依存しないよう port を組み立てます。

データフローは `Interface / CLI -> ACL -> Application -> Domain` を基本とし、外部 I/O は Application port を実装する Infrastructure に限定します。Research の display-only レイヤーは Radar report や Telegram に補足表示できますが、READY / EXECUTE / Gate / Position Sizing / Trader の入力にはなりません。

### 非バイパス原則

どの module も `NO TRADE`、`Participation Gate`、`Trend Cohesion Gate`、`Exit Gate` を迂回して取引 action を生成してはなりません。判断順序は次のとおりです。

```text
状態判定
→ Gate 制約
→ 最終アクション
→ 表示出力 / 実行出力
```

`NO TRADE` は新規 position の構築を禁止しますが、既存 position の強制決済を意味しません。`Participation Gate` は市場参加条件、`Trend Cohesion Gate` は追随可能な主導構造、`Exit Gate` は既存 position の `HOLD / TRIM / EXIT` をそれぞれ判断します。Interface は Gate と矛盾する action へ書き換えません。

### Gate と action の関係

Gate 通過後の実行意図は `ADD / HOLD / TRIM / EXIT`、表示上の統一意図は `ADD / HOLD / TRIM / EXIT / WATCH` に収束します。`Trend Cohesion Topology` の `NO_LEADER / SINGLE_LEADER / FRAGMENTED_LEADERS` は構造の説明であり、新しい取引方向や action を追加するものではありません。

### `NO TRADE` 時の Breakout 表示

`Breakout Detection` は構造的な観測証拠であり、取引 action ではありません。`NO TRADE` 時は `EMERGING_BREAKOUT`、`CONFIRMED_BREAKOUT`、失敗 risk が高い観測対象を優先し、通常の反発や押し目修復を長い候補一覧として表示しません。

Domain の `BreakoutEvaluator` は分類を担当し、`PresentationAssembler` は表示上の情報量を調整します。`report.rs` は ViewModel を描画するだけで、Domain threshold や Gate を再判定しません。

### `NO TRADE` 時の表示順序

`markdown_body` と `telegram_html_body` は、判断、主な理由、監視対象、状態遷移証拠の順に表示します。`archival_markdown` は証拠全文を保持します。状態遷移証拠の圧縮は `output.compact_transition_evidence_in_no_trade` で制御します。

### 1.2 市場データの共有型

市場データの共有 primitive は `src/features/shared/domain/market_data.rs` に置きます。`DailyBar` は取引日、終値、任意の出来高を保持し、`TickerHistory` は日付昇順の bar、推定取引日数、任意の最新 quote timestamp を保持します。外部 provider 固有の DTO は ACL / Infrastructure でこの共有型へ正規化します。

### 1.3 判断と表示の主要オブジェクト

Radar の判断結果は `src/features/radar/domain/decision.rs` の `DecisionPacket` に集約します。この packet は基準日、market features、market regime、market state、portfolio policy、asset action decision、trend cohesion、transition log、trend recognition evidence を保持します。

表示用の `PresentationPacket` は `src/features/radar/interface/presentation.rs` に置きます。Domain の判断結果から localized な summary、action view、risk view、breakout view、terminal row を構築します。表示層は Domain の Gate や action を再計算せず、`DecisionPacket` の意味を変更しません。

Valuation Gravity の `ValuationGravitySnapshot` は Research bounded context の観測記録です。Radar の `DecisionPacket` や `PresentationPacket` へ判断入力として格納せず、Markdown / Telegram / daily-calibration の read-only appendix としてのみ合成します。

## 2. 境界条件とフォールバック

| 例外シナリオ | 所有レイヤー | 現行の扱い |
| --- | --- | --- |
| 外部 market data の取得失敗 | Radar ACL / Application | asset ごとの取得結果を集約し、完全失敗と部分失敗を区別する。完全失敗時は診断 report を生成するが、判断履歴は保存しない。 |
| 履歴不足 | Radar Domain | 必要な計算値を形成できない場合は推測で補完せず、data quality と観測不能状態へ反映する。 |
| 休日または最新 bar の不在 | Market data boundary | wall-clock の当日値で上書きせず、取得できた最新取引日を基準日に使用する。 |
| Valuation Gravity の過去日指定 | Research Application / Infrastructure | 指定日の snapshot だけを replay し、存在しない場合や invariant 違反は typed persistence health として報告する。 |
| Valuation Gravity の未来日指定 | Research Application / CLI | live quote を未来日の事実として保存せず、source / repository を呼ぶ前に拒否する。 |
| Research source の失敗 | Research Application / Interface | Reality Layer の欠損として表示し、Gate、execution、position sizing、Trader へ接続しない。 |
| 設定不整合 | Config boundary | `AppConfig::load` の validation error として返し、実行途中の暗黙 fallback を行わない。 |

## 3. 並行実行

外部取得の並行数と timeout は各 Application use case が所有します。Valuation Gravity は最大 10 asset を並行処理し、collection 全体を 10 秒以内に制限します。並行処理の失敗は asset 単位の unavailable observation に閉じ込め、Radar 本体の判断や通知を上書きしません。

---
tags: [architecture, design, technical, rust]
keywords: [data-structures, concurrency, error-handling, logic]
---

# 🐕 Stock Sentinel - アーキテクチャおよび詳細設計ドキュメント

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
    include_summary: bool,
}

struct RulesConfig {
    trend: TrendConfig,
    deviation_bands: Vec<(String, f64)>, // 内部的には f64 の降順にソートされた配列として保持
    actions: std::collections::HashMap<String, String>,
    bear_mode: BearModeConfig,
}

struct BearModeConfig {
    enabled: bool,
    fallback_action: String,
    caution_action: Option<String>,
}

struct WatchlistEntry {
    symbol: String,
    name: Option<String>,
    market: String,
    owner_ma_days: usize,
    leash_ma_days: usize,
    caution_ma_days: Option<usize>,
    deviation_basis: DeviationBasis, // 列挙型: Owner | Leash
    enable: bool,
}
```

### 1.2 基礎データレイヤー (Market Data)
APIから取得した生の時系列データです。
```rust
struct DailyBar {
    date: NaiveDate,
    close: f64,
    volume: Option<f64>,
}

struct TickerHistory {
    symbol: String,
    bars: Vec<DailyBar>, // 日付の昇順にソート
}
```

### 1.3 コア流転オブジェクト：銘柄スナップショット (TickerSnapshot)
エンジンモジュールによって生成される中心的なオブジェクトであり、計算結果をすべて含みます。レポート出力層に直接渡されます。
```rust
#[derive(Serialize)]
struct TickerSnapshot {
    symbol: String,
    name: String,
    current_date: NaiveDate,      // データの最終日
    dog_price: f64,               // 最新の終値
    owner_ma: Option<f64>,        // 飼い主平均線（データ不足時は None）
    leash_ma: Option<f64>,        // リード平均線
    trend_status: TrendStatus,    // トレンド: Up / Down / Flat / Unknown
    deviation_pct: Option<f64>,   // 乖離率 %
    deviation_basis_used: String, // "owner" または "leash"
    state_code: String,           // ヒットした band のステータスキー名 (例: "overheat_1")
    action_text: String,          // 最終的な提案アクション文（bear_mode/caution_mode を反映）
    is_bear_mode_active: bool,
    is_caution_mode_active: bool,
}

enum TrendStatus { Up, Down, Flat, Unknown }
```

## 2. 境界条件および例外処理マトリックス (Edge Cases Fallback)

| 例外シナリオ | システムの挙動 / 原因 | フォールバック戦略 (Fallback) |
| :--- | :--- | :--- |
| **API レート制限** | HTTP 429 エラーまたは接続切断 | 指数バックオフ（ジッター付き）を実装（MaxRetries=3, InitialDelay=2s）。完全に失敗した場合は、その銘柄のみレポートにエラーを表示し、全体の処理は継続する。 |
| **新規上場銘柄等のデータ不足** | 履歴データが50本しかないが、設定が `owner_ma_days=120` | MA計算は `None` を返す。トレンドは `Unknown`、乖離率は `None` とし、最終ステータス欄に「データ不足」と表示する。ランタイムエラーは発生させない。 |
| **祝休日・週末の実行** | 当日の新しいK線データが存在しない | 「今日は何日か」を強制せず、取得できた `bars.last()` をそのまま使用する。レポートヘッダーにはデータの最終取引日（Trade Date）を表示する。 |
| **無状態 V-Shape Recovery**| 暴落から急回復した場合のフラッピング | 外部の JSON に状態を保存せず、推論時に過去60日間の MA を再計算するサンドボックス（Simulation）を実行。`recover_days` の連続条件を満たした場合のみ安全状態に復帰する。 |
| **重力極端値 (Confidence 计算)** | 異なる銘柄が同じ DEFEND に落ちる | `dev_z_score` (価格がMAから何標準偏差離れているか) と `owner_ma_slope_pct` (下落の加速度)、`curvature` (二階微分による拐点) を用い、システムの確信度を 0~100% でスコアリングし出力する。 |
| **銘柄の取引停止・上場廃止** | 空の配列を取得、または最終データが数年前のもの | `bars.last().date` が現在の日付から7日以上離れている場合、システムは `[STALE] データが古い` という警告を表示する。 |
| **構成ステータス名の欠落** | `bands` に `fear_3` があるが `actions` に文案がない | 起動時の `config::load()` フェーズで検証を行い、キーが一致しない場合はプログラムを panic させ、設定エラーを通知する（Fail Fast 原則）。 |
| **極端な暴落による下限突破** | 乖離率が -50% で、設定した最低閾値 `fear_1: -25%` を下回る | 最下層の閾値状態を使用する（最後の要素にフォールバック）。 |
| **下山中の長期トレンド維持 (CAUTION)** | `bear_mode = true` かつトレンドがDownだが、`leash_ma` (ない場合は現在価格) が `caution_ma_days` (例: MA200) より上 | 完全な買い停止(DEFEND)にはせず、`caution_action`（警戒しつつ定投など）に留め、`is_caution_mode_active = true` とする。 |
| **下山中の長期トレンド崩壊 (DEFEND)** | `bear_mode = true` かつトレンドがDownで、`leash_ma` が近 N 日間で `confirm_threshold` 回以上 `caution_ma_days` (例: MA200) の緩衝ライン (例: 0.97x) より下 | 強制的に防御アクション（`fallback_action`）へダウングレードし、`is_bear_mode_active = true` とする。 |
| **極端な恐慌と下山トレンドの衝突 (Fear Exemption)** | `bear_mode = true` かつ `fear_x` 該当。さらに **1. 過去 N 日間で長期 MA を下回った回数が閾値未満** かつ **2. 長期 MA 自体が下落トレンドではない** | 牛市中の「黄金坑（絶好の買い場）」と見なし、`bear_mode` による Action 上書きを**スキップ**し、逆張り抄底を許可する。 |
| **長期トレンド崩壊中の極端な恐慌 (Fear Downtrend)** | `bear_mode = true` かつ `fear_x` 該当だが、**1. 过去 N 日間で長期 MA を下回った回数が閾値以上** または **2. 長期 MA 自体が下落トレンド** | 構造的な崩壊と見なし、豁免権を剥奪。Action 上書きを行い、`state_code` を `fear_downtrend` に設定して強制防御（DEFEND）を実行する。落ちるナイフは掴まない。 |

## 3. 並列モデルの提案
50銘柄程度の取得であれば、`tokio` を使用した並列リクエストによりネットワークI/Oの待機時間を大幅に短縮できます。`futures::stream::StreamExt` の `buffer_unordered(10)` などを使用し、Yahoo Finance APIへの過度な負荷を避けるため、最大並列数を10程度に制限することを推奨します。
---

## Author

Ray

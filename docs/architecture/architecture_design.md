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

本システムは以下のステートレスなパイプラインで構成されています：

1.  **Fetcher (`fetcher.rs`)**: `config` に基づき Yahoo Finance から時系列データを非同期で取得。
2.  **Calc (`calc.rs`)**: 移動平均、標準偏差、Z-Score、モメンタム傾斜、曲率（加速度）を算出。
3.  **Engine (`engine.rs`)**: 物理量ベクトルから `State` を判定し、`Confidence` を算出。
4.  **Main (`main.rs`)**: 各銘柄の出力を `GravityHealth`（序参量を含む状態ベクトル）に集約。資本配分比率（Trend vs Reversion）を計算し `CAPITAL STATE` を決定。
5.  **Report (`report.rs`)**: 
    - **Persistence**: Daily JSON, `run_status_YYYY-MM-DD.json`, `telemetry.csv` (20列 序参量データセット) の追記保存。
6.  **Backtest (`backtest.rs`)**: 歴史的な価格データを用いて全ロジックをシミュレート。Calibration Error や Alpha 分離度をレポート。

#### 3.1 不可绕过原则 (Non-Bypass Principle)

本系统中的任何模块都不得绕过 `NO TRADE`、`Participation Gate` 或 `Exit Gate` 直接生成最终交易动作。

系统必须始终遵循以下顺序：

```text
状态判断
→ Gate 约束
→ 最终动作
→ 展示输出 / 执行输出
```

这意味着：

1. `NO TRADE` 只表示禁止主动开新仓，不等于强制清仓
2. `Participation Gate` 未通过时，不允许任何模块直接输出进攻性动作
3. `Exit Gate` 触发后，不允许继续保留与之冲突的进攻性动作语义
4. 展示层不得改写与 Gate 冲突的最终动作

以下实现被视为违规：

1. 某个“强信号”直接触发买入
2. 某个“个股特别好”直接允许加仓
3. 展示层或临时规则绕过 Gate 改写最终动作
4. Gate 未通过时仍输出 `ADD / ACCUMULATE`
5. `Exit Gate` 已触发时仍保留 `ADD` 或其他进攻性表达

本原则是系统级约束，不属于某一轮任务的临时规则。任何“例外买入”“特批加仓”“高优先级捷径”都视为破坏系统一致性的违规实现。

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

/// 物理量と分配ロジックを集約したマクロ状態ベクトル
struct GravityHealth {
    up_count: usize,
    flat_count: usize,
    total_count: usize, // watchlist_size
    up_weight: f64,
    flat_weight: f64,
    down_weight: f64,
    total_weight: f64,
    global_gravity_strength: f64,
    global_potential_energy: f64,
    trend_alloc_weight: f64,
    reversion_alloc_weight: f64,
    config_hash: String, // 参数宇宙隔离标识
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
    reason_code: Option<String>,
    action_text: String,          // 最終的な提案アクション文
    confidence_score: u8,         // 置信度 (0-100)
    owner_ma_slope_pct: Option<f64>, // 重力強度
    dev_z_score: Option<f64>,        // Z-Score
    curvature: Option<f64>,          // 曲率
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
| **极端恐慌与下山模式** | - | (Logic consolidated into MarketRegime and PortfolioPolicy in major refactor) |

## 3. 並列モデルの提案
50銘柄程度の取得であれば、`tokio` を使用した並列リクエストによりネットワークI/Oの待機時間を大幅に短縮できます。`futures::stream::StreamExt` の `buffer_unordered(10)` などを使用し、Yahoo Finance APIへの過度な負荷を避けるため、最大並列数を10程度に制限することを推奨します。
---

## Author

Ray

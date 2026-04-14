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

本系统中的任何模块都不得绕过 `NO TRADE`、`Participation Gate`、`Trend Cohesion Gate` 或 `Exit Gate` 直接生成最终交易动作。

系统必须始终遵循以下顺序：

```text
状态判断
→ Gate 约束
→ 最终动作
→ 展示输出 / 执行输出
```

这意味着：

1. `NO TRADE` 只表示禁止主动开新仓，不等于强制清仓
2. `Participation Gate` 负责回答“市场参与条件是否成熟”
3. `Trend Cohesion Gate` 负责回答“是否存在可跟随主线”
4. `Exit Gate` 负责回答“已有持仓是否需要收缩或退出”
5. 展示层不得改写与 Gate 冲突的最终动作

以下实现被视为违规：

1. 某个“强信号”直接触发买入
2. 某个“个股特别好”直接允许加仓
3. 展示层或临时规则绕过 Gate 改写最终动作
4. Gate 未通过时仍输出 `ADD / ACCUMULATE`
5. `Trend Cohesion Gate` 未通过时仍输出 `ADD / ACCUMULATE`
6. `Exit Gate` 已触发时仍保留 `ADD` 或其他进攻性表达

本原则是系统级约束，不属于某一轮任务的临时规则。任何“例外买入”“特批加仓”“高优先级捷径”都视为破坏系统一致性的违规实现。

#### 3.2 当前三层 Gate 与意图层

当前系统的动作约束已明确分为三层 Gate：

1. `Participation Gate`
   - 由 `ParticipationReadiness` 计算
   - 回答“市场参与条件是否成熟”
   - 属于市场参与条件判断，不直接承担主线识别职责

2. `Trend Cohesion Gate`
   - 由 `TrendCohesionEvaluator` 计算
   - 回答“当前是否存在可跟随的主线结构”
   - 其 `gate_passed` 进入底层动作链，直接约束主动加仓/建仓类动作
   - 同时输出 `status`、`topology`、`formation_conditions` 与 `unmet_conditions`

3. `Exit Gate`
   - 由 `ExitDecision` 计算
   - 回答“已有持仓应当 HOLD / TRIM / EXIT”
   - 与 `NO TRADE` 解耦，`NO TRADE` 不等于必须卖出

在 Gate 之后，系统再进入统一动作层：

- `Position Intent`
  - 执行层意图：`ADD / HOLD / TRIM / EXIT`
- `Unified Position Intent`
  - 展示层统一动作原语：`ADD / HOLD / TRIM / EXIT / WATCH`
  - 负责把 Entry/Exit 结果收敛成单一最终动作语言

此外，`Trend Cohesion Topology` 负责区分主线结构类型：

- `NO_LEADER`
- `SINGLE_LEADER`
- `FRAGMENTED_LEADERS`

它属于结构解释层增强，不等于交易方向判断，也不等于新的交易动作。

#### 3.3 `NO TRADE` 场景下的 Breakout 展示原则

`Breakout Detection` 在系统中属于结构证据层，而不是交易动作层。尤其在 `NO TRADE` 场景下，其展示目标必须从“提示可能的机会”收敛为“服务观察与告警”。

这意味着：

1. `Breakout Summary` 的语义目标是 **观察 / 告警**，不是 **建议 / 排序**
2. `Breakout Detection` 不得通过展示语气削弱上方的 `NO TRADE / 主线未形成 / 无主线` 主结论
3. `NO TRADE` 场景下，只允许以下对象作为 breakout 主项展开：
   - `EMERGING_BREAKOUT`
   - `CONFIRMED_BREAKOUT`
   - 高失败风险异常个体
4. `NO_BREAKOUT + 普通反弹 / 回撤修复` 在 `NO TRADE` 场景下默认不得展开成长列表
5. 若某个 `NO_BREAKOUT` 个体在 `NO TRADE` 下仍被保留展示，其原因必须是高失败风险，且前台应优先显示风险解释，而不是中性描述

实现边界要求如下：

1. `BreakoutEvaluator` 继续只负责领域分类，不直接承担展示降噪职责
2. `NO TRADE` 下的 breakout 降噪必须在 `PresentationAssembler` 中完成
3. `report.rs` 只渲染 breakout 结果，不新增展示判断
4. 展示阈值必须与领域阈值分离，避免为了前台观感反向污染领域语义

本原则的目的不是减少信息，而是避免在 `NO TRADE` 场景下把用户重新拉回“挑票模式”。

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

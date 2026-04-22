---
author: Ray
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

本システムは以下のステートレスなパイプラインで構成されています：

1.  **Fetcher (`fetcher.rs`)**: `config` に基づき Yahoo Finance から時系列データを非同期で取得。
2.  **Calc (`calc.rs`)**: 移動平均、標準偏差、Z-Score、モメンタム傾斜、曲率（加速度）を算出。
3.  **Engine (`engine.rs`)**: 物理量ベクトルから `State` を判定し、`Confidence` を算出。
4.  **Main (`main.rs`)**: 各銘柄の出力を `GravityHealth`（序参量を含む状態ベクトル）に集約。資本配分比率（Trend vs Reversion）を計算し `CAPITAL STATE` を決定。
5.  **Report (`report.rs`)**: 
    - **Persistence**: Daily JSON, `run_status_YYYY-MM-DD.json`, `telemetry.csv` (20列 序参量データセット) の追記保存。
6.  **Backtest (`backtest.rs`)**: 歴史的な価格データを用いて全ロジックをシミュレート。Calibration Error や Alpha 分離度をレポート。

#### 3.1 非バイパス原則 (Non-Bypass Principle)

本システムのいかなるモジュールも、`NO TRADE`、`Participation Gate`、`Trend Cohesion Gate`、または `Exit Gate` をバイパスして最終的な取引アクションを直接生成してはなりません。

システムは常に以下の順序に従う必要があります：

```text
状態判定
→ Gate 制約
→ 最終アクション
→ 表示出力 / 実行出力
```

これは以下のことを意味します：

1. `NO TRADE` は新規のポジション構築を禁止することのみを意味し、強制的な全決済を意味するものではありません。
2. `Participation Gate` は「市場への参加条件が整っているか」という問いに答える責任を負います。
3. `Trend Cohesion Gate` は「追随可能なメインライン（主導権）構造が存在するか」という問いに答える責任を負います。
4. `Exit Gate` は「既存のポジションを縮小または撤退させる必要があるか」という問いに答える責任を負います。
5. 表示レイヤーは、Gate と競合する最終アクションを書き換えてはなりません。

以下の実装は違反とみなされます：

1. 特定の「強いシグナル」が直接買いをトリガーする。
2. 「特定の銘柄が非常に良い」という理由で直接加筆を許可する。
3. 表示レイヤーまたは一時的なルールが Gate をバイパスして最終アクションを書き換える。
4. Gate を通過していないにもかかわらず、`ADD / ACCUMULATE` を出力する。
5. `Trend Cohesion Gate` を通過していないにもかかわらず、`ADD / ACCUMULATE` を出力する。
6. `Exit Gate` がトリガーされているにもかかわらず、`ADD` やその他の攻撃的な表現を保持する。

本原則はシステムレベルの制約であり、特定のタスクの一時的なルールではありません。「例外的な買い」「特別許可された加筆」「高優先度のショートカット」はいずれもシステムの一貫性を破壊する違反実装とみなされます。

#### 3.2 現在の3層 Gate と意図レイヤー

現在のシステムのアクション制約は、明確に以下の3つの Gate に分かれています：

1. `Participation Gate`
   - `ParticipationReadiness` によって計算されます。
   - 「市場への参加条件が整っているか」に答えます。
   - 市場参加条件の判定であり、メインラインの識別責任は直接負いません。

2. `Trend Cohesion Gate`
   - `TrendCohesionEvaluator` によって計算されます。
   - 「現在、追随可能なメインライン構造が存在するか」に答えます。
   - その `gate_passed` は下層のアクションチェーンに入り、能動的な加筆/ポジション構築アクションを直接制約します。
   - 同時に `status`、`topology`、`formation_conditions`、および `unmet_conditions` を出力します。

3. `Exit Gate`
   - `ExitDecision` によって計算されます。
   - 「既存のポジションを HOLD / TRIM / EXIT すべきか」に答えます。
   - `NO TRADE` とは分離されており、`NO TRADE` は必ずしも売却を意味しません。

Gate の後、システムは統一されたアクションレイヤーに入ります：

- `Position Intent`（実行レイヤーの意図）
  - `ADD / HOLD / TRIM / EXIT`
- `Unified Position Intent`（表示レイヤーの統一アクション原語）
  - `ADD / HOLD / TRIM / EXIT / WATCH`
  - Entry/Exit の結果を単一の最終アクション言語に収束させる責任を負います。

さらに、`Trend Cohesion Topology` はメインライン構造のタイプを区別します：

- `NO_LEADER`
- `SINGLE_LEADER`
- `FRAGMENTED_LEADERS`

これは構造解釈レイヤーの強化であり、取引方向の判定や新しい取引アクションと同義ではありません。

#### 3.3 `NO TRADE` シナリオにおける Breakout 表示原則

`Breakout Detection` はシステムにおいて構造的証拠レイヤーに属し、取引アクションレイヤーには属しません。特に `NO TRADE` シナリオでは、その表示目標は「可能性のあるチャンスの提示」から「監視とアラートへのサービス」へと収束させる必要があります。

これは以下のことを意味します：

1. `Breakout Summary` のセマンティック目標は **監視 / アラート** であり、**提案 / ランキング** ではありません。
2. `Breakout Detection` は、表示の語気によって上位の `NO TRADE / メインライン未形成 / メインラインなし` という主結論を弱めてはなりません。
3. `NO TRADE` シナリオでは、以下のオブジェクトのみを breakout の主項目として展開することを許可します：
   - `EMERGING_BREAKOUT`
   - `CONFIRMED_BREAKOUT`
   - 失敗リスクが高い異常個体
4. `NO_BREAKOUT + 通常の反発 / 押し目修復` は、`NO TRADE` シナリオではデフォルトで長いリストとして展開してはなりません。
5. もしある `NO_BREAKOUT` 個体が `NO TRADE` 下で表示保持される場合、その理由は高い失敗リスクである必要があり、フロントエンドでは中立的な記述ではなくリスクの説明を優先的に表示すべきです。

実装境界の要件は以下の通りです：

1. `BreakoutEvaluator` は引き続きドメイン分類のみを担当し、表示のノイズ除去責任を直接負いません。
2. `NO TRADE` 下の breakout ノイズ除去は、`PresentationAssembler` で完了させる必要があります。
3. `report.rs` は breakout の結果をレンダリングするのみで、新しい表示判定を追加してはなりません。
4. 表示閾値はドメイン閾値と分離し、フロントエンドの外観のためにドメインセマンティクスを逆流汚染することを避ける必要があります。

本原則の目的は情報を減らすことではなく、`NO TRADE` シナリオにおいてユーザーを「銘柄選びモード」に引き戻すことを避けることにあります。

#### 3.4 `NO TRADE` フロントエンド情報階層契約（実行優先）

実行速度と一貫性を保証するため、`NO TRADE` シナリオにおけるフロントエンド表示（`markdown_body` / `telegram_html_body`）は固定された階層を採用します：

1. 意思決定レイヤー：`NO TRADE` + `新規ポジション上限 · 0%`
2. 簡略化された原因レイヤー：`安定性 x/10`、`連続性 x/3`、`メインライン構造`
3. 監視重点レイヤー：`Breakout Detection`（`第1日` などの経過時間を含む）
4. 証拠レイヤー：`状態遷移の証拠` を後方に配置（圧縮表示可）

説明：

1. アーカイブ出力（`archival_markdown`）は完全な証拠を保持し、情報の削除は行いません。
2. フロントエンドで `NO TRADE` 証拠の展開を圧縮するかどうかは、`output.compact_transition_evidence_in_no_trade` によって制御されます。

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
    total_count: usize, // ウォッチリストのサイズ
    up_weight: f64,
    flat_weight: f64,
    down_weight: f64,
    total_weight: f64,
    global_gravity_strength: f64,
    global_potential_energy: f64,
    trend_alloc_weight: f64,
    reversion_alloc_weight: f64,
    config_hash: String, // パラメータ宇宙の隔離識別子
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
| **重力極端値 (Confidence 計算)** | 異なる銘柄が同じ DEFEND に落ちる | `dev_z_score` (価格がMAから何標準偏差離れているか) と `owner_ma_slope_pct` (下落の加速度)、`curvature` (二階微分による拐点) を用い、システムの確信度を 0~100% でスコアリングし出力する。 |
| **銘柄の取引停止・上場廃止** | 空の配列を取得、または最終データが数年前のもの | `bars.last().date` が現在の日付から7日以上離れている場合、システムは `[STALE] データが古い` という警告を表示する。 |
| **構成ステータス名の欠落** | `bands` に `fear_3` があるが `actions` に文案がない | 起動時の `config::load()` フェーズで検証を行い、キーが一致しない場合はプログラムを panic させ、設定エラーを通知する（Fail Fast 原則）。 |
| **極端な暴落による下限突破** | 乖離率が -50% で、設定した最低閾値 `fear_1: -25%` を下回る | 最下層の閾値状態を使用する（最後の要素にフォールバック）。 |
| **極端な恐慌と下山モード** | - | (ロジックは大規模なリファクタリングにより MarketRegime と PortfolioPolicy に統合されました) |

## 3. 並列モデルの提案
50銘柄程度の取得であれば、`tokio` を使用した並列リクエストによりネットワークI/Oの待機時間を大幅に短縮できます。`futures::stream::StreamExt` の `buffer_unordered(10)` などを使用し、Yahoo Finance APIへの過度な負荷を避けるため、最大並列数を10程度に制限することを推奨します。

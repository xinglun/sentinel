# 🐕 Stock Sentinel - 架构与核心设计文档

本文档对 PRD 中的系统需求进行技术层面的细化，主要涵盖核心数据结构设计与边界异常处理矩阵，为后续 Rust 编码提供明确指引。

## 1. 核心数据结构设计 (Data Structures)

为了保证逻辑清晰且易于测试，我们将系统拆分为“配置层”、“基础数据层”和“业务快照层”。

### 1.1 配置映射 (Config Schema)
对应 `config.toml` 的序列化结构。
```rust
struct AppConfig {
    output: OutputConfig,
    rules: RulesConfig,
    watchlist: Vec<WatchlistEntry>,
}

struct OutputConfig {
    timezone: String, // 如 "Asia/Shanghai"
    format: String,   // "markdown", "json"
    save_to: String,  // "./reports"
    include_summary: bool,
}

struct RulesConfig {
    trend: TrendConfig,
    deviation_bands: Vec<(String, f64)>, // 内部需转为按 f64 降序排列的数组
    actions: std::collections::HashMap<String, String>,
    bear_mode: BearModeConfig,
}

struct WatchlistEntry {
    symbol: String,
    name: Option<String>,
    market: String,
    owner_ma_days: usize,
    leash_ma_days: usize,
    deviation_basis: DeviationBasis, // 枚举: Owner | Leash
    enable: bool,
}
```

### 1.2 基础数据层 (Market Data)
从 API 拉取到的原始时间序列数据。
```rust
struct DailyBar {
    date: NaiveDate,
    close: f64,
    volume: Option<f64>,
}

struct TickerHistory {
    symbol: String,
    bars: Vec<DailyBar>, // 按日期升序排列
}
```

### 1.3 核心流转对象：标的快照 (TickerSnapshot)
这是引擎模块产出的核心对象，包含了该标的所有计算结果，直接供报告渲染层使用。
```rust
#[derive(Serialize)]
struct TickerSnapshot {
    symbol: String,
    name: String,
    current_date: NaiveDate,      // 数据的最后一天
    dog_price: f64,               // 最新收盘价
    owner_ma: Option<f64>,        // 主人均线值（数据不足时为 None）
    leash_ma: Option<f64>,        // 绳子均线值
    trend_status: TrendStatus,    // 趋势: Up / Down / Flat / Unknown
    deviation_pct: Option<f64>,   // 乖离率 %
    deviation_basis_used: String, // "owner" 或 "leash"
    state_code: String,           // 匹配到的 band 状态键名 (如 "overheat_1")
    action_text: String,          // 最终建议文案（包含 bear_mode）
    is_bear_mode_active: bool,
}

enum TrendStatus { Up, Down, Flat, Unknown }
```

## 2. 边缘情况与异常处理矩阵 (Edge Cases Fallback)

| 异常情况场景 | 系统现象 / 原因 | 降级处理策略 (Fallback) |
| :--- | :--- | :--- |
| **API 触发限流** | HTTP 返回 429 或连接被直接 Reset | 实现带抖动的指数退避重试（MaxRetries=3，初始延迟=2s）。若完全失败，该标的在报表中报错但不中断整体进度。 |
| **新股上市数据不足** | 库中只有 50 根日线，但配置要求 `owner_ma_days=120` | MA 计算返回 `None`。趋势(Trend)置为 `Unknown`，乖离率(Dev)置为 `None`，并在最终状态栏显示“数据不足”。不抛掷 Runtime Error。 |
| **节假日/周末执行** | 今天没有产生新 K 线数据 | 系统不强制查询“今天是哪天”，而是直接取拉取到的 `bars.last()`。报告头部显示数据的最后交易日日期（Trade Date）。 |
| **个股停牌或退市** | 获取到空数组，或最后一天数据停留在两年前 | 若 `bars.last().date` 距离当前自然日超过 7 天，系统标记为 `[STALE] 数据陈旧` 警告。 |
| **配置状态名遗漏** | `bands` 里配了 `fear_3`，但 `actions` 字典里没写对应文案 | 在应用启动时的 `config::load()` 阶段执行校验，若两边 keys 不对齐，程序直接 panic 并提示配置错误（Fail Fast 原则）。 |
| **极端暴跌跌穿所有下限** | 跌幅为 -50%，低于配置的最低阈值 `fear_1: -25%` | 使用最底层的阈值状态（兜底匹配最后一个元素）。 |

## 3. 并发模型建议
在拉取 50 只以内的股票集合时，通过 `tokio` 进行并发请求可以大幅减少网络 I/O 的等待时间。建议采用 `futures::stream::StreamExt` 提供的 `buffer_unordered(10)` 限制最大并发数为 10，避免瞬间把 Yahoo Finance 的接口打挂。

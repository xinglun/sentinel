# 🐕 Stock Sentinel (Decision Radar) 需求与架构文档

## 1. 系统背景与核心理念
企业内在价值（远景、业务护城河、管理层、财务状况）是牵绳的人。移动平均线是绳子，当前价格是狗。
人向上走，移动平均线和狗的距离做技术分析，才有实质意义。
**Stock Sentinel** 定位为一个“观测+建议系统（Decision Radar）”，而非全自动交易机器人。系统通过每日观测“人-绳-狗”的具体距离（乖离率）与趋势，输出当前资产状态、行动建议以及投资组合的总体温度。

### 核心要素定义
* **人（Owner）**：企业长期方向的代理变量，由长周期 MA（如 60/120/200）表示。
* **绳（Leash）**：短中期牵引缓冲，由短周期 MA（如 20）表示。
* **狗（Dog）**：最新收盘价（当前市场情绪）。
* **距离（Deviation）**：狗与人/绳之间的距离（乖离率），是产生“过热/恐慌”状态的核心依据。

## 2. 核心功能诉求与输入输出
### 2.1 输入：配置驱动 (config.toml)
通过修改单一配置文件来驱动整个系统，包含但不限于以下维度：
* **Watchlist（关注列表）**：针对每个标的独立设置 ticker、数据源市场、以及专属的“主人和绳子”周期（例如宽基指数采用MA200做主人，高爆发科技股采用MA60/120做主人）。
* **Rules（全局规则）**：定义明确的阶梯式下限乖离阈值（过热、恐慌等），以及对应的买卖/观察动作。
* **Bear Mode（防御模式）**：核心安全网，当判定“主人下山（趋势向下）”时，剥夺买入建议，强制降级为防御动作。

### 2.2 处理逻辑（引擎）
1. **数据拉取**：每日盘后拉取至少1年的日线历史数据（计算MA200必备），自带指数退避重试（Exponential Backoff Retry）以应对网络和限流问题。
2. **指标计算**：计算每日的 `owner_ma`、`leash_ma` 和 `deviation`。
3. **趋势判定（Trend）**：对比今日与 N天前（如20天前）的主人MA值。引入微小防抖阈值（如 ±0.5%）以判定 Up / Down / Flat。
4. **状态推演（State Machine）**：根据 `deviation` 落入的区间，结合 Trend，生成对应的 Action。

### 2.3 输出：多维度展示与推送
1. **纯文本 Markdown 报告**：生成携带高亮 Emoji、表格排版精美的只读报告（保存于 `./reports`）。
2. **命令行交互界面 (CLI Console)**：支持终端彩色高亮输出，方便极客本地随时排查。
3. **JSON 数据输出**：结构化保存执行结果，供后续 Web 仪表盘或回测系统直接使用。
4. **多渠道推送（优先 Telegram）**：完美契合个人用户的高信噪比、零收费以及原生 Markdown 解析需求。

## 3. 系统架构实现方案 (Rust)
本系统采用 Rust 构建，以保证跨平台、稳定和极高执行效率。

### 3.1 核心模块划分
* **`config`**：读取与校验 TOML，使用 `serde` 和 `toml`。
* **`datasource`**：对接行情数据（如 Yahoo Finance API），负责 HTTP 重试机制 (`reqwest` + `tokio`)。
* **`calc`**：专注于时间序列的无状态计算（MA计算、百分比变化等）。
* **`engine`**：状态机模块，将算出的 Dev% 映射到相应的状态，并在发现趋势向下时触发 Bear Mode 拦截。
* **`report`**：渲染器模块，根据需求生成终端表单 (`tabled`)、Markdown 文本及 JSON 文件。
* **`notify`**：Webhook推送模块，封装 Telegram API 进行每日自动触达。

### 3.2 配置文件标准范例 (TOML)
```toml
version = 1

[output]
timezone = "Asia/Shanghai"
format = "markdown"
save_to = "./reports"
include_summary = true

[rules.trend]
lookback_days = 20
flat_threshold_pct = 0.5 # [-0.5%, +0.5%] 视为走平

[rules.deviation_bands]
# 阶梯下限阈值 (>=命中)
overheat_2 = 30.0   
overheat_1 = 15.0   
cruise     = 5.0    
optimal    = -5.0   
pullback   = -15.0  
fear_1     = -25.0  

[rules.actions]
# 键名与 bands 一一对应
overheat_2 = "停止买入，现金缓冲"
overheat_1 = "减半买入，现金缓冲"
cruise     = "照常定投"
optimal    = "照常定投（效率最佳）"
pullback   = "适度加仓（分批）"
fear_1     = "手动介入：用现金缓冲加仓"
fear_2     = "手动介入：大幅加仓（极端恐慌）"

[rules.bear_mode]
enabled = true
fallback_action = "【防御降级】：主人下山，停止加仓/仅观察"

[[watchlist]]
symbol = "TSLA"
name = "Tesla"
market = "US"
owner_ma_days = 120
leash_ma_days = 20
deviation_basis = "owner"
enable = true

[[watchlist]]
symbol = "SPY"
name = "S&P 500"
market = "US"
owner_ma_days = 200
leash_ma_days = 20
deviation_basis = "leash"
enable = true
```

## 4. MVP 验收条件 (Acceptance Criteria)
1. **配置驱动**：无需重新编译代码，仅修改 `config.toml` 即可增删股票及调整各阈值。
2. **容错机制**：任一一只股票数据抓取失败，不能导致程序崩溃或影响其他股票的运行；失败标的在报告中显著标红。
3. **正确的状态映射**：严格按照阶梯下限执行。当判定 `bear_mode = true` 时，必须强行覆盖建议状态为防御提示。
4. **文件落地**：单次运行需同名生成 `reports/YYYY-MM-DD.md` 和 `reports/YYYY-MM-DD.json`。
5. **TG 抵达（二期/扩展）**：程序可通过读取终端环境变量传入的 BOT_TOKEN，将报告完整推送到指定的 Telegram Chat 中并完成 Markdown 渲染。

# Stock Sentinel 自动观测系统
# GitHub Actions 托管运行要求说明书

## 一、目标
将 Stock Sentinel 部署到 GitHub Actions，实现完全无人值守的自动观测运行环境，确保系统可以长期、稳定、连续地执行以下核心任务：

### 每日自动执行：
1. 获取股票市场数据
2. 运行资本引力观测引擎
3. 生成观测报告（Markdown）
4. 记录观测数据（telemetry.csv）
5. 推送 Telegram 通知
6. 将观测数据永久保存到 GitHub Repository

**最终目标是建立：长期连续、不可篡改、可追溯的资本结构时间序列数据库（系统的核心资产）。**

*   **GitHub Actions 运行分支**：`main`
*   **观测数据提交分支**：`data` (使用 Git Worktree 隔离)

## 二、运行频率要求
GitHub Actions 必须支持：

1.  **每日自动运行**
    *   **日本时间**：22:30 JST
    *   **对应 UTC**：13:30 UTC
    *   **逻辑**：美股收盘后数据稳定，避免盘中噪音。

2.  **允许手动运行**
    *   **触发方式**：GitHub 页面点击 `Run workflow`。
    *   **用途**：调试、参数调整验证、临时观测。
    *   **要求**：手动运行数据必须与自动运行同样记录至 `telemetry.csv`（依赖 timestamp 区分）。

## 三、运行环境要求
*   **操作系统**：Ubuntu Latest
*   **Rust 环境**：Stable toolchain，支持 `cargo build --release` 和 `cargo run --release`。

## 四、执行程序要求
*   **执行指令**：`cargo run --release`
*   **禁止使用 debug 模式**：确保 release 模式与研究数据保持一致，保证观测结果可复现。

每次运行必须保存并提交到 `data` 分支：
*   **目录**：`/reports/`
*   **核心资产**：
    *   `telemetry.csv`：多维时间序列（必须持续追加）。
    *   `run_status_YYYY-MM-DD.json`：机器可读的运行健康快照（P0 校验项）。
    *   `YYYY-MM-DD.md`：人类可读的归档报告。
    *   `decision_packet_YYYY-MM-DD.json`：单一事实源决策包。
    *   `portfolio_snapshot_YYYY-MM-DD.json`：组合暴露快照。
    *   `account_snapshot_YYYY-MM-DD.json`：账户资金快照。
    *   `state_transitions.csv` / `state_transitions.jsonl`：市场状态迁移日志。
    *   `execution_gate_log.jsonl`：执行门禁审计。
    *   `data_quality_log.jsonl`：数据质量日志。

每次运行后必须自动推送至 `data` 分支。禁止直接提交至 `main` 分支以保持代码与数据的物理隔离。
**目的**：将 GitHub Repository 作为永久观测档案库。

## 七、Telegram 通知要求
*   **Secret 管理**：必须从 GitHub Secrets 读取 `TELEGRAM_BOT_TOKEN` 和 `TELEGRAM_CHAT_ID`。
*   **禁止行为**：禁止将敏感 Token 写入代码仓库。

## 八、参数宇宙隔离要求
`telemetry.csv` 必须保留 `config_hash`，确保未来参数变更后，历史数据仍然可区分。这是量化研究的关键基础。

## 九、运行失败处理要求
*   如果执行失败，GitHub 必须标记为 `Failed` 以提醒系统异常，禁止静默失败。

## 十、Repository 标准结构
```text
stock-sentinel/ (main branch)
├── .github/workflows/daily_radar.yml
└── ... (source code)

stock-sentinel/ (data branch)
├── backtest/
│   ├── summary_latest.md
│   └── archive/
│       └── summary_YYYY-MM-DD.md
└── reports/
    ├── YYYY-MM-DD.md
    ├── decision_packet_YYYY-MM-DD.json
    ├── telemetry.csv
    ├── run_status_YYYY-MM-DD.json
    ├── portfolio_snapshot_YYYY-MM-DD.json
    ├── account_snapshot_YYYY-MM-DD.json
    ├── decision_history.jsonl
    ├── state_transitions.csv
    ├── state_transitions.jsonl
    ├── execution_gate_log.jsonl
    ├── data_quality_log.jsonl
    ├── ledger.csv
    └── freshness.json
```

## 十一、最终目标定义
Stock Sentinel GitHub Actions 托管运行的最终目标不是自动交易或预测市场，而是**建立完整、连续、可信的资本结构观测历史**。

## 十二、核心哲学
> 这不是 CI/CD。
> 
> 这是把你的资本望远镜放进轨道，让它每天自动看宇宙。
> 
> 人类负责思考。机器负责观测。时间负责证明一切。

---

## Author
Ray

# Data Branch Layout Standard

本文件定义 `data` 分支的目标目录结构、命名规则和验收口径。

## 1. Scope

本标准适用于：

1. `daily_radar.yml` 每日归档产物
2. `weekly_backtest.yml` 每周回测产物
3. `reports/` 与 `backtest/` 的长期保留结构

## 2. Core Rule

`reports/` 中所有“单日资产”必须使用同一个日期键：

1. `packet.date`
2. 即市场数据所属日期
3. 不是 workflow 运行当天日期

这意味着以下文件必须共享同一个 `YYYY-MM-DD`：

1. `YYYY-MM-DD.md`
2. `decision_packet_YYYY-MM-DD.json`
3. `portfolio_snapshot_YYYY-MM-DD.json`
4. `account_snapshot_YYYY-MM-DD.json`
5. `run_status_YYYY-MM-DD.json`

## 3. Target Layout

```text
data branch
├── backtest/
│   ├── summary_latest.md
│   └── archive/
│       └── summary_YYYY-MM-DD.md
├── reports/
│   ├── YYYY-MM-DD.md
│   ├── decision_packet_YYYY-MM-DD.json
│   ├── portfolio_snapshot_YYYY-MM-DD.json
│   ├── account_snapshot_YYYY-MM-DD.json
│   ├── run_status_YYYY-MM-DD.json
│   ├── decision_history.jsonl
│   ├── state_transitions.csv
│   ├── state_transitions.jsonl
│   ├── execution_gate_log.jsonl
│   ├── data_quality_log.jsonl
│   ├── telemetry.csv
│   ├── ledger.csv
│   └── freshness.json
└── README.md
```

## 4. File Semantics

### Daily dated files

1. `YYYY-MM-DD.md`
   - 人读版每日归档报告

2. `decision_packet_YYYY-MM-DD.json`
   - 当日单一事实源决策包

3. `portfolio_snapshot_YYYY-MM-DD.json`
   - 当日组合持仓和浮盈快照

4. `account_snapshot_YYYY-MM-DD.json`
   - 当日账户资金与购买力快照

5. `run_status_YYYY-MM-DD.json`
   - 当日运行健康快照
   - 记录 `decisioning / archival / notification / execution`

### Append-only files

1. `decision_history.jsonl`
   - 决策历史主轴

2. `state_transitions.csv`
   - 市场状态迁移表格版

3. `state_transitions.jsonl`
   - 市场状态迁移结构化版

4. `execution_gate_log.jsonl`
   - 风控网关通过/拦截审计

5. `data_quality_log.jsonl`
   - 数据抓取质量日志

6. `telemetry.csv`
   - 研究级时间序列观测数据

7. `ledger.csv`
   - 成交账本

### Auxiliary file

1. `freshness.json`
   - workflow freshness gate 辅助文件
   - 不是核心研究资产

## 5. Legacy Files

以下文件属于旧命名体系，不应继续生成：

1. `reports/YYYY-MM-DD.json`

处理原则：

1. 历史遗留文件可保留或一次性清理
2. 当前代码和 workflow 不再依赖它们
3. 新归档标准一律使用 `decision_packet_YYYY-MM-DD.json`

## 6. Validation Rules

`daily_radar.yml` 每次运行后必须至少验证以下文件非空：

1. `reports/YYYY-MM-DD.md`
2. `reports/decision_packet_YYYY-MM-DD.json`
3. `reports/decision_history.jsonl`
4. `reports/state_transitions.csv`
5. `reports/state_transitions.jsonl`
6. `reports/execution_gate_log.jsonl`
7. `reports/portfolio_snapshot_YYYY-MM-DD.json`
8. `reports/account_snapshot_YYYY-MM-DD.json`
9. `reports/data_quality_log.jsonl`
10. `reports/run_status_YYYY-MM-DD.json`
11. `reports/telemetry.csv`

## 7. Operational Notes

1. 如果 `reports/` 中出现 `run_status` 日期和 `decision_packet` 日期不一致，视为归档命名异常。
2. 如果 `reports/` 中重新出现 `YYYY-MM-DD.json`，视为旧产物回归。
3. `backtest/` 是每周节奏，不应混入每日 `reports/` 资产。

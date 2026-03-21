# Sentinel 状态机 V1.2 验证与可观测性方案

## 1. 目标

V1.1 已完成状态机判定原语的配置化与结构化审计。  
V1.2 不再修改核心判定逻辑，重点做两件事：

1. 用回测和历史样本验证 V1.1 是否真的减少了错误 reset 和状态抖动
2. 把 `transition_audit` 接入日报/调试输出，形成日常可观测能力

本轮目标不是“改状态机”，而是“验证状态机”和“看见状态机”。

---

## 2. 范围

### 2.1 做什么

1. 增加状态机回测验证指标
2. 增加 `transition_audit` 的落盘与摘要输出
3. 给日报/调试输出增加状态迁移摘要

### 2.2 不做什么

1. 不改 Telegram 主样式
2. 不引入新市场状态
3. 不新增交易动作
4. 不继续扩展执行层

---

## 3. 工作流

### 3.1 Backtest Validation

目标：

评估 V1.1 相比旧状态机，是否真正改善了以下问题：

1. 错误 reset 次数下降
2. 多级跳变下降
3. 状态抖动下降
4. 防御触发更有解释性

建议新增指标：

1. `reset_count`
2. `blocked_reset_count`
3. `multi_step_downgrade_attempt_count`
4. `duration_lock_count`
5. `soft_reset_count`
6. `defensive_override_count`
7. `state_flip_count_5d`
   - 5 日内状态来回切换次数

建议输出：

1. `backtest/state_machine_metrics.json`
2. `backtest/state_machine_metrics.md`

### 3.2 Transition Audit Surfacing

目标：

把 `transition_audit` 从“仅存在于 packet 中的调试数据”升级成：

1. 每日运行可见
2. 调试时可追踪
3. 回放时可聚合

建议输出层：

1. `run_status_[DATE].json`
   - 已有，继续保留完整审计
2. `reports/[DATE].md`
   - 增加简洁的状态迁移摘要
3. `decision_packet_[DATE].json`
   - 继续保留结构化 audit

建议增加的日报摘要字段：

1. `Transition`
   - `from -> to`
2. `Reset`
   - `Confirmed / Blocked / N/A`
3. `Duration Lock`
   - `Triggered / Not Triggered`
4. `Core Breakdown`
   - `Yes / No`
5. `Soft Reset`
   - `Applied / Not Applied`

---

## 4. 具体任务

### P0-1 回测指标接入

修改范围：

1. [backtest.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/backtest.rs)
2. `market_regime` 相关统计聚合路径

要求：

1. 能按历史窗口统计状态机事件频次
2. 能和旧版本结果做横向对比

验收：

1. 输出结构化指标文件
2. 能回答“V1.1 是否减少错误 reset”

### P0-2 日报接入 Transition Summary

修改范围：

1. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

要求：

在归档 Markdown 中增加一个极简块：

```text
Transition
- Lifecycle: ESTABLISHED -> EARLY_CONFIRMATION
- Reset: Blocked
- Duration Lock: No
- Core Breakdown: No
- Soft Reset: Yes
```

验收：

1. 不破坏 Telegram 当前产品化样式
2. 归档报告里能快速解释状态变化

### P1-1 调试输出接入

目标：

为本地调试或开发模式增加更清楚的状态迁移日志。

要求：

1. 在 CLI debug/info 输出中打印 transition 摘要
2. 不污染 Telegram 文案

验收：

1. 一眼能看出：
   - 为什么降级
   - 为什么 reset 被挡住
   - 为什么触发防御

### P1-2 回测对比报告

目标：

生成 V1.0 vs V1.1 的状态机质量对比。

建议指标：

1. reset 次数
2. blocked reset 次数
3. state flip 次数
4. defensive 触发次数
5. 平均持续时间

输出：

1. `backtest/state_machine_comparison.md`

---

## 5. 验收标准

### 5.1 功能验收

1. 每次 backtest 都能生成状态机质量指标
2. 每次 daily pipeline 都能在归档报告中看到 transition 摘要
3. `run_status`、`decision_packet`、Markdown 三者的 transition 口径一致

### 5.2 工程验收

1. `cargo test -q`
2. `cargo clippy --all-targets --all-features -- -D warnings`

### 5.3 设计验收

1. 报告里能解释“为什么今天不是 reset”
2. 报告里能解释“为什么今天降级了”
3. 报告里能解释“为什么今天进入防御”

---

## 6. 一句话要求

V1.2 不是继续改状态机。  
V1.2 是把 V1.1 的状态机变成可验证、可观测、可回放的系统。

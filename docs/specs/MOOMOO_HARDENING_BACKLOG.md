# Moomoo Hardening Backlog

## 1. Purpose

本文件记录 Sentinel 在 moomoo/OpenD 接入层剩余的硬化任务。

当前状态不是“未接入”，而是：

1. 核心交易链路已接通
2. DryRun / Live 基础模式已成立
3. 但 broker integration 仍有若干生产级增强项未完成

本 backlog 只关注接入层，不重复策略层任务。

## 2. Current Position

截至当前版本，已完成：

1. OpenD 连接
2. 历史 K 线拉取
3. 交易解锁
4. 资金查询
5. 下单主路径
6. DryRun / Live 分离
7. 运行审计与失败语义闭环

剩余任务主要是：

1. 订单生命周期闭环
2. 行情权限自检
3. broker-side reconciliation
4. 限流与 quota awareness

## 3. Priority Model

### P1

高价值硬化项。  
不阻断当前日频低频使用，但直接影响生产可靠性。

### P2

增强项。  
用于提升扩展性、可观测性和未来实时化能力。

## 4. P1 Tasks

### P1-1: Order Lifecycle Closure

**Problem**

当前订单成功更多表示 “submitted to broker”，而不是完整生命周期闭环。

系统尚未结构化处理：

1. `Filled`
2. `Partially Filled`
3. `Cancelled`
4. `Rejected`
5. `Expired`

**Why It Matters**

如果没有订单生命周期闭环：

1. `ledger` 可能与 broker 实际状态偏离
2. `run_status` 只能记录提交结果，不能记录最终结果
3. 执行归因会失真

**Task**

1. 增加 broker-side 订单状态查询能力
2. 在下单后引入状态确认或轮询机制
3. 将最终状态写入结构化审计
4. 明确本地 `ledger` 与 broker 结果的同步规则

**Suggested Output**

新增或扩展：

1. `TradeExecutionAudit`
2. `ExecutionSummary`
3. `ledger.csv`
4. `run_status_[DATE].json`

建议记录字段：

1. `symbol`
2. `side`
3. `qty_requested`
4. `qty_filled`
5. `order_id`
6. `submit_status`
7. `final_status`
8. `avg_fill_price`
9. `broker_error`

**Acceptance Criteria**

1. 每笔订单不只记录 `Submitted`
2. `run_status` 可反映最终订单状态
3. `ledger` 与 broker 状态不一致时有显式标记
4. 至少有一组集成测试覆盖：
   - success
   - partial fill / partial failure
   - reject / cancel

### P1-2: Quote Authority Preflight

**Problem**

当前尚未在启动阶段自动检查 watchlist 对应的行情权限。

**Why It Matters**

如果缺少 quote right：

1. 运行时会在 fetch 阶段失败
2. 操作员无法区分“网络问题”和“权限问题”
3. Live 模式下可能在无效数据条件下运行

**Task**

1. 启动时检查当前账户/环境的行情权限
2. 检查 watchlist 涉及市场是否在权限范围内
3. 明确权限不足时的行为：
   - 降级为观察模式
   - 禁止进入 Live
   - 记录结构化原因

**Suggested Output**

建议新增：

1. `quote_authority_status`
2. `preflight_result`
3. `run_status` 中的权限摘要

**Acceptance Criteria**

1. 若 watchlist 中存在无权限市场或数据类型，系统能明确报出
2. 不能出现“权限不足但静默继续 live execution”
3. 失败原因必须结构化落盘

## 5. P2 Tasks

### P2-1: Unified Rate Limiter

**Problem**

当前仅有交易执行路径上的 1 秒 sleep，属于保守节流，不是统一限流器。

**Task**

1. 为交易请求增加集中式 limiter
2. 为历史数据/实时数据请求增加可配置 limiter
3. 预留 quota-aware 调度能力

**Acceptance Criteria**

1. 不同 API 通道有明确限流策略
2. 不依赖散落的 `sleep()` 作为长期方案

### P2-2: Broker-side Position Reconciliation

**Problem**

当前组合快照高度依赖本地 `ledger`。

**Task**

1. 增加 broker 持仓查询
2. 定期比对 broker positions 与 `ledger`
3. 输出 reconciliation 结果

**Acceptance Criteria**

1. 可检测本地账本与 broker 持仓偏差
2. 偏差可记录并告警

### P2-3: Subscription-based Quote Path

**Problem**

当前主链仍然是 batch/radar 方式，没有进入 OpenAPI 的实时订阅优势。

**Task**

1. 增加 quote subscription 实验链
2. 处理 push 消息
3. 评估 quota 与稳定性

**Acceptance Criteria**

1. 有独立的实时订阅模式或实验路径
2. 不影响现有日频稳定性

### P2-4: Product Boundary Documentation

**Problem**

如果不明确写清边界，团队可能误以为当前已支持全部市场和全部交易模式。

**Task**

在规范文档中明确：

1. 当前生产目标为美股
2. 当前不支持日本股票自动交易
3. 当前未完成高频/实时事件驱动交易

**Acceptance Criteria**

1. README 与 specs 口径一致
2. 不再出现“能力外延大于实现边界”的描述

## 6. Recommended Execution Order

建议顺序：

1. `P1-1 Order Lifecycle Closure`
2. `P1-2 Quote Authority Preflight`
3. `P2-1 Unified Rate Limiter`
4. `P2-2 Broker-side Position Reconciliation`
5. `P2-4 Product Boundary Documentation`
6. `P2-3 Subscription-based Quote Path`

## 7. Governance Note

本文件属于 `specs/`，因为它定义的是当前 moomoo 接入层的正式剩余范围。  
如果其中任务完成，应同步更新：

1. `MOOMOO_OPENAPI_ASSESSMENT.md`
2. `MOOMOO_INTEGRATION_CHECKLIST.md`
3. `README.md`

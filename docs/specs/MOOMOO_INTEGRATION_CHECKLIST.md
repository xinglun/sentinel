# Moomoo Integration Checklist

## 1. Purpose

本清单用于 Sentinel 接入 moomoo/OpenD 时的工程检查、上线前检查和运维检查。

目标不是“能跑就行”，而是确保：

1. 账号可用
2. 权限齐全
3. OpenD 正常
4. DryRun / Live 语义清晰
5. 交易与数据风险可控

## 2. Account Readiness

上线前必须确认：

1. 已开通对应交易市场账户
2. moomoo App / OpenD 账号可正常登录
3. 账户已完成所有必要协议确认
4. 账户 ID 已确认可用于 OpenAPI

必须确认的具体项：

1. `acc_id`
2. 目标市场是否开通交易权限
3. 账户是否为证券通用账户
4. 是否使用模拟账户还是实盘账户

## 3. Environment Readiness

必须确认：

1. OpenD 已安装
2. OpenD 正在运行
3. OpenD 版本可用
4. 主机与端口可连通
5. 网络环境允许与 OpenD 保持连接

配置项检查：

1. `futu.opend_ip`
2. `futu.opend_port`
3. `futu.trd_env`
4. `futu.market`
5. `FUTU_ACC_ID`
6. `FUTU_UNLOCK_PASSWORD_MD5`

## 4. Market Scope Check

必须先明确本次支持的市场范围。

当前 Sentinel 应按以下口径执行：

1. 美股：支持
2. 美股 ETF：支持
3. 美股期权：只有在策略和执行链明确支持后才可启用
4. 日股：当前不纳入自动交易目标

## 5. Quote Authority Check

必须确认：

1. 当前 watchlist 涉及的市场都有对应行情权限
2. 若未来使用订阅/盘口/逐笔，则对应 quote right 已开通
3. 若权限不足，必须明确降级策略

降级策略示例：

1. 订阅失败时回退到 snapshot / daily history
2. 关键 symbol 无权限时禁止进入 Live

## 6. Trading Authority Check

必须确认：

1. 目标市场具备交易权限
2. 当前账户确实允许该产品交易
3. 模拟盘与实盘环境被明确区分

上线前必须明确：

1. `trd_env = 1` 代表模拟
2. `trd_env = 0` 代表实盘
3. `trading.enabled = false` 时不得进入实盘执行

## 7. Runtime Preflight

每次 `daemon` 或 `live` 执行前，必须做：

1. OpenD 连通性检查
2. 账户资金查询
3. 交易解锁检查
4. 交易市场与账户匹配检查
5. 当前模式确认：
   - `Disabled`
   - `DryRun`
   - `Live`

若任一项失败：

1. 记录 `run_status`
2. 不得静默继续
3. 在 `Live` 模式下必须中止

## 8. Execution Safety

必须确认：

1. `ExecutionGate` 已生效
2. `global_budget` 已生效
3. `max_daily_budget` 已生效
4. `buying_power` 已生效
5. `TradingDisabled` 能被明确记录

## 9. Rate Limit and Quota Controls

必须逐项检查：

1. 下单频率是否低于官方限制
2. watchlist 拉取频率是否会撞历史 K 线额度
3. 若启用订阅，是否有 quota awareness

当前建议：

1. 维持日频/低频交易模式
2. 不在当前版本引入高频下单
3. 若引入批量下单，先增加统一限流器

## 10. Broker Reconciliation

当前版本上线前至少应明确：

1. 本地 `ledger` 不是最终权威来源
2. 资金快照来自 broker
3. 后续应增加：
   - 订单状态回查
   - broker-side 仓位查询
   - 成交与持仓对账

## 11. Logging and Audit

必须确保以下文件持续可用：

1. `execution_gate_log.jsonl`
2. `run_status_YYYY-MM-DD.json`
3. `account_snapshot_YYYY-MM-DD.json`
4. `portfolio_snapshot_YYYY-MM-DD.json`
5. `ledger.csv`

对 moomoo/OpenD 接入来说，最低验收要求是：

1. 每一次交易尝试都有审计记录
2. 每一次失败都有结构化原因
3. Live 失败会打红 workflow 或本地任务

## 12. Go / No-Go Decision

### Go

可以进入 DryRun / Live 的条件：

1. 账户已开通
2. OpenD 连通
3. 资金查询成功
4. 解锁逻辑正常
5. 目标市场受支持
6. Watchlist 权限满足
7. 审计与 run_status 正常落盘

### No-Go

以下任一项成立，就不应进入 Live：

1. OpenD 不稳定
2. `get_funds()` 失败
3. 交易市场权限不明确
4. 目标品类不在当前产品边界内
5. workflow / audit / run_status 不完整
6. 无法确认是模拟盘还是实盘

## 13. Current Sentinel Positioning

截至当前版本，Sentinel 对 moomoo/OpenD 的定位应写成：

1. 已接通美股日频观测与执行主路径
2. 已接通 DryRun 与 Live 基本模式
3. 已完成从柜台拉取真实持仓的自动化对账
4. 已完成订单生命周期的全自动回查与撤单确认
5. 尚未完成实时订阅（Qot_Sub）与毫秒级执行流

# Moomoo OpenAPI Assessment

## 1. Purpose

本文档定义 moomoo OpenAPI 对 Sentinel 的实际意义、当前官方能力边界，以及当前工程的实现完成度。

本文件用于回答三个问题：

1. 官方能力是否支持 Sentinel 当前方向
2. 当前代码已经实现到什么程度
3. 距离“稳定生产交易接入层”还差什么

## 2. Primary Sources

1. [OpenAPI Introduction](https://openapi.moomoo.com/moomoo-api-doc/en/intro/intro.html)
2. [Authorities and Limitations](https://openapi.moomoo.com/futu-api-doc/en/intro/authority.html)
3. [Place Orders](https://openapi.moomoo.com/moomoo-api-doc/en/trade/place-order.html)
4. [Subscribe and Unsubscribe](https://openapi.moomoo.com/moomoo-api-doc/en/quote/sub.html)

## 3. High-Level Conclusion

结论很明确：

1. Sentinel 接入 moomoo/OpenD 做美股行情与交易，方向成立
2. 当前工程已经实现了最核心的交易骨架
3. 但当前仍应被定义为“已接通核心链路”，不是“已完成全部生产级 broker integration hardening”

## 4. Official Capability Summary

### 4.1 Architecture

官方 OpenAPI 由两部分组成：

1. `OpenD`
   - 本地或云端运行的网关进程
   - 通过 TCP 暴露协议接口
2. `moomoo API`
   - 官方 SDK
   - 支持 Python / Java / C# / C++ / JavaScript
   - 非官方语言也可直接对接协议

这与 Sentinel 当前的 `FutuClient -> OpenD -> moomoo` 架构一致。

### 4.2 Functional Scope

OpenAPI 的两大能力：

1. `Quotation`
   - 实时订阅
   - 快照
   - 历史 K 线
   - Tick / Order Book 等
2. `Trading`
   - Paper Trading
   - Live Trading

### 4.3 Market Scope Relevant to Sentinel

对 Sentinel 当前目标最重要的结论：

1. 美股股票 / ETF：支持行情、模拟交易、实盘交易
2. 美股期权：支持
3. 日本股票：当前对 moomoo users 不支持

因此：

1. Sentinel 当前以美股为核心目标是正确的
2. 不应把当前接入口径误扩展为“已支持日股自动交易”

## 5. Official Constraints That Matter

### 5.1 Account and Authority

官方限制：

1. 需要先开通对应市场的交易业务账户
2. 行情权限和市场权限不是天然全开
3. 不同市场、不同数据类型需要对应 authority

工程意义：

1. 代码接好不等于账号就可用
2. 上线前必须做 account/authority preflight

### 5.2 Rate Limits

官方明确指出，交易接口存在限频。

以 `Place Order` 为例：

1. 同一 `acc_id` 下 30 秒最多 15 次请求
2. 两次连续请求间隔不能小于 0.02 秒

工程意义：

1. 日频/低频策略暂时问题不大
2. 未来批量下单或事件驱动模式必须增加限流保护

### 5.3 Quotas

官方明确指出：

1. 实时订阅有 quota
2. 历史 K 线也有 quota

工程意义：

1. watchlist 扩大或高频重复拉历史数据时需要关注额度
2. 后续若启用订阅接口，必须增加 quota awareness

### 5.4 Trading Session Constraints

官方明确指出：

1. Live account 交易前需要 unlock
2. Paper trading 不需要 unlock
3. US 24-hour trading 有订单类型限制

工程意义：

1. Sentinel 当前的 `ExecutionMode + trd_env + unlock_trade` 设计方向正确
2. 后续如果扩到盘前盘后/夜盘，不能继续默认当前订单逻辑

## 6. Sentinel Implementation Status

### 6.1 Already Implemented

当前工程已具备：

1. OpenD TCP 连接
2. 历史 K 线拉取
3. 交易解锁
4. 资金查询
5. 下单
6. 模拟/实盘切换
7. 失败语义与运行审计

对应代码：

1. `src/adapters/futu/client.rs`
2. `src/adapters/futu/provider.rs`
3. `src/adapters/futu/trader.rs`
4. `src/cli.rs`
5. `src/core/execution_gate.rs`
6. `src/core/trader_agent.rs`

### 6.2 Not Yet Fully Implemented

以下能力仍未完全落地到生产主链：

1. 权限前置检查
2. 订阅额度/历史 K 线额度检查
3. 全局限流器
4. 订单状态回查
5. 仓位查询与 broker-side authoritative reconciliation
6. 撤单/改单
7. 订阅式实时行情主链

## 7. Capability Matrix

| Capability | Official Support | Sentinel Status | Assessment |
| --- | --- | --- | --- |
| OpenD gateway | Yes | Implemented | Ready |
| Historical daily K-line | Yes | Implemented | Ready |
| Account funds | Yes | Implemented | Ready |
| Unlock trade | Yes | Implemented | Ready |
| Place order | Yes | Implemented | Ready |
| Paper/Live switch | Yes | Implemented | Ready |
| Quote subscriptions | Yes | Not in main path | Pending |
| Order book / tick stream | Yes | Not in main path | Pending |
| Order status reconciliation | Yes | Not in main path | Pending |
| Position reconciliation | Yes | Not in main path | Pending |
| Modify/cancel order | Yes | Not implemented | Pending |
| Authority preflight | Required | Not complete | Gap |
| Rate limiting | Required | Not complete | Gap |
| Quota awareness | Required | Not complete | Gap |

## 8. Product Boundary

当前对外可成立的产品边界是：

1. 基于 moomoo/OpenD 的美股日频观测
2. 基于 moomoo/OpenD 的美股模拟交易
3. 基于 moomoo/OpenD 的美股低频/日频实盘执行主路径

当前不应对外宣称：

1. 日本股票自动交易已支持
2. 高频交易已支持
3. 实时订阅驱动策略已完成
4. 完整 broker-side reconciliation 已完成

## 9. Engineering Decision

当前建议将 moomoo/OpenAPI 集成定义为：

1. `Core execution path connected`
2. `Production-hardening still required for broker integration`

这意味着：

1. 策略层和执行主路径可以继续推进
2. 但经纪商接入层仍需专项硬化，不应视为已完成

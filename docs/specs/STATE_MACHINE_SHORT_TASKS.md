# Sentinel 状态机改造短任务单

## P0

### P0-1 实现 InertiaLayer

目标：

1. 在原始信号和状态判定之间增加惯性层
2. 将 `reset gate`、`downgrade gate`、`duration lock`、`defensive override` 集中实现

修改范围：

1. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
2. 相关配置加载路径

验收：

1. 非防御性回调不再直接 reset 到 `IGNITION`
2. `ANY -> DEFENSIVE` 仍保持最高优先级

### P0-2 固化 Reset Gate

目标：

只有满足以下全部条件，才允许 `IGNITION` reset：

1. `TrendDominant == false`
2. `Structural < 25`
3. `Stability < 10` 连续 3 天
4. `Flow <= 0`
5. `CoreAssetsBreakdown == true`

验收：

1. `ESTABLISHED -> IGNITION` 必须是稀有且可解释事件
2. `market_regime.reasons` 中必须留下 reset/blocked-reset 原因

### P0-3 阶梯式降级

目标：

普通生命周期降级最多单级：

1. `CONFIRMED -> ESTABLISHED`
2. `ESTABLISHED -> EARLY_CONFIRMATION`
3. `EARLY_CONFIRMATION -> NEWBORN`

禁止：

1. `ESTABLISHED -> IGNITION`
2. `CONFIRMED -> NEWBORN`

例外：

1. `ANY -> DEFENSIVE`

验收：

1. 正常回调只会单级降级
2. 结构破坏仍可直接进入 `DEFENSIVE`

## P1

### P1-1 Duration Lock

目标：

1. 升级和 reset 必须受时间锁保护
2. 防御性降级不受阻断

建议实现：

1. `min_upgrade_duration`
2. `soft_downgrade_lock`
3. `hard_defensive_override`

验收：

1. 不再出现“今天 reset，明天恢复”的抖动
2. 不会把防抖做成风险迟钝

### P1-2 Core Assets 配置化

目标：

1. `core_assets` 从代码常量提升为配置
2. `CoreAssetsBreakdown` 判定依赖配置，而非 symbol 硬编码

验收：

1. `core_assets` 可在配置中定义
2. `reset gate` 使用配置化核心资产集合

### P1-3 State Transition Log

目标：

结构化记录每次状态变化与阻断原因。

最少字段：

1. `from`
2. `to`
3. `blocked_reset`
4. `core_assets_breakdown`
5. `reasons`

验收：

1. 可以定位“为什么没降级 / 为什么没 reset / 为什么进入防御”

## P2

### P2-1 个股恢复门槛

目标：

强制个股恢复路径：

`DEFEND -> CAUTION -> CRUISE -> OPTIMAL`

验收：

1. 不允许 `DEFEND -> OPTIMAL`
2. 每一步都要经过 cooldown

### P2-2 Historical Penalty

目标：

最近 20 日内曾处于 `DEFEND` 的资产：

1. 默认 `max_state = CRUISE`
2. 满足额外结构确认后才可解除限制

验收：

1. 弱资产不能单日洗白

### P2-3 FORMING 保护

目标：

`FORMING` 资产不能因市场 reset 或市场改善被自动抬升为强状态。

验收：

1. `FORMING` 只能在满足独立结构成熟条件后进入正常状态机

## 交付顺序

1. `P0-1`
2. `P0-2`
3. `P0-3`
4. `P1-1`
5. `P1-2`
6. `P1-3`
7. `P2-1`
8. `P2-2`
9. `P2-3`

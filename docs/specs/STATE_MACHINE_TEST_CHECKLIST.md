# Sentinel 状态机改造测试清单

## 1. 市场状态机

### T1 非防御性回调不允许直接 reset

场景：

1. 前态 `ESTABLISHED`
2. `confidence / flow / stability` 变弱
3. 核心资产仍未崩坏

期望：

1. 允许降级到 `EARLY_CONFIRMATION`
2. 不允许直接回到 `IGNITION`

### T2 单级降级约束

场景：

1. 前态 `CONFIRMED`
2. 普通回调

期望：

1. 只能到 `ESTABLISHED`
2. 不允许跳到 `NEWBORN`

### T3 Reset Gate 通过

场景：

以下全部成立：

1. `TrendDominant == false`
2. `Structural < 25`
3. `Stability < 10` 连续 3 天
4. `Flow <= 0`
5. `CoreAssetsBreakdown == true`

期望：

1. 允许 reset 到 `IGNITION`

### T4 Reset Gate 被阻断

场景：

1. 上述条件缺 1 项或多项

期望：

1. 不允许 reset
2. 结构化日志中记录 `blocked_reset`

### T5 Defensive Override

场景：

1. 满足明确防御触发

期望：

1. 任意状态可直接进入 `DEFENSIVE`
2. 不受 `max_step` 或 duration lock 阻断

## 2. Duration Lock

### T6 升级时间锁

场景：

1. 状态刚进入某生命周期
2. 未满足最小停留时间

期望：

1. 不允许继续升级

### T7 防抖不能阻断防御

场景：

1. 虽处于锁定窗口
2. 但明确触发 `DEFENSIVE`

期望：

1. 仍直接进入 `DEFENSIVE`

## 3. Core Assets

### T8 Core Assets 配置生效

场景：

1. 自定义 `core_assets` 配置

期望：

1. `CoreAssetsBreakdown` 使用配置集，而非硬编码 symbol

### T9 核心资产未崩坏时禁止 reset

场景：

1. 普通信号走弱
2. 核心资产仍保持主结构

期望：

1. 不允许 reset 到 `IGNITION`

## 4. 个股恢复

### T10 禁止一步洗白

场景：

1. 前态 `DEFEND`
2. 单日价格/偏离改善

期望：

1. 不允许直接变成 `OPTIMAL`

### T11 阶梯恢复路径

场景：

1. `DEFEND`
2. 连续满足恢复条件

期望：

1. `DEFEND -> CAUTION`
2. `CAUTION -> CRUISE`
3. `CRUISE -> OPTIMAL`

### T12 Cooldown 生效

场景：

1. 恢复信号满足
2. 但 cooldown 未到

期望：

1. 不允许升级

### T13 Historical Penalty 生效

场景：

1. 最近 20 日内出现过 `DEFEND`
2. 短期结构改善

期望：

1. 默认 `max_state = CRUISE`
2. 不允许直接进入 `OPTIMAL`

### T14 FORMING 保护

场景：

1. `FORMING` 资产
2. 市场状态改善或 reset

期望：

1. 不会被自动抬升为强状态

## 5. 日志与报告一致性

### T15 State Transition Log 完整性

期望最少字段：

1. `from`
2. `to`
3. `blocked_reset`
4. `core_assets_breakdown`
5. `reasons`

### T16 Telegram / 报告一致性

同一批资产必须共享同一套 bucket：

1. `Top Actions`
2. `战术分区`
3. `风险与机会`

期望：

1. 不出现 “Top Actions 里的机会标的不在机会区”
2. 不出现 “防御资产同时出现在机会区”

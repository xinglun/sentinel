# Sentinel 状态机改造首页摘要

## 目标

当前系统的问题，不是信号不足，而是缺少时间惯性层。  
这会导致状态机偏“瞬时响应”，容易把：

1. 趋势减弱
2. 内部洗盘
3. 局部失衡

误判成：

1. 趋势重启
2. 生命周期归零
3. 弱资产即时洗白

本次改造的核心目标是：

**把状态机从反应型系统，升级成有记忆的时间系统。**

## 最终分层结构

1. `RawSignalLayer`
2. `InertiaLayer`
3. `RegimeDecisionLayer`
4. `ExecutionLayer`
5. `NarrativeLayer`

其中当前最缺失、且优先级最高的是：

**`InertiaLayer`**

## 固化的系统原则

1. `Decision Primitive != Narrative`
2. `Execution State != Market State`
3. `Trend = Continuity + Strength`
4. `Continuity` 有惯性，`Strength` 可快速波动
5. `Reset` 必须被证明，不能被猜测
6. `Narrative` 只能消费最终状态，不能反向参与判定

## 本次改造要落地的核心规则

### 1. Reset Gate

只有满足以下全部条件，才允许回到 `IGNITION`：

1. `TrendDominant == false`
2. `Structural < 25`
3. `Stability < 10` 连续 3 天
4. `Flow <= 0`
5. `CoreAssetsBreakdown == true`

其中：

1. `CoreAssetsBreakdown` 必须存在
2. 不允许仅根据单日 `confidence / stability` 触发 reset
3. `core_assets` 必须来自配置，不能写死在代码常量里

### 2. Downgrade Gate

普通生命周期降级：

`max_step = 1`

允许：

1. `CONFIRMED -> ESTABLISHED`
2. `ESTABLISHED -> EARLY_CONFIRMATION`
3. `EARLY_CONFIRMATION -> NEWBORN`

禁止：

1. `ESTABLISHED -> IGNITION`
2. `CONFIRMED -> NEWBORN`

例外：

1. `ANY -> DEFENSIVE` 永远允许
2. 防御性降级不受普通 `step` 限制

### 3. Duration Lock

`Duration Lock` 只保护：

1. 升级
2. reset

不阻断：

1. 防御性降级

补充约束：

1. `soft downgrade` 只受弱约束或不受约束
2. `hard defensive override` 永远最高优先级

### 4. 个股恢复路径

个股状态恢复必须阶梯化：

`DEFEND -> CAUTION -> CRUISE -> OPTIMAL`

不允许一步洗白。每一步都要有 cooldown。

### 5. Historical Penalty

最近 20 日内曾处于 `DEFEND` 的资产：

1. 默认最大状态上限锁定为 `CRUISE`
2. 解除限制前，必须满足额外结构确认条件

## 下一步只做 3 件事

1. 实现 `InertiaLayer`
2. 增加 `State Transition Log`
3. 固化 `core_assets` 配置

## 给开发的最终一句

**不是继续调参数。  
是先把状态机从“瞬时响应器”升级成“有记忆的时间系统”。**

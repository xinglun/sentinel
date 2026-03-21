# Sentinel 状态机惯性加固规范 (STATE_MACHINE_INERTIA_HARDENING.md)

## 1. 目的

本规范用于修正以下问题：

1. 市场状态在正常回调中被过度重置为 `IGNITION`。
2. `stability` / `regime_age` 在非崩坏场景下被清零，导致趋势生命周期失真。
3. 个股状态缺乏历史惯性，弱资产可能在短期局部反弹后被错误恢复为 `OPTIMAL`。
4. Telegram / 报告层出现“市场重启”与“个股仍强/突然转强”并存的逻辑冲突。

本规范是对现有 [TRANSITION_RULES.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/TRANSITION_RULES.md) 和 [STATE_DEFINITIONS.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/STATE_DEFINITIONS.md) 的加固补充。

## 2. 给开发的修正规格

### 2.1 核心目标

状态机必须满足以下行为约束：

1. 趋势可以减弱，但不会一夜归零。
2. `IGNITION` 只能表示“新趋势刚启动”，不能被用作普通回调的默认降级状态。
3. `ESTABLISHED` / `EARLY_CONFIRMATION` 的回调应优先表现为“降级”而不是“重启”。
4. 个股状态必须具备恢复门槛；历史弱资产不能在单日局部改善后直接恢复为 `OPTIMAL`。

### 2.2 必须实现的修正

1. 为市场状态机增加 `reset gate`。
   - 不满足严格 reset 条件时，禁止回到 `IGNITION`。
2. 为生命周期增加“阶梯式降级”规则。
   - 默认只允许单级降级，不允许跨级归零。
3. 为个股状态增加“恢复路径”。
   - `DEFEND -> OPTIMAL` 必须拆成多步恢复。
4. 为近期弱资产增加“历史惩罚”。
   - 最近处于 `DEFEND` / `CAUTION` 的资产必须经过额外确认窗口。
5. 对 `stability` / `age` 的 reset 增加保护。
   - 非硬 reset 场景下，`regime_age` 不得回到 `1`。

### 2.3 代码落点建议

1. 市场状态机：
   - [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
2. 个股状态与动作联动：
   - `asset_state` 相关模块
   - [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)
3. 报告层诊断输出：
   - [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)
   - `market_regime.reasons` 中需要明确记录：
     - 为什么降级
     - 为什么没有 reset
     - 为什么个股恢复被拦截

### 2.4 验收标准

以下场景必须通过测试：

1. `ESTABLISHED` 在中等回调中降级为 `EARLY_CONFIRMATION`，而不是 `IGNITION`。
2. `EARLY_CONFIRMATION` 在回调中降级为 `NEWBORN`，而不是 `IGNITION`。
3. 只有满足严格 reset 条件时，生命周期才允许回到 `IGNITION`。
4. 最近 20 个交易日内出现过 `DEFEND` 的资产，不能在单日改善后直接变成 `OPTIMAL`。
5. `DEFEND -> CAUTION -> CRUISE -> OPTIMAL` 的恢复路径必须被单元测试覆盖。

---

## 3. 具体状态迁移规则

### 3.1 市场状态升级规则

沿用现有升级主路径：

1. `NONE -> IGNITION`
2. `IGNITION -> NEWBORN`
3. `NEWBORN -> EARLY_CONFIRMATION`
4. `EARLY_CONFIRMATION -> ESTABLISHED`
5. `ESTABLISHED -> CONFIRMED`

升级逻辑保持“慢确认”原则，不是本次改动重点。

### 3.2 市场状态降级规则

默认采用阶梯式降级：

1. `CONFIRMED -> ESTABLISHED`
2. `ESTABLISHED -> EARLY_CONFIRMATION`
3. `EARLY_CONFIRMATION -> NEWBORN`
4. `NEWBORN -> IGNITION`
5. `ANY -> DEFENSIVE` 仅保留给硬风险触发

### 3.3 允许直接进入 `DEFENSIVE` 的条件

以下条件保留快速逃生优先级：

1. `system_confidence < 50`
2. 核心资产群集体跌入 `DEFEND / CAUTION`
3. `risk_overlay` 达到 `DEFENSIVE / BROKEN`
4. 结构性破坏已明确，不属于普通回调

### 3.4 Reset Gate：允许回到 `IGNITION` 的严格条件

从 `EARLY_CONFIRMATION / ESTABLISHED / CONFIRMED` 回到 `IGNITION`，必须同时满足：

1. `TrendDominant == false` 或等价主趋势判定失效
2. `stability_structural < 25`
3. `stability_score < 10` 连续 `3` 天
4. `flow_acceleration <= 0`
5. 核心资产群不再维持主升结构

任一条件不满足：

1. 禁止 reset
2. 只能执行阶梯式降级

### 3.5 Age / Stability 保护规则

1. 只有在通过 `reset gate` 后，`regime_age` 才允许重置为 `1`。
2. 普通降级时：
   - `regime_age` 允许延续
   - 或按规则做“软回退”，但不得归零
3. `stability_score` 不允许因为生命周期降级而直接清零，除非：
   - 进入 `DEFENSIVE`
   - 或通过 `reset gate`

### 3.6 建议的诊断标签

为了便于 Telegram / 审计解释，建议在 `market_regime.reasons` 中加入标准化标签：

1. `DowngradeOnly`
2. `ResetBlockedByInertia`
3. `ResetConfirmed`
4. `CoreStructureStillIntact`
5. `StructuralBreakConfirmed`

---

## 4. 个股恢复门槛规则

### 4.1 恢复路径

禁止以下一步到位恢复：

1. `DEFEND -> OPTIMAL`
2. `DEFEND -> PULLBACK`
3. `CAUTION -> OPTIMAL`

建议的恢复路径为：

1. `DEFEND -> CAUTION`
2. `CAUTION -> CRUISE`
3. `CRUISE -> PULLBACK / OPTIMAL`

### 4.2 DEFEND 恢复门槛

`DEFEND -> CAUTION` 至少需要：

1. 长周期破坏条件解除
2. 关键均线斜率不再继续恶化
3. 连续 `N=3` 天未再次触发 `DEFEND`

### 4.3 CAUTION 恢复门槛

`CAUTION -> CRUISE` 至少需要：

1. 重新站回核心引力带
2. 波动收敛
3. 连续 `N=3~5` 天结构稳定

### 4.4 CRUISE 恢复为强状态的门槛

`CRUISE -> PULLBACK / OPTIMAL` 至少需要：

1. 趋势斜率重新转正
2. Owner/Leash 结构恢复一致
3. 不能存在近期 `DEFEND` 未解锁惩罚

### 4.5 历史惩罚规则

若资产在最近 `20` 个交易日内曾处于 `DEFEND`：

1. 默认上限锁定为 `CAUTION / CRUISE`
2. 只有在满足额外恢复窗口后，才允许进入 `PULLBACK / OPTIMAL`
3. 建议额外恢复窗口：
   - 连续 `5` 天结构稳定
   - 长周期斜率恢复
   - 无新的破位事件

### 4.6 FORMING 资产限制

`FORMING` 资产不得因为市场状态 reset 而被自动抬升为强状态。

1. `FORMING` 只能保持 `FORMING / OBSERVE`
2. 需要单独满足结构成熟条件后，才允许进入正常个股状态机

### 4.7 报告层一致性要求

以下输出必须共享同一套个股 bucket 结果：

1. `Top Actions`
2. `战术分区`
3. `风险与机会`

如果某资产被标为：

1. `DEFEND / CAUTION`
   - 不得同时进入“机会”或“加仓区”
2. `OPTIMAL / PULLBACK`
   - 不得在同一条消息中被归入“防御区”

---

## 5. 测试清单

建议新增或强化以下测试：

1. `ESTABLISHED` 回调不直接 reset 到 `IGNITION`
2. `EARLY_CONFIRMATION` 回调只降级到 `NEWBORN`
3. 满足全部 reset gate 条件时才允许 `IGNITION`
4. `DEFEND` 资产单日反弹后不能直达 `OPTIMAL`
5. `DEFEND` 资产经过多日修复后可按路径恢复
6. Telegram 输出中：
   - `Top Actions`
   - `战术分区`
   - `风险与机会`
   使用同一套 bucket

---

## 6. 一句话规则

趋势可以减弱，但不会一夜消失。  
状态可以降级，但不能轻易归零。  
弱资产可以修复，但不能瞬间洗白。

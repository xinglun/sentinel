# Sentinel Exit Decision Layer 任务单

## 1. 目标

本任务用于为 Sentinel 新增独立的卖出决策层：

**Exit Decision Layer**

当前系统已经具备：

1. `Market Regime`
2. `Participation Readiness`
3. `Asset State / Ranking`
4. `ActionMatrix`
5. `ExecutionGate`

但当前卖出语义仍不够独立，容易被混入：

1. 买入映射逻辑
2. 报告文案逻辑
3. 局部资产状态解释

本任务目标不是优化买点，而是显式回答：

1. 什么时候必须退出
2. 什么时候应该减仓
3. 什么时候只是停止加仓
4. 什么时候只做利润管理

---

## 2. 设计原则

### 2.1 卖出不是买入的反面

买入是在判断：

1. 现在能不能开始承担风险

卖出是在判断：

1. 现在是不是必须停止承担这类风险

因此：

1. 退出逻辑不能继续堆进 `ActionMatrix`
2. 退出优先级必须高于买入映射

### 2.2 Exit Gate 的职责

Exit Decision Layer 必须负责：

1. 识别硬风险并强制退出
2. 识别主线掉队并触发减仓
3. 识别市场级降温并统一收手
4. 识别过热并执行利润管理

---

## 3. 建议执行顺序

建议将引擎流程调整为：

```text
Raw Signals
→ Inertia / Memory
→ Regime Decision
→ Participation Decision
→ Asset State / Ranking
→ Exit Decision
→ Action Mapping
→ Execution
```

关键原则：

1. 先判断要不要撤
2. 再判断还能不能加

---

## 4. 新增概念

### 4.1 Position Intent

建议统一输出：

```text
position_intent
- ADD
- HOLD
- TRIM
- EXIT
```

### 4.2 Exit 元信息

建议新增结构：

```rust
pub struct ExitDecision {
    pub position_intent: PositionIntent,
    pub asset_exit_state: AssetExitState,
    pub exit_priority: u8,
    pub exit_reasons: Vec<String>,
}
```

建议枚举：

```rust
pub enum AssetExitState {
    None,
    DefensiveExit,
    StrengthLoss,
    ParticipationExit,
    OverheatProfitTake,
}
```

---

## 5. 优先级规则

同一资产可能同时出现多个信号，必须按统一优先级覆盖：

```text
EXIT > TRIM > HOLD > ADD
```

强制要求：

1. `Exit intent always overrides add intent`

也就是说：

1. 如果某资产同时满足 `ADD` 与 `TRIM`
2. 必须以 `TRIM` 为准

---

## 6. 第一版规则

首版只做结构化卖出，不做复杂盈亏管理。

### Rule 1: Defensive Exit

条件：

1. `asset_state == DEFEND`
2. 或 `risk_overlay == DEFENSIVE`
3. 或已存在明确硬风险信号

动作：

1. `position_intent = EXIT`

说明：

1. 这是保命层
2. 不等待 2-3 天确认
3. 先活下来，再讲后续

### Rule 2: Strength Loss Exit

条件：

1. `asset_out_of_top_tier_streak >= 3`
2. 或 `(asset_state from OPTIMAL/CRUISE -> CAUTION) persists >= 2d`

动作：

1. `position_intent = TRIM`

说明：

1. 这是主线管理层
2. 目的不是宣判失败
3. 而是把仓位让给更强资产

### Rule 3: Participation Exit

条件：

1. `participation_ready` 发生 `true -> false`

动作：

1. 禁止新买
2. 弱资产 `TRIM`
3. 核心强资产 `HOLD / FREEZE`

说明：

1. 市场门关了
2. 先停手，再分类处理
3. 不等于全清仓

### Rule 4: Overheat Profit-Take

条件：

1. `asset_state == OVERHEAT`

动作：

1. `position_intent = TRIM`
2. `take_profit_mode = partial`

说明：

1. 只减，不清
2. 这是利润管理，不是趋势反转判断

---

## 7. 需要新增的数据原语

不要从 UI 倒推退出逻辑，应该持久化系统原语。

建议 `DecisionPacket` 至少新增或明确暴露：

1. `top_tier_symbols`
2. `participation`
3. `participation_changed`
4. `asset_top_tier_streak`
5. `asset_out_of_top_tier_streak`
6. `asset_state_streak`
7. `risk_overlay`

建议补充：

1. `asset_previous_state`
2. `asset_exit_blockers`

用途：

1. `asset_previous_state`
   - 用于判断 `OPTIMAL/CRUISE -> CAUTION`
2. `asset_exit_blockers`
   - 用于解释为什么这次没有触发 exit

---

## 8. 建议代码落点

建议新增：

1. [exit.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/exit.rs)

并在以下位置接入：

1. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/engine.rs)
2. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs)
3. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)
4. [execution_gate.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/execution_gate.rs)
5. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

### 8.1 Engine

负责：

1. 在 asset ranking 完成后计算 exit decisions
2. 将 exit 结果与 action mapping 统一收敛
3. 形成最终 `position_intent`

### 8.2 ActionMatrix

职责收缩为：

1. 负责买入/持有的基础映射
2. 不负责定义退出规则
3. 最终结果受 Exit Layer 覆盖

### 8.3 ExecutionGate

负责消费：

1. `position_intent`

而不是继续直接猜测：

1. `ACCUMULATE / REDUCE / HOLD`

---

## 9. 开发任务拆解

### P0-1 新增 ExitDecision 模块

任务要求：

1. 新建 `exit.rs`
2. 定义：
   - `PositionIntent`
   - `AssetExitState`
   - `ExitDecision`
3. 实现首版 4 条规则

### P0-2 扩展 DecisionPacket / 资产决策结构

任务要求：

1. 将 exit 结果写入 `DecisionPacket`
2. 对每个资产记录 exit 相关字段
3. 保证历史包可追踪 streak / previous state / exit intent

### P0-3 Engine 接入统一决策顺序

任务要求：

1. 先算 participation
2. 再算 asset state / ranking
3. 再算 exit decisions
4. 最后合并成 `position_intent`

### P1-1 ExecutionGate 消费 position_intent

任务要求：

1. `ADD` -> 买入路径
2. `TRIM` -> 减仓路径
3. `EXIT` -> 清仓或强制退出路径
4. `HOLD` -> 无交易

### P1-2 报告层补齐退出诊断

任务要求：

1. 显示：
   - 哪些资产被 `TRIM`
   - 哪些资产被 `EXIT`
   - 原因是什么
2. 区分：
   - 保命退出
   - 掉队减仓
   - 市场降温减仓
   - 过热止盈

### P1-3 文档与 schema 更新

任务要求：

1. 更新 [DECISION_PACKET_SCHEMA.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/DECISION_PACKET_SCHEMA.md)
2. 如有必要，更新：
   - `ACTION_MATRIX.md`
   - `STATE_MACHINE_HOME_SUMMARY.md`

---

## 10. 测试清单

至少补以下测试：

1. `DEFEND` 当天必须触发 `EXIT`
2. 连续 3 天掉出 `Top Tier` 必须触发 `TRIM`
3. `participation_ready true -> false` 后禁止新买
4. `OVERHEAT` 只部分减仓，不全清
5. 强资产在市场转冷时不应与弱资产同样处理
6. 强资产仅掉队 1 天，不应触发 `TRIM`
7. 弱资产刚回到 Top Tier 1 天，不应解除历史惩罚
8. `EXIT` 与 `ADD` 冲突时，必须以 `EXIT` 为准
9. `TRIM` 与 `ADD` 冲突时，必须以 `TRIM` 为准

建议落点：

1. `src/core/exit.rs`
2. `tests/pipeline_integration.rs`
3. `src/core/report_ui_tests.rs`
4. `src/core/execution_gate.rs`

---

## 11. 验收标准

本任务完成后，系统必须满足：

1. 卖出逻辑拥有独立层，不再混在 `ActionMatrix`
2. `position_intent` 已成为统一执行原语
3. `EXIT > TRIM > HOLD > ADD` 有明确覆盖规则
4. 系统能区分：
   - 保命退出
   - 结构减仓
   - 市场级去风险
   - 过热止盈
5. 报告层能解释为什么要卖，而不是只展示结果

---

## 12. 非目标

本轮不做：

1. 浮盈回撤模型
2. ATR 止盈
3. 成本线止盈
4. 分批止盈策略优化
5. 更复杂的盈亏归因系统

本轮只做：

**让卖出系统先具备结构正确性。**

---

## 13. 建议开发顺序

建议开发按以下顺序提交：

1. `exit.rs` + 数据结构
2. `DecisionPacket` / schema 扩展
3. `engine.rs` 决策顺序接入
4. `execution_gate.rs` 消费 `position_intent`
5. `report.rs` 增加退出解释
6. 完整测试

---

## 14. 完成定义

当以下条件全部满足时，本任务视为完成：

1. 已存在独立 `Exit Decision Layer`
2. 每个资产都有明确 `position_intent`
3. 退出优先级覆盖规则已生效
4. 买入和卖出不再在 `ActionMatrix` 中互相污染
5. 所有核心退出路径均有测试覆盖
6. 报告层能清楚解释“为什么该收手”

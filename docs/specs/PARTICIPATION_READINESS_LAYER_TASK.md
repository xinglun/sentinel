# Sentinel Participation Readiness Layer 任务单

## 1. 目标

本任务用于在 Sentinel 当前架构中新增一层明确的“参与许可机制”。

当前系统已经具备：

1. 市场层状态机 `Market Regime`
2. 个股层连续性与记忆 `Relative Strength Memory`
3. 动作映射层 `ActionMatrix`
4. 执行风控层 `ExecutionGate`

但仍缺失一个关键层：

**系统尚未显式回答“现在是否允许开始参与市场”。**

当前问题表现为：

1. 资产层已经可以根据连续性升到 `OPTIMAL`
2. 市场层也可以识别 `IGNITION / NEWBORN / ...`
3. 但系统缺少一个全局开关，判断“当前是否可以相信这些资产层判断”

因此，本任务不是继续优化选股，而是新增：

**Participation Readiness Layer**

用于回答：

1. 当前是否允许参与风险资产
2. 当前是否只允许观察候选
3. 何时从“候选阶段”转入“可参与阶段”

---

## 2. 为什么要新增这一层

### 2.1 当前缺口

现在的系统只有：

1. 资产层连续 3 天后，允许 `<= CRUISE -> OPTIMAL`
2. 市场层在 `IGNITION + Stability < 10` 时抑制 `ACCUMULATE`

但这两者之间仍有空档：

1. 某个资产可能连续 3 天很强
2. 但市场整体仍然处于混沌启动期
3. 系统会出现“局部正确、全局错误”的风险

### 2.2 要解决的不是“买什么”

这层解决的是：

1. 什么时候开始允许考虑机会

而不是：

1. 具体应该买哪只

也就是说，系统需要从：

1. 纯信号系统

升级为：

1. 有“交易许可层”的决策系统

---

## 3. 设计目标

Participation Readiness Layer 必须满足：

1. 把“能不能参与市场”从隐式逻辑变成显式字段
2. 不把新规则继续堆进 `ActionMatrix`
3. 不改变资产层 memory/friction 的职责
4. 为报告层提供可解释的 readiness 诊断
5. 为执行层提供统一的开关

---

## 4. 新增概念与输出字段

建议新增独立结构体，例如：

```rust
pub struct ParticipationReadiness {
    pub participation_ready: bool,
    pub stability_ready: bool,
    pub core_tier_streak_ready: bool,
    pub core_tier_streak: usize,
    pub reasons: Vec<String>,
}
```

最低要求字段：

1. `participation_ready: bool`
2. `core_tier_streak: usize`
3. `reasons: Vec<String>`

推荐补充字段：

1. `stability_ready: bool`
2. `core_tier_streak_ready: bool`

---

## 5. 首版判定规则

首版规则明确如下：

```text
participation_ready =
    stability_score >= 10
    AND core_tier_streak >= 3
```

### 5.1 Stability 条件

直接复用当前系统已经统一好的标准：

1. `stability_score >= 10` 才算通过

### 5.2 Core Tier Streak 条件

这里不是“某个资产连续 3 天出现”，而是：

**Top Tier 集合连续稳定 >= 3 天**

这是本任务最重要的定义之一。

---

## 6. Core Tier 的定义

首版建议不要做复杂相似度算法，先采用可解释、易测试的版本。

### 6.1 建议首版定义

将每日 Top Tier 集合定义为以下之一：

1. 当日 `ACCUMULATE + HOLD` 中排名最前的前 2 或前 3 个资产
2. 或者更严格：当日 `OPTIMAL / PULLBACK / CRUISE` 且位于报告 Top Actions 的资产集合

建议首版优先采用：

1. **直接复用最终排序后的前 3 个核心候选集合**

要求：

1. 定义必须稳定
2. 报告层、判定层、测试层使用同一口径

### 6.2 连续性的判定

首版建议使用严格规则：

1. 最近连续 3 天 Top Tier 集合完全相同

例如：

1. Day1: `TSLA / MSFT / FIG`
2. Day2: `TSLA / MSFT / FIG`
3. Day3: `TSLA / MSFT / FIG`

则：

1. `core_tier_streak = 3`

而如果：

1. Day1: `TSLA / MSFT / FIG`
2. Day2: `PLTR / MSFT / FIG`

则 streak 断开，重新计数。

注意：

1. 本轮不做模糊相似度
2. 本轮先要确定“主线稳定”，不是“每天都有强者”

---

## 7. 建议代码落点

建议新增独立模块，不要塞进 `ActionMatrix`。

推荐新增文件：

1. [participation.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/participation.rs)

并在以下位置接入：

1. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/engine.rs)
2. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs)
3. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)
4. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)
5. [DECISION_PACKET_SCHEMA.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/DECISION_PACKET_SCHEMA.md)

### 7.1 Engine 负责什么

在资产最终排序完成后：

1. 计算当日 Top Tier 集合
2. 结合历史 `DecisionPacket` 计算 `core_tier_streak`
3. 生成 `ParticipationReadiness`
4. 将结果写入 `DecisionPacket`

### 7.2 ActionMatrix 负责什么

不再自行定义 readiness 规则。

仅消费：

1. `participation_ready`

若为 `false`：

1. 禁止输出 `ACCUMULATE`

### 7.3 Report 负责什么

显式展示：

1. 当前是否 `Participation Ready`
2. 当前 `core_tier_streak`
3. readiness 未通过的原因

---

## 8. DecisionPacket 扩展

建议在 [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs) 中新增字段：

```rust
pub participation: ParticipationReadiness
```

要求：

1. 序列化持久化
2. 可被历史包读取
3. 可用于后续 streak 计算

如果不想一次改太大，也至少要保证：

1. 当日 Top Tier 集合可持久化
2. readiness 结果可持久化

建议同步记录：

1. `top_tier_symbols: Vec<String>`

这样后续 streak 计算不必依赖 UI 推导。

---

## 9. 行为规则

### 9.1 未准备好时

如果：

1. `participation_ready == false`

则：

1. `ACCUMULATE` 一律禁止
2. 强资产只能展示为候选
3. 报告层必须明确提示：
   - 当前处于候选阶段
   - 尚未获得市场参与许可

### 9.2 准备好时

如果：

1. `participation_ready == true`

则：

1. 恢复正常动作映射
2. 报告层从“候选”转向“可参与”

### 9.3 与现有规则的关系

本层是全局 gating 层，不替代：

1. Asset Memory
2. Promotion Cap
3. Upgrade Friction
4. ExecutionGate

它位于这些规则之上，用于决定：

1. 当前是否允许把资产层强度转译成参与动作

---

## 10. 开发任务拆解

### P0-1 新增 ParticipationReadiness 模块

任务要求：

1. 新建独立结构体和计算入口
2. 输入：
   - 当前 `MarketFeatures`
   - 当前 Top Tier 集合
   - 历史 `DecisionPacket`
3. 输出：
   - `participation_ready`
   - `core_tier_streak`
   - `reasons`

### P0-2 在 DecisionPacket 中落盘

任务要求：

1. 增加 readiness 字段
2. 增加当日 Top Tier 集合字段
3. 保证历史包可用于后续 streak 计算

### P0-3 ActionMatrix 改为消费 readiness

任务要求：

1. 不再直接用零散条件定义候选期
2. `participation_ready == false` 时禁止 `ACCUMULATE`
3. 保留已有资产状态映射，但最终受 readiness 覆盖

### P1-1 报告层增加 readiness 诊断

任务要求：

1. 显示 `Participation: Ready / Not Ready`
2. 显示 `Core Tier Streak`
3. 原因展示：
   - `Stability below threshold`
   - `Top tier continuity not confirmed`

### P1-2 测试与文档同步

任务要求：

1. 新增单元测试
2. 新增集成测试
3. 更新文档：
   - `DECISION_PACKET_SCHEMA.md`
   - 如有需要更新 `STATE_MACHINE_HOME_SUMMARY.md`

---

## 11. 测试清单

至少补以下测试：

1. `stability >= 10` 但 `core_tier_streak < 3`
   - `participation_ready == false`
2. `stability < 10` 但 `core_tier_streak >= 3`
   - `participation_ready == false`
3. `stability >= 10` 且 `core_tier_streak >= 3`
   - `participation_ready == true`
4. Top Tier 集合变更时 streak 重置
5. `participation_ready == false` 时禁止 `ACCUMULATE`
6. 报告层显示 readiness 原因

建议落点：

1. `src/core/participation.rs`
2. `tests/pipeline_integration.rs`
3. `src/core/report_ui_tests.rs`

---

## 12. 验收标准

本任务完成后，系统必须满足：

1. “可不可以参与市场”有单独显式字段
2. readiness 判定不再散落在 `ActionMatrix` 和 report 中
3. 资产层连续性与市场参与许可被清晰拆分
4. 当市场未 ready 时，所有强资产都只能按候选处理
5. 当市场 ready 时，系统才允许把强资产转译为正式参与动作

---

## 13. 非目标

本轮不做：

1. 模糊集合相似度算法
2. 更复杂的 Top Tier clustering
3. 重构现有 Asset Memory 公式
4. 改写 ExecutionGate 风控框架
5. 调整现有 regime 升降级阈值

本轮只做一件事：

**把“是否允许参与市场”从隐式逻辑升级为显式决策层。**

---

## 14. 建议开发顺序

建议按以下顺序实施：

1. 新增 `ParticipationReadiness` 结构与计算器
2. 将结果写入 `DecisionPacket`
3. 在 `ActionMatrix` 中改为消费 readiness
4. 更新 report 展示
5. 补齐测试与文档

---

## 15. 完成定义

当以下条件全部满足时，本任务视为完成：

1. 代码中已存在独立的 readiness 层
2. `participation_ready` 已进入持久化包体
3. `core_tier_streak` 已可追踪
4. `ACCUMULATE` 仅在 readiness 通过后才允许出现
5. 测试覆盖 readiness 的正反场景
6. 报告层可以清楚解释为什么当前只能看候选

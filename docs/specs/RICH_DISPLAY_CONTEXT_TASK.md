# Sentinel Rich Display Context 任务单

## 1. 目标

本任务用于在现有 `DisplayAdapter` 基础上继续推进展示语义的精细化。

当前系统已经完成：

1. `display_intent` 与旧 `AssetAction` 解耦
2. `HOLD / OBSERVE` 由 `has_position` 驱动
3. Telegram / CLI / Tactical Summary 共用同一展示原语

但当前展示上下文仍然偏粗：

1. 只有 `has_position`
2. 仍不足以表达：
   - 有仓但已掉队
   - 无仓但高优先候选
   - 核心持仓 vs 边缘持仓
   - 市场未 ready 下的候选资产

本轮目标是：

**把展示语义从单一事实驱动，升级为 richer display context 驱动。**

---

## 2. 为什么要做这一层

当前：

1. `has_position = true` -> 往往显示 `HOLD`
2. `has_position = false` -> 往往显示 `OBSERVE`

这已经比旧版好很多，但还不够。

因为实际展示语义至少有 4 种不同对象：

1. 核心持仓
2. 非核心但仍持有
3. 无仓候选
4. 已经掉队、应收缩战线

如果展示层不能区分这些状态：

1. 用户看到的 UI 会太平
2. 解释力会下降
3. 后续 Web / App 多端也难以统一表达

---

## 3. 设计目标

新增一层更明确的展示上下文，例如：

```rust
pub struct DisplayContext {
    pub has_position: bool,
    pub is_core_holding: bool,
    pub is_candidate_only: bool,
    pub is_top_tier: bool,
    pub participation_ready: bool,
}
```

最低要求：

1. `has_position`
2. `is_core_holding`
3. `is_candidate_only`

推荐补充：

1. `is_top_tier`
2. `participation_ready`

---

## 4. 核心语义定义

### 4.1 Core Holding

定义建议：

1. 当前有持仓
2. 且仍位于 Top Tier / 核心持仓集合

### 4.2 Candidate Only

定义建议：

1. 当前无持仓
2. 但仍位于候选核心集合
3. 主要用于显示 `OBSERVE`

### 4.3 Non-Core Holding

定义建议：

1. 当前有持仓
2. 但已不在核心集合

该类资产不应与 `Core Holding` 同样展示。

---

## 5. DisplayIntent 第二阶段规则

建议首版规则：

1. `PositionIntent::ADD -> DisplayIntent::ADD`
2. `PositionIntent::TRIM -> DisplayIntent::TRIM`
3. `PositionIntent::EXIT -> DisplayIntent::EXIT`
4. `PositionIntent::HOLD` 时：
   - `is_core_holding == true` -> `DisplayIntent::HOLD`
   - `is_candidate_only == true` -> `DisplayIntent::OBSERVE`
   - `has_position == true && !is_core_holding` -> 仍显示 `HOLD`，但允许附加弱化标签
   - `has_position == false && !is_candidate_only` -> `DisplayIntent::OBSERVE`

注意：

1. 本轮先不新增更多 intent 枚举
2. 先通过 richer context 提升解释力

---

## 6. 建议输出扩展

建议在资产级决策结果中新增：

1. `display_context`
2. `display_tags`
3. `display_notes`

例如：

```rust
pub struct DisplayContext {
    pub has_position: bool,
    pub is_core_holding: bool,
    pub is_candidate_only: bool,
    pub is_top_tier: bool,
    pub participation_ready: bool,
}
```

```rust
pub display_tags: Vec<String>
```

建议标签示例：

1. `Core Holding`
2. `Candidate`
3. `Non-Core Holding`
4. `Participation Blocked`

---

## 7. 建议代码落点

继续使用：

1. [display.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/display.rs)

并在以下位置接入：

1. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/engine.rs)
2. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)
3. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs)

### 7.1 Engine

负责：

1. 生成 `DisplayContext`
2. 生成 `display_intent`
3. 生成可选展示标签

### 7.2 Report

负责：

1. 消费 `display_intent`
2. 可选择展示 `display_tags`
3. 不再自行推断上下文

---

## 8. 开发任务拆解

### P0-1 新增 DisplayContext

任务要求：

1. 定义 `DisplayContext`
2. 持久化进资产级决策结构
3. 为历史兼容增加 `serde(default)`

### P0-2 在 Engine 中生成 richer display context

任务要求：

1. 基于持仓、Top Tier、Participation 状态生成 `DisplayContext`
2. 不再只依赖 `has_position`

### P0-3 DisplayAdapter 消费 DisplayContext

任务要求：

1. `derive_display_intent(...)` 输入改为 `DisplayContext`
2. 不再只接收单个布尔值

### P1-1 报告层增加轻量标签

任务要求：

1. 在 Top Actions 或原因中可选展示：
   - `Core`
   - `Candidate`
   - `Blocked`
2. 保持报告简洁，不要过度信息化

### P1-2 UI / 回归测试

任务要求：

1. 覆盖 Core Holding
2. 覆盖 Candidate Only
3. 覆盖 Non-Core Holding
4. 覆盖 Participation blocked candidate

---

## 9. 测试清单

至少补以下测试：

1. `has_position=true && is_core_holding=true` -> `HOLD`
2. `has_position=false && is_candidate_only=true` -> `OBSERVE`
3. `has_position=true && is_core_holding=false` -> 保持 `HOLD`，但有弱化标签
4. `participation_ready=false && candidate_only=true` -> 仍显示 `OBSERVE`，并带 blocked 语义
5. Telegram Top Actions / 战术分区 / 风险机会 共用同一上下文口径

---

## 10. 验收标准

本任务完成后，系统必须满足：

1. `display_intent` 不再只依赖 `has_position`
2. 展示层能区分核心持仓、候选资产、非核心持仓
3. 报告解释力提升，但不重新引入旧 `action` 依赖
4. 所有展示模块继续共享统一的 DisplayAdapter 输出

---

## 11. 非目标

本轮不做：

1. UI 美化
2. 新增更多执行意图
3. Web / App 组件开发
4. 复杂标签系统

本轮只做：

**让展示上下文从“单一事实”升级为“结构化事实”。**

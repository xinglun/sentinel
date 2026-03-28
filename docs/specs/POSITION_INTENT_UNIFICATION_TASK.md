# Position Intent 统一层任务文档

## 1. 目标

本任务不是新增策略，而是把当前系统中分散的买入、持有、减仓、退出判断，收敛成唯一的最终动作原语。

当前系统已经具备：

1. `NO TRADE`
   - 负责回答“今天能不能主动开新仓”
2. `Exit Decision Summary`
   - 负责回答“已有持仓该怎么处理”
3. `候选观察名单`
   - 负责回答“以后如果可以动，先看谁”

这些展示层已经闭环，但系统内部仍然存在两条并行语义链：

1. `Entry / Participation` 语义
2. `Exit / Position handling` 语义

本任务目标是：

> 让每个资产最终只输出一个唯一动作语义，作为系统内部和多端展示的统一真相源。

---

## 2. 为什么要做

当前报表已经能回答：

1. 能不能买
2. 要不要卖
3. 先看什么

但系统内部仍然是分层表达，再由展示层和用户共同完成最终合成。

这会带来 3 个长期问题：

1. 决策链上存在“多种动作语言”
2. 多端展示需要分别理解 `NO TRADE`、`Exit Summary`、`候选名单`
3. 后续如果接真实执行层，缺少统一的“最终动作原语”

统一后的目标结构应是：

```text
Domain Facts
→ Entry / Participation Gate
→ Exit Decision
→ Position Intent Synthesizer
→ Presentation Assembler
→ Report / UI / Execution
```

---

## 3. 设计原则

### 3.1 Position Intent 是唯一最终动作

首版统一为：

1. `ADD`
2. `HOLD`
3. `TRIM`
4. `EXIT`
5. `WATCH`

说明：

1. `ADD`
   表示允许主动增加风险暴露
2. `HOLD`
   表示已有持仓继续持有
3. `TRIM`
   表示已有持仓减仓
4. `EXIT`
   表示已有持仓退出
5. `WATCH`
   表示不允许新开仓，也没有退出动作，但需要继续观察

### 3.2 NO_TRADE 不被删除，而是作为全局门

必须明确：

1. `NO TRADE` 仍然存在
2. 它表示全局行为限制：禁止主动开新仓
3. 但它不是最终资产动作

也就是说：

1. `NO TRADE` 是组合级约束
2. `Position Intent` 是资产级最终动作

### 3.3 Entry 与 Exit 通过 Intent 收口，而不是互相覆盖

必须避免：

1. `NO TRADE == EXIT`
2. `Exit == Market Bearish`
3. `WATCH == Candidate List`

统一原则：

1. `NO TRADE` 回答“能不能开新仓”
2. `Position Intent` 回答“这个资产最终怎么处理”

### 3.4 report.rs 只消费统一后的 Intent

一旦 `Position Intent` 统一层落地：

1. `report.rs` 不再自己拼接 entry / exit 双重语义
2. 所有文案、分区、动作标签都优先从统一 intent 派生

---

## 4. 推荐模型

### 4.1 统一枚举

建议位置：

[position_intent.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/position_intent.rs)

建议定义：

```rust
pub enum UnifiedPositionIntent {
    Add,
    Hold,
    Trim,
    Exit,
    Watch,
}
```

### 4.2 统一解释结构

建议：

```rust
pub struct PositionIntentDecision {
    pub intent: UnifiedPositionIntent,
    pub reasons: Vec<String>,
    pub source: PositionIntentSource,
}

pub enum PositionIntentSource {
    EntryGate,
    ExitGate,
    Synthesized,
}
```

说明：

1. `intent`
   最终动作
2. `reasons`
   面向 presentation 的原因来源
3. `source`
   用于调试与审计

---

## 5. 统一规则

### 5.1 优先级

统一优先级固定为：

```text
EXIT > TRIM > HOLD > ADD > WATCH
```

### 5.2 基础映射

首版映射建议：

1. `ExitDecision == EXIT`
   -> `UnifiedPositionIntent::Exit`
2. `ExitDecision == TRIM`
   -> `UnifiedPositionIntent::Trim`
3. `NO TRADE && has_position`
   -> `Hold` 或 `Watch`
   取决于资产状态与退出规则
4. `NO TRADE && !has_position`
   -> `Watch`
5. `participation_ready && action/add path allowed`
   -> `Add`
6. `participation_ready && no add/no trim/no exit`
   -> `Hold`

### 5.3 Watch 的定义

`WATCH` 必须被明确定义：

> 不允许主动开新仓，当前也未触发卖出条件，但该资产/状态仍值得持续关注。

这既适用于：

1. 无持仓候选资产
2. 有持仓但未触发退出的“观察态”资产

如果后续认为这两者应拆分，可在下一阶段再细化。

---

## 6. 实现范围

### Step 1: 新增统一层

新增：

1. `src/core/position_intent.rs`
2. 或在现有 `intent_synthesizer.rs` 上演进

职责：

1. 接收 `ParticipationReadiness`
2. 接收 `ExitDecision`
3. 接收资产持仓事实 / 状态事实
4. 输出统一后的 `UnifiedPositionIntent`

### Step 2: DecisionPacket 保持纯领域事实

要求：

1. 不要把 presentation 文案塞回 `DecisionPacket`
2. 如需持久化，持久化统一 intent 的结构化结果，而不是说明文字

### Step 3: PresentationAssembler 改为消费统一 Intent

要求：

1. `Top Actions`
2. `Exit Summary`
3. `候选观察名单`
4. `战术分区`

都优先消费统一后的 `UnifiedPositionIntent`

### Step 4: report.rs 继续只渲染

要求：

1. 不新增任何 intent 判断
2. 只消费 assembler 产出的统一 view model

---

## 7. 关于“无持仓，无需处理”的未来扩展占位

这条建议应纳入本任务，而不是留到完全独立的后续。

当前：

```text
当前无持仓，无需处理。
```

建议升级为：

```text
当前无持仓，无需处理。
未触发任何退出条件。
```

原因：

1. 与未来有持仓时的“退出判定语气”保持一致
2. 让 `Exit Layer` 永远像判定层，而不是说明层
3. 便于后续统一成 `WATCH` / `HOLD` / `TRIM` / `EXIT` 的同一叙事体系

要求：

1. 这条说明应由 assembler 统一生成
2. 不要在 `report.rs` 临时拼接
3. 多语言必须同步

---

## 8. 验收标准

### 8.1 结构验收

必须满足：

1. 系统存在单一的统一动作原语
2. Entry / Exit 不再各自形成一套最终动作语言
3. `report.rs` 只消费统一 intent 派生结果

### 8.2 行为验收

必须满足：

1. `NO TRADE` 场景下：
   - 无持仓资产 -> `WATCH`
   - 有持仓资产 -> `HOLD / TRIM / EXIT / WATCH`
   - 不得全部一律解释成 `SELL`

2. `participation_ready` 场景下：
   - 允许出现 `ADD`
   - 但若 exit 规则触发，必须由 `TRIM / EXIT` 覆盖

### 8.3 展示验收

必须满足：

1. 同一资产在报告中不得出现冲突动作
2. `Top Actions`、`持仓处理建议`、`战术分区` 不得各说各话
3. `无持仓，无需处理` 的场景保留判定语气

### 8.4 质量门

必须通过：

1. `cargo fmt`
2. `cargo test --quiet`
3. `cargo clippy --all-targets --all-features -- -D warnings`

---

## 9. 测试要求

至少补齐：

1. `NO TRADE + 无持仓 -> WATCH`
2. `NO TRADE + 核心持仓 -> HOLD`
3. `NO TRADE + 弱持仓 -> TRIM`
4. `DEFEND -> EXIT`
5. `OVERHEAT -> TRIM`
6. `participation_ready + strong asset -> ADD`
7. `无持仓，无需处理。未触发任何退出条件。` 的多语言输出

并新增至少一条完整 UI 契约测试，锁住：

1. 组合级 `NO TRADE`
2. 资产级 `Position Intent`
3. `持仓处理建议`
4. `候选观察名单`

这 4 层在最终报表里同时存在且不冲突。

---

## 10. 非目标

本轮不做：

1. 不重写底层策略
2. 不扩展复杂止盈止损体系
3. 不回退 `DecisionPacket` 为展示承载模型
4. 不在 `report.rs` 增加业务判断

本轮只做：

> 用统一的 `Position Intent` 原语，把“不能买”和“要不要卖”收进同一套系统语言。

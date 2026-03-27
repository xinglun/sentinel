# Sentinel Architecture Boundary Hardening 任务单

## 1. 目标

本任务不是新增策略能力，而是收紧当前工程的模块边界，避免继续出现“规则能跑，但职责越来越糊”的演化风险。

当前系统已经完成：

1. `Market Regime`
2. `Participation Readiness`
3. `Asset State / Memory`
4. `Exit Decision`
5. `Action Matrix`
6. `Display Adapter`
7. `Execution Gate`

但当前工程出现了 3 个新的架构问题：

1. `ExitDecision` 已经越界到买入语义
2. `Engine` 过胖，直接承担展示层派生逻辑
3. `DecisionPacket` 同时承载结构化事实与旧式 Telegram 文案，形成双轨语义源

本任务目标是：

1. 让退出层重新只负责“是否必须收手”
2. 让引擎回到纯编排角色
3. 让展示语义从核心决策包中解耦
4. 为后续继续扩策略、扩展示端保留清晰边界

---

## 2. 当前架构问题

### 2.1 ExitDecision 越界

当前 `ExitDecision::compute()` 不只输出 `EXIT / TRIM / HOLD`，还会在安全条件下回落为 `ADD`。

这导致：

1. 退出层开始表达 entry 许可
2. `ActionMatrix` 与 `ExitDecision` 共同拥有买入语义
3. 最终 `position_intent` 需要在 `Engine` 再做一次合成

结果：

1. 边界不清
2. 调试困难
3. 后续再加规则时容易出现冲突

### 2.2 Engine 过胖

当前 `Engine` 同时负责：

1. 特征提取
2. 市场状态推进
3. 资产状态计算
4. 排名
5. 参与许可
6. 退出决策
7. 最终意图合成
8. `DisplayContext`
9. `DisplayIntent`

这意味着：

1. 决策变化要改 `engine.rs`
2. 展示变化也要改 `engine.rs`
3. 引擎成为新的“神文件”

### 2.3 DecisionPacket 双轨语义源

当前 `DecisionPacket` 同时保存：

1. 结构化事实：`market_regime / participation / assets / display_*`
2. 旧式文案输入：`telegram.headline / summary / bias`

这会导致：

1. 测试里可以手工塞 `telegram`
2. 运行时又会根据结构化字段重新组织报表
3. 同一语义存在两套来源

长期风险：

1. 报表层和数据层漂移
2. 测试只验证字符串，不验证真实结构化链路

---

## 3. 目标边界

期望的职责分层如下：

```text
Raw Data
→ Features
→ Market Regime
→ Participation Readiness
→ Asset State / Ranking
→ Exit Decision
→ Action Mapping
→ Intent Synthesis
→ Presentation Assembly
→ Report / Execution
```

核心原则：

1. 退出层不负责买入许可
2. 引擎只编排，不做展示解释
3. 报表消费 presentation output，不消费半成品语义

---

## 4. 本轮改造范围

### P0-1 收紧 ExitDecision 边界

要求：

1. `ExitDecision` 不再返回 `ADD`
2. `ExitDecision` 只表达：
   - `EXIT`
   - `TRIM`
   - `NONE` 或等价“无退出动作”
3. 最终 `ADD / HOLD` 仅由：
   - `ActionMatrix`
   - `ParticipationReadiness`
   - Intent synthesis
   共同决定

建议重构：

```rust
pub enum ExitIntent {
    None,
    Trim,
    Exit,
}
```

或保留 `PositionIntent`，但强制限制 `ExitDecision` 只允许输出：

1. `EXIT`
2. `TRIM`
3. `HOLD`

严禁在退出层生成 `ADD`。

### P0-2 新增独立 Intent Synthesizer

不要继续在 `engine.rs` 里手写最终 intent 合成。

建议新增：

1. `src/core/intent.rs`
2. 或 `src/core/intent_synthesizer.rs`

职责：

1. 接收 `ActionMatrix` 基础动作
2. 接收 `ParticipationReadiness`
3. 接收 `ExitDecision`
4. 输出唯一的 `position_intent`

优先级明确为：

```text
EXIT > TRIM > HOLD > ADD
```

### P0-3 把展示上下文派生从 Engine 挪出

建议新增独立 presentation assembler，例如：

1. `src/core/presentation.rs`

职责：

1. 根据领域事实生成 `DisplayContext`
2. 调用 `DisplayAdapter` 生成 `DisplayIntent`
3. 生成展示层需要的 view models

`engine.rs` 不应继续直接构造：

1. `DisplayContext`
2. `DisplayIntent`

### P0-4 收紧 DecisionPacket 语义

本轮不强制删除 `telegram`，但必须明确降级其职责。

要求：

1. `telegram` 不再被视为核心语义源
2. `DecisionPacket` 的结构化字段是唯一真相源
3. `telegram` 若保留，只能作为：
   - legacy compatibility
   - 最小摘要输入

建议：

1. 在文档中标记 `telegram` 为 legacy summary layer
2. 增加注释，禁止后续将策略语义继续塞入 `telegram`

---

## 5. 建议模块落点

建议新增或调整如下：

1. [intent.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/intent.rs)
   - 统一合成最终 `position_intent`

2. [presentation.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/presentation.rs)
   - 生成 `DisplayContext`
   - 生成 `DisplayIntent`
   - 生成面向 report 的 presentation data

3. [exit.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/exit.rs)
   - 收敛为纯退出决策

4. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/engine.rs)
   - 只负责 orchestration

5. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs)
   - 标注 `telegram` 的 legacy 定位

---

## 6. 执行顺序

### Step 1

先做 `ExitDecision` 边界收紧。

验收：

1. `exit.rs` 中不再出现“默认 ADD”语义
2. 退出层单测仍覆盖 defensive / strength loss / participation / overheat

### Step 2

新增独立 intent synthesis 模块。

验收：

1. `engine.rs` 不再手写 `EXIT > TRIM > HOLD > ADD` 合成逻辑
2. intent 合成规则有独立测试

### Step 3

将 `DisplayContext / DisplayIntent` 派生挪入 presentation 模块。

验收：

1. `engine.rs` 不再直接 new `DisplayContext`
2. `engine.rs` 不再直接调用 `DisplayAdapter::derive_display_intent`

### Step 4

收紧 `DecisionPacket` 语义，并更新 schema / 文档。

验收：

1. `telegram` 的定位被显式标明
2. `DECISION_PACKET_SCHEMA.md` 完整记录结构化字段优先级

---

## 7. 非目标

本轮不要做以下事情：

1. 不修改 `Market Regime` 阈值
2. 不修改 `ParticipationReadiness` 判定规则
3. 不修改 `AssetState` 分类标准
4. 不改交易接入层
5. 不改 Telegram 视觉样式
6. 不新增复杂策略信号

---

## 8. 测试要求

至少补以下测试：

1. `ExitDecision` 不会再输出 `ADD`
2. `IntentSynthesizer` 在各种组合下输出唯一正确的 `position_intent`
3. `PresentationAssembler` 生成的 `DisplayContext / DisplayIntent` 与旧逻辑一致
4. 旧历史包仍可反序列化
5. `DecisionPacket` 结构化字段优先时，report 输出不依赖手工伪造 `telegram`

建议新增：

1. `test_exit_decision_never_promotes_add`
2. `test_intent_synthesizer_priority`
3. `test_presentation_assembler_display_context`
4. `test_legacy_packet_compatibility_after_boundary_refactor`

---

## 9. 验收标准

本任务完成，必须同时满足：

1. `ExitDecision` 不再拥有 entry 语义
2. `Engine` 不再直接承担展示层派生
3. `position_intent` 由独立模块统一合成
4. `DecisionPacket` 的结构化字段成为唯一真相源
5. 全量测试通过

如果只是“代码挪位置”但职责仍未收紧，不算完成。

---

## 10. 给开发的明确指令

请按本任务单实施，不要扩 scope。本轮目标不是新增策略，而是做边界治理。

执行顺序：

1. 先收紧 `ExitDecision`
2. 再抽出 `IntentSynthesizer`
3. 再抽出 `PresentationAssembler`
4. 最后收紧 `DecisionPacket` 的语义定位与文档

交付要求：

1. 每一步尽量独立提交
2. 不要把边界治理和视觉/文案修改混在一起
3. 提交时附测试结果
4. 若发现现有 UI 测试过度依赖 legacy `telegram`，应同步修正为依赖结构化字段

验收以第 8 节和第 9 节为准；没有把职责边界真正收紧，不算完成。

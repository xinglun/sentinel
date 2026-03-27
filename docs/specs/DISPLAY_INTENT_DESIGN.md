# 展示语义收口设计说明 (PositionIntent vs DisplayIntent)

## 1. 背景与现状
当前系统已引入 `PositionIntent` (ADD/HOLD/TRIM/EXIT) 作为统一执行原语。但在展示层（Telegram/Terminal），“持有”与“观察”的区分仍被迫依赖底层的旧 `AssetAction`。
这导致 `report.rs` 中存在大量零散的匹配逻辑，展示层职责不够纯粹。

## 2. 职责边界定义 (Core Concept)

| 维度 | PositionIntent (执行语义) | DisplayIntent (展示语义) |
| :--- | :--- | :--- |
| **定义者** | Exit Decision Layer | UI Adaptation Layer (Engine/Report) |
| **关注点** | **“我该买多少/卖多少？”** | **“我该给用户看什么？”** |
| **枚举值** | ADD, HOLD, TRIM, EXIT | ADD, HOLD, OBSERVE, TRIM, EXIT |
| **逻辑重点** | 优先级覆盖（EXIT > ADD） | 身份转换（HOLD intent -> “持有” or “观察”） |
| **消费者** | ExecutionGate, TraderAgent | Telegram, Report, Dashboard |

## 3. 生成规则 (Mapping Rules)

```rust
pub enum DisplayIntent {
    ADD,      // 对应执行 ADD 且表现为加仓
    HOLD,     // 对应执行 HOLD 且已在持仓中
    OBSERVE,  // 对应执行 HOLD 且不在持仓中 (观察状态)
    TRIM,     // 对应执行 TRIM
    EXIT,     // 对应执行 EXIT
}
```

**映射逻辑建议：**
1. If `PositionIntent == TRIM` -> `DisplayIntent::TRIM`
2. If `PositionIntent == EXIT` -> `DisplayIntent::EXIT`
3. If `PositionIntent == ADD` -> `DisplayIntent::ADD`
4. If `PositionIntent == HOLD`:
   - If `AssetAction == ACCUMULATE/HOLD` -> `DisplayIntent::HOLD`
   - Else -> `DisplayIntent::OBSERVE`

## 4. 实施影响
- **DecisionPacket**: 增加 `display_intent` 字段。
- **Engine**: 在完成 Intent 合并后，立即计算 `display_intent` 并填充。
- **Report**: 彻底删除对 `action` 的匹配，仅根据 `display_intent` 进行分桶和标签打印。

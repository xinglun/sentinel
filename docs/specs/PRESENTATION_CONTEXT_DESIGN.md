# 展示语义真正独立化设计 (Presentation Context Isolation)

## 1. 核心矛盾
当前的 `DisplayAdapter` 虽然隔离了代码位置，但其核心逻辑 `derive_display_intent` 仍然依赖 `AssetAction` 来判断是 `HOLD`（持有）还是 `OBSERVE`（观察）。
这种实现仍然是基于“底层信号名”的推测，而不是基于“业务事实”（有没有持仓）的陈述。

## 2. 展示上下文原语 (Presentation Context)

我们将引入显式的展示上下文，用于指导 `DisplayAdapter` 进行翻译。

| 字段 | 定义 | 生成时机 |
| :--- | :--- | :--- |
| **`has_position`** | 账户中是否实际持有该资产。 | 引擎管道根据 `positions` 实时生成。 |
| **`is_candidate`** | 是否仅为入库观察对象（无历史持仓）。 | 引擎管道根据持仓历史/状态生成。 |

## 3. 逻辑重塑 (Rule Refactoring)

`DisplayAdapter` 唯一的输入将是 `(PositionIntent, PresentationContext)`：

- **PositionIntent::ADD** -> `DisplayIntent::ADD`
- **PositionIntent::TRIM** -> `DisplayIntent::TRIM`
- **PositionIntent::EXIT** -> `DisplayIntent::EXIT`
- **PositionIntent::HOLD** -> 
    - `has_position == true` -> `DisplayIntent::HOLD`
    - `has_position == false` -> `DisplayIntent::OBSERVE`

## 4. 数据结构变更

```rust
pub struct AssetActionDecision {
    // ... 执行层字段 ...
    pub position_intent: PositionIntent,
    
    // ... 展示层原语 ...
    pub has_position: bool,      // NEW
    pub is_candidate: bool,     // NEW
    pub display_intent: DisplayIntent,
}
```

## 5. 收益
- **真正解耦**：即使未来 `AssetAction` 重命名或废弃，展示层逻辑依然稳固。
- **语义明确**：UI 展示直接映射到“账户事实”，而不是“信号推断”。
- **测试友好**：可以通过模拟 `has_position=true` 来强制测试“持有”标签，而无需构造复杂的指标信号。

# 展示适配层设计说明 (Display Adapter Design)

## 1. 概念界定 (Boundary Definition)

为了实现展示逻辑与核心引擎的解耦，我们引入三个层级的概念：

| 概念 | 定义 | 归属 | 核心职责 |
| :--- | :--- | :--- | :--- |
| **PositionIntent** | 执行意图 | `exit.rs` | **“怎么做”**。确定交易方向（买、卖、持、清）。 |
| **DisplayIntent** | 展示意图 | `display.rs` | **“怎么看”**。将执行动作转换为用户可理解的分类（加仓、持有、观察、减仓、退出）。 |
| **DisplayBucket** | 展示分区/布局 | `display.rs` | **“放哪看”**。决定标的在报表中的物理位置（加仓区、防御区、观察区）。 |

## 2. 职责迁移 (Responsibilities)

- **`engine.rs`**: 不再感知 `DisplayIntent` 的具体映射细节。它只负责将参与度、排名、退出决策合成为 `PositionIntent`。
- **`display.rs` (NEW)**: 
  - 封装 `DisplayIntent` 的生成逻辑（Input: `PositionIntent` + `AssetAction` + `HoldingStatus`）。
  - 封装资产分桶逻辑（Categorization）。
  - 提供统一的展示标签（Labels）。
- **`report.rs`**: 纯粹作为模板渲染器。它直接消费 `DisplayAdapter` 处理好的分桶数据和标签，不进行任何推断。

## 3. 接口预览 (Interface)

```rust
pub struct DisplayAdapter;

impl DisplayAdapter {
    /// 基于执行意图、基础动作及持仓状态，计算展示意图
    pub fn compute_display_intent(
        pos_intent: PositionIntent,
        base_action: AssetAction,
        is_held: bool,
    ) -> DisplayIntent;

    /// 统一资产分桶逻辑
    pub fn categorize(decisions: &[AssetActionDecision]) -> DisplayBuckets;
}
```

## 4. 收益
- **高内聚**：所有 UI 展示文案和规则集中一处。
- **易测试**：可以独立测试 `OBSERVE` 与 `HOLD` 的分离逻辑，无需启动完整引擎。
- **一致性**：Telegram 与终端报告共享同一套适配逻辑，杜绝口径差异。

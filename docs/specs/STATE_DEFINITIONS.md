# Sentinel 状态定义手册 (STATE_DEFINITIONS.md)

## 1. 市场状态 (Market Regime)

市场状态是最高层级的决策过滤器，决定了组合层面的风险暴露上限和动作倾向。

### 1.1 主状态定义

| 状态名称 | 核心语义 | 允许动作 | 禁止动作 |
| --- | --- | --- | --- |
| `IGNITION` | 趋势发端，波动极高 | 极小量试探 (Pilot) | 大仓追涨 |
| `NEWBORN` | 趋势确立初期，尚未稳固 | 分批建仓 | 全力加仓 |
| `EARLY_CONFIRMATION` | 趋势获得首次回撤确认 | 标准加仓 | 无 |
| `ESTABLISHED` | 强劲趋势中后期，高度稳定 | 持有、顺势加仓 | 激进翻仓 |
| `CONFIRMED` | 趋势极度成熟，伴随过热风险 | 减仓、停止买入 | 任何新增暴露 |
| `DEFENSIVE` | 结构破坏，进入防御模式 | 强制清仓、大幅缩表 | 抄底、补仓 |

### 1.2 内部双层属性

为了实现更精细的逻辑，内部采用双层表示法：

*   **Lifecycle State**: `IGNITION` -> `NEWBORN` -> `EARLY_CONFIRMATION` -> `ESTABLISHED` -> `CONFIRMED`
*   **Risk Overlay**:
    *   `NORMAL`: 风险受控，按计划执行。
    *   `DECELERATING`: 动能衰减，减少加仓频率。
    *   `DEFENSIVE`: 结构受损，执行防御性卖出或冻结。
    *   `BROKEN`: 趋势彻底终结。

---

## 2. 个股状态 (Asset State)

个股状态基于其相对于引力中心（Owner MA）的位置和特征决定。

| 状态名称 | 语义描述 | 典型特征 |
| --- | --- | --- |
| `OPTIMAL` | 最佳持有期 | 处于引力带上方，趋势斜率向上 |
| `CRUISE` | 稳定巡航期 | 略高于引力带，低波动 |
| `PULLBACK` | 健康回撤 | 回落至引力带附近，结构未破坏 |
| `CAUTION` | 警觉期 | 跌破关键支撑，但尚未触发全面防御 |
| `OVERHEAT` | 极度过热 | 偏离引力带超过 2σ，随时可能大幅修正 |
| `DEFEND` | 防御模式 | 结构性破位，必须缩减仓位 |
| `FORMING` | 结构形成中 | 历史数据不足或引力结构混乱 |

---

## 3. 状态映射关系

*   **对外展示**: 最终展示给用户的是经过组合后的单一状态（如 `ESTABLISHED (Decelerating)`）。
*   **动作驱动**: 动作由 `(Market Regime, Asset State)` 组成的矩阵唯一确定。

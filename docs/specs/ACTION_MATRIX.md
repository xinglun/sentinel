# Sentinel 动作矩阵 (ACTION_MATRIX.md)

## 1. 核心动作定义

| 动作名称 | 语义描述 | 组合层约束 |
| --- | --- | --- |
| `ACCUMULATE` | 分批买入/加仓 | 允许新增暴露，通常在 `PULLBACK` 或 `IGNITION` 时执行 |
| `HOLD` | 继续持有 | 不主动增仓，也不减仓 |
| `REDUCE` | 部分卖出/减仓 | 在 `OVERHEAT` 或 `CAUTION` 时启动，锁定利润或控制风险 |
| `FREEZE` | 冻结动作 | 禁止一切买入，仅允许持有或清算 |
| `AVOID` | 避险/观望 | 不持仓，不建仓 |
| `OBSERVE` | 观察期 | 结构未明，暂不动作 |

---

## 2. 状态矩阵映射 (Market Regime x Asset State)

| 市场状态 \ 个股状态 | `OPTIMAL` | `CRUISE` | `PULLBACK` | `CAUTION` | `OVERHEAT` | `DEFEND` | `FORMING` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `IGNITION` | `ACCUM` | `HOLD` | `HOLD` | `AVOID` | `REDUCE` | `AVOID` | `OBSERVE` |
| `NEWBORN` | `ACCUM` | `HOLD` | `ACCUM` | `HOLD` | `REDUCE` | `AVOID` | `OBSERVE` |
| `EARLY_CONFIRM` | `ACCUM` | `HOLD` | `ACCUM` | `HOLD` | `REDUCE` | `REDUCE` | `OBSERVE` |
| `ESTABLISHED` | `HOLD` | `HOLD` | `ACCUM` | `HOLD` | `REDUCE` | `REDUCE` | `OBSERVE` |
| `CONFIRMED` | `HOLD` | `HOLD` | `HOLD` | `REDUCE` | `REDUCE` | `REDUCE` | `OBSERVE` |
| `DEFENSIVE` | `FREEZE` | `FREEZE` | `AVOID` | `AVOID` | `REDUCE` | `AVOID` | `OBSERVE` |

### 2.1 动作优先级
1. `DEFENSIVE` 市场状态具有最高优先级，可覆盖（Override）所有个股层面的买入动作。
2. `OVERHEAT` 个股状态在所有市场状态下都优先触发 `REDUCE`。
3. `FORMING` 始终映射为 `OBSERVE`。

---

## 3. 仓位缩放系数 (Sizing Multipliers)

| 动作 | 默认系数 | 说明 |
| --- | --- | --- |
| `ACCUMULATE` | 1.0 | 标准加仓单元 |
| `HOLD` | 1.0 | 维持当前权重 |
| `REDUCE` | 0.5 | 减仓 50% 或减至目标权重 |
| `FREEZE` | 0.0 | 禁止新增买入 |
| `AVOID` | 0.0 | 清仓或不参与 |

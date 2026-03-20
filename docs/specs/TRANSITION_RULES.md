# Sentinel 状态迁移规则 (TRANSITION_RULES.md)

## 1. 市场状态迁移 (Market Regime Transitions)

### 1.1 升级路径 (Lifecycle Progression)
升级通常需要满足一定的置信度（Confidence）和时间（Stability）要求。

*   **None -> IGNITION**: 
    *   `stability_structural` 首次突破阈值。
    *   主导资产开始脱离底部。
*   **IGNITION -> NEWBORN**:
    *   置信度 >= 60。
    *   持续时间 >= 5 天。
*   **NEWBORN -> EARLY_CONFIRMATION**:
    *   经历过一次成功的 `PULLBACK` 且未破位。
    *   置信度 >= 70。
*   **EARLY_CONFIRMATION -> ESTABLISHED**:
    *   `stability_temporal` >= 30 天。
    *   置信度 >= 80。
*   **ESTABLISHED -> CONFIRMED**:
    *   `maturity` 指标过高 (> 80)。
    *   动能出现极端背离。

### 1.2 快速降级 (Defensive Trigger)
降级通常比升级更快，遵循“先逃生，后确认”原则。

*   **ANY -> DEFENSIVE**:
    *   `flow_acceleration` 出现大幅负向波动。
    *   核心资产集体跌破 `CAUTION` 状态。
    *   宏观置信度跌破 50。
*   **ESTABLISHED -> EARLY_CONFIRMATION (降级)**:
    *   置信度连续 3 天低于 70。

---

## 2. 个股状态迁移 (Asset State Transitions)

个股状态迁移基于引力指标（Deviation, Z-Score, Slope）。

| 起始状态 | 目标状态 | 触发条件 |
| --- | --- | --- |
| `PULLBACK` | `OPTIMAL` | 缩量回踩引力中心后放量拉升 |
| `OPTIMAL` | `OVERHEAT` | Z-Score > 2.0 且偏离度过大 |
| `ESTABLISHED` | `CAUTION` | 跌破 Owner MA 但斜率尚未走平 |
| `CAUTION` | `DEFEND` | 跌破 Leash MA 且斜率转负 |

---

## 3. 抖动抑制 (Hysteresis & Smoothing)

*   **确认天数**: 升级通常需要 N 天确认，降级仅需 1-2 天。
*   **缓冲区 (Buffer)**: 价格在阈值附近波动时，引入 1-3% 的缓冲区以避免频繁切换。
*   **状态锁定 (State Lock)**: 进入 `DEFENSIVE` 后，至少锁定 3 天不进行升级。

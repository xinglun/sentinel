# 多端展示语义标准 (DISPLAY_SEMANTICS_STANDARD.md)

本规范定义了 Sentinel 决策包在不同展示端（Telegram, CLI, Web/App）的一致性渲染原则。

## 1. 展示意图映射 (DisplayIntent Mapping)

| 展示意图 (DisplayIntent) | 业务上下文 | Telegram 语义 | Web/App 语义 |
| :--- | :--- | :--- | :--- |
| **ADD** | 建议建立头寸或加仓 | 加仓 | 🟢 绿色加号 / BUY |
| **HOLD** | 建议继续持有 | 持有 | 🔵 蓝色圆点 / HOLD |
| **OBSERVE** | 处于观察区，暂无持仓 | 观察 | ⚪️ 灰白色圆点 / WATCH |
| **TRIM** | 建议减仓（非清仓） | 减仓 | 🟠 橙色减号 / REDUCE |
| **EXIT** | 建议清仓 | 清仓 | 🔴 红色叉号 / EXIT |

## 2. 展示标签规范 (Strategic Tags)

所有渲染端应根据 `DisplayContext` 事实自动挂载以下标签：

### [Core] (核心持仓)
- **判定逻辑**: `has_position == true && is_core_holding == true`
- **目的**: 强化长期持有信心。
- **展示建议**: 醒目样式或金色/高亮色。

### [Candidate] (重点候选)
- **判定逻辑**: `has_position == false && is_candidate_only == true`
- **目的**: 明确当前重点关注的非持仓标的。
- **展示建议**: 虚线框或浅色高亮。

### [Blocked] (已拦截)
- **判定逻辑**: `is_candidate_only == true && participation_ready == false`
- **目的**: 说明为何“看起来很好做”但系统未下令 ADD。
- **展示建议**: 锁定图标或灰色斜杠。

## 3. 分桶原则 (Standard Buckets)

多端应统一采用 `DisplayIntent` 作为分桶主轴：
1. **Top Actions**: 所有 `ADD`, `TRIM`, `EXIT` 意图，以及关键的 `HOLD` 变更。
2. **持有区 (Holdings)**: `DisplayIntent::HOLD` 且 `has_position == true`。
3. **观察区 (Watchlist)**: `DisplayIntent::OBSERVE`。
4. **风险/机会区**: 根据特定规则映射到展示区的额外注释。

## 4. 优先级冲突处理

1. **退出优先**: `DisplayIntent::EXIT` 拥有最高展示权重。
2. **拦截优先**: 如果 `participation_ready == false`，任何买入建议应被转化为“等待”或显式挂载 `[Blocked]`。
3. **诊断优先**: `exit_decision` 中如果有明确的 `Protection` 触发，必须在展示详细原因。

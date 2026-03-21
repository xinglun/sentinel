# Sentinel 状态机 V1.1 优化方案

## 1. 目的

V1.0 已完成以下能力：

1. `reset gate`
2. `single-step downgrade`
3. `duration lock`
4. `soft reset`
5. `asset recovery ladder`
6. `transition_audit`

V1.1 的目标不是继续加指标，而是把以下抽象进一步做实：

1. `TrendDominant == false` 不再依赖单一代理指标
2. `CoreAssetsBreakdown` 从简化启发式升级为复合判定
3. 状态机解释层和判定层继续解耦
4. 审计与报告对“为什么没 reset / 为什么被阻断”给出更稳定的结构化输出

---

## 2. 本轮优化范围

### 2.1 核心方向

本轮只做三件事：

1. 实现 `TrendDominant` 复合判定
2. 升级 `CoreAssetsBreakdown` 判定
3. 扩展状态迁移审计输出

### 2.2 明确不做

本轮不做：

1. 新增技术指标
2. 调整策略参数来“掩盖”状态机问题
3. 修改 Telegram 样式
4. 修改执行层风控逻辑

---

## 3. V1.1 核心设计

### 3.1 TrendDominant 复合判定

当前实现：

1. `check_reset_gate()` 使用 `dominance_margin <= 0.0` 作为 `TrendDominant == false` 的代理

V1.1 目标：

引入显式的 `trend_dominant` 复合判定函数，而不是单一阈值代理。

建议定义：

```text
trend_dominant = (
    dominance_margin > 0
    AND up_weight >= down_weight
    AND system_confidence >= trend_dominant_min_confidence
)
```

可选增强：

1. 引入 `up_count / down_count` 的 breadth 约束
2. 引入 `flow_acceleration` 的方向约束

要求：

1. `TrendDominant` 必须是显式字段或显式函数结果
2. `reset gate` 不能再直接消费单一代理值

### 3.2 CoreAssetsBreakdown 复合判定

当前实现：

1. `TrendStatus::Down`
2. 或 `deviation < -5.0`
3. 且损坏核心资产数量超过 50%

V1.1 目标：

将 `CoreAssetsBreakdown` 升级成配置化复合判定。

建议定义：

```text
core_assets_breakdown = (
    count_assets_below_threshold(core_assets) >= breakdown_k
    OR avg_core_deviation <= breakdown_avg_deviation
    OR core_breadth <= breakdown_breadth_floor
)
```

建议配置项：

1. `breakdown_k`
2. `breakdown_avg_deviation`
3. `breakdown_breadth_floor`

要求：

1. `core_assets` 继续来自配置
2. breakdown 阈值也必须来自配置
3. 默认值可以内置，但不能只写死在代码里

### 3.3 Transition Audit 扩展

当前 `transition_audit` 已有：

1. `from`
2. `to`
3. `is_reset_blocked`
4. `is_downgrade_clamped`
5. `core_breakdown`
6. `duration_locked`

V1.1 建议扩展：

1. `trend_dominant`
2. `reset_gate_passed`
3. `indicator_cap`
4. `soft_reset_applied`
5. `defensive_override`

要求：

1. 审计字段优先结构化
2. `reasons` 继续保留，但不作为唯一真相来源

---

## 4. 具体实现建议

### 4.1 文件落点

1. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs)
   - 增加 `trend_dominant`
   - 升级 `core_assets_breakdown`

2. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
   - `check_reset_gate()` 改为消费显式 `trend_dominant`
   - 扩展 `transition_audit`

3. [config.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/config.rs)
   - 增加 `core_assets_breakdown` 阈值配置
   - 增加 `trend_dominant` 判定阈值配置

4. [IMPLEMENTATION_WALKTHROUGH.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/architecture/IMPLEMENTATION_WALKTHROUGH.md)
   - 同步更新为 V1.1 审计与判定语义

### 4.2 建议配置结构

建议在 `rules` 下增加：

```toml
[rules.inertia]
min_state_duration = 3
trend_dominant_min_confidence = 55.0
core_breakdown_k = 2
core_breakdown_avg_deviation = -5.0
core_breakdown_breadth_floor = 0.0
```

说明：

1. 不要求名字完全一致
2. 但配置必须允许后续实验和回测

---

## 5. 验收标准

### 5.1 TrendDominant

必须满足：

1. `TrendDominant` 不再由单一代理值直接表达
2. `reset gate` 使用显式的 `trend_dominant` 结果

### 5.2 CoreAssetsBreakdown

必须满足：

1. `CoreAssetsBreakdown` 依赖配置阈值
2. 不再只依赖“超过一半资产是 Down”

### 5.3 Transition Audit

必须满足：

1. 可以结构化解释：
   - 为什么没 reset
   - 为什么被 duration lock 拦住
   - 为什么进入 `DEFENSIVE`

### 5.4 基线

必须通过：

1. `cargo test -q`
2. `cargo clippy --all-targets --all-features -- -D warnings`

---

## 6. 一句话要求

V1.1 不是继续扩展规则，而是把 `TrendDominant` 和 `CoreAssetsBreakdown` 从“代理启发式”升级成“明确的可配置复合判定”。 

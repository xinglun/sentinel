# Relative Strength Memory Layer

## 1. 模块设计目标

本模块的目标不是重新定义市场状态机，而是修正资产层在排序与状态提升上的“短视化”问题。

当前系统已经具备：

1. `Market InertiaLayer`
2. `Reset Gate`
3. `Downgrade Gate`
4. `Duration Lock`

但资产层仍然存在一个明显缺口：

1. 更偏重横截面快照
2. 更容易把“短期整洁”误判为“持续更强”
3. 更容易把“持续强者的短期波动”误判成状态劣化

本模块要解决的核心问题是：

1. 防止强者被短期波动踢出核心区
2. 防止弱者因短期整洁而快速上位
3. 让资产状态排序从“单日结构评分”升级为“当前结构 + 时间连续性”的组合判断

一句话：

**Relative Strength Memory Layer 用来保护持续强者，抑制短期假修复。**

---

## 2. 模块边界

本模块只负责资产层的时间连续性，不负责市场层的 regime 判定。

### 2.1 它负责什么

1. 资产相对强度的短中期记忆
2. 资产 Top Tier 的状态保护
3. 弱资产提升的时间成本与上限约束
4. 资产排序时的时间连续性补偿

### 2.2 它不负责什么

1. 不负责 `MarketState` 判定
2. 不负责 `Reset Gate`
3. 不负责 `DEFENSIVE` 风险覆盖
4. 不负责 Telegram 文案
5. 不直接决定交易动作

### 2.3 层级位置

建议放在：

1. `RawSignalLayer`
2. `RelativeStrengthMemoryLayer`
3. `AssetState Decision`
4. `ActionMatrix`

也就是说，它是资产状态机的输入增强层，不是动作层。

---

## 3. 最小规则集

V1 版本只落最小规则，不做复杂因子。

## 3.1 Top Tier Lock

如果某资产在过去 `10` 个交易日中：

1. 有至少 `6` 天位于强度前 `3`

则：

1. `min_state = CRUISE`

含义：

1. 持续强者不能因为一次短期回调直接掉到 `OBSERVE`
2. 这是一种“最低状态保护”

目标：

1. 保护 `NVDA / GOOG / SPY` 这类持续强资产

---

## 3.2 Weak Asset Promotion Cap

如果某资产在过去 `20` 个交易日内满足任一条件：

1. 曾进入 `DEFEND`
2. 多数天低于 `CRUISE`

则：

1. 默认 `max_state = CRUISE`

除非满足额外解除条件，才允许提升到 `OPTIMAL`。

目标：

1. 防止长期弱资产因为 1-2 天“横截面更干净”就直接上位

---

## 3.3 Promotion Unlock Conditions

对受 `Promotion Cap` 限制的资产，解除限制前至少需要满足：

1. 长周期结构恢复
2. 波动收敛
3. 连续 `N` 天状态稳定

V1 可以先复用现有资产恢复条件，不新增复杂特征。

---

## 3.4 Rolling Strength Memory

资产最终排序不再只看“当前结构分”。

建议最小实现：

```text
memory_adjusted_score =
    current_structure_score * 0.7
  + rolling_strength_rank_score * 0.3
```

其中：

1. `current_structure_score`
   - 继续使用现有资产状态输入
2. `rolling_strength_rank_score`
   - 由过去 5-10 天的相对强度名次转换而来

V1 目标不是追求完美，而是让排序具备时间连续性。

---

## 3.5 No Instant Redemption

禁止以下模式：

1. `DEFEND -> OPTIMAL`
2. `OBSERVE -> OPTIMAL`
3. `FORMING -> OPTIMAL`

除非通过现有的资产恢复阶梯，并满足 memory layer 的解除条件。

目标：

1. 防止“空气票”或“刚恢复资产”直接进入核心区

---

## 4. 数据结构草案

## 4.1 AssetStrengthMemory

建议新增：

```rust
pub struct AssetStrengthMemory {
    pub top3_days_last_10: u8,
    pub top5_days_last_10: u8,
    pub defend_days_last_20: u8,
    pub below_cruise_days_last_20: u8,
    pub rolling_strength_rank: f64,
    pub top_tier_locked: bool,
    pub promotion_capped: bool,
}
```

说明：

1. `top3_days_last_10`
   - 过去 10 天中进入 Top 3 的次数
2. `top5_days_last_10`
   - 作为辅助字段，可用于后续扩展
3. `defend_days_last_20`
   - 用于弱资产惩罚
4. `below_cruise_days_last_20`
   - 用于判断长期弱结构
5. `rolling_strength_rank`
   - 滚动相对强度分
6. `top_tier_locked`
   - 是否触发 Top Tier Lock
7. `promotion_capped`
   - 是否触发弱资产提升上限

---

## 4.2 AssetStrengthDecision

建议在资产状态机输入前产生一个中间结果：

```rust
pub struct AssetStrengthDecision {
    pub symbol: String,
    pub raw_score: f64,
    pub memory_score: f64,
    pub adjusted_score: f64,
    pub min_state: Option<AssetState>,
    pub max_state: Option<AssetState>,
    pub reasons: Vec<String>,
}
```

用途：

1. 用于最终排序
2. 用于状态上下限裁剪
3. 用于报告与调试解释

---

## 4.3 挂接位置建议

建议不要把这层塞进 `ActionMatrix`。

更合理的位置：

1. `features / asset feature aggregation`
2. `asset_state.rs` 之前
3. 或 `asset_state.rs` 内部作为独立 helper

推荐接口方向：

1. `compute_asset_strength_memory(...)`
2. `apply_strength_memory(...)`
3. `clamp_asset_state_by_memory(...)`

---

## 5. 接口草案

## 5.1 计算 Memory

```rust
pub fn compute_asset_strength_memory(
    symbol: &str,
    history: &[HistoricalAssetSnapshot],
) -> AssetStrengthMemory
```

输入：

1. symbol
2. 最近 10-20 天的资产历史快照

输出：

1. 该资产的相对强度记忆结构

---

## 5.2 生成 Memory Decision

```rust
pub fn build_asset_strength_decision(
    current: &AssetFeatures,
    memory: &AssetStrengthMemory,
) -> AssetStrengthDecision
```

输出：

1. 当前结构分
2. memory 调整分
3. 状态上下限
4. 原因列表

---

## 5.3 状态裁剪

```rust
pub fn clamp_asset_state_with_memory(
    proposed: AssetState,
    decision: &AssetStrengthDecision,
) -> AssetState
```

逻辑：

1. 若命中 `min_state`，则不允许低于该状态
2. 若命中 `max_state`，则不允许高于该状态

---

## 5.4 排序增强

```rust
pub fn rank_assets_with_memory(
    assets: &[AssetFeatures],
    decisions: &HashMap<String, AssetStrengthDecision>,
) -> Vec<RankedAsset>
```

逻辑：

1. 排序时使用 `adjusted_score`
2. 不再只依赖当前横截面

---

## 6. 给开发的实施说明

这轮不要改 `MarketRegime`。  
也不要动 Telegram 结构。  
更不要继续调参数掩盖资产层排序问题。

### 6.1 实施顺序

1. 先补 `AssetStrengthMemory` 数据结构
2. 再补 `compute_asset_strength_memory()`
3. 再补 `AssetStrengthDecision`
4. 最后才把它接入资产状态裁剪和排序

### 6.2 本轮最小交付

必须完成：

1. `Top Tier Lock`
2. `Weak Asset Promotion Cap`
3. `rolling_strength_rank` 最小版
4. 状态上下限裁剪
5. 结构化原因输出

本轮不要做：

1. 不要加新因子
2. 不要加复杂机器学习评分
3. 不要改动作矩阵
4. 不要改 Telegram 大结构

### 6.3 验收标准

至少补以下测试：

1. 持续强者不会因单日波动直接掉到 `OBSERVE`
2. 过去 20 天内弱势资产不会直接升到 `OPTIMAL`
3. `FORMING` 资产不会借短期整洁直接上位
4. 排序从纯横截面变成“当前 + 时间连续性”
5. 原因日志中能清楚看到：
   - `top_tier_locked`
   - `promotion_capped`
   - `memory_adjusted_score`

### 6.4 给开发的最终一句

这轮不是继续修市场状态机。  
这轮是在资产层补“相对强度记忆”，防止强者被短期波动踢出，也防止弱者因短期整洁上位。

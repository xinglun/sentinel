# 周度状态复盘（自动草稿）

- 截至: 2026-09-07
- 状态: 数据不可用；仅基于已保存历史
- 最新摘要: 启动期 | 无交易窗口
- 分析天数: 7
- 平均置信度: 58.9
- 平均稳定度: 10.0
- 趋势凝聚 ready 天数: 2

## 市场状态计数
- IGNITION: 3
- NEWBORN: 4

## 风险覆盖计数
- NORMAL: 7

## 状态机周度汇总
- 有状态摘要的天数: 7
- 重置确认 / 阻止: 0 / 0
- 软重置 / duration lock / 防御覆盖: 0 / 0 / 0
- 核心破坏 / 对账不一致: 0 / 0

## 日度状态机时间线
- 2026-08-27: IGNITION -> IGNITION | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-08-28: IGNITION -> IGNITION | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-09-01: IGNITION -> NEWBORN | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-09-02: NEWBORN -> NEWBORN | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-09-03: NEWBORN -> NEWBORN | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-09-04: NEWBORN -> NEWBORN | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-09-07: NEWBORN -> DATA_UNAVAILABLE | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 边界: 仅为审计事实；不生成评分、建议或交易判断。

## 战略上下文快照
- 趋势广度模式: BroadExpansion
- 市场周期位置: LateAcceptance
- 持仓效率: TimeCostRising
- 战略上下文行:
  - 市场结构模式: 结构整理 / 无明确主导
  - 长期方向: 长期结构趋势增强
  - 周期位置: LATE_ACCEPTANCE
  - 周期特征: 高预期 / 核心资产集中 / 盈利兑现要求提高
  - 拥挤风险: WATCH
  - 宏观重力: 利率压力 RISING / 实际利率 TIGHT / 信用压力 NORMAL / 成长股估值 COMPRESSING / 流动性 NEUTRAL / 收益率曲线 FLAT
  - 宏观重力: 只解释折现率与流动性环境，不生成交易信号
  - 证据持续性: 持续累积
  - 证据覆盖: AI 投入产出验证 (Capex Payoff) / 业绩实质性确认 (Earnings Quality) / 订单能见度提升 (Order Visibility)
  - 战略证据状态: NO TRADE，等待结构扩散
- 边界: 仅为快照；不生成评分、建议或交易判断。

## Signal Context（信息质量上下文）
- Information Content: UNAVAILABLE
- Primary Context: Holiday Liquidity: NYSE Holiday Liquidity [official NYSE holiday calendar]
- Context Quality: MEDIUM
- Event Fact: NYSE Holiday Liquidity [official NYSE holiday calendar] / 2026-09-07 / NYSE Holidays & Trading Hours
- Source Diagnostics: 今天: NYSE Holiday Liquidity [official NYSE holiday calendar]
- Interpretation: 当前来源覆盖不完整，无法确认机械性再平衡是否为主要驱动。
- 边界: Signal Context 仅作周度追溯沉淀；不接入 Gate、Execution、Trader、READY / EXECUTE 或 Position Sizing。

## Market Interpretation Snapshot
- decision_weight: 0%
- dayType: holiday
- reason: market_closed
- exceptionalFactors: [market closed]
- Narrative:
  - 今天是 NYSE 休市日，没有新的交易日观察，以下延续最近一个有效交易日的结构。
  - Available event context: 今天: NYSE Holiday Liquidity [official NYSE holiday calendar]
  - 没有观察到新的急剧恶化，但市场仍处于缺乏主导者、扩散不足的脆弱结构中（Leader absence: 13 trading days）。
  - 短期相对强度开始在 NVDA 等个别资产恢复，尚不足以构成新的 Leadership。
  - RS Recovery Breadth：1/9 非基准资产改善；Strong/Moderate Recovery：1/9 强/中等恢复；RS Diffusion：NOT_CONFIRMED。Actionable Diffusion：NOT_CONFIRMED
  - Reason：没有确认 Leader、没有 breakout、Action Matrix 未转强确认。
  - 动作分布：观察 1 / 持有 0 / 收缩 9。
- Leadership Confidence: LOW
- Leadership Metrics:
  - 综合主导者: [none]
  - Secondary Leaders: []
  - Leadership Watch Candidates: []
  - leadershipBreadth: broad
  - Tactical Leadership Structure: LEADERLESS / FRAGMENTED
  - Leader Absence Duration: 13 trading days
- universe_breadth_expansion:
  - 观察池广度原始值: 60.0%
  - 观察池广度标签: BROAD_WITHIN_UNIVERSE
  - 观察池广度分类分数: 60.0
  - concentrationScore: 34
  - rotationScore: 14
- Rotation Observation:
  - rotationType: no_rotation
  - from: []
  - to: []
  - interpretation: 上涨主要来自 Sentinel 观察池内部的广度改善；全市场 breadth 未被本层测量。
  - observationOnly: true
- Observation Confidence:
  - trend: MEDIUM
  - macro: MEDIUM
  - supply: HIGH
  - expectation: UNAVAILABLE
  - gravity: MEDIUM
  - flow: MEDIUM
  - overall: MEDIUM
- Interpretation Priority:
  - Trend: ★★★
  - Supply: ★★
  - Macro: ★
- Leader Persistence:
  - 综合主导者: none
  - Current Leader: none
  - Previous Snapshot Leader: none
  - Leader Absence Since: 2026-08-13
  - Tactical Leadership Structure: LEADERLESS / FRAGMENTED
  - 连续领导天数: 0 天
  - 领导评分: 0.0
  - 领导状态: ABSENT
  - Leader Absence Duration: 13 trading days
  - 较昨日变化: +1 天，评分下降
  - 边界：仅用于观察；本区块不改变 Decision、Gate、Execution、Trader 或 Position Sizing。 数据质量：降级，部分历史指标缺失。
- Boundary: market interpretation is observation only. Decision weight stays at 0% and it does not enter Gate, Execution, Trader, Action Matrix, Position Sizing, or any decision threshold.

## 宏观引力快照
- 利率压力: RISING
- 实际收益率: TIGHT
- 收益率曲线: FLAT
- 信用压力: NORMAL
- 流动性: NEUTRAL
- 成长估值: COMPRESSING
- 边界: 仅说明贴现率与流动性上下文；不作为 Gate 输入或交易指令。

## Capital Dynamics（供需观察）
- 边界: Capital Dynamics 仅作 Observation shell，Current decision weight 为 0%，不接入 Gate、Execution、Trader、Action Matrix 或 Position Sizing。

### 6.1 Supply Layer（Capital Absorption）
- 最新观测日: 2026-09-07
- 最新 Near-Term Supply 数量: 0
- 最新 Future Queue 数量: 2
- 7 日 Future Queue 最小值 / 最大值: 0 / 2
- 已报道 / 已确认: 2 / 0
- 潜在供给压力: NORMAL
- 边界: 仅为潜在未来供给观察；不生成市场结论、风险升级或交易信号。

### 6.2 Demand Layer（Flow Layer）
- Flow Layer 未配置
- 边界: Flow Layer 仅作 Observation Only 观察，decision weight 固定为 0%，不覆盖 Trend Layer，也不生成交易信号。

## 认知校准快照
- 研究关注条目: 9
- 资产命题条目: 9
- 边界: 认知校准只管理注意力和命题复核；不生成交易信号。

## Expectation Layer（市场预期观测）
- 观测日: 2026-09-07
- decision_weight: 0%
- trade_signal: false
- observation_count: 16
- subjects: GOOG, ISRG, MSFT, NVDA, PLTR, TSLA
- 边界: Expectation Layer 仅用于观测市场预期，不进入 Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing，也不生成交易信号。

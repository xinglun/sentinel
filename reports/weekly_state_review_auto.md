# 周度状态复盘（自动草稿）

- 截至: 2026-07-10
- 状态: 使用当前市场判断
- 最新摘要: 启动期 | 无交易窗口
- 分析天数: 7
- 平均置信度: 55.5
- 平均稳定度: 42.5
- 趋势凝聚 ready 天数: 7

## 市场状态计数
- IGNITION: 7

## 风险覆盖计数
- NORMAL: 7

## 状态机周度汇总
- 有状态摘要的天数: 7
- 重置确认 / 阻止: 0 / 0
- 软重置 / duration lock / 防御覆盖: 0 / 0 / 0
- 核心破坏 / 对账不一致: 0 / 0

## 日度状态机时间线
- 2026-07-01: IGNITION -> IGNITION | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-07-02: IGNITION -> IGNITION | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-07-06: IGNITION -> IGNITION | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-07-07: IGNITION -> IGNITION | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-07-08: IGNITION -> IGNITION | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-07-09: IGNITION -> IGNITION | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 2026-07-10: IGNITION -> IGNITION | reset C/B false / false | soft_reset false | duration_lock false | defensive_override false | mismatch 0
- 边界: 仅为审计事实；不生成评分、建议或交易判断。

## 战略上下文快照
- 趋势广度模式: NarrowLeadership
- 市场周期位置: CrowdedExpectation
- 持仓效率: Neutral
- 战略上下文行:
  - 市场结构模式: 核心资产主导期
  - 长期方向: 长期结构趋势增强
  - 周期位置: CROWDED_EXPECTATION
  - 周期特征: 预期拥挤 / 核心资产集中 / 好消息钝化风险
  - 拥挤风险: ACTIVE
  - 宏观重力: 利率压力 RISING / 实际利率 TIGHT / 信用压力 NORMAL / 成长股估值 COMPRESSING / 流动性 NEUTRAL / 收益率曲线 FLAT
  - 宏观重力: 只解释折现率与流动性环境，不生成交易信号
  - 证据持续性: 持续累积
  - 证据覆盖: AI 投入产出验证 (Capex Payoff) / 业绩实质性确认 (Earnings Quality) / 订单能见度提升 (Order Visibility)
  - 战术状态: READY，等待执行层确认
- 边界: 仅为快照；不生成评分、建议或交易判断。

## Signal Context（信息质量上下文）
- Information Content: LOW
- Primary Context: None
- Context Quality: LOW
- Event Fact: N/A
- Source Diagnostics: No major event today. Current monitoring remains idle.
- Interpretation: 今天未识别到高信息量宏观事件。官方经济日历未命中 CPI、FOMC、就业、GDP 等事件。今日价格变化更可能由企业消息、板块轮动、技术走势驱动。
- 边界: Signal Context 仅作周度追溯沉淀；不接入 Gate、Execution、Trader、READY / EXECUTE 或 Position Sizing。

## Market Interpretation Snapshot
- decision_weight: 0%
- dayType: normal
- reason: trend_continuation
- exceptionalFactors: []
- Narrative:
  - 今天是正常趋势延续。
  - No major event today. Current monitoring remains idle.
  - 没有结构性恶化证据。
- Leadership Confidence: HIGH
- Leadership Metrics:
  - Primary Leader: [SPY]
  - Secondary Leaders: [GOOG, U]
  - Watchlist Leaders: [PLTR, ISRG, MSFT]
  - leadershipBreadth: rotation
- very_narrow:
  - breadthScore: 35
  - concentrationScore: 82
  - rotationScore: 18
- Rotation Observation:
  - rotationType: mega_cap_internal_rotation
  - from: [PLTR, ISRG, MSFT]
  - to: [SPY, GOOG, U]
  - interpretation: 资金不是撤退，而是在主导大盘的核心资产之间轮动。
  - observationOnly: true
- Observation Confidence:
  - trend: HIGH
  - macro: LOW
  - supply: HIGH
  - expectation: UNAVAILABLE
  - gravity: UNAVAILABLE
  - flow: MEDIUM
  - overall: MEDIUM
- Interpretation Priority:
  - Trend: ★★★★★
  - Supply: ★★
  - Macro: ★
  - Flow: ☆
  - Expectation: ☆
- Leader Persistence:
  - 当前 Leader: SPY
  - 连续领导天数: 1 天
  - 领导评分: 10.2
  - 领导状态: NEW
  - 较昨日变化: SPY：首次观察
  - 边界：仅用于观察；本区块不改变 Decision、Gate、Execution、Trader 或 Position Sizing。
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
- 最新观测日: 2026-07-10
- 最新 Near-Term Supply 数量: 0
- 最新 Future Queue 数量: 1
- 7 日 Future Queue 最小值 / 最大值: 0 / 1
- 已报道 / 已确认: 1 / 0
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
- 观测日: 2026-07-11
- decision_weight: 0%
- trade_signal: false
- observation_count: 16
- subjects: GOOG, ISRG, MSFT, NVDA, PLTR, TSLA
- 边界: Expectation Layer 仅用于观测市场预期，不进入 Gate、Execution、Trader、Action Matrix、READY / EXECUTE、Position Sizing，也不生成交易信号。

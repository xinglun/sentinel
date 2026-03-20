# Sentinel 决策引擎重构路线图

## 1. 文档目的

本文档定义 Sentinel 从“会说话的仪表盘”演进为“决策引擎”的最终态架构、模块边界、阶段任务、依赖关系与验收标准。

本文档关注的是策略内核，不包含 Figma、Dashboard 等展示层目标。最终对外输出仍以 Telegram 为主，同时保留机器可读的 JSON 产物用于回放、告警和后续自动化。

## 2. 北极星目标

Sentinel 最终必须每天稳定回答以下问题：

1. 市场当前处于什么状态。
2. 当前状态允许做什么，不允许做什么。
3. 组合层面的目标仓位、节奏和风险约束是什么。
4. 每只资产当前处于什么执行状态。
5. 市场状态和个股状态组合后，最终动作是什么。
6. 如果状态发生迁移，迁移的触发条件和原因是什么。

最终系统的核心输出不是解读文案，而是一个确定性的 `decision_packet.json`，Telegram 文本只是该决策包的消费层之一。

## 3. 设计原则

1. 先判断市场状态，再决定个股动作。
2. 升级慢，降级快。
3. 动作由状态决定，不由情绪决定。
4. 个股强弱必须服从市场状态。
5. 不预测，只响应结构变化。
6. Telegram 是输出通道，不是策略逻辑承载层。
7. 回测必须复用与实盘相同的决策管线。

## 4. 最终态架构

```text
Data Providers
  -> Feature Layer
  -> Market Regime State Machine
  -> Portfolio Policy Engine
  -> Asset State Engine
  -> Action Matrix
  -> Decision Packet
  -> Telegram Renderer / JSON Persistence / Backtest Replay / Trading Hooks
```

### 4.1 目标分层

1. `Data Layer`
   输入行情、情绪、持仓、历史状态。
2. `Feature Layer`
   统一产出市场特征与个股特征，不做决策。
3. `Decision Layer`
   包含市场状态机、组合策略、个股执行状态、动作矩阵。
4. `Delivery Layer`
   负责 Telegram、JSON、transition log、backtest 输出。

### 4.2 市场状态模型

对外第一版采用 6 个主状态：

1. `IGNITION`
2. `NEWBORN`
3. `EARLY_CONFIRMATION`
4. `ESTABLISHED`
5. `CONFIRMED`
6. `DEFENSIVE`

对内建议采用双层表示，避免后续扩展时把所有语义挤在一个枚举里：

1. `lifecycle_state`
   `IGNITION / NEWBORN / EARLY_CONFIRMATION / ESTABLISHED / CONFIRMED`
2. `risk_overlay`
   `NORMAL / DECELERATING / DEFENSIVE / BROKEN`

对外展示时，将内部双层状态映射为主状态和附加标签，例如：

1. `ESTABLISHED + NORMAL -> ESTABLISHED`
2. `ESTABLISHED + DECELERATING -> ESTABLISHED (Decelerating)`
3. `ANY + DEFENSIVE/BROKEN -> DEFENSIVE`

### 4.3 个股状态模型

个股执行状态保留为独立子状态机：

1. `OPTIMAL`
2. `CRUISE`
3. `PULLBACK`
4. `CAUTION`
5. `OVERHEAT`
6. `DEFEND`
7. `FORMING`

个股状态不直接决定最终动作，必须经过市场状态与组合策略约束后，再由动作矩阵产出执行结果。

## 5. 核心输出契约

最终输出以 `decision_packet.json` 为准，建议结构如下：

```json
{
  "date": "2026-03-19",
  "market_regime": {
    "market_state": "ESTABLISHED",
    "lifecycle_state": "ESTABLISHED",
    "risk_overlay": "NORMAL",
    "scores": {
      "confidence": 80.58,
      "stability": 30.0,
      "structural": 33.6,
      "maturity": 45.0,
      "flow_acceleration": 0.0
    },
    "transition": {
      "from": "EARLY_CONFIRMATION",
      "to": "ESTABLISHED",
      "changed": true,
      "reasons": [
        "stability crossed 25",
        "maturity crossed 35",
        "confidence remained above 78"
      ]
    }
  },
  "portfolio_policy": {
    "target_exposure_min": 0.60,
    "target_exposure_max": 0.80,
    "allow_chase": false,
    "allow_pullback_buy": true,
    "allow_new_risk": true,
    "risk_assets_mode": "DEFEND"
  },
  "assets": [
    {
      "symbol": "NVDA",
      "asset_state": "OPTIMAL",
      "action": "HOLD",
      "reasons": ["trend intact", "market regime allows hold"]
    }
  ],
  "telegram": {
    "headline": "Market State: ESTABLISHED",
    "summary": "Hold core leaders, buy controlled pullbacks, no chasing."
  }
}
```

JSON 是主产物，Telegram 文本必须从该结构渲染，禁止在渲染阶段补做策略判断。

## 6. 模块设计

| 模块 | 责任 | 主要输入 | 主要输出 | 上游依赖 |
| --- | --- | --- | --- | --- |
| `src/core/features.rs` | 统一提取市场与个股特征 | 行情、情绪、历史 telemetry、持仓 | `MarketFeatures`、`AssetFeatures` | `data/*`、`ledger` |
| `src/core/market_regime.rs` | 市场状态识别与迁移 | `MarketFeatures`、前序状态历史 | `MarketRegimeSnapshot` | `features` |
| `src/core/portfolio_policy.rs` | 组合层策略约束 | `MarketRegimeSnapshot` | `PortfolioPolicy` | `market_regime` |
| `src/core/asset_state.rs` | 个股状态识别 | `AssetFeatures` | `AssetStateSnapshot` | `features` |
| `src/core/action_matrix.rs` | 市场状态 × 个股状态 -> 动作 | `MarketRegimeSnapshot`、`PortfolioPolicy`、`AssetStateSnapshot` | `AssetActionDecision` | `market_regime`、`portfolio_policy`、`asset_state` |
| `src/core/decision.rs` | 聚合最终决策包 | 全部上游结果 | `DecisionPacket` | 所有决策模块 |
| `src/core/report.rs` | Telegram 与 Markdown 渲染 | `DecisionPacket` | Telegram 文本、Markdown | `decision` |
| `src/core/transition_log.rs` | 迁移日志持久化 | `DecisionPacket`、历史状态 | `transition_log.jsonl`、`state_transitions.csv` | `decision` |
| `src/backtest.rs` | 决策回放与指标评估 | 历史行情、同一决策管线 | 回测报告、迁移矩阵、策略指标 | `decision` |
| `src/cli.rs` | 管线装配与命令入口 | 配置、provider、持久化路径 | 执行结果 | 全部模块 |

## 7. 模块依赖与实现顺序

### 7.1 强依赖链

1. `features.rs` 是整个系统的数据底座。
2. `market_regime.rs` 必须建立在 `MarketFeatures` 之上。
3. `portfolio_policy.rs` 必须建立在 `MarketRegimeSnapshot` 之上。
4. `asset_state.rs` 与 `market_regime.rs` 可以并行开发，但 `action_matrix.rs` 必须等待两者完成。
5. `decision.rs` 必须等全部决策模块完成后接入。
6. `report.rs` 与 `backtest.rs` 必须改为消费 `DecisionPacket`，不能继续各自维护一套策略推理。

### 7.2 现有代码的重构方向

1. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/engine.rs)
   现有个股状态逻辑逐步拆到 `asset_state.rs`，保留指标计算部分到 `features.rs`。
2. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)
   现有 `GravityHealth` 与 `CapitalPosture` 的宏观决策逻辑迁出，报告模块只负责消费决策结果。
3. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
   改为明确的 pipeline orchestrator。
4. [backtest.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/backtest.rs)
   从“状态统计器”升级为“状态机回放实验室”。

## 8. 分阶段任务拆解

### Phase 0: 规格冻结

**目标**

冻结状态定义、迁移规则、动作矩阵与决策包契约。

**交付物**

1. `docs/specs/STATE_DEFINITIONS.md`
2. `docs/specs/TRANSITION_RULES.md`
3. `docs/specs/ACTION_MATRIX.md`
4. `docs/archive/decision_engine_roadmap.md`
5. `decision_packet` schema 草案

**依赖**

无。

**验收标准**

1. 每个市场状态都有明确定义、允许动作、禁止动作、升级条件、降级条件。
2. 每个个股状态在所有市场状态下都有唯一动作映射。
3. `decision_packet` 字段定义完整，没有“以后再补”的关键空洞。

### Phase 1: 特征层重建

**目标**

把宏观和个股的特征提取从报告与状态判断中剥离出来。

**交付物**

1. `src/core/features.rs`
2. `MarketFeatures`、`AssetFeatures` 数据结构
3. 历史状态读取接口，用于迁移判定与 regime age 计算

**依赖**

Phase 0。

**验收标准**

1. 同样输入下，特征计算结果稳定且可复现。
2. Radar 与 Backtest 复用同一套特征提取函数。
3. 单元测试覆盖关键特征：`stability`、`structural`、`maturity`、`flow_acceleration`、`dominance_margin`。

### Phase 2: 市场状态机

**目标**

实现确定性的市场状态识别、升级和降级机制。

**交付物**

1. `src/core/market_regime.rs`
2. `MarketRegimeSnapshot`
3. 状态迁移原因生成器
4. 连续天数确认和快速降级规则

**依赖**

Phase 1。

**验收标准**

1. 支持 `IGNITION -> NEWBORN -> EARLY_CONFIRMATION -> ESTABLISHED -> CONFIRMED` 升级路径。
2. 支持 `ANY -> DEFENSIVE` 快速降级。
3. 支持连续天数规则，例如 “跌破阈值 2 天后降级”。
4. 对每次状态变化都能产出结构化原因列表。
5. 测试覆盖升级、降级、边界值、抖动抑制。

### Phase 3: 组合策略引擎

**目标**

把“市场状态”转换为“组合层约束”。

**交付物**

1. `src/core/portfolio_policy.rs`
2. `PortfolioPolicy`
3. 各状态下的 exposure、允许动作、禁止动作规则

**依赖**

Phase 2。

**验收标准**

1. 每个市场状态都能产出目标仓位区间。
2. 每个市场状态都能明确 `allow_chase`、`allow_pullback_buy`、`allow_new_risk`。
3. `DEFENSIVE` 状态下，组合约束可直接冻结风险资产动作。

### Phase 4: 个股执行状态与动作矩阵

**目标**

把个股识别与最终动作解耦，形成标准动作矩阵。

**交付物**

1. `src/core/asset_state.rs`
2. `src/core/action_matrix.rs`
3. `AssetStateSnapshot`
4. `AssetActionDecision`

**依赖**

Phase 1、Phase 2、Phase 3。

**验收标准**

1. 个股状态机与市场状态机解耦。
2. 动作矩阵对所有 `market_state × asset_state` 组合都是完备的。
3. 每个资产最终动作只能来自动作矩阵，不能在报告层额外改写。
4. 动作结果至少包含 `ACCUMULATE / HOLD / REDUCE / FREEZE / AVOID / OBSERVE`。

### Phase 5: 决策包与 Telegram 输出重构

**目标**

以 `DecisionPacket` 作为唯一事实源，重写输出链路。

**交付物**

1. `src/core/decision.rs`
2. `DecisionPacket`
3. `report.rs` 重构为纯渲染模块
4. 每日 `decision_packet.json`
5. 由决策包直接生成的 Telegram 文本

**依赖**

Phase 2、Phase 3、Phase 4。

**验收标准**

1. Telegram 输出不再自行推断策略，只渲染 `DecisionPacket`。
2. JSON 与 Telegram 的核心结论一致。
3. 每日主输出文件包含市场状态、组合策略、资产动作、迁移原因。

### Phase 6: 状态持久化与迁移日志

**目标**

让系统具备可追溯、可回放、可审计能力。

**交付物**

1. `src/core/transition_log.rs`
2. `transition_log.jsonl`
3. `state_transitions.csv`
4. 扩展后的 `telemetry.csv`

**依赖**

Phase 5。

**验收标准**

1. 任意一天都可追溯前一状态、当前状态、迁移原因。
2. `telemetry.csv` 可支持后续迁移回放和 regime age 计算。
3. 日志结构可以直接供 backtest/replay 读取。

### Phase 7: 回测与回放框架重构

**目标**

让回测验证“状态机质量”和“动作约束效果”，而不只是单点状态胜率。

**交付物**

1. `backtest.rs` 改造为复用 `DecisionPacket` 管线
2. 市场状态迁移矩阵
3. 状态持续时间统计
4. 降级响应速度统计
5. 组合暴露与回撤对比指标

**依赖**

Phase 5、Phase 6。

**验收标准**

1. Backtest 与 Radar 使用同一决策逻辑。
2. 能输出状态迁移频率、平均持续时间、升级/降级滞后。
3. 能衡量 `DEFENSIVE` 触发后对回撤的抑制效果。

### Phase 8: 交易接入与执行门禁

**目标**

将决策引擎结果安全接到自动交易代理，但保持 Telegram 仍是主要对外输出。

**交付物**

1. `trader_agent` 接入 `DecisionPacket`
2. 动作到交易指令的门禁规则
3. 风险预算、单日预算、状态级熔断

**依赖**

Phase 5。

**验收标准**

1. 交易代理不能绕过组合策略和动作矩阵。
2. `DEFENSIVE` 状态下禁止新增风险暴露。
3. 自动交易可以关闭，但 Telegram 输出不受影响。

## 9. 阶段性里程碑

| 里程碑 | 意义 | 完成标准 |
| --- | --- | --- |
| M1 | Sentinel 能定义市场状态 | Phase 0-2 完成 |
| M2 | Sentinel 能定义组合约束 | Phase 3 完成 |
| M3 | Sentinel 能给出统一动作 | Phase 4 完成 |
| M4 | Sentinel 成为真正决策引擎 | Phase 5 完成 |
| M5 | Sentinel 具备可追溯能力 | Phase 6 完成 |
| M6 | Sentinel 具备可验证能力 | Phase 7 完成 |
| M7 | Sentinel 可安全联动交易 | Phase 8 完成 |

## 10. 全局验收标准

以下条件同时满足，才可认为决策引擎重构完成：

1. 日报主产物是 `decision_packet.json`，Telegram 由其渲染。
2. 报告模块不再承担策略判断。
3. 所有资产最终动作均由动作矩阵决定。
4. 所有市场迁移都有结构化原因。
5. Radar、Backtest、Telegram、Trading 复用同一套决策管线。
6. `DEFENSIVE` 的降级触发速度显著快于升级速度。
7. 历史回放可以复原任意交易日的状态、策略与动作。

## 11. 非目标

以下内容不属于当前路线图的主要目标：

1. Figma 设计稿或 Dashboard 前端。
2. 复杂机器学习分类器替代规则状态机。
3. 先做视觉美化再反推策略逻辑。
4. 在状态机未完成前直接扩充自动交易复杂度。

## 12. 当前推荐执行顺序

如果按结果优先推进，建议实际开发顺序如下：

1. 先完成 Phase 0，把状态定义、迁移规则、动作矩阵和决策包字段冻结。
2. 再完成 Phase 1 和 Phase 2，先让系统能稳定判断市场状态。
3. 然后完成 Phase 3 和 Phase 4，把“状态”真正变成“动作约束”。
4. 再做 Phase 5 和 Phase 6，把 Telegram 和持久化接上统一决策包。
5. 最后做 Phase 7 和 Phase 8，用同一内核做回测与交易门禁。

在这个顺序下，Telegram 输出不会被削弱，只会从“解释结果”升级成“表达决策”。

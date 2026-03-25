# Sentinel 状态机信号对齐任务单

## 1. 目的

本任务单用于修正 Sentinel 当前在市场状态、展示文案、执行动作之间存在的语义错位问题。

当前系统已经具备以下能力：

1. 市场层 `InertiaLayer`
2. 个股层 `Relative Strength Memory`
3. `Promotion Cap`
4. `Upgrade / Downgrade Friction`

但最近输出暴露出一个核心问题：

1. 报告层显示 `IGNITION + Stability 0 + 多个 OPTIMAL`
2. 动作层仍输出“加仓”
3. 使用者实际应当理解为“候选强者观察期”，而不是“可执行加仓期”

因此，本次任务的目标不是继续增强选股逻辑，而是完成以下对齐：

1. **状态语义对齐**：`Age` / `Stability` 的真实含义必须一致。
2. **展示语义对齐**：报告不能夸大早期候选信号。
3. **执行语义对齐**：脆弱启动期不得输出过于进攻的动作建议。

---

## 2. 当前问题定义

### 2.1 Stability 量纲不统一

当前 `stability_score` 在特征层按 `0..1` 计算，但在部分逻辑和文案中被当作 `0..100` 使用。

已确认现状：

1. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs) 中：
   - `stability_score = (stability_structural / 50.0) * trend_maturity`
   - 结果是 `0..1`
2. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs) 中：
   - `if features.stability_score * 100.0 < 10.0`
   - 逻辑把它临时转成百分制判断
3. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs) 中：
   - 直接以 `"{:.0}"` 打印 `stability_score`
   - 导致早期阶段经常显示为 `0`

结果：

1. 使用者会把 `Stability 0` 理解为“完全没有稳定性”
2. 实际上它经常只是“百分制展示缺失”

### 2.2 Regime Age 存在双重递增风险

当前 `regime_age` 在状态机中存在两次递增：

1. `transition()` 先计算 `current_potential_age = prev_age + 1`
2. `compute_next_state()` 又默认 `next_age = regime_age + 1`

这会带来两个问题：

1. 生命周期判断容易产生 off-by-one
2. 观察层看到的 `Age` 可能不等于真实的状态停留天数

结果：

1. 如果外部看到 `Age` 停住，问题可能在历史链路
2. 如果外部看到 `Age` 跳升，问题可能在状态机内部

这两种情况都会破坏“时间进入系统”的可信度。

### 2.3 IGNITION 脆弱期仍输出进攻性动作

当前动作矩阵中：

1. `IGNITION + OPTIMAL -> ACCUMULATE`

这会导致以下矛盾同时出现：

1. 市场层明确处于脆弱启动期
2. 个股层只是初步候选强者
3. 报告层却把它展示为“加仓区 / Top Actions”

这不符合当前策略真实意图。

当前真实策略意图应当是：

1. `IGNITION + 低稳定度`
2. 所有 `OPTIMAL` 只视为“候选强者”
3. 行为上只允许观察或轻仓跟踪，不允许主动加仓

### 2.4 报告缺少“候选 vs 确认”标签

目前 Telegram / 报告层虽然显示：

1. `Confidence`
2. `Stability`
3. `Regime Age`

但没有把以下关键语义显式表达出来：

1. 现在是“候选名单生成”
2. 不是“确认机会成立”

结果是：

1. 文案看起来很克制
2. 但结构上仍像一个交易建议

---

## 3. 修改目标

本轮开发完成后，系统必须满足以下行为：

### 3.1 Stability 语义统一

必须二选一，且全系统保持一致：

1. 全部改为 `0..100`
2. 全部保留 `0..1`，但展示时显式转成百分比

验收要求：

1. 状态机门槛判断与报告展示使用同一语义
2. `Stability 0` 不得因为格式问题被误报

### 3.2 Age 单次推进

`regime_age` 必须只在一个地方推进一次。

验收要求：

1. 正常无状态切换时，每日 `+1`
2. 软降级时按规则回退
3. 硬 reset 时才回到 `1`
4. 不允许出现“内部双跳”或“无原因卡住”

### 3.3 IGNITION 脆弱期动作降级

新增一条显式执行规则：

1. `IGNITION && Stability < 阈值`
2. 禁止 `ACCUMULATE`
3. 只允许 `OBSERVE` 或 `HOLD`

建议阈值：

1. 与当前 reset / fragility 语义一致
2. 可先使用 `stability < 10`

该规则应优先表达为产品语义，而不是仅靠文案弱化。

### 3.4 报告输出候选标签

在脆弱启动期，报告必须把个股强信号标记为“候选”而不是“确认”。

建议输出方式至少实现其一：

1. 在 Top Actions 原因中追加 `候选强者，等待连续性确认`
2. 在 Signals 区域追加统一诊断提示
3. 在 Tactical Summary 中把“加仓区”改为“候选区”

验收要求：

1. 用户一眼能分辨“候选”与“确认”
2. 不再出现“Fragile + 加仓建议”这种认知冲突

---

## 4. 开发任务拆解

### P0-1 统一 Stability 量纲

涉及文件：

1. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs)
2. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
3. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)
4. [telemetry.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/telemetry.rs)

任务要求：

1. 明确定义 `stability_score` 的内部单位
2. 统一门槛判断、报表展示、telemetry 输出
3. 修正标签函数与数值区间的对应关系

建议补充：

1. 为 `stability_score` 加注释，标明单位

### P0-2 修正 Regime Age 推进逻辑

涉及文件：

1. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/engine.rs)
2. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
3. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs)

任务要求：

1. 确认 `prev_age` 进入状态机后的唯一推进点
2. 消除双重递增
3. 保证升级门槛判断与最终落盘值一致
4. 如有必要，新增审计字段说明：
   - `evaluated_age`
   - `next_age`

### P0-3 增加 IGNITION 脆弱期执行闸门

涉及文件：

1. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)
2. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs)
3. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

任务要求：

1. 在 `IGNITION + Fragile` 条件下覆盖原有 `ACCUMULATE`
2. 将 `OPTIMAL` 的行为降级为：
   - `OBSERVE`
   - 或 `HOLD`
3. 原因中必须显式说明：
   - `Candidate only`
   - `Execution suppressed in fragile ignition`

注意：

1. 这里是产品策略层显式约束
2. 不能只靠 `Execution Disabled` 运行模式来掩盖问题

### P1-1 报告层增加“候选强者”诊断

涉及文件：

1. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)
2. [report_ui_tests.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report_ui_tests.rs)

任务要求：

1. 在 `IGNITION + Fragile` 时增加统一文案
2. 文案需明确说明：
   - 当前不是买点确认
   - 当前是主线筛选期
3. 避免改变成熟阶段文案

### P1-2 增加历史链路排查与测试保护

涉及文件：

1. [persistence.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/persistence.rs)
2. [pipeline_integration.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/tests/pipeline_integration.rs)
3. 如有需要补充：
   - `backtest`
   - telemetry 相关测试

任务要求：

1. 确认历史包按时间顺序加载并用于 age/memory 连续计算
2. 为连续两日、连续三日场景增加集成测试
3. 保证：
   - `Age` 连续推进
   - `Stability` 连续演化
   - 候选资产不会在脆弱期直接触发加仓动作

---

## 5. 验收标准

本任务完成后，以下场景必须成立：

### A. 脆弱启动首日

输入特征：

1. 市场进入 `IGNITION`
2. `Stability` 很低
3. 多只资产为 `OPTIMAL`

预期输出：

1. 报告明确显示“候选观察期”
2. Top Actions 不得表现为积极加仓
3. `OPTIMAL` 资产只可作为候选强者显示

### B. 脆弱启动连续 2-3 日

输入特征：

1. 候选资产名单连续存在
2. `Age` 每日推进
3. `Stability` 逐步提升但未过阈值

预期输出：

1. `Age` 连续推进，不跳日、不冻结
2. 动作仍维持克制
3. 报告强调“等待连续性确认”

### C. 启动期转入确认期

输入特征：

1. `IGNITION -> NEWBORN` 或更高阶段
2. `Stability` 通过阈值
3. 候选强者连续保留

预期输出：

1. 个股可恢复正常 `ACCUMULATE` / `HOLD` 映射
2. 报告措辞从“候选”转向“确认”
3. 不影响成熟阶段既有逻辑

### D. 普通降级与硬 reset

预期输出：

1. 普通降级不重置 `Age` 到 `1`
2. 只有通过 reset gate 才允许硬重置
3. 报告与 telemetry 中的 `Age`、`Stability` 一致可解释

---

## 6. 测试清单

至少新增或修正以下测试：

1. `stability_score` 单位一致性测试
2. `report` 展示数值与内部数值一致性测试
3. `regime_age` 正常推进测试
4. `regime_age` 软回退测试
5. `regime_age` 硬 reset 测试
6. `IGNITION + fragile + OPTIMAL` 不再输出 `ACCUMULATE`
7. `IGNITION + fragile` 报告输出“候选强者”提示
8. `NEWBORN` 或更高阶段恢复正常动作映射

建议优先落在：

1. [pipeline_integration.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/tests/pipeline_integration.rs)
2. [report_ui_tests.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report_ui_tests.rs)
3. [market_regime.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/market_regime.rs)
4. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)

---

## 7. 非目标

本轮不处理以下事项：

1. 更换选股 universe
2. 重写 Relative Strength Memory 评分公式
3. 重新定义 `OPTIMAL / CRUISE / PULLBACK` 的资产层分类标准
4. 接入真实交易执行逻辑优化
5. Telegram 基础设施或外部通知通道改造

本轮重点只有一件事：

**让状态机、报告、动作三者说同一种话。**

---

## 8. 建议实施顺序

建议开发按以下顺序提交：

1. P0-1 `Stability` 量纲统一
2. P0-2 `Age` 推进修正
3. P0-3 `IGNITION Fragile` 动作闸门
4. P1-1 报告候选标签
5. P1-2 集成测试与历史链路保护

建议每一项单独提交，避免把数值语义修复和产品行为修复混在同一个 commit 中。

---

## 9. 完成定义

当以下条件全部满足时，本任务视为完成：

1. 开发已完成上述 P0 项
2. 相关测试通过
3. 新报告中不再出现：
   - `Fragile + 多个 OPTIMAL + 加仓建议`
4. 使用者可以稳定区分：
   - 候选强者
   - 确认强者
5. `Age` 与 `Stability` 在连续多日输出中可解释、可追踪

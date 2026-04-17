# NO_TRADE 硬约束化任务文档

## 1. 背景

当前 Telegram 战情板已经能够展示 `NO TRADE`，并能在低稳定度、连续性不足、Participation 未通过时给出“禁止动作”的主结论。

但当前实现仍然存在最后一层语义漏水：

1. `0-10%` 这类仓位表达仍会给用户留下“可以试一点点”的心理逃生口。
2. `NO TRADE` 仍偏展示结论，而不是行为硬门。
3. 候选观察名单虽然已降级，但缺少更强的规则约束，后续存在文案漂回“交易候选”的风险。
4. `NO TRADE` 的设计语义未被明确写死，后续容易被误解成 `DEFENSIVE` 或“市场看空”。

本轮任务的目标不是继续美化，而是把 `NO TRADE` 从“提醒你别动”升级成“系统明确禁止你动”。

---

## 2. 设计原则

### 2.1 NO_TRADE 的正式定义

必须在文档、代码语义和展示输出中明确写死：

> `NO TRADE` 是展示态，不等于市场看空；它表示“当前不允许主动开新仓”。

这意味着：

1. `NO TRADE` 不是 `DEFENSIVE`
2. `NO TRADE` 不是强制清仓
3. `NO TRADE` 不是趋势反转判断
4. `NO TRADE` 是“当前不允许新增主动风险暴露”的行为约束

### 2.2 行为约束优先于信息展示

`NO TRADE` 场景下的输出顺序必须遵循：

1. 先禁令
2. 再行为模式
3. 再额度限制
4. 再战况摘要
5. 最后才解释原因和候选名单

不得把“候选名单”放到“行为禁令”之前。

### 2.3 候选名单必须继续降级

`NO TRADE` 场景下，任何资产列表都只能表达：

1. 观察
2. 候选
3. 筹备
4. 强度确认中
5. 回撤观察

不得出现：

1. 加仓
2. 买入
3. 建仓
4. 交易候选

---

## 3. 最终展示契约

`NO TRADE` 场景下，最终展示顺序固定为：

```text
### 禁止动作（NO TRADE）

> 任何主动交易行为都将违反系统规则。

> 状态：未确认启动期
> 行为：禁止交易
> 新开仓上限 · 0%

> 战情总览 · 观察 8 | 持有 0 | 收缩 0
> 机会 · 暂无明确机会
> 风险 · 暂无明显风险

- 未就绪原因
- 候选观察名单
- 监控信号
```

当前实现已进一步收敛为“执行优先”顺序：

```text
1) 决策层（第一屏）
   - 禁止动作（NO TRADE）
   - 新开仓上限 · 0%

2) 原因层（简化版）
   - 稳定性 x/10
   - 连续性 x/3
   - 主线结构（如：无主线）

3) 观察重点层
   - 突破识别（Breakout）
   - breakout 状态展示包含时间感（如：突破萌芽（第1天））

4) 证据层（次屏）
   - 状态转移证据（前台可紧凑，归档保持完整）
```

其中有 3 条是硬规则：

1. `NO TRADE` 第一屏必须出现
2. `新开仓上限 · 0%` 必须出现
3. `任何主动交易行为都将违反系统规则。` 必须出现

补充硬规则：

4. 前台证据层不得抢占决策层之前的位置
5. 任何模板占位符（如 `{}` / `{:.1}`）不得出现在最终报告文本中

---

## 4. 数据模型改动

### 4.1 DecisionSummaryViewModel 新增字段

文件：
[presentation.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/presentation.rs)

新增字段：

```rust
pub state_tag_label: String,
pub state_tag_value: String,
pub action_tag_label: String,
pub action_tag_value: String,
pub hard_rule_note: String,
pub entry_cap_label: String,
pub entry_cap_value: String,
pub entry_cap_note: Option<String>,
```

字段职责：

1. `state_tag_*`
   用于输出 `状态：未确认启动期`

2. `action_tag_*`
   用于输出 `行为：禁止交易`

3. `hard_rule_note`
   用于输出行为禁令，例如：
   `任何主动交易行为都将违反系统规则。`

4. `entry_cap_*`
   用于输出：
   `新开仓上限 · 0%`

5. `entry_cap_note`
   用于输出副说明，例如：
   `仅允许已有持仓自然波动，不允许主动开仓。`

---

## 5. PresentationAssembler 组装规则

文件：
[presentation_assembler.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/presentation_assembler.rs)

### 5.1 行为规则

首版明确规则：

```text
if is_data_missing || !participation_ready:
    action_status = NO_TRADE
```

但必须同时满足语义说明：

```text
NO_TRADE != DEFENSIVE
NO_TRADE 表示禁止主动开新仓
```

### 5.2 state_tag 规则

建议首版：

1. `IGNITION + !participation_ready`
   -> `未确认启动期`

2. `is_data_missing`
   -> `数据不可用`

3. 其他非 ready 场景
   -> `参与条件未满足`

### 5.3 action_tag 规则

固定为：

1. `NO_TRADE` -> `禁止交易`
2. `PROBE` -> `试探参与`
3. `ACCUMULATE` -> `主动加仓`
4. `TREND_FOLLOW` -> `趋势跟随`
5. `DEFENSIVE` -> `防御收缩`

### 5.4 entry_cap 规则

`NO TRADE` 场景下固定为：

```text
entry_cap_label = 新开仓上限
entry_cap_value = 0%
entry_cap_note = 仅允许已有持仓自然波动，不允许主动开仓。
```

不得再生成：

1. `仓位建议 · 0-10%`
2. `0-10%`

### 5.5 candidate_only_note 规则

必须由 assembler 统一输出：

```text
以下仅为候选观察名单，不构成交易指令。
```

不得在 `report.rs` 临时拼接。

---

## 6. i18n 词典要求

文件：
[i18n.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/i18n.rs)

至少新增以下词典项：

```rust
state_tag
action_tag
entry_cap
entry_cap_note
state_ignition_unconfirmed
state_data_unavailable
state_participation_blocked
no_trade_rule
```

### 6.1 中文建议文案

1. `state_tag`: `状态`
2. `action_tag`: `行为`
3. `entry_cap`: `新开仓上限`
4. `entry_cap_note`: `仅允许已有持仓自然波动，不允许主动开仓。`
5. `state_ignition_unconfirmed`: `未确认启动期`
6. `state_data_unavailable`: `数据不可用`
7. `state_participation_blocked`: `参与条件未满足`
8. `no_trade_rule`: `任何主动交易行为都将违反系统规则。`

### 6.2 日文建议文案

1. `state_tag`: `状態`
2. `action_tag`: `行動`
3. `entry_cap`: `新規建て上限`
4. `entry_cap_note`: `既存保有の自然変動のみ許容し、新規建ては行わない。`
5. `state_ignition_unconfirmed`: `未確認始動期`
6. `state_data_unavailable`: `データ利用不可`
7. `state_participation_blocked`: `参加条件未達`
8. `no_trade_rule`: `あらゆる能動売買はシステム規律違反となる。`

### 6.3 英文建议文案

1. `state_tag`: `State`
2. `action_tag`: `Action`
3. `entry_cap`: `New Entry Cap`
4. `entry_cap_note`: `Existing holdings may drift naturally; no new entries are allowed.`
5. `state_ignition_unconfirmed`: `Ignition Unconfirmed`
6. `state_data_unavailable`: `Data Unavailable`
7. `state_participation_blocked`: `Participation Blocked`
8. `no_trade_rule`: `Any discretionary trade would violate system rules.`

---

## 7. report 层职责

文件：
[report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

要求：

1. 只消费 `DecisionSummaryViewModel`
2. 按固定顺序渲染
3. 不新增任何业务判断
4. 不在 `report.rs` 中拼接规则文案

### 7.1 禁止做的事

`report.rs` 不允许：

1. 根据 `market_state` 自己推断是否是 `NO TRADE`
2. 根据 `participation_ready` 自己拼状态标签
3. 根据任何资产状态自己拼候选说明

---

## 8. Top Actions / 候选观察名单约束

`NO TRADE` 场景下：

1. 标题必须降级为 `候选观察名单`
2. 资产条目不得出现“交易语义”

### 8.1 禁止出现的语义

候选观察名单中的任何资产不得出现：

1. 加仓
2. 买入
3. 建仓

### 8.2 允许出现的语义

可以出现：

1. 观察
2. 候选
3. 筹备
4. 回撤
5. 强度确认中

---

## 9. 验收标准

### 9.1 功能验收

`NO TRADE` 场景下必须满足：

1. 第一屏出现 `禁止动作（NO TRADE）`
2. 必须出现 `状态：未确认启动期`
3. 必须出现 `行为：禁止交易`
4. 必须出现 `新开仓上限 · 0%`
5. 必须出现 `任何主动交易行为都将违反系统规则。`
6. 必须出现 `以下仅为候选观察名单，不构成交易指令。`

### 9.2 反向验收

`NO TRADE` 场景下必须不满足：

1. 不得出现 `0-10%`
2. 不得出现旧字段 `仓位建议`
3. 候选观察名单中不得出现 `加仓`
4. 候选观察名单中不得出现 `买入`
5. 候选观察名单中不得出现 `建仓`

### 9.3 架构验收

1. 所有规则类文案必须由 assembler 生成
2. `report.rs` 不得新增业务判断
3. `DecisionPacket` 不得混入展示字段
4. 继续保持 `DecisionPacket -> PresentationPacket -> report` 单向链路

### 9.4 质量门

必须同时通过：

1. `cargo fmt`
2. `cargo test --quiet`
3. `cargo clippy --all-targets --all-features -- -D warnings`

---

## 10. 测试要求

### 10.1 presentation_tests

至少新增或补强以下断言：

1. `entry_cap_value == "0%"`
2. `hard_rule_note` 存在
3. `state_tag_value == "未确认启动期"`
4. `action_tag_value == "禁止交易"`
5. `candidate_only_note` 存在

### 10.2 report_ui_tests

至少新增或补强以下断言：

1. 包含 `新开仓上限 · 0%`
2. 不包含 `0-10%`
3. 不包含旧字段 `仓位建议`
4. 包含 `状态：未确认启动期`
5. 包含 `行为：禁止交易`
6. 候选观察名单中不包含 `加仓 / 买入 / 建仓`

### 10.3 i18n 回归测试

至少验证：

1. 中文、英文、日文的 `NO TRADE` 规则文案都能正常注入
2. `entry_cap_note` 不缺词
3. `state_tag / action_tag` 不缺词

---

## 11. 非目标

本轮不做：

1. 修改 `ParticipationReadiness` 判定规则
2. 修改 `ExitDecision` 判定规则
3. 修改 `Engine`
4. 修改 `DecisionPacket`
5. 修改交易执行层

本轮只做：

1. `NO TRADE` 展示硬约束化
2. 仓位表达收紧
3. 候选名单降级协议固化

---

## 12. 最终交付定义

本轮完成的标志不是“文案更顺了”，而是：

> 系统在无优势状态下，不再提醒用户别动，而是明确禁止用户主动开新仓。

只有当 `NO TRADE` 具备：

1. 状态标签
2. 行为标签
3. 新开仓上限 0%
4. 规则禁令
5. 候选名单降级说明

这五项同时成立时，本任务才算完成。

# Exit Decision Summary 任务文档

## 1. 目标

本任务不是重做退出系统，而是把已经存在于底层的 `ExitDecision` 提升到和 `NO TRADE` 同级别的前台结论层。

当前系统已经能明确回答：

1. 今天能不能新开仓
2. 当前是否处于 `NO TRADE`
3. 新开仓上限是多少

但还不能在报表第一屏明确回答：

1. 已有持仓该继续持有还是减仓
2. 是否已经触发退出
3. 当前的“不允许买入”是否同时意味着“需要卖出”

本轮目标是补齐这一层：

> 让报表不仅能说“不能买”，还能明确说“要不要卖 / 减仓 / 持有”。

---

## 2. 背景问题

当前 Telegram / Markdown 战情板已经具备：

1. `NO TRADE` 行为禁令
2. 候选观察名单
3. 监控信号
4. 战术分区
5. 风险与机会

但“已有持仓处理建议”仍然缺席。

这会导致一个典型问题：

1. 系统能阻止错误买入
2. 但不能明确指导已有仓位是 `HOLD`、`TRIM` 还是 `EXIT`

从产品角度看，这意味着：

1. `Entry Gate` 已经闭环
2. `Exit Gate` 仍停留在底层语义，未进入 front page

---

## 3. 设计原则

### 3.1 ExitDecisionSummary 必须与 NO_TRADE 解耦

必须明确：

1. `NO TRADE` 只表示禁止主动开新仓
2. `NO TRADE` 不等于必须全部卖出
3. `ExitDecisionSummary` 单独决定已有持仓如何处理

也就是说：

| 场景 | 允许买入 | 允许继续持有 | 允许减仓/退出 |
|---|---|---|---|
| `NO TRADE` | 否 | 是 | 是 |
| `DEFENSIVE` | 否 | 视标的而定 | 是 |
| `ACCUMULATE` | 是 | 是 | 是 |

### 3.2 最小闭环优先

首版不要引入复杂止盈、成本线、ATR 或盈亏回撤逻辑。

只做 4 条核心规则：

1. `DEFEND -> EXIT`
2. `掉出核心 >= 3d -> TRIM`
3. `participation true -> false` 时：
   - 强资产 `HOLD`
   - 弱资产 `TRIM`
4. `OVERHEAT -> TRIM`

### 3.3 report.rs 只渲染

退出判断必须全部由 `PresentationAssembler` 基于底层事实生成。

`report.rs` 只能渲染：

1. 标题
2. 状态标签
3. 退出建议
4. 原因

不得新增退出逻辑判断。

---

## 4. 数据模型改动

### 4.1 新增 ExitDecisionSummaryViewModel

文件：
[presentation.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/presentation.rs)

建议新增：

```rust
pub struct ExitDecisionSummaryViewModel {
    pub title: String,
    pub items: Vec<ExitDecisionItemViewModel>,
}

pub struct ExitDecisionItemViewModel {
    pub symbol: String,
    pub intent: ExitDisplayIntent,
    pub intent_label: String,
    pub reason: String,
}

pub enum ExitDisplayIntent {
    Hold,
    Trim,
    Exit,
    Watch,
}
```

说明：

1. `Hold`
   表示已有持仓继续持有

2. `Trim`
   表示已有持仓减仓

3. `Exit`
   表示已有持仓退出

4. `Watch`
   表示未触发卖出条件，但仍需关注

### 4.2 PresentationPacket 扩展

在 `PresentationPacket` 中新增：

```rust
pub exit_summary: Option<ExitDecisionSummaryViewModel>,
```

要求：

1. 没有已有持仓或没有可展示项时可为 `None`
2. 有持仓处理建议时必须出现在第一屏

---

## 5. PresentationAssembler 组装规则

文件：
[presentation_assembler.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/presentation_assembler.rs)

### 5.1 输入事实来源

Assembler 必须基于已有领域事实和决策输出构造 `ExitDecisionSummary`，不得自行猜测新的交易规则。

可用输入包括：

1. `asset.exit_decision`
2. `asset.position_intent`
3. `asset.display_context.has_position`
4. `asset.display_context.is_core_holding`
5. `participation_ready`
6. 资产状态与 streak 信息

### 5.2 首版规则

首版最小规则固定为：

1. `DEFEND -> EXIT`
2. `asset_out_of_top_tier_streak >= 3 -> TRIM`
3. `participation_ready` 从 `true -> false`：
   - `is_core_holding == true -> HOLD`
   - 否则 `TRIM`
4. `OVERHEAT -> TRIM`
5. 其他已有持仓但未触发退出规则 -> `WATCH` 或 `HOLD`

### 5.3 和 NO_TRADE 的关系

必须明确：

1. `NO TRADE` 场景下仍然允许 `HOLD`
2. `NO TRADE` 场景下仍然可能出现 `TRIM`
3. `NO TRADE` 场景下不能把所有资产一律输出成 `EXIT`

### 5.4 推荐输出样式

中文：

```text
### 📉 持仓处理建议

- NVDA · 持有
  结构未破坏，继续持有

- TSLA · 观察
  回撤中，尚未触发减仓

- FIG · 减仓
  已掉出核心区超过 3 天
```

日文：

```text
### 📉 ポジション処理提案

- NVDA · 継続保有
  構造未破壊、継続保有

- TSLA · 監視継続
  押し目中、まだ減資条件未達

- FIG · 減資
  核心圏離脱が 3 日継続
```

---

## 6. i18n 词典要求

文件：
[i18n.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/i18n.rs)

至少新增以下字段：

```rust
exit_summary_title
exit_intent_hold
exit_intent_trim
exit_intent_exit
exit_intent_watch
exit_reason_defend
exit_reason_strength_loss
exit_reason_participation_fallback
exit_reason_overheat
exit_reason_hold_core
exit_reason_watch_pullback
```

要求：

1. 中英日三语必须完整
2. 不允许在 `report.rs` 中硬编码退出原因
3. 原因必须是产品语言，而不是底层调试语句

---

## 7. report.rs 渲染要求

文件：
[report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

新增区块：

```text
### 📉 持仓处理建议
```

放置顺序固定为：

1. 市场摘要
2. 决策结论
3. 持仓处理建议
4. 候选观察名单 / 主要动作
5. 监控信号
6. 战术分区
7. 风险与机会

要求：

1. `report.rs` 只渲染 `exit_summary`
2. 不得在 `report.rs` 中重新判断 `HOLD / TRIM / EXIT / WATCH`
3. `NO TRADE` 场景下允许同时出现：
   - `禁止动作（NO TRADE）`
   - `持仓处理建议`

---

## 8. 验收标准

### 8.1 功能验收

第一屏必须能同时表达：

1. 是否允许买入
2. 已有持仓是否需要卖出 / 减仓 / 持有

也就是说：

1. `NO TRADE` 不得被误解成“全部卖出”
2. `ExitDecisionSummary` 不得缺席
3. 必须区分：
   - `HOLD`
   - `TRIM`
   - `EXIT`
   - `WATCH`

### 8.2 结构验收

必须满足：

1. `PresentationAssembler` 是唯一退出建议组装层
2. `report.rs` 只渲染，不新增退出逻辑
3. `DecisionPacket` 仍保持纯领域事实，不新增展示字段回写

### 8.3 语义验收

必须满足：

1. `NO TRADE` 仍然表示禁止主动开新仓
2. `HOLD` / `TRIM` / `EXIT` 表示已有持仓处理
3. 两者不得混淆

### 8.4 质量门

必须通过：

1. `cargo fmt`
2. `cargo test --quiet`
3. `cargo clippy --all-targets --all-features -- -D warnings`

---

## 9. 测试要求

### 9.1 Presentation Tests

至少补齐：

1. `DEFEND -> EXIT`
2. `out_of_top_tier_streak >= 3 -> TRIM`
3. `NO TRADE + core holding -> HOLD`
4. `NO TRADE + weak holding -> TRIM`
5. `OVERHEAT -> TRIM`

### 9.2 Report UI Tests

至少补齐：

1. 第一屏同时出现：
   - `禁止动作（NO TRADE）`
   - `持仓处理建议`
2. `NO TRADE` 场景下不得把所有资产都渲染成 `卖出`
3. `HOLD / TRIM / EXIT / WATCH` 的本地化文案正确

### 9.3 多语言回归测试

至少覆盖：

1. `zh-cn`
2. `ja-jp`
3. `en-us`

并验证：

1. 标题存在
2. 意图标签存在
3. 原因文案不回退成英文调试文本

---

## 10. 非目标

本轮不做以下事情：

1. 不引入 ATR、成本线、浮盈回撤等复杂止盈
2. 不修改 `Market Regime`
3. 不修改 `ParticipationReadiness`
4. 不修改 `ActionMatrix`
5. 不重做 `ExitDecision` 底层规则体系

本轮只做：

> 把已有退出语义提升到和 `NO TRADE` 同级别的展示结论层。

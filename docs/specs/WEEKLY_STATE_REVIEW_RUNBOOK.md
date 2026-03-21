# Weekly State Review Runbook

## 1. 目的

本文件定义 `V1.3` 观察期的标准周复盘流程。

目标不是自动生成策略结论，而是把复盘分成两层：

1. `CI` 自动产出原始数据和周聚合草稿
2. 人工补充异常解释和后续动作判断

一句话：

**CI 负责统计，人工负责判断。**

---

## 2. 周复盘时间

当前 workflow 节奏：

1. `daily_radar.yml`
   - 每周一至周五 `23:30 JST`

2. `weekly_backtest.yml`
   - 每周六 `01:00 JST`

推荐周复盘时间：

1. **每周六 `09:00-10:00 JST`**
2. 如需更保守，建议 **每周六 `10:00 JST` 之后**

原因：

1. 周五最后一次 daily 数据已经落盘
2. 周六 weekly backtest 通常已经完成
3. `data` 分支中的周度产物基本已同步完成

---

## 3. 数据来源

### 3.1 标准来源

标准周复盘默认直接使用 `data` 分支中的产物。

需要查看的文件：

#### 每日观察

1. `reports/run_status_[DATE].json`
2. `reports/decision_packet_[DATE].json`
3. `reports/[DATE].md`

#### 周度汇总

1. `reports/weekly_state_metrics.json`
2. `reports/weekly_state_review_auto.md`
3. `backtest/state_machine_metrics_latest.json`
4. `backtest/state_machine_metrics_latest.md`
5. `backtest/summary_latest.md`

### 3.2 关键原则

`data` 分支是结果分支，不是执行分支。

因此：

1. 在 `data` 分支上**不执行** `cargo run -- review`
2. 在 `data` 分支上只读取结果文件

---

## 4. 两种操作方案

## 4.1 方案 A：标准周复盘

这是默认方案。

适用场景：

1. 你只是做每周复盘
2. CI 已正常运行
3. `data` 分支已有最新产物

是否需要执行命令：

**不需要。**

操作方式：

1. 等待本周 `daily_radar` 与 `weekly_backtest` 运行完成
2. 打开 `data` 分支中的：
   - `reports/weekly_state_metrics.json`
   - `reports/weekly_state_review_auto.md`
   - `backtest/state_machine_metrics_latest.md`
3. 基于这些文件填写人工周复盘

## 4.2 方案 B：代码分支人工重算

这是调试方案，不是标准周复盘方案。

适用场景：

1. `CI` 没有产出 `weekly_state_metrics.json`
2. 需要验证 `review` 聚合逻辑
3. 需要在代码分支本地重算一次周汇总

执行前提：

1. 必须在代码工作区
2. 必须是 `main` / `develop` / 功能分支工作区
3. 工作区内必须存在：
   - `Cargo.toml`
   - `src/`
   - `reports/`

命令：

```bash
cargo run -- review
```

执行结果：

1. 生成 `reports/weekly_state_metrics.json`
2. 生成 `reports/weekly_state_review_auto.md`

注意：

1. 这不是在 `data` 分支执行
2. 这不是周复盘的默认入口
3. 这是代码侧调试或补算手段

---

## 5. 每周标准执行步骤

## 5.1 Step 1：确认 CI 已完成

确认以下 workflow 本周均已成功：

1. `daily_radar.yml`
2. `weekly_backtest.yml`

若任一失败：

1. 先处理 CI 失败
2. 不进入正式周复盘

## 5.2 Step 2：确认 `data` 分支已有本周产物

至少确认这些文件存在：

1. `reports/weekly_state_metrics.json`
2. `reports/weekly_state_review_auto.md`
3. `backtest/state_machine_metrics_latest.md`

如果缺失：

1. 先排查 workflow 是否失败
2. 如需调试，再由开发在代码分支执行 `cargo run -- review`

## 5.3 Step 3：打开自动汇总

按以下顺序看：

1. `reports/weekly_state_metrics.json`
2. `reports/weekly_state_review_auto.md`
3. `backtest/state_machine_metrics_latest.md`

阅读顺序建议：

1. 先看 `weekly_totals`
2. 再看 `daily_summaries`
3. 再看自动草稿中的异常日

## 5.4 Step 4：生成人工周复盘文件

不要直接修改模板原件。

模板原件：

1. [weekly_state_review.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/templates/weekly_state_review.md)

建议每周新建实际复盘文件，例如：

1. `reports/weekly_state_review_2026-03-21.md`

---

## 6. 你需要填写什么

## 6.1 Weekly Totals

从 `reports/weekly_state_metrics.json -> weekly_totals` 填写：

1. `reset_confirmed_total`
2. `reset_blocked_total`
3. `soft_reset_total`
4. `duration_lock_total`
5. `defensive_override_total`
6. `core_breakdown_total`
7. `reconciliation_mismatch_total`

## 6.2 Daily Timeline

从 `daily_summaries` 填每一天：

1. `from_state -> to_state`
2. `reset_confirmed / reset_blocked`
3. `soft_reset_applied`
4. `duration_locked`
5. `defensive_override`
6. `reconciliation_mismatch_count`

## 6.3 Key Observations & Anomalies

这是人工填写的重点。

至少回答：

1. 本周有没有异常 reset
2. 本周有没有大量 blocked reset
3. 有没有状态抖动过多
4. 有没有防御触发过于频繁
5. 有没有对账差异异常累积

## 6.4 Logic Feedback

重点判断：

1. 资产恢复是否过快
2. 资产恢复是否过慢
3. 是否存在过度防御
4. 是否存在“应该 reset 但没 reset”
5. 是否存在“本不该 reset 却 reset”

## 6.5 Proposed Adjustments

这部分不是每周都要填。

只有在连续观察 `2-4` 周后，才决定是否进入 `V1.4` 参数收敛。

---

## 6.6 每周只看这 5 个问题

为了避免周复盘变成无边界发散，人工复盘时只需要固定回答以下 5 个问题。

### 1. 这周有没有再出现“自洽但不真实”的案例

重点看：

1. 市场状态在消息里看起来说得通
2. 但资产排序明显不符合历史直觉
3. 或 Telegram 读起来很顺，但你一眼就觉得“不像真实市场结构”

如果继续出现这类问题，说明系统虽然内部一致，但仍未真正贴近现实语义。

### 2. 持续强者有没有被短期踢出

重点盯：

1. `NVDA / GOOG / SPY` 这类持续强资产
2. 是否因为单日回调就掉出核心区
3. 是否被直接降到 `OBSERVE`
4. `Top Actions` 连续性是否异常变差

如果继续发生，说明：

1. `Top Tier Lock`
2. `State Transition Friction`

还不够强，或接入位置不够有效。

### 3. 历史弱者有没有因短期整洁快速上位

重点盯：

1. 像 `PLTR` 这类曾经长期弱的标的
2. 是否 1-2 天就冲进 `OPTIMAL`
3. 是否直接进入 `Top Actions`
4. 是否“看起来更干净”就压过长期强票

如果还会发生，说明：

1. `Promotion Cap`
2. `Relative Strength Memory`

还不够。

### 4. Top Actions 是不是更稳定了，但没有僵死

要同时看两件事：

1. 是否明显更稳定
   - 不再天天换
   - 核心票更连续
2. 是否稳定过头
   - 长时间完全不动
   - 明显该换时也不换
   - 新机会长期进不来

如果出现第 2 种，说明系统开始偏保守或偏迟钝。

### 5. 当前问题更像“过敏”，还是“迟钝”

每周最后只做这个判断：

#### 过敏

1. reset 太多
2. blocked reset 太多
3. 强票容易掉下去
4. 文案反复回到“试探”

#### 迟钝

1. 应该换的没换
2. 明显弱化了还保留高评级
3. `Top Actions` 过于僵硬

---

## 6.7 每周最终只写一句结论

建议格式：

1. `本周未见新的“自洽但不真实”案例，系统稳定，继续观察。`
2. `本周强者保护正常，但弱者上位仍偏快，继续观察。`
3. `本周 Top Actions 稳定性明显提升，但已有轻微僵化迹象，需再观察一周。`

---

## 7. 自动化边界

当前自动化边界如下：

### 7.1 已自动化

1. 扫描最近 7 日 `run_status`
2. 聚合 `weekly_totals`
3. 生成 `weekly_state_metrics.json`
4. 生成 `weekly_state_review_auto.md`
5. 将产物同步到 `data` 分支

### 7.2 仍需人工

1. 异常日业务解释
2. 系统是否过敏/迟钝判断
3. 是否进入 `V1.4`
4. 是否需要参数收敛

### 7.3 明确不自动化

1. 不自动生成参数调整建议
2. 不自动判断“系统过敏”
3. 不自动写最终业务结论

---

## 8. 标准周复盘口径

周复盘的默认操作口径如下：

1. **默认使用方案 A**
2. **默认不手动执行 `cargo run -- review`**
3. **默认直接读取 `data` 分支结果**

只有在以下情况才使用方案 B：

1. CI 没产出 `weekly_state_metrics.json`
2. 需要验证 `review` 聚合逻辑
3. 需要在代码分支本地补算

---

## 9. 给开发的执行要求

开发如果继续参与 V1.3 观察期，只需要保证：

1. `daily_radar.yml` 与 `weekly_backtest.yml` 正常把数据推到 `data`
2. `cargo run -- review` 在代码分支工作区内可用
3. 自动草稿与 JSON 聚合结果保持一致

不要做：

1. 不要继续改状态机逻辑
2. 不要自动生成策略结论
3. 不要把人工判断混进 CI

---

## 10. 一句话结论

标准周复盘流程是：

1. `CI` 自动产出数据和草稿
2. 你在每周六上午查看 `data` 分支结果
3. 你人工填写异常解释和后续动作判断

默认情况下，**你不需要手动执行 `cargo run -- review`**。

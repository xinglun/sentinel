# 多端展示组件语义标准 (DISPLAY_COMPONENT_STANDARD.md)

本规范定义了 Sentinel 决策包如何转化为高层 UI 组件语义，确保跨端（Telegram, CLI, Web/App）表达的一致性。

## 1. Top Action 组件语义

Top Action 是系统最重要的决策输出，反映了对核心资产的立即建议。

### ViewModel 结构 (精简版)
- **`title`**: 标的代码 (e.g., "NVDA")。
- **`primary_label`**: 主动作文案 (e.g., "加仓", "持有", "清仓")。
- **`indicator`**: 状态图标 (e.g., "🟢", "◎", "🔴")。
- **`secondary_area`**: 次行细节（显示变更标签、最高优先级 Context Tag 及诊断理由）。

### 表达映射原则
- **信息分层 (Hierarchy)**:
    - **主行 (Minimal)**: 必须仅包含 `{Symbol} {Label} {Icon} {State}`，确保首屏重心在于“决策结论”。
    - **次行 (Decluttered)**: 采用 `({变更·标签}) | {诊断理由}` 格式。
    - **细节行 (Optional)**: 采用 `└ {Reason}` 格式展示详细解释。
- **Icon 映射**:
    - ADD -> 🟢
    - HOLD -> ◎
    - OBSERVE -> △
    - TRIM -> 🟠
    - EXIT -> 🔴
- **Tag 优先级**: `Blocked` > `Core` > `Candidate` (仅保留一个最高级标签)。

## 2. Tactical Summary (战术分区) 组件语义

用于将所有监控资产按“意图分桶”。

### ViewModel 结构
- **`bucket_id`**: 枚举 (ACCUMULATE, HOLDINGS, WATCHLIST, ACTIONS)。
- **`display_name`**: 产品化名称 (e.g., "加仓区", "持有区", "观察区", "收缩区")。
- **`items`**: 包含标的代码的列表。

### 准入与排序
- **加仓区**: `DisplayIntent::ADD`。
- **持有区**: `DisplayIntent::HOLD`。
- **观察区**: 所有的 `DisplayIntent::OBSERVE` 行为。
- **收缩区**: 所有的减仓/退出（TRIM, EXIT）行为。

## 3. Risk & Opportunity (风险与机会) 组件语义

提取系统通过诊断发现的极端情况。采用统一的“标的 + 触发词”口径。

## 4. Monitoring Signals (监控信号) 组件语义

报告的监控版块必须完全中文化，避免暴露底层工程术语。

- **Confidence** -> **信心指数** (高/中/低)
- **Stability** -> **稳定性** (稳定/纠结/脆弱)
- **Participation** -> **参与状态** (已就绪/未就绪)
- **Streak** -> **连续性** (e.g., "连续 3天")
- **Regime Age** -> **周期长度**
- **Flow** -> **资金流向**

## 5. 变更维护原则 (Maintenance Principles)

为了防止“实现变了、契约没变”的回归，任何涉及展示语义的修改必须遵循 **“三位一体同步 (Trinity Sync)”**：

> [!IMPORTANT]
> 1. **代码同步**: 更新 `DisplayAdapter` (`display.rs`) 的 ViewModel 转换逻辑。
> 2. **断言同步**: 修正 `report_ui_tests.rs` 中的所有相关字符串断言。
> 3. **规范同步**: 在本核心规范 (`DISPLAY_COMPONENT_STANDARD.md`) 中同步更新格式定义。

---
**禁止修改渲染符号而保持旧有测试断言，这会导致标准化语义发生事实性漂移。**

# Sentinel Decision Engine: Implementation Walkthrough

本文档详细介绍了 Sentinel 决策引擎 2.0 的核心逻辑、架构及其加固后的最终状态。

## 1. 核心管线 (The Pipeline)
决策管线已在 `src/core/engine.rs` 中抽象为 `run_daily_pipeline`。无论是实时生成的 `radar` 模式，还是作为 `daemon` 运行，亦或是历史回测 `backtest`，均共用同一套决策逻辑，确保了研究与实盘的高度一致。

## 2. 市场惯性层与持续时间控制 (Inertia Layer & Calibration)
系统通过 `PersistenceLayer` 加载每日状态数据。为了防止“频繁振荡”，引入了惯性层：
- **Duration Lock**：升级（Upgrade）和重置（Reset to Ignition）均受持续时间锁限制（一般需在当前状态停留 >= 3 天），除非触发了 `DEFENSIVE` 强控隐患网关。
- **TrendDominant 复合判定** (V1.1)：不再依赖单一阈值，而是通过 `dominance_margin > 0`、`up_weight >= down_weight` 以及信心值达标三者协同判定趋势主导权。
- **CoreAssetsBreakdown 复合判定** (V1.1)：支持配置化阈值（`breakdown_k`, `avg_deviation`, `breadth_floor`），从简单的启发式升级为多维风险判定。
- **Soft Reset**：
    - 若发生“重置”至 `IGNITION`（由于信心大幅下降或核心资产判定为 Breakdown），`regime_age` 硬重置为 1。
    - 若发生普通级别降级（例如 `ESTABLISHED -> EARLY_CONFIRMATION`），则触发 **Soft Reset**，`regime_age` 仅下调 30%（即保留 70% 记忆），实现平滑回归。
- **产出**：DecisionPacket 内部存储的 `regime_age` 和 `duration_in_state` 为判定后的最新值，并同步更新结构化的 `transition_audit` 审计日志。

## 3. 动作引擎语义 (Action Engine Semantics)
- **Band-based Classification**：`AssetStateMachine` 不再硬编码阈值，而是动态解析配置文件中的 `deviation_bands`。
- **PULLBACK 路径保护**：修正了 z-score 过滤逻辑。深度回调（低 z-score）现在被准确识别为 `PULLBACK`（买入机会），而非由于过度防御逻辑导致的误杀。
- **Action Matrix**：扩展了“市场状态 × 个股状态”的映射矩阵，涵盖了从 Ignition（点火期）到 Defensive（防御期）的完整生命周期映射。

## 4. 持久化与分层存档 (Persistence & 10-Asset Standard)

每次成功运行决策管线，系统均会产生 10 类核心资产，确保了 6 个月后的深度审计能力。这 10 类资产无论是否执行交易均会产出：
1. `decision_history.jsonl`：流式记录所有历史决策主轴。
2. `state_transitions.jsonl` / `.csv`：结构化与表格版的状态变迁日志。
3. `ledger.csv`：成交审计，记录所有已执行的交易。
4. `execution_gate_log.jsonl`：风控网关审计，记录信号被拦截或通过的详细原因。
5. `decision_packet_[DATE].json`：当日引擎决策的完整特征快照。
6. `portfolio_snapshot_[DATE].json`：当日组合持仓与浮盈快照。
7. `account_snapshot_[DATE].json`：当日账户资金与购买力快照。
8. `run_status_[DATE].json` 中的 `reconciliation` 字段：持仓对账嵌入式报告。
9. `data_quality_log.jsonl`：数据源质量监控（fetch 状态与 Bar 计数）。
10. `reports/[DATE].md`：人读版日报，包含市场头条与决策摘要。


## 5. 风控網門与交易集成 (Execution Gate & Kill Switch)
- **ExecutionMode Enum**: システムは `Disabled`, `DryRun`, `Live` の3つの明示的な実行モードを持ちます。`daemon` 起動時でも `trading.enabled` が `false` なら自動的に `DryRun` へフォールバックします。
- **TradingDisabled 理由コード**: トレードがブロックされた際、監査ログに `TradingDisabled` という明確な理由が記録されます。
- **Strict Config Enforcement**: `AppConfig` および関連構造体に `#[serde(deny_unknown_fields)]` を適用。未定義のフィールド（例：レガシーな `bear_mode`）が `config.toml` に残っている場合はパース時にエラーとなり、静黙な設定不整合を防ぎます。

## 6. データ完全性と研究コントラクト (Data Integrity)
- **Research Telemetry (20-column)**: `telemetry.csv` は 20 列の固定スキーマを持つ研究用コントラクトへと昇格しました。これには `config_hash` が含まれ、どの設定パラメーターでその物理量が算出されたかを完全に追跡可能です。
- **Structured Run Outcomes**: 毎日の実行結果は `run_status_[DATE].json` に保存されます。`decisioning`, `archival`, `notification`, `execution`, `reconciliation` 各ステージの成否が記録され、データ不整合や API 障害を即座に検知可能です。

## 7. 検証証明と回測一致性 (Verification)
- **Zero-Warning Base**: 全量 `cargo fmt`, `cargo check` 以及 `cargo test` 均保持全绿通过。
- **Kill-Switch Integration Test**: `tests/product_grade_kill_switch.rs` により、グローバルスイッチが物理的にトレードを遮断することを保証。
- **Archive Integrity**: 実行成否に関わらず、定められたアーカイブ資産が常に一貫した形式で出力されることを確認。

## 8. 状态机 V1.2 验证与可观测性 (Validation & Observability)
V1.2 不再修改判定逻辑，重点加强了对 V1.1 优化效果的“可见性”：
- **回测质量指标**：`backtest.rs` 现自动统计并产出 `state_machine_metrics.json/md`。覆盖了 Reset 成功率、Duration Lock 频率、Soft Reset 覆盖率以及状态抖动指数（State Flips），为调优提供了量化依据。
- **审计数据透出**：在归档 Markdown 报告中增加了 `State Transition Audit` 摘要块，详细展示了每次决策的判定路径（从 A 到 B 的路径、锁状态、核心资产状态、Soft Reset 应用情况等）。
- **实时调试透明**：终端输出现包含针对状态迁移的摘要信息，使得在开发/预览模式下能即时理解状态机为何“拒绝 Reset”或“触发加固”。

## 9. 状态机 V1.3 实盘观察期基础设施 (Observation Infrastructure)
V1.3 标志着系统进入为期 2-4 周的实盘观察期，重点在于指标的自动化收集与标准化复盘：
- **运行指标聚合**：`run_status_[DATE].json` 现已整合 `StateMachineSummary` 结构。每日运行会自动汇总状态迁移、Reset 状态、持续时间锁、对账差异等指标。
- **CI 链路原子化同步** (Hardened)：修正了 GitHub Actions 在同步 `data` 分支时的时序缺陷。现统一采取“先 Fetch -> 先 Rebase -> 后覆盖本地产物”的原子化路径，彻底解决了因并发写入导致的 `rebase` 冲突。
- **复盘辅助自动化**：通过 `cargo run -- review` 命令，系统可自动扫描过去 7 日数据产出 `weekly_state_metrics.json` 以及 **`weekly_state_review_auto.md` (复盘底稿草案)**。草案中自动汇总了全周量化指标、每日对照表以及自动识别的异常日，为人工填写最终复盘报告提供底噪过滤后的事实依据。
- **标准化复盘流程**：建立了 [WEEKLY_STATE_REVIEW_RUNBOOK.md](../specs/WEEKLY_STATE_REVIEW_RUNBOOK.md) 作为 V1.3 观察期的标准复盘手册，明确了 CI 统计与人工判断的边界。
- **标准化复盘模板**：在 `docs/templates/weekly_state_review.md` 中建立了每周巡检的标准格式，要求开发/运维人员定期对比各指标的稳定性。
- **异常定位辅助**：通过将“决策层指标”与“执行层对账”强关联，系统现在能一眼定位出“由于状态机过度敏感导致的频繁调仓”或“由于个股波动导致的恢复受阻”。

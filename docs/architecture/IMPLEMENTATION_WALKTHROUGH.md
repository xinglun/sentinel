# Sentinel Decision Engine: Implementation Walkthrough

本文档详细介绍了 Sentinel 决策引擎 2.0 的核心逻辑、架构及其加固后的最终状态。

## 1. 核心管线 (The Pipeline)
决策管线已在 `src/core/engine.rs` 中抽象为 `run_daily_pipeline`。无论是实时生成的 `radar` 模式，还是作为 `daemon` 运行，亦或是历史回测 `backtest`，均共用同一套决策逻辑，确保了研究与实盘的高度一致。

## 2. 状态持续时间控制 (regime_age Calibration)
系统通过 `PersistenceLayer` 加载上一日的状态数据。`regime_age` 逻辑已加固：
- 输入：上一日状态与持续天数。
- 逻辑：判定当日新状态。若状态一致，age 递增；若发生迁移，则 age 重置为 1。
- 产出：DecisionPacket 内部存储的 age 为判定后的最新值，并同步更新相关的 `trend_maturity` 指标。

## 3. 动作引擎语义 (Action Engine Semantics)
- **Band-based Classification**：`AssetStateMachine` 不再硬编码阈值，而是动态解析配置文件中的 `deviation_bands`。
- **PULLBACK 路径保护**：修正了 z-score 过滤逻辑。深度回调（低 z-score）现在被准确识别为 `PULLBACK`（买入机会），而非由于过度防御逻辑导致的误杀。
- **Action Matrix**：扩展了“市场状态 × 个股状态”的映射矩阵，涵盖了从 Ignition（点火期）到 Defensive（防御期）的完整生命周期映射。

## 4. 持久化与分层存档 (Persistence & 9-Asset Standard)

每次成功运行决策管线，系统均会产生 9 类核心资产，确保了 6 个月后的深度审计能力。这 9 类资产无论是否执行交易均会产出：
1. `decision_history.jsonl`：流式记录所有历史决策主轴。
2. `state_transitions.jsonl` / `.csv`：结构化与表格版的状态变迁日志。
3. `ledger.csv`：成交审计，记录所有已执行的交易。
4. `execution_gate_log.jsonl`：风控网关审计，记录信号被拦截或通过的详细原因。
5. `decision_packet_[DATE].json`：当日引擎决策的完整特征快照。
6. `portfolio_snapshot_[DATE].json`：当日组合持仓与浮盈快照。
7. `account_snapshot_[DATE].json`：当日账户资金与购买力快照。
8. `data_quality_log.jsonl`：数据源质量监控（fetch 状态与 Bar 计数）。
9. `reports/[DATE].md`：人读版日报，包含市场头条与决策摘要。


## 5. 风控網門与交易集成 (Execution Gate & Kill Switch)
- **ExecutionMode Enum**: システムは `Disabled`, `DryRun`, `Live` の3つの明示的な実行モードを持ちます。`daemon` 起動時でも `trading.enabled` が `false` なら自動的に `DryRun` へフォールバックします。
- **TradingDisabled 理由コード**: トレードがブロックされた際、監査ログに `TradingDisabled` という明確な理由が記録されます。
- **Strict Config Enforcement**: `AppConfig` および関連構造体に `#[serde(deny_unknown_fields)]` を適用。未定義のフィールド（例：レガシーな `bear_mode`）が `config.toml` に残っている場合はパース時にエラーとなり、静黙な設定不整合を防ぎます。

## 6. データ完全性と研究コントラクト (Data Integrity)
- **Research Telemetry (20-column)**: `telemetry.csv` は 20 列の固定スキーマを持つ研究用コントラクトへと昇格しました。これには `config_hash` が含まれ、どの設定パラメーターでその物理量が算出されたかを完全に追跡可能です。
- **Structured Run Outcomes**: 毎日の実行結果は `run_status_[DATE].json` に保存されます。`decisioning`, `archival`, `notification`, `execution` 各ステージの成否が記録され、バッチジョブの監視が容易になります。

## 7. 検証証明と回測一致性 (Verification)
- **Zero-Warning Base**: 全量 `cargo check` および `cargo test` をクリア。
- **Kill-Switch Integration Test**: `tests/product_grade_kill_switch.rs` により、グローバルスイッチが物理的にトレードを遮断することを保証。
- **Archive Integrity**: 実行成否に関わらず、定められたアーカイブ資産が常に一貫した形式で出力されることを確認。

---
author: Ray
title: Sentinel Decision Engine: 実装ガイド (Implementation Walkthrough)
description: Sentinel Decision Engine: 実装ガイド (Implementation Walkthrough) に関する Sentinel の設計・運用情報。
key: docs-architecture-implementation-walkthrough
---

# Sentinel Decision Engine: 実装ガイド (Implementation Walkthrough)

本ドキュメントでは、Sentinel 意思決定エンジン 2.0 のコアロジック、アーキテクチャ、および堅牢化後の最終状態について詳しく説明します。

## 1. コアパイプライン (The Pipeline)

意思決定パイプラインは `src/core/engine.rs` において `run_daily_pipeline` として抽象化されています。リアルタイム生成の `radar` モード、`daemon` としての実行、あるいは過去データによる `backtest` のいずれであっても、共通の意思決定ロジックを使用することで、研究と実盤の高度な一致を保証しています。

## 2. 市場慣性層と持続時間制御 (Inertia Layer & Calibration)

システムは `PersistenceLayer` を通じて日々の状態データを読み込みます。「頻繁な変動（チャタリング）」を防止するため、以下の慣性層を導入しています：

- **Duration Lock**: 昇格（Upgrade）および初期化（Reset to Ignition）は持続時間ロックによって制限されます（通常、現在の状態で >= 3 日間滞在する必要があります）。ただし、`DEFENSIVE` の強制防御ゲートがトリガーされた場合は除きます。
- **TrendDominant 複合判定 (V1.1)**: 単一の閾値に依存せず、`dominance_margin > 0`、`up_weight >= down_weight`、および確信値（Confidence）の達成という3つの要素を組み合わせてトレンドの主導権を判定します。
- **CoreAssetsBreakdown 複合判定 (V1.1)**: 設定可能な閾値（`breakdown_k`, `avg_deviation`, `breadth_floor`）をサポートし、単純なヒューリスティックから多次元的なリスク判定へとアップグレードされました。
- **Soft Reset**:
    - 確信度の大幅な低下やコア資産の Breakdown 判定により `IGNITION` へリセットされる場合、`regime_age` は 1 にハードリセットされます。
    - 通常レベルの降格（例：`ESTABLISHED -> EARLY_CONFIRMATION`）が発生した場合は **Soft Reset** がトリガーされ、`regime_age` は 30% 下方修正されます（つまり、70% の記憶を保持）。これにより、スムーズな回帰を実現します。
- **アウトプット**: DecisionPacket 内部に保存される `regime_age` と `duration_in_state` は判定後の最新値となり、構造化された `transition_audit` 監査ログにも同期して記録されます。

## 3. アクションエンジンのセマンティクス (Action Engine Semantics)

- **Band-based Classification**: `AssetStateMachine` は閾値をハードコードせず、設定ファイル `config.toml` 内の `deviation_bands` を動的に解析します。
- **PULLBACK パス保護**: z-score フィルタリングロジックを修正しました。深い押し目（低 z-score）は、過度な防御ロジックによる誤判定を避け、正確に `PULLBACK`（買いチャンス）として識別されます。
- **Action Matrix**: 「市場状態 × 個別資産状態」のマッピングマトリックスを拡張し、点火期（Ignition）から防御期（Defensive）までの完全なライフサイクルマッピングをカバーしています。

## 4. 永続化と階層化アーカイブ (Persistence & 10-Asset Standard)

意思決定パイプラインが実行されるたびに、システムは 10 種類のコア資産（成果物）を生成します。これにより、6ヶ月後の深い監査が可能になります。これらの資産は、取引の実行有無に関わらず生成されます：

1. `decision_history.jsonl`: すべての意思決定のメイン軸をストリーミング形式で記録。
2. `state_transitions.jsonl` / `.csv`: 構造化およびテーブル形式の状態遷移ログ。
3. `ledger.csv`: 約定監査ログ。実行されたすべての取引を記録。
4. `execution_gate_log.jsonl`: 執行ゲート監査ログ。シグナルが遮断または通過した詳細な理由を記録。
5. `decision_packet_[DATE].json`: 当日のエンジン意思決定の完全な特徴スナップショット。
6. `portfolio_snapshot_[DATE].json`: 当日のポートフォリオ持倉と評価損益のスナップショット。
7. `account_snapshot_[DATE].json`: 当日の勘定資金と購買力のスナップショット。
8. `run_status_[DATE].json` の `reconciliation` フィールド: 持倉照合の埋め込みレポート。
9. `data_quality_log.jsonl`: データソース品質モニタリング（取得ステータスと Bar 数）。
10. `reports/[DATE].md`: 人間が読める形式の日報。市場のヘッドラインと意思決定の要約を含む。

## 5. 風控網門とトレード統合 (Execution Gate & Kill Switch)

- **ExecutionMode Enum**: システムは `Disabled`, `DryRun`, `Live` の3つの明示的な実行モードを持ちます。`daemon` 起動時でも `trading.enabled` が `false` なら自動的に `DryRun` へフォールバックします。
- **TradingDisabled 理由コード**: トレードがブロックされた際、監査ログに `TradingDisabled` という明確な理由が記録されます。
- **Strict Config Enforcement**: `AppConfig` および関連構造体に `#[serde(deny_unknown_fields)]` を適用。未定義のフィールド（例：レガシーな `bear_mode`）が `config.toml` に残っている場合はパース時にエラーとなり、静黙な設定不整合を防ぎます。

## 6. データ完全性と研究コントラクト (Data Integrity)

- **Research Telemetry (20-column)**: `telemetry.csv` は 20 列の固定スキーマを持つ研究用コントラクトへと昇格しました。これには `config_hash` が含まれ、どの設定パラメーターでその物理量が算出されたかを完全に追跡可能です。
- **Structured Run Outcomes**: 毎日の実行結果は `run_status_[DATE].json` に保存されます。`decisioning`, `archival`, `notification`, `execution`, `reconciliation` 各ステージの成否が記録され、データ不整合や API 障害を即座に検知可能です。

## 7. 検証証明とバックテスト一致性 (Verification)

- **Zero-Warning Base**: `cargo fmt`, `cargo check` および `cargo test` のすべてにおいて、警告なし・テスト合格を維持しています。
- **Kill-Switch Integration Test**: `tests/product_grade_kill_switch.rs` により、グローバルスイッチが物理的にトレードを遮断することを保証しています。
- **Archive Integrity**: 実行成否に関わらず、定められたアーカイブ資産が常に一貫した形式で出力されることを確認しています。

## 8. 状態機 V1.2 検証と可観測性 (Validation & Observability)

V1.2 では判定ロジックの変更は行わず、V1.1 までの最適化効果の「可視化」に重点を置いています：

- **バックテスト品質指標**: `backtest.rs` は `state_machine_metrics.json/md` を自動統計・出力します。リセット成功率、Duration Lock 頻度、Soft Reset カバレッジ、および状態フリップ（State Flips）指数をカバーし、チューニングのための定量的根拠を提供します。
- **監査データの出力**: アーカイブ用の Markdown レポートに `State Transition Audit` 概要ブロックを追加しました。各意思決定の判定パス（AからBへのパス、ロック状態、コア資産状態、Soft Reset の適用状況など）を詳細に表示します。
- **リアルタイムデバッグの透明化**: ターミナル出力に状態遷移に関する要約情報が含まれるようになり、開発/プレビューモードで状態機がなぜ「リセットを拒否したか」や「加固をトリガーしたか」を即座に理解できるようになりました。

## 9. 状態機 V1.3 実盤観察期インフラ (Observation Infrastructure)

V1.3 は、システムが 2〜4 週間の実盤観察期に入ることを示しており、指標の自動収集と標準的な復習（レビュー）に重点を置いています：

- **実行指標の集約**: `run_status_[DATE].json` に `StateMachineSummary` 構造を統合しました。毎日の実行で、状態遷移、リセット状態、持続時間ロック、照合の差異などの指標が自動的に集計されます。
- **通知メッセージの最適化 (Refined)**: Telegram レポートが全面的に最適化されました。`Age` タグはより説明的な `Regime Age` に更新されました。また、脆弱な状態（Fragile + IGNITION）に対しては、アクション指引を維持しつつ、トーンを調整した（例：「軽微な追跡に適しています」）文言ロジックを導入し、積極性とリスク提示のバランスを取っています。
- **CI リンクの原子化同期 (Hardened)**: `data` ブランチ同期時における GitHub Actions のタイミングの欠陥を修正しました。一貫して「Fetch -> Rebase -> ローカル成果物の書き込み」の原子化パスを採用し、並列書き込みによる `rebase` 衝突を完全に解決しました。
- **復習補助の自動化**: `cargo run -- review` コマンドにより、過去 7 日間のデータを自動スキャンし、`weekly_state_metrics.json` および **`weekly_state_review_auto.md` (復習下書き案)** を生成します。下書きには全週の定量指標、日次の対照表、および自動識別された異常日が集約され、最終的な手動レビューのための事実根拠を提供します。
- **標準化された復習フロー**: `WEEKLY_STATE_REVIEW_RUNBOOK.md` を V1.3 観察期の標準マニュアルとして確立し、CI 統計と手動判断の境界を明確にしました。
- **標準化された復習テンプレート**: `docs/templates/weekly_state_review.md` に毎週の点検用標準フォーマットを作成し、開発/運用担当者が定期的に指標の安定性を比較することを義務付けています。
- **異常の切り分け支援**: 「意思決定層의 指標」と「執行層의 照合」を強く関連付けることで、「状態機의 過敏による頻繁な調倉」なのか「個別資産의 変動による回復遅延」なのかを一目で判別可能にしました。

## 10. 資産層の連続性と強度記憶 (Asset Layer Continuity)

このモジュールでは「資産層の相対強度記憶」を導入し、資産次元における判定ロジックの「短視（近視眼的判断）」問題を解決しました：

- **Top Tier Lock**: 過去 10 日間のうち 6 日間で強度トップ 3 に入った銘柄に対して「強者ロック」をトリガーし、状態が `CRUISE` を下回らないようにすることで、ノイズによる主力資産の頻繁な除外を防止します。
- **Promotion Cap**: 過去 20 日間に `DEFEND` 記録がある、または長期的に弱勢の資産に対しては、最高状態を `CRUISE` に制限し、質の低い資産が単日の急騰によって `OPTIMAL` 評価を得るのを防ぎます。
- **状態遷移の摩擦 (State Transition Friction)**:
    - **降格摩擦**: `OPTIMAL` 状態にある資産は、2日連続で失格条件を満たした場合のみ降格が許可されます。
    - **昇格摩擦**: `OPTIMAL` への昇格は、3日連続で条件を満たした場合のみロックが解除されます。
- **70/30 メモリ調整ランキング**: 資産の最終的なランキングは現在の離散度だけでなく、20日間の歴史的パフォーマンススコア（重み 30%）を組み合わせて平滑化処理を行い、トップアクションの安定性を大幅に向上させました。

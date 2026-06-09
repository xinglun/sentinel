---
author: Ray
title: データブランチのレイアウト標準
description: data ブランチに保存する日次成果物と週次校正成果物の配置、命名規則、検収基準を定義する。
key: data-branch-layout
---

# データブランチのレイアウト標準 (DATA_BRANCH_LAYOUT.md)

本ドキュメントは、`data` ブランチの目標とするディレクトリ構造、命名規則、および検収基準を定義します。`data` ブランチは長期検証用の記録分岐であり、日次 Radar の事実 artifact と週次校正 artifact を分けて保持します。

## 1. 目標と戦略的価値 (Goal & Strategic Value)

`data` ブランチは単なる観測成果物のストレージではなく、システムの**コア研究資産**です。長期的に蓄積されたデータは、主に以下のシナリオで使用されます：

1. **長期観測の蓄積**: 資本構造の連続的な時系列を記録し、改ざん不可能な歴史的アーカイブを形成します。
2. **モデルの校正とパラメータ評価**: 後続のパラメータスキャン (Parameter Sweep)、状態マシンの安定性分析、および Regime 遷移統計のための基礎データを提供します。
3. **リプレイ分析と再学習**: 過去の意思決定の振り返りリプレイをサポートし、将来のモデル再学習のための構造化されたデータセットを提供します。

**今回のアップデートのコアロジック**:
目標そのもの（長期蓄積、将来の研究）は一切変わっていません。変わったのは**アーカイブ標準とエンジニアリングの厳密さ**です。
- **旧版**: 単純な「記録」（`.json`, `.md`）に重点を置いていました。
- **新版**: 「研究可能なエンジニアリングデータセット」（`decision_packet`, `run_status`, `portfolio_snapshot`, `execution_log` など）に重点を置いています。

一言で言えば：**「同じ研究目標を、よりエンジニアリング的で透明性の高い方法で実現する」**ということです。

## 2. 適用範囲 (Scope)

本標準は以下に適用されます：

1. `daily_radar.yml` 毎日のアーカイブ成果物
2. `weekly_backtest.yml` 毎週のバックテスト成果物
3. `reports/` および `backtest/` の長期保持構造
4. 認知校正、Gray Rhino、Macro Gravity などの読み取り専用 context の週次保存方針

## 3. コアルール (Core Rule)

`reports/` 内のすべての「単一日の資産」は、同じ日付キーを使用しなければなりません：

1. `packet.date`
2. つまり、市場データが属する日付
3. ワークフローが実行された当日ではない

これは、以下のファイルが同じ `YYYY-MM-DD` を共有しなければならないことを意味します：

1. `YYYY-MM-DD.md`
2. `decision_packet_YYYY-MM-DD.json`
3. `portfolio_snapshot_YYYY-MM-DD.json`
4. `account_snapshot_YYYY-MM-DD.json`
5. `run_status_YYYY-MM-DD.json`

## 4. 目標レイアウト (Target Layout)

```text
data branch
├── backtest/
│   ├── summary_latest.md
│   └── archive/
│       └── summary_YYYY-MM-DD.md
├── reports/
│   ├── YYYY-MM-DD.md
│   ├── decision_packet_YYYY-MM-DD.json
│   ├── portfolio_snapshot_YYYY-MM-DD.json
│   ├── account_snapshot_YYYY-MM-DD.json
│   ├── run_status_YYYY-MM-DD.json
│   ├── evidence_collection_status_latest.json
│   ├── weekly_state_metrics.json
│   ├── weekly_state_review_auto.md
│   ├── gray_rhino_candidates.jsonl
│   ├── gray_rhino_discovery_runs.jsonl
│   ├── gray_rhino_snapshots.jsonl
│   ├── gray_rhino_refresh_status.jsonl
│   ├── gray_rhino_refresh_status_latest.json
│   ├── gray_rhino_sources/
│   ├── decision_history.jsonl
│   ├── evidence_records.jsonl
│   ├── state_transitions.csv
│   ├── state_transitions.jsonl
│   ├── execution_gate_log.jsonl
│   ├── data_quality_log.jsonl
│   ├── telemetry.csv
│   ├── ledger.csv
│   └── freshness.json
└── README.md
```

## 5. ファイルのセマンティクス (File Semantics)

### 日付付きファイル (Daily dated files)

1. `YYYY-MM-DD.md`
   - 人間が読める形式の日次アーカイブレポート。

2. `decision_packet_YYYY-MM-DD.json`
   - 当日の「単一の真実（SSOT）」となる意思決定パッケージ。

3. `portfolio_snapshot_YYYY-MM-DD.json`
   - 当日のポートフォリオ持分と含み損益のスナップショット。

4. `account_snapshot_YYYY-MM-DD.json`
   - 当日のアカウント資金と購買力のスナップショット。

5. `run_status_YYYY-MM-DD.json`
   - 当日の実行状態（ヘルス）のスナップショット。
   - `decisioning / evidence_collection / archival / notification / execution` を記録。

### 追記型ファイル (Append-only files)

1. `decision_history.jsonl`
   - 意思決定履歴のタイムライン。

2. `evidence_records.jsonl`
   - 実体的証拠の構造化レコード。
   - 手動注入、外部ソース抽出、価格フォロースルーなどを `dedupe_key` 付きで保持する。

3. `state_transitions.csv`
   - 市場状態の遷移（テーブル版）。

4. `state_transitions.jsonl`
   - 市場状態の遷移（構造化版）。

5. `execution_gate_log.jsonl`
   - リスクコントロールゲートの通過/遮断の監査ログ。

6. `data_quality_log.jsonl`
   - データ取得品質のログ。

7. `telemetry.csv`
   - 研究レベルの時系列観測データ。

8. `ledger.csv`
   - 取引帳簿。

### 週次校正ファイル (Weekly calibration files)

1. `weekly_state_metrics.json`
   - 週次レビューの machine-readable metric。
   - `latest_context` として Strategic Context、Macro Gravity、Cognitive Calibration の最新読み取り専用 snapshot を保持する。
   - 長期校正の標準粒度はこの週次ファイルであり、`daily-calibration` の全文を毎日保存することは標準ではありません。

2. `weekly_state_review_auto.md`
   - 週次レビューの人間向け下書き。
   - Markdown label と境界文は `output.language` に従う。
   - `weekly_state_metrics.json` の key は machine-readable contract として英語のまま維持する。
   - `Strategic Context Snapshot`、`Macro Gravity Snapshot`、`Cognitive Calibration Snapshot` を含む。
   - スコア、推奨、売買判断は生成しない。

### Gray Rhino ファイル (Gray Rhino files)

1. `gray_rhino_candidates.jsonl`
   - 自動発見 candidate の追記型記録。

2. `gray_rhino_discovery_runs.jsonl`
   - discovery run の監査 log。

3. `gray_rhino_snapshots.jsonl`
   - Gray Rhino escalation の構造化 snapshot。
   - `daily-calibration` の全文保存ではなく、Gray Rhino 状態を再生するための最小構造 record として扱う。

4. `gray_rhino_refresh_status.jsonl` / `gray_rhino_refresh_status_latest.json` / `gray_rhino_refresh_status_YYYY-MM-DD.json`
   - SEC、Finnhub、FRED refresh の provider-level outcome。

5. `gray_rhino_sources/**`
   - SEC、Finnhub、FRED などの source cache。
   - candidate / evidence の追跡可能性を確保するために保存する。

### 補助ファイル (Auxiliary file)

1. `freshness.json`
   - ワークフローの鮮度（freshness gate）補助ファイル。
   - コア研究資産ではありません。

2. `evidence_collection_status_latest.json`
   - `daily_radar.yml` の前段で実行した証拠収集ジョブの最新状態。
   - `succeeded / failed / skipped` のみを記録し、失敗しても radar 本体の実行は止めません。
   - 取引判断ではなく、`run_status` と `audit_daily` の監査情報としてのみ使用します。

## 6. レガシーファイル (Legacy Files)

以下のファイルは旧命名体系に属しており、今後生成されるべきではありません：

1. `reports/YYYY-MM-DD.json`

取り扱い原則：

1. 過去の遺留ファイルは保持するか、一括クリーンアップできます。
2. 現在のコードおよびワークフローはそれらに依存しません。
3. 新しいアーカイブ標準では、一律に `decision_packet_YYYY-MM-DD.json` を使用します。

## 7. バリデーションルール (Validation Rules)

`daily_radar.yml` は実行のたびに、少なくとも以下のファイルが空でないことを検証しなければなりません：

1. `reports/YYYY-MM-DD.md`
2. `reports/decision_packet_YYYY-MM-DD.json`
3. `reports/decision_history.jsonl`
4. `reports/state_transitions.csv`
5. `reports/state_transitions.jsonl`
6. `reports/execution_gate_log.jsonl`
7. `reports/portfolio_snapshot_YYYY-MM-DD.json`
8. `reports/account_snapshot_YYYY-MM-DD.json`
9. `reports/data_quality_log.jsonl`
10. `reports/run_status_YYYY-MM-DD.json`
11. `reports/evidence_collection_status_latest.json`
12. `reports/telemetry.csv`

`reports/evidence_records.jsonl` は実体的証拠がない日には空であることが正当なため、存在確認のみを行い、非空は強制しません。

`daily-calibration` の全文 Markdown は標準検証対象に含めません。認知校正の長期比較は `weekly_state_metrics.json` と `weekly_state_review_auto.md` の週次粒度で行います。

## 8. 運用の注意点 (Operational Notes)

1. `reports/` 内で `run_status` の日付と `decision_packet` の日付が一致しない場合、アーカイブの命名異常とみなします。
2. `reports/` 内に再び `YYYY-MM-DD.json` が現れた場合、旧成果物の回帰とみなします。
3. `backtest/` は週単位のリズムであり、日次の `reports/` 資産と混ぜないでください。
4. 認知校正の全文を毎日保存しないでください。必要な校正 context は週次成果物に集約します。
5. Gray Rhino の snapshot や refresh status は構造化検証用 record であり、daily-calibration の全文保存とは区別します。

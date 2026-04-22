---
author: Ray
---

# データブランチのレイアウト標準 (DATA_BRANCH_LAYOUT.md)

本ドキュメントは、`data` ブランチの目標とするディレクトリ構造、命名規則、および検収基準を定義します。

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
│   ├── decision_history.jsonl
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
   - `decisioning / archival / notification / execution` を記録。

### 追記型ファイル (Append-only files)

1. `decision_history.jsonl`
   - 意思決定履歴のタイムライン。

2. `state_transitions.csv`
   - 市場状態の遷移（テーブル版）。

3. `state_transitions.jsonl`
   - 市場状態の遷移（構造化版）。

4. `execution_gate_log.jsonl`
   - リスクコントロールゲートの通過/遮断の監査ログ。

5. `data_quality_log.jsonl`
   - データ取得品質のログ。

6. `telemetry.csv`
   - 研究レベルの時系列観測データ。

7. `ledger.csv`
   - 取引帳簿。

### 補助ファイル (Auxiliary file)

1. `freshness.json`
   - ワークフローの鮮度（freshness gate）補助ファイル。
   - コア研究資産ではありません。

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
11. `reports/telemetry.csv`

## 8. 運用の注意点 (Operational Notes)

1. `reports/` 内で `run_status` の日付と `decision_packet` の日付が一致しない場合、アーカイブの命名異常とみなします。
2. `reports/` 内に再び `YYYY-MM-DD.json` が現れた場合、旧成果物の回帰とみなします。
3. `backtest/` は週単位のリズムであり、日次の `reports/` 資産と混ぜないでください。

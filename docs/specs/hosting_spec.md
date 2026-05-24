---
author: Ray
title: Stock Sentinel 自動観測システム
description: Stock Sentinel 自動観測システム に関する Sentinel の設計・運用情報。
key: docs-specs-hosting-spec
---

# Stock Sentinel 自動観測システム
# GitHub Actions ホスティング実行要件仕様書 (hosting_spec.md)

## 1. 目標

Stock Sentinel を GitHub Actions にデプロイし、完全に無人化された自動観測実行環境を実現します。これにより、システムが長期的、安定、かつ連続的に以下のコアタスクを実行できるようにします：

### デイリー自動実行項目：
1. 市場データの取得
2. 資本重力観測エンジンの実行
3. 観測レポート（Markdown）の生成
4. 観測データの記録（telemetry.csv）
5. Telegram 通知の送信
6. 観測データの GitHub リポジトリへの永久保存

**最終目標：** 長期的かつ連続的で、改ざん不可能かつ追跡可能な「資本構造時系列データベース（システムのコア資産）」を構築すること。

- **GitHub Actions 実行ブランチ**: `main`
- **観測データ提出ブランチ**: `data` (Git Worktree を使用して隔離)

## 2. 実行頻度要件

GitHub Actions は以下の実行モードをサポートしなければなりません：

1. **デイリー自動実行**
   - **実行時間**: 22:30 JST (13:30 UTC)
   - **ロジック**: 米国市場閉場後のデータが安定したタイミングで実行し、日中ノイズを回避します。

2. **手動実行の許可**
   - **トリガー**: GitHub UI 上の `Run workflow` ボタン。
   - **用途**: デバッグ、パラメータ調整の検証、臨時観測。
   - **要件**: 手動実行データも自動実行と同様に `telemetry.csv` に記録されること（タイムスタンプで区別）。

## 3. 実行環境要件

- **オペレーティングシステム**: Ubuntu Latest
- **Rust 環境**: Stable toolchain。`cargo build --release` および `cargo run --release` をサポート。

## 4. 実行バイナリ要件

- **実行コマンド**: `cargo run --release`
- **デバッグモードの禁止**: 研究データとの一貫性を保ち、観測結果の再現性を保証するため、必ず release モードを使用します。

## 5. アーカイブ要件

毎回の実行結果は必ず `data` ブランチに保存・コミットされる必要があります：

- **ディレクトリ**: `/reports/`
- **コア資産**:
  - `telemetry.csv`: 多次元時系列データ（継続的に追記）。
  - `run_status_YYYY-MM-DD.json`: マシンリーダブルな実行ヘルス快照（P0 検証項目）。
  - `YYYY-MM-DD.md`: 人間が読める形式のアーカイブ報告書。
  - `decision_packet_YYYY-MM-DD.json`: 単一の真実（SSOT）となる意思決定パッケージ。
  - `portfolio_snapshot_YYYY-MM-DD.json`: ポートフォリオ・エクスポージャーのスナップショット。
  - `account_snapshot_YYYY-MM-DD.json`: アカウント資金のスナップショット。
  - `state_transitions.csv` / `state_transitions.jsonl`: 市場状態遷移ログ。
  - `execution_gate_log.jsonl`: 実行ゲート監査ログ。
  - `data_quality_log.jsonl`: データ品質ログ。

実行後は必ず自動的に `data` ブランチへプッシュします。コードとデータの物理的隔離を維持するため、`main` ブランチへの直接コミットは禁止します。

## 6. Telegram 通知要件

- **シークレット管理**: `TELEGRAM_BOT_TOKEN` および `TELEGRAM_CHAT_ID` は GitHub Secrets から読み取らなければなりません。
- **禁止事項**: 機密性の高いトークンをコードベースに直接書き込むことを厳禁します。

## 7. パラメータ宇宙の隔離要件

`telemetry.csv` は必ず `config_hash` を保持し、将来パラメータが変更された後でも歴史的データが区別できるようにしなければなりません。これは量化研究における重要な基礎です。

## 8. 実行失敗時の処理

- 実行が失敗した場合、GitHub Actions は `Failed` としてマークされ、異常を通知しなければなりません。サイレント・フェイル（静かな失敗）は禁止します。

## 9. リポジトリの標準構造

```text
stock-sentinel/ (main branch)
├── .github/workflows/daily_radar.yml
└── ... (source code)

stock-sentinel/ (data branch)
├── backtest/
│   ├── summary_latest.md
│   └── archive/
│       └── summary_YYYY-MM-DD.md
└── reports/
    ├── YYYY-MM-DD.md
    ├── decision_packet_YYYY-MM-DD.json
    ├── telemetry.csv
    ├── run_status_YYYY-MM-DD.json
    ├── portfolio_snapshot_YYYY-MM-DD.json
    ├── account_snapshot_YYYY-MM-DD.json
    ├── decision_history.jsonl
    ├── state_transitions.csv
    ├── state_transitions.jsonl
    ├── execution_gate_log.jsonl
    ├── data_quality_log.jsonl
    ├── ledger.csv
    └── freshness.json
```

## 10. 最終目標の定義

Stock Sentinel の GitHub Actions ホスティングの最終目標は、自動取引や市場予測ではなく、**「完全かつ連続的で信頼できる資本構造の観測履歴」を確立すること**です。

## 11. コア哲学

> これは単なる CI/CD ではありません。
>
> これは、あなたの「資本望遠鏡」を軌道に乗せ、毎日自動的に宇宙を観測させる仕組みです。
>
> 人間が思考し、機械が観測し、時間がすべてを証明します。
